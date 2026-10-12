# OpenWire

A native Linux soundboard and real-time voice effects processor for PipeWire.

OpenWire plays sounds from a soundboard into a virtual microphone, processes your
own microphone (pitch, formant, EQ, radio effect, noise gate, ducking), and lets
you control everything from the keyboard, including the numpad.

This project is a fork of [AdrescorGiti/OpenWire](https://github.com/AdrescorGiti/OpenWire),
rewritten without a webview, Node.js or Tauri. It is a single native Rust binary
with an egui interface.

## Features

### Soundboard

- Groups of sound pads. A new soundboard starts with two groups of five pads.
  Add, rename or delete groups at any time.
- Pads are shown as a grid of tiles, or as a list with a duration column.
- Playback modes per pad: one-shot, loop, and toggle (loops until stopped).
- Pause and resume, and seek by clicking or dragging on the waveform.
- Elapsed and total time on the waveform, and a progress indicator on every
  playing pad.
- Trim start and end, bass boost, and per-pad monitor and virtual-mic volume.
- Audio formats: MP3, WAV, FLAC, OGG, AAC, M4A/MP4, AIFF and more, through
  Symphonia.

### Keybinds

- Any key can be bound to a pad or to a playback action, including numpad keys
  and media keys. Pick **Assign keybind**, then press the key.
- Press ESC while assigning to remove the keybind.
- Playback actions: play the selected pad, pause or resume, next and previous
  sound in the group, stop the last sound, and stop all sounds.
- The **Keybinds** page lists every playback action and every sound that has a
  keybind.
- ESC on the keyboard stops all sounds when you are not assigning a key.

### Monitor and virtual microphone

- A global **Monitor** level in the header controls what you hear of the
  soundboard. It does not change what your microphone sends to other people.
  Set it to zero to play sounds silently for yourself.
- The virtual microphone is a PipeWire source named "OpenWire Virtual Mic" that
  you select in your apps.
- Choose the real microphone to process in **Settings → Audio input**. The
  choice is remembered.

### Voice & DSP

- Presets: Ordinary Voice, Deep Voice, High Voice, Radio Comms, Telephone and
  Monster. You can save your own presets next to the built-in ones.
- Pitch shift and formant, a four-band equalizer, a radio effect (band-pass,
  drive and noise), a noise gate, and ducking of your microphone while sounds
  play.
- A signal-chain strip shows which stages are active.

### Window and tray

- Closing the window (the X button) hides it to the system tray, so OpenWire
  keeps running. Use the tray icon's **Show window** to bring it back, or
  **Quit OpenWire** to exit.
- The tray menu also offers stop all sounds, disable effects, and next preset.
- The header has **Hide to tray** and **Quit** buttons.
- Setting: **Settings → Behavior → Closing the window (X) hides it to the tray**.

## Requirements

- Linux with PipeWire running (the default on most current distributions).
- A StatusNotifier host for the tray icon. KDE Plasma has one built in. GNOME
  needs the AppIndicator extension.
- Membership in the `input` group, to read keyboard events for keybinds.
- For native file dialogs: `xdg-desktop-portal` and your desktop's backend, for
  example `xdg-desktop-portal-kde`.

## Installing

### Arch Linux package

```bash
cd packaging
./build.sh
sudo pacman -U openwire-*.pkg.tar.zst
```

### From source

```bash
cargo build --release
./target/release/openwire
```

Build dependencies: a recent Rust toolchain (1.77 or newer), `clang` (used by the
PipeWire bindings), and the PipeWire development files (`libpipewire`).

### Keybinds permission

Keybinds are read from `/dev/input`. Add your user to the `input` group, then log
out and back in:

```bash
sudo usermod -aG input "$USER"
```

Only keyboard devices are read. OpenWire ignores any key that has no keybind, and
does not store or send key presses anywhere.

## Usage

### Soundboard

1. Open **Soundboard**.
2. Click a pad to select it. Click **Import file** in the right panel to load a
   sound. Click or drag the waveform to choose where playback starts.
3. Use **+ Group** to add a group. Right-click a group to rename or delete it.
4. Use **Layout: Grid / List** to switch views. Your choice is remembered.

### Keybinds

1. Open **Keybinds**, or select a pad and press **Assign keybind** in the right
   panel.
2. Press the key or numpad key you want. Press ESC to remove the keybind.
3. If a key is already in use, it moves to the new target.

### Settings

- **Folders:** where the soundboard, presets and settings are stored.
- **Audio input:** which microphone is processed.
- **Behavior:** whether hearing yourself is on by default, whether ducking is on,
  and whether closing the window hides it to the tray.

## Files

- Settings: `~/.config/OpenWire/settings.toml`
- Soundboard and presets: `~/Documents/OpenWire/` by default. The folders can be
  changed in **Settings → Folders**.

Sound files are referenced where you imported them. Deleting a group or a pad does
not delete the audio file.

## Troubleshooting

- **Keybinds do nothing:** check that you are in the `input` group, and that you
  logged out and back in afterwards.
- **The tray icon does not appear:** your desktop needs a StatusNotifier host. On
  GNOME, install the AppIndicator extension.
- **The window is gone and cannot be restored:** use the tray icon, or start
  OpenWire again. **Quit** in the header exits cleanly.
- **Other people do not hear the sounds:** select "OpenWire Virtual Mic" as the
  microphone in the app you are using.

## Project structure

```text
src/
  main.rs              start-up: backend, PipeWire, workers, tray, window
  audio/
    engine.rs          PipeWire graph: microphone capture, virtual mic, monitor
    mixer.rs           real-time voice mixer for soundboard playback
    dsp.rs             effect chain: gate, pitch, formant, EQ, radio, ducking
    decode.rs          audio decoding (Symphonia) and resampling
  playback/
    mod.rs             Sampler: groups, pads, play, pause, seek, transport
    timing.rs          elapsed-time tracking for playing sounds
    worker.rs          background worker thread and its actions
  board/mod.rs         groups and pads (soundboard.toml)
  input/
    keybinds.rs        keybind capture and dispatch from /dev/input
  presets/mod.rs       built-in and saved voice presets
  config/
    paths.rs           where files live
    settings.rs        settings.toml
  system/
    tray.rs            system tray icon and menu
    window.rs          close-to-tray behavior and the windowing backend
  ui/
    app.rs             main window, header, sidebar, shared state
    board_view.rs      soundboard page and pad panel
    keybinds_view.rs   keybinds page
    voice_view.rs      voice and DSP page
    settings_view.rs   settings page
    widgets.rs         reusable drawing helpers
    theme.rs           colors and the visual style
packaging/             PKGBUILD, build script, desktop entry, install hints
assets/                application icon
```

## License

OpenWire is free software, released under the GNU General Public License,
version 3 or (at your option) any later version. See [LICENSE](LICENSE).
This fork keeps the license of the original project.
