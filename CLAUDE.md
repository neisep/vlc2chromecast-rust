# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

vlc2chromecast is a cross-platform desktop app (Rust + egui) that streams local video files to Chromecast by launching VLC with the appropriate `--sout=#chromecast` arguments. It replaces an older C# Windows Forms version (https://github.com/neisep/vlc2chromecast).

## Build & Run Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo run                # Build and run
cargo test               # Run all unit tests
cargo test config        # Run only config module tests
cargo test vlc           # Run only vlc module tests
```

Requires Rust 1.85+ (edition 2024). On Linux, you may need system libraries for egui/winit:
```bash
# Debian/Ubuntu/Mint
sudo apt install libgtk-3-dev libxdo-dev
```

## Architecture

```
src/
├── main.rs      — Entry point: configures eframe window, centers on active monitor
├── app.rs       — VlcChromecastApp: egui UI, playback controls, monitor thread management
├── config.rs    — Config struct: load/save settings as JSON to platform config dir
├── vlc.rs       — VLC process management, HTTP API commands, playback state polling
└── monitor.rs   — Multi-monitor support: queries X11 for cursor position to center window
```

**Data flow:** User configures Chromecast IP + VLC path → selects video (file picker or drag-and-drop) → app spawns VLC headless (`--intf dummy`) with chromecast sout arguments → background thread polls VLC's HTTP interface (`localhost:54212`) every second for playback state → progress bar, pause/resume, seek, and stop controls are shown in the UI.

**VLC HTTP interface:** VLC is launched with `--extraintf http --http-port 54212 --http-password vlc2cc`. The app sends commands via raw HTTP GET requests to `/requests/status.json?command=...`:
- `pl_pause` — toggle pause/resume
- `pl_stop` — stop stream (sent before killing VLC)
- `seek&val=<seconds>` — seek to position

**Process lifecycle:** VLC is spawned in its own process group (`process_group(0)`). On stop/exit, the app first sends `pl_stop` via HTTP, then `kill -TERM` to the process group, then `SIGKILL` as a fallback.

**Multi-monitor:** On Linux, `monitor.rs` uses X11/Xinerama to find the cursor position and center the window on the monitor where the mouse is. Falls back to `centered: true` on other platforms.

**Config location:**
- Linux: `~/.config/vlc2chromecast/settings.json`
- Windows: `%AppData%\vlc2chromecast\settings.json`

**No Chromecast API is used** — VLC's built-in chromecast module handles all streaming. The app is purely a launcher and controller.

## Key Dependencies

- `eframe` — egui framework for native desktop GUI
- `rfd` — Native file dialogs (video file picker, VLC executable picker)
- `dirs` — Cross-platform config directory resolution
- `serde` + `serde_json` — Settings serialization and VLC status JSON parsing
- `x11-dl` (Linux only) — X11/Xinerama queries for multi-monitor cursor detection
