use eframe::egui::{self, ViewportCommand};

fn native_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

pub fn configure_backend() {
    let wants_wayland = matches!(std::env::var("WINIT_UNIX_BACKEND").as_deref(), Ok("wayland"));
    if !wants_wayland && std::env::var_os("DISPLAY").is_some() && native_wayland() {
        std::env::remove_var("WAYLAND_DISPLAY");
    }
    if native_wayland() {
        eprintln!("[openwire] window backend: native Wayland (closing minimizes the window)");
    } else {
        eprintln!("[openwire] window backend: X11 / XWayland (closing hides the window)");
    }
}

pub fn hide_to_tray(ctx: &egui::Context) {
    if native_wayland() {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    } else {
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
    }
}

pub fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::Focus);
}
