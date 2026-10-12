use eframe::egui;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCmd {
    Show,
    StopAll,
    DisableEffects,
    NextPreset,
    Quit,
}

const ICON_SIZE_SMALL: i32 = 32;
const ICON_SIZE_LARGE: i32 = 64;
const ICON_SMALL: &[u8] = include_bytes!("../../assets/openwire-32.argb");
const ICON_LARGE: &[u8] = include_bytes!("../../assets/openwire-64.argb");

pub type Wake = Arc<Mutex<Option<egui::Context>>>;

pub fn new_wake() -> Wake {
    Arc::new(Mutex::new(None))
}

struct OpenWireTray {
    tx: Sender<TrayCmd>,
    wake: Wake,
}

impl OpenWireTray {
    fn emit(&self, cmd: TrayCmd) {
        let _ = self.tx.send(cmd);
        if let Ok(slot) = self.wake.lock() {
            if let Some(ctx) = slot.as_ref() {
                ctx.request_repaint();
            }
        }
    }
}

impl ksni::Tray for OpenWireTray {
    fn id(&self) -> String {
        "openwire".into()
    }

    fn title(&self) -> String {
        "OpenWire".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![
            ksni::Icon {
                width: ICON_SIZE_SMALL,
                height: ICON_SIZE_SMALL,
                data: ICON_SMALL.to_vec(),
            },
            ksni::Icon {
                width: ICON_SIZE_LARGE,
                height: ICON_SIZE_LARGE,
                data: ICON_LARGE.to_vec(),
            },
        ]
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};
        vec![
            StandardItem {
                label: "Show window".into(),
                activate: Box::new(|t: &mut Self| t.emit(TrayCmd::Show)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Stop all sounds".into(),
                activate: Box::new(|t: &mut Self| t.emit(TrayCmd::StopAll)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Disable effects".into(),
                activate: Box::new(|t: &mut Self| t.emit(TrayCmd::DisableEffects)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next preset".into(),
                activate: Box::new(|t: &mut Self| t.emit(TrayCmd::NextPreset)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit OpenWire".into(),
                activate: Box::new(|t: &mut Self| t.emit(TrayCmd::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn(tx: Sender<TrayCmd>, wake: Wake) {
    let service = ksni::TrayService::new(OpenWireTray { tx, wake });
    let _handle = service.spawn();
}
