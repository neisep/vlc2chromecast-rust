use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use eframe::egui;

use crate::config::Config;
use crate::vlc::{self, PlaybackState};

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts"];

pub struct VlcChromecastApp {
    config: Config,
    video_file: String,
    status: String,
    status_is_error: bool,
    vlc_process: Option<Child>,
    playback: Arc<Mutex<PlaybackState>>,
    monitor_stop: Arc<AtomicBool>,
    monitor_threads: Option<(JoinHandle<()>, JoinHandle<()>)>,
}

impl VlcChromecastApp {
    pub fn new() -> Self {
        let config = Config::load();
        Self {
            config,
            video_file: String::new(),
            status: String::new(),
            status_is_error: false,
            vlc_process: None,
            playback: Arc::new(Mutex::new(PlaybackState::default())),
            monitor_stop: Arc::new(AtomicBool::new(false)),
            monitor_threads: None,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status = msg.into();
        self.status_is_error = is_error;
    }

    fn start_monitor(&mut self, ctx: &egui::Context, stderr: std::process::ChildStderr) {
        // Old VLC is already dead (killed by launch_vlc) — safe to join directly
        self.monitor_stop.store(true, Ordering::Relaxed);
        if let Some((stderr_handle, poll_handle)) = self.monitor_threads.take() {
            let _ = poll_handle.join();
            let _ = stderr_handle.join();
        }
        self.monitor_stop = Arc::new(AtomicBool::new(false));
        self.playback = Arc::new(Mutex::new(PlaybackState::default()));
        self.monitor_threads = Some(vlc::start_playback_monitor(
            Arc::clone(&self.playback),
            Arc::clone(&self.monitor_stop),
            ctx.clone(),
            stderr,
        ));
    }

    /// Idempotent cleanup: kill VLC, then join monitor threads.
    /// VLC must die first — its stderr pipe closing is what unblocks the
    /// stderr reader thread.  Reversing the order deadlocks.
    /// Called from the Stop button, on_exit, and Drop.
    fn cleanup(&mut self) {
        self.monitor_stop.store(true, Ordering::Relaxed);
        vlc::kill_previous(&mut self.vlc_process);
        if let Some((stderr_handle, poll_handle)) = self.monitor_threads.take() {
            let _ = poll_handle.join();
            let _ = stderr_handle.join();
        }
        *self.playback.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::default();
    }
}

impl Drop for VlcChromecastApp {
    fn drop(&mut self) {
        // Safety net: ensures VLC is killed even if on_exit wasn't called
        self.cleanup();
    }
}

impl eframe::App for VlcChromecastApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cleanup();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle drag-and-drop
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(path) = i.raw.dropped_files[0].path.as_ref() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                        self.video_file = path.display().to_string();
                        self.set_status(format!("File selected: {}", self.video_file), false);
                    } else {
                        self.set_status(
                            "Unsupported file type. Supported: mp4, mkv, avi, mov, wmv, flv, webm, m4v, ts"
                                .to_string(),
                            true,
                        );
                    }
                }
            }
        });

        egui::TopBottomPanel::top("settings").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading("Settings");
            ui.add_space(4.0);

            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Chromecast IP:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.config.chromecast_ip)
                            .hint_text("e.g. 192.168.1.100")
                            .desired_width(ui.available_width()),
                    );
                    ui.end_row();

                    ui.label("VLC Path:");
                    ui.horizontal(|ui| {
                        let browse_width = 80.0;
                        ui.add(
                            egui::TextEdit::singleline(&mut self.config.vlc_path)
                                .hint_text("Path to VLC executable")
                                .desired_width(ui.available_width() - browse_width),
                        );
                        if ui.button("Browse...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Select VLC Executable")
                                .pick_file()
                            {
                                self.config.vlc_path = path.display().to_string();
                            }
                        }
                    });
                    ui.end_row();
                });

            ui.add_space(4.0);
            if ui.button("Save Settings").clicked() {
                match self.config.save() {
                    Ok(()) => self.set_status("Settings saved.".to_string(), false),
                    Err(e) => self.set_status(e, true),
                }
            }
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let pb = self.playback.lock().unwrap_or_else(|e| e.into_inner()).clone();

            // Surface VLC errors to the status bar
            if let Some(ref error) = pb.error {
                if self.vlc_process.is_some() {
                    self.set_status(error.clone(), true);
                    self.cleanup();
                }
            }

            // Auto-cleanup when playback finishes naturally
            if pb.finished && self.vlc_process.is_some() {
                self.cleanup();
                self.set_status("Playback finished.".to_string(), false);
            }

            // Playback progress bar
            if pb.duration_secs > 0.0 && self.vlc_process.is_some() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let pause_label = if pb.is_playing { "⏸ Pause" } else { "▶ Resume" };
                    if ui.button(pause_label).clicked() {
                        vlc::toggle_pause();
                    }

                    if ui.button("⏹ Stop").clicked() {
                        self.cleanup();
                        self.set_status("Stream stopped.".to_string(), false);
                    }

                    let fraction =
                        (pb.position_secs / pb.duration_secs).clamp(0.0, 1.0) as f32;
                    let time_text = format!(
                        "{} / {}",
                        vlc::format_time(pb.position_secs),
                        vlc::format_time(pb.duration_secs),
                    );
                    let bar = ui.add(
                        egui::ProgressBar::new(fraction)
                            .text(time_text)
                            .desired_width(ui.available_width()),
                    );
                    let seek_area = ui.interact(
                        bar.rect,
                        egui::Id::new("seek_bar"),
                        egui::Sense::click(),
                    );
                    if seek_area.clicked() {
                        if let Some(pos) = seek_area.interact_pointer_pos() {
                            let ratio = ((pos.x - bar.rect.left()) / bar.rect.width())
                                .clamp(0.0, 1.0) as f64;
                            vlc::seek(ratio * pb.duration_secs);
                        }
                    }
                });
                ui.add_space(4.0);
            }

            // Status message
            if !self.status.is_empty() {
                ui.add_space(2.0);
                let color = if self.status_is_error {
                    egui::Color32::from_rgb(220, 50, 50)
                } else {
                    egui::Color32::from_rgb(50, 160, 50)
                };
                ui.colored_label(color, &self.status);
                ui.add_space(4.0);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.heading("Drag a video file here");
                ui.add_space(10.0);
                ui.label("— or —");
                ui.add_space(10.0);

                if ui.button("Select Video File").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Select Video File")
                        .add_filter("Video Files", VIDEO_EXTENSIONS)
                        .pick_file()
                    {
                        self.video_file = path.display().to_string();
                        self.set_status(format!("File selected: {}", self.video_file), false);
                    }
                }

                if !self.video_file.is_empty() {
                    ui.add_space(20.0);
                    ui.group(|ui| {
                        ui.label(format!("Selected: {}", self.video_file));
                    });
                    ui.add_space(10.0);

                    let cast_button = egui::Button::new(
                        egui::RichText::new("Cast to Chromecast").size(18.0),
                    );
                    if ui.add(cast_button).clicked() {
                        match vlc::launch_vlc(
                            &self.config.vlc_path,
                            &self.video_file,
                            &self.config.chromecast_ip,
                            &mut self.vlc_process,
                        ) {
                            Ok(stderr) => {
                                self.set_status(
                                    "Launching VLC — streaming to Chromecast...".to_string(),
                                    false,
                                );
                                self.start_monitor(ctx, stderr);
                            }
                            Err(e) => self.set_status(e, true),
                        }
                    }
                }
            });
        });
    }
}
