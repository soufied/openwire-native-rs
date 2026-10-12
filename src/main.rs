mod audio;
mod board;
mod config;
mod input;
mod playback;
mod presets;
mod system;
mod ui;

use crate::config::{paths, settings};
use std::error::Error;
use std::sync::{mpsc, Arc};
use std::time::Duration;
use ui::Wiring;

fn main() -> Result<(), Box<dyn Error>> {
    system::window::configure_backend();

    let settings = settings::Settings::load();
    let data = paths::data_dir(&settings)?;
    paths::ensure(&data)?;
    let presets_dir = paths::presets_dir(&settings)?;

    let mut initial = audio::dsp::DspSnapshot::default();
    initial.monitor_enabled = settings.monitor_enabled;
    initial.ducking_enabled = settings.ducking_enabled;
    let params = audio::dsp::Params::with_snapshot(&initial);

    let (control, bus, mut reaper) = audio::mixer::MixerBus::split();
    let engine = match audio::engine::start(params.clone(), bus, settings.input_device.clone()) {
        Ok(engine) => Some(engine),
        Err(err) => {
            eprintln!("[openwire] PipeWire engine failed to start: {err:#}");
            None
        }
    };
    let engine_running = engine.is_some();
    let (meters, inputs, engine_commands) = match &engine {
        Some(e) => (e.meters.clone(), e.inputs.clone(), Some(e.commands.clone())),
        None => (Default::default(), Default::default(), None),
    };

    std::thread::Builder::new()
        .name("openwire-reaper".into())
        .spawn(move || loop {
            reaper.drain();
            std::thread::sleep(Duration::from_millis(50));
        })?;

    let sampler = Arc::new(playback::Sampler::new(
        data.join("soundboard.toml"),
        control.clone(),
    ));
    let (actions, rx) = mpsc::channel();
    playback::run_worker(sampler.clone(), rx);

    let bindings = input::keybinds::new_bindings();
    let capture = input::keybinds::new_capture();
    input::keybinds::rebuild(&bindings, &sampler.snapshot(), &settings.transport_keys);
    input::keybinds::watch(bindings.clone(), capture.clone(), actions.clone());

    let (tray_tx, tray_rx) = mpsc::channel();
    let wake = system::tray::new_wake();
    system::tray::spawn(tray_tx, wake.clone());

    let presets = presets::PresetStore::new(&presets_dir)?;

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("OpenWire")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1180.0, 720.0]),
        ..Default::default()
    };

    let wiring = Wiring {
        sampler,
        actions,
        params,
        meters,
        control,
        bindings,
        settings,
        presets,
        tray_rx,
        engine_commands,
        inputs,
        engine_running,
        capture,
    };

    eframe::run_native(
        "OpenWire",
        options,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, wiring, wake)))),
    )?;
    drop(engine);
    Ok(())
}
