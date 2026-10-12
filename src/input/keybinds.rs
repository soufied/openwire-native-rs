use crate::board::Soundboard;
use crate::playback::Action;
use crate::config::settings::{Transport, TransportKeys};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Pad(usize, usize),
    Transport(Transport),
}

pub type Bindings = Arc<RwLock<HashMap<u16, Target>>>;
pub type CaptureSlot = Arc<Mutex<Option<Sender<u16>>>>;

pub const KEY_ESC: u16 = 1;
const EV_KEY: u16 = 0x01;
const KEY_A: u32 = 30;
const EVENT_SIZE: usize = 24;

const KEYS: &[(&str, u16)] = &[
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
    ("Q", 16),
    ("W", 17),
    ("E", 18),
    ("R", 19),
    ("T", 20),
    ("Y", 21),
    ("U", 22),
    ("I", 23),
    ("O", 24),
    ("P", 25),
    ("A", 30),
    ("S", 31),
    ("D", 32),
    ("F", 33),
    ("G", 34),
    ("H", 35),
    ("J", 36),
    ("K", 37),
    ("L", 38),
    ("Z", 44),
    ("X", 45),
    ("C", 46),
    ("V", 47),
    ("B", 48),
    ("N", 49),
    ("M", 50),
    ("F1", 59),
    ("F2", 60),
    ("F3", 61),
    ("F4", 62),
    ("F5", 63),
    ("F6", 64),
    ("F7", 65),
    ("F8", 66),
    ("F9", 67),
    ("F10", 68),
    ("F11", 87),
    ("F12", 88),
    ("SPACE", 57),
    ("TAB", 15),
    ("ENTER", 28),
    ("BACKSPACE", 14),
    ("DELETE", 111),
    ("INSERT", 110),
    ("HOME", 102),
    ("END", 107),
    ("PAGEUP", 104),
    ("PAGEDOWN", 109),
    ("UP", 103),
    ("DOWN", 108),
    ("LEFT", 105),
    ("RIGHT", 106),
    ("CAPSLOCK", 58),
    ("PRINTSCREEN", 99),
    ("SCROLLLOCK", 70),
    ("PAUSE", 119),
    ("NUMLOCK", 69),
    ("KP0", 82),
    ("KP1", 79),
    ("KP2", 80),
    ("KP3", 81),
    ("KP4", 75),
    ("KP5", 76),
    ("KP6", 77),
    ("KP7", 71),
    ("KP8", 72),
    ("KP9", 73),
    ("KPDOT", 83),
    ("KPENTER", 96),
    ("KPPLUS", 78),
    ("KPMINUS", 74),
    ("KPASTERISK", 55),
    ("KPSLASH", 98),
    ("PLAYPAUSE", 164),
    ("NEXTSONG", 163),
    ("PREVIOUSSONG", 165),
    ("STOPCD", 166),
    ("MUTE", 113),
    ("VOLUMEDOWN", 114),
    ("VOLUMEUP", 115),
];

pub fn new_bindings() -> Bindings {
    Arc::new(RwLock::new(HashMap::new()))
}

pub fn new_capture() -> CaptureSlot {
    Arc::new(Mutex::new(None))
}

pub fn key_code(label: &str) -> Option<u16> {
    let up = label.trim().to_uppercase();
    if let Some(raw) = up.strip_prefix("KEY ") {
        return raw.trim().parse().ok();
    }
    KEYS.iter()
        .find(|(name, _)| *name == up.as_str())
        .map(|(_, code)| *code)
}

pub fn key_label(code: u16) -> String {
    if code == KEY_ESC {
        return "ESC".into();
    }
    KEYS.iter()
        .find(|(_, c)| *c == code)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| format!("KEY {code}"))
}

pub fn rebuild(bindings: &Bindings, board: &Soundboard, transport: &TransportKeys) {
    let mut map = HashMap::new();
    for bank in &board.banks {
        for pad in &bank.pads {
            if let Some(code) = pad.hotkey.as_deref().and_then(key_code) {
                map.insert(code, Target::Pad(pad.bank, pad.id));
            }
        }
    }
    for t in Transport::ALL {
        if let Some(code) = transport.get(t).and_then(key_code) {
            map.insert(code, Target::Transport(t));
        }
    }
    if let Ok(mut slot) = bindings.write() {
        *slot = map;
    }
}

fn keyboard_nodes() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir("/sys/class/input") else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("event") {
                return None;
            }
            let caps = std::fs::read_to_string(entry.path().join("device/capabilities/key")).ok()?;
            let last = caps.split_whitespace().last()?;
            let word = u64::from_str_radix(last, 16).ok()?;
            if word & (1u64 << KEY_A) != 0 {
                Some(PathBuf::from(format!("/dev/input/{name}")))
            } else {
                None
            }
        })
        .collect()
}

pub fn watch(bindings: Bindings, capture: CaptureSlot, tx: Sender<Action>) {
    let spawned = std::thread::Builder::new()
        .name("openwire-hotkey-scan".into())
        .spawn(move || {
            let known: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
            loop {
                for path in keyboard_nodes() {
                    let fresh = known.lock().map(|mut set| set.insert(path.clone())).unwrap_or(false);
                    if !fresh {
                        continue;
                    }
                    match File::open(&path) {
                        Ok(file) => {
                            let bindings = bindings.clone();
                            let capture = capture.clone();
                            let tx = tx.clone();
                            let worker_known = known.clone();
                            let owned = path.clone();
                            let spawned = std::thread::Builder::new()
                                .name("openwire-hotkey".into())
                                .spawn(move || {
                                    listen(file, bindings, capture, tx);
                                    if let Ok(mut set) = worker_known.lock() {
                                        set.remove(&owned);
                                    }
                                });
                            if spawned.is_err() {
                                known.lock().map(|mut set| set.remove(&path)).ok();
                            }
                        }
                        Err(err) => eprintln!(
                            "[openwire][hotkeys] cannot open {} ({err}); the input group is required",
                            path.display()
                        ),
                    }
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    if let Err(err) = spawned {
        eprintln!("[openwire][hotkeys] cannot start keyboard scanner: {err}");
    }
}

fn listen(mut file: File, bindings: Bindings, capture: CaptureSlot, tx: Sender<Action>) {
    let mut buf = [0u8; EVENT_SIZE];
    loop {
        if file.read_exact(&mut buf).is_err() {
            break;
        }
        let kind = u16::from_le_bytes([buf[16], buf[17]]);
        let code = u16::from_le_bytes([buf[18], buf[19]]);
        let value = i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
        if kind != EV_KEY || value != 1 {
            continue;
        }
        let sender = capture.lock().ok().and_then(|slot| slot.clone());
        if let Some(sender) = sender {
            let _ = sender.send(code);
            continue;
        }
        if code == KEY_ESC {
            let _ = tx.send(Action::StopAll);
            continue;
        }
        let target = bindings.read().ok().and_then(|m| m.get(&code).copied());
        match target {
            Some(Target::Pad(bank, slot)) => {
                let _ = tx.send(Action::Play(bank, slot));
            }
            Some(Target::Transport(t)) => {
                let _ = tx.send(Action::Transport(t));
            }
            None => {}
        }
    }
}
