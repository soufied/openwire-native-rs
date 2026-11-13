# OpenWire

Native Linux soundboard and real-time voice effects for PipeWire. Plays sounds into a
virtual microphone, processes your microphone, and controls everything from the keyboard.

Fork of [AdrescorGiti/OpenWire](https://github.com/AdrescorGiti/OpenWire), rewritten as a
single native Rust binary with an egui interface.

## Features

- **Soundboard:** groups of pads (grid or list), one-shot, loop and toggle modes,
  pause, seek on the waveform, trim, bass boost and per-pad volumes.
- **Soundpad mode** (Settings → Interface mode): categories with a searchable sound list.
- **Drag and drop:** drop files or folders into the window. Imports run in the background.
- **Folders per category:** link a category to a folder. Its audio files are added, and
  sounds downloaded from browser links are saved there.
- **Browser links:** open a `soundpad://sound/url/...` link in your browser and OpenWire
  downloads the sound into the category you have open.
- **Multi-play** (Settings → Behavior): off by default, so a new sound stops the playing one.
- **Favorites and play counts:** mark a sound as a favorite to keep it at the top of its
  category. Each sound counts its plays. Sort by manual order, most played, date added or name.
- **Keybinds:** any key, including numpad and media keys, bound to a pad or a playback action.
- **Voice & DSP:** presets, pitch, formant, equalizer, radio effect, noise gate, ducking.
  A switch at the top bypasses all processing, so your microphone is sent unprocessed.
- **Tray:** closing the window hides it to the tray.

## Requirements

- Linux with PipeWire.
- A StatusNotifier host for the tray (KDE; GNOME needs the AppIndicator extension).
- Your user in the `input` group, for keybinds.
- `xdg-desktop-portal` with your desktop backend, for file dialogs.

## Installing

Arch Linux:

```bash
cd packaging && ./build.sh && sudo pacman -U openwire-*.pkg.tar.zst
```

From source (Rust 1.77+, `clang`, `libpipewire` development files):

```bash
cargo build --release
./target/release/openwire
```

Keybinds are read from `/dev/input`:

```bash
sudo usermod -aG input "$USER"
```

Log out and back in afterwards.

## Browser links

The package installs `/usr/share/applications/mimeapps.list`, which makes OpenWire the default
handler for `soundpad:` links. To choose a different handler for your user only:

```bash
xdg-mime default openwire.desktop x-scheme-handler/soundpad
```

Then click the sound button on the website and allow the browser to open it with OpenWire.
If OpenWire is already running, the link is handed to it. A linked folder receives the file;
without one, sounds go to the `Downloads` folder inside your data folder.

## Files

- Settings: `~/.config/OpenWire/settings.toml`
- Soundboard and presets: `~/.config/OpenWire/` (changeable in Settings → Folders).
  On first start the soundboard and presets are copied from the old `~/Documents/OpenWire/`.

Sound files are referenced in place. Removing a pad or a sound never deletes the audio file.
Downloaded browser links go to the linked folder of the category, or to `Downloads` in the data folder.

## Troubleshooting

- **Keybinds do nothing:** check the `input` group and log out and back in.
- **No tray icon:** your desktop needs a StatusNotifier host.
- **Window lost:** click the tray icon, or start OpenWire again.
- **Others do not hear the sounds:** select "OpenWire Virtual Mic" as the microphone in your app.

## License

GNU GPL v3 or later. See [LICENSE](LICENSE). This fork keeps the license of the original project.
