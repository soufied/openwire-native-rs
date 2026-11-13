use crate::config::paths;
use crate::links;
use crate::system::tray::Wake;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Open(String),
    Show,
}

pub fn socket_path() -> PathBuf {
    let base = dirs::runtime_dir().unwrap_or_else(|| {
        paths::config_file()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    });
    base.join("openwire.sock")
}

pub fn claim_or_forward(initial: Vec<Message>, tx: Sender<Message>, wake: Wake) -> bool {
    let path = socket_path();
    if let Ok(mut stream) = UnixStream::connect(&path) {
        let outgoing = if initial.is_empty() {
            vec![Message::Show]
        } else {
            initial
        };
        for message in &outgoing {
            let line = encode(message);
            if stream.write_all(format!("{line}\n").as_bytes()).is_err() {
                eprintln!("[openwire] cannot forward to the running instance");
                return false;
            }
        }
        return false;
    }

    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("[openwire] cannot listen on {}: {err}", path.display());
            for message in initial {
                let _ = tx.send(message);
            }
            return true;
        }
    };

    for message in initial {
        let _ = tx.send(message);
    }

    let spawned = std::thread::Builder::new()
        .name("openwire-ipc".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                let wake = wake.clone();
                let _ = std::thread::Builder::new()
                    .name("openwire-ipc-client".into())
                    .spawn(move || serve_client(stream, tx, wake));
            }
        });
    if let Err(err) = spawned {
        eprintln!("[openwire] cannot start the link listener: {err}");
    }
    true
}

fn serve_client(stream: UnixStream, tx: Sender<Message>, wake: Wake) {
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        let Some(message) = decode(&line) else {
            continue;
        };
        if tx.send(message).is_err() {
            return;
        }
        if let Ok(slot) = wake.lock() {
            if let Some(ctx) = slot.as_ref() {
                ctx.request_repaint();
            }
        }
    }
}

fn encode(message: &Message) -> String {
    match message {
        Message::Open(url) => url.replace(['\n', '\r'], ""),
        Message::Show => "show".to_string(),
    }
}

fn decode(line: &str) -> Option<Message> {
    let trimmed = line.trim();
    if trimmed.eq_ignore_ascii_case("show") {
        Some(Message::Show)
    } else if links::is_soundpad_link(trimmed) {
        Some(Message::Open(trimmed.to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_show_and_soundpad_lines_only() {
        assert_eq!(decode("show"), Some(Message::Show));
        assert_eq!(decode("  SHOW \n"), Some(Message::Show));
        assert_eq!(
            decode("soundpad://sound/url/https://example.com/a.mp3"),
            Some(Message::Open("soundpad://sound/url/https://example.com/a.mp3".to_string()))
        );
        assert_eq!(decode("rm -rf /"), None);
    }

    #[test]
    fn encodes_messages_as_single_lines() {
        assert_eq!(encode(&Message::Show), "show");
        assert_eq!(encode(&Message::Open("soundpad://a\nb".to_string())), "soundpad://ab");
    }
}
