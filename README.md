<p align="center">
  <h1 align="center">vlc2chromecast</h1>
  <p align="center">
    Stream local video files to your Chromecast — powered by VLC
    <br />
    <br />
    <a href="https://github.com/neisep/vlc2chromecast/releases">Download</a>
    &middot;
    <a href="https://github.com/neisep/vlc2chromecast/issues">Report Bug</a>
    &middot;
    <a href="https://github.com/neisep/vlc2chromecast/issues">Request Feature</a>
  </p>
</p>

<p align="center">
  <a href="https://github.com/neisep/vlc2chromecast/releases"><img src="https://img.shields.io/github/v/release/neisep/vlc2chromecast?style=flat-square" alt="Latest Release"></a>
  <a href="https://github.com/neisep/vlc2chromecast/blob/master/LICENSE"><img src="https://img.shields.io/github/license/neisep/vlc2chromecast?style=flat-square" alt="License"></a>
  <a href="https://github.com/neisep/vlc2chromecast/actions"><img src="https://img.shields.io/github/actions/workflow/status/neisep/vlc2chromecast/main.yml?style=flat-square" alt="Build Status"></a>
</p>

---

## About

**vlc2chromecast** is a lightweight desktop application that lets you cast any local video file to a Chromecast device on your network. Just point it at your Chromecast, pick a video, and hit play.

Under the hood it launches [VLC](https://www.videolan.org/) in headless mode with Chromecast streaming arguments — no complex setup, no command line needed.

### Features

- **Drag & drop** or browse to select video files
- **Playback controls** — pause, resume, stop, and seek via a clickable progress bar
- **Real-time progress** — shows current position, duration, and play/pause state
- **Multi-monitor aware** — window opens centered on the screen where your mouse is
- **Cross-platform** — runs on Linux and Windows
- **Persistent settings** — Chromecast IP and VLC path are saved between sessions
- **Clean shutdown** — gracefully stops the Chromecast stream when you close the app
- **Supports all major formats** — MP4, MKV, AVI, MOV, WMV, FLV, WebM, M4V, TS

### How It Works

```
┌──────────────────┐       VLC (headless)       ┌────────────────┐
│  vlc2chromecast   │ ──── spawns VLC with ────► │   Chromecast   │
│  (egui desktop)  │       --sout=#chromecast    │   on your TV   │
└──────────────────┘                             └────────────────┘
        │                        ▲
        │   HTTP API on :54212   │
        └────── controls ────────┘
          pause/resume/seek/stop
```

The app doesn't stream video itself — it orchestrates VLC's built-in Chromecast module and provides a friendly GUI with playback controls via VLC's HTTP interface.

---

## Getting Started

### Prerequisites

- **[VLC Media Player](https://www.videolan.org/)** installed on your system
- **A Chromecast device** on the same network as your computer

### Installation

#### Download a Release

Grab the latest binary from the [Releases](https://github.com/neisep/vlc2chromecast/releases) page.

#### Build from Source

You need [Rust](https://rustup.rs/) 1.85 or later.

**Linux (Debian/Ubuntu/Mint):**

```bash
# Clone and build
git clone https://github.com/neisep/vlc2chromecast.git
cd vlc2chromecast
cargo build --release

# Binary is at target/release/vlc2chromecast
```

**Windows:**

```bash
git clone https://github.com/neisep/vlc2chromecast.git
cd vlc2chromecast
cargo build --release

# Binary is at target\release\vlc2chromecast.exe
```

---

## Usage

1. **Launch** `vlc2chromecast`
2. **Enter your Chromecast IP** address in the settings panel
3. **Set the VLC path** — on Linux this defaults to `vlc` (works if VLC is in your PATH). On Windows, browse to `vlc.exe`
4. **Click Save** to persist your settings
5. **Select a video** — drag a file onto the window or click "Select Video File"
6. **Click "Cast to Chromecast"** — VLC launches in the background and starts streaming
7. **Control playback** — use the pause/resume button, stop button, or click anywhere on the progress bar to seek

### Settings Location

Settings are stored as JSON and persist between sessions:

| Platform | Path |
|----------|------|
| Linux | `~/.config/vlc2chromecast/settings.json` |
| Windows | `%AppData%\vlc2chromecast\settings.json` |

---

## Project Structure

```
src/
├── main.rs      — Entry point, window configuration, multi-monitor centering
├── app.rs       — GUI layout, playback controls, drag-and-drop handling
├── config.rs    — Settings serialization and persistence
├── vlc.rs       — VLC process management, HTTP API commands, playback polling
└── monitor.rs   — X11/Xinerama queries for multi-monitor cursor detection
```

---

## Contributing

Contributions are welcome! Feel free to open an issue or submit a pull request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes (`git commit -m 'Add my feature'`)
4. Push to the branch (`git push origin feature/my-feature`)
5. Open a Pull Request

### Running Tests

```bash
cargo test
```

---

## History

This project started as a [C# Windows Forms application](https://github.com/neisep/vlc2chromecast/tree/dotnet-legacy) and was rewritten in Rust with [egui](https://github.com/emilk/egui) to support Linux and provide a better cross-platform experience.

---

## License

Distributed under the **GPL-3.0** License. See [`LICENSE`](LICENSE) for more information.

---

## Acknowledgements

- [VLC Media Player](https://www.videolan.org/) — the engine behind the streaming
- [egui](https://github.com/emilk/egui) — immediate mode GUI framework for Rust
- [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) — native app framework for egui
