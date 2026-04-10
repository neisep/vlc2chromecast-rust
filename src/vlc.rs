use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const VLC_HTTP_PORT: u16 = 54212;
const VLC_HTTP_PASSWORD: &str = "vlc2cc";
/// Base64 of ":vlc2cc"
const VLC_AUTH_HEADER: &str = "OnZsYzJjYw==";

#[derive(Clone, Default, Debug)]
pub struct PlaybackState {
    pub position_secs: f64,
    pub duration_secs: f64,
    pub is_playing: bool,
    pub finished: bool,
    pub error: Option<String>,
}

/// Build the VLC command arguments for Chromecast streaming.
/// Runs VLC headless with an HTTP interface for status polling.
pub fn build_vlc_args(video_file: &str, chromecast_ip: &str) -> Vec<String> {
    vec![
        video_file.to_string(),
        "--sout=#chromecast".to_string(),
        format!("--sout-chromecast-ip={chromecast_ip}"),
        "--demux-filter=cc_demux".to_string(),
        "--intf".to_string(),
        "dummy".to_string(),
        "--extraintf".to_string(),
        "http".to_string(),
        "--http-host".to_string(),
        "localhost".to_string(),
        "--http-port".to_string(),
        VLC_HTTP_PORT.to_string(),
        "--http-password".to_string(),
        VLC_HTTP_PASSWORD.to_string(),
    ]
}

/// Send a command to VLC's HTTP interface. Returns `true` if the command was sent successfully.
fn send_vlc_command(command: &str) -> bool {
    let addr: std::net::SocketAddr = ([127, 0, 0, 1], VLC_HTTP_PORT).into();
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let request = format!(
        "GET /requests/status.json?command={command} HTTP/1.0\r\nAuthorization: Basic {VLC_AUTH_HEADER}\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    // Drain response so VLC finishes processing before we close the connection
    let mut buf = [0u8; 512];
    let _ = stream.read(&mut buf);
    true
}

/// Toggle pause/resume on the current stream.
/// Returns `true` if the command was sent successfully.
pub fn toggle_pause() -> bool {
    send_vlc_command("pl_pause")
}

/// Seek to a specific position in seconds.
/// Returns `true` if the command was sent successfully.
pub fn seek(seconds: f64) -> bool {
    send_vlc_command(&format!("seek&val={}", seconds as u64))
}

/// Stop playback, then kill the VLC process.
/// Shutdown sequence: HTTP pl_stop → SIGTERM (unix) → SIGKILL as fallback.
pub fn kill_previous(child: &mut Option<Child>) {
    if let Some(proc) = child {
        if proc.try_wait().ok().flatten().is_none() {
            // Gracefully stop the stream via HTTP
            send_vlc_command("pl_stop");
            thread::sleep(Duration::from_millis(300));

            // Send SIGTERM to let VLC clean up (close Chromecast connection, sockets)
            #[cfg(unix)]
            if proc.try_wait().ok().flatten().is_none() {
                unsafe {
                    libc::kill(proc.id() as i32, libc::SIGTERM);
                }
                for _ in 0..10 {
                    thread::sleep(Duration::from_millis(100));
                    if proc.try_wait().ok().flatten().is_some() {
                        break;
                    }
                }
            }

            // SIGKILL as last resort
            if proc.try_wait().ok().flatten().is_none() {
                let _ = proc.kill();
            }
            let _ = proc.wait();
        }
    }
    *child = None;
}

/// Launch VLC with Chromecast streaming arguments.
/// Kills any previous VLC instance we spawned before starting a new one.
/// Returns VLC's stderr handle on success for error monitoring.
pub fn launch_vlc(
    vlc_path: &str,
    video_file: &str,
    chromecast_ip: &str,
    previous: &mut Option<Child>,
) -> Result<ChildStderr, String> {
    if vlc_path.is_empty() {
        return Err("VLC path is not set".to_string());
    }
    if video_file.is_empty() {
        return Err("No video file selected".to_string());
    }
    if chromecast_ip.is_empty() {
        return Err("Chromecast IP is not set".to_string());
    }
    if chromecast_ip.parse::<std::net::IpAddr>().is_err() {
        return Err(format!("Invalid Chromecast IP address: {chromecast_ip}"));
    }
    if !std::path::Path::new(video_file).exists() {
        return Err(format!("Video file not found: {video_file}"));
    }

    kill_previous(previous);

    // Verify the HTTP control port is free after killing any previous instance
    if TcpStream::connect_timeout(
        &([127, 0, 0, 1], VLC_HTTP_PORT).into(),
        Duration::from_millis(200),
    )
    .is_ok()
    {
        return Err(format!(
            "Port {VLC_HTTP_PORT} is already in use — another VLC instance or process may be running"
        ));
    }

    let args = build_vlc_args(video_file, chromecast_ip);

    let mut cmd = Command::new(vlc_path);
    cmd.args(&args).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch VLC: {e}"))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture VLC stderr".to_string())?;

    *previous = Some(child);
    Ok(stderr)
}

/// Query VLC's HTTP interface for current playback state.
fn query_playback() -> Option<PlaybackState> {
    let addr: std::net::SocketAddr = ([127, 0, 0, 1], VLC_HTTP_PORT).into();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .ok()?;

    let request = format!(
        "GET /requests/status.json HTTP/1.0\r\nAuthorization: Basic {VLC_AUTH_HEADER}\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;

    let body = response.split("\r\n\r\n").nth(1)?;
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let json_state = json["state"].as_str();

    Some(PlaybackState {
        position_secs: json["time"].as_f64().unwrap_or(0.0),
        duration_secs: json["length"].as_f64().unwrap_or(0.0),
        is_playing: json_state == Some("playing"),
        finished: json_state == Some("stopped"),
        error: None,
    })
}

/// Extract the most relevant error line from the shared stderr buffer.
/// Prefers lines containing error keywords; falls back to the last non-empty line.
fn last_stderr_error(buf: &Arc<Mutex<String>>) -> Option<String> {
    let content = buf.lock().unwrap_or_else(|e| e.into_inner());
    let error_keywords = ["error:", "cannot", "failed", "fatal"];
    // Prefer lines with known error keywords (most recent first)
    if let Some(line) = content.lines().rev().find(|l| {
        let lower = l.to_lowercase();
        error_keywords.iter().any(|kw| lower.contains(kw))
    }) {
        return Some(format!("VLC error: {line}"));
    }
    // Fallback: last non-empty line
    content
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|line| format!("VLC error: {line}"))
}

/// Start a background thread that polls VLC for playback status every second.
/// Captures VLC's stderr and reports errors through [`PlaybackState::error`].
pub fn start_playback_monitor(
    state: Arc<Mutex<PlaybackState>>,
    stop: Arc<AtomicBool>,
    ctx: eframe::egui::Context,
    stderr: ChildStderr,
) -> (JoinHandle<()>, JoinHandle<()>) {
    const STDERR_MAX_BYTES: usize = 4096;

    // Collect VLC stderr output in a shared buffer (capped at 4KB)
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let stderr_writer = Arc::clone(&stderr_buf);
    let stop_stderr = Arc::clone(&stop);
    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if stop_stderr.load(Ordering::Relaxed) {
                break;
            }
            if let Ok(line) = line {
                let mut buf = stderr_writer.lock().unwrap_or_else(|e| e.into_inner());
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(&line);
                // Trim to keep only the tail when the buffer exceeds the cap
                if buf.len() > STDERR_MAX_BYTES {
                    let start = buf.len() - STDERR_MAX_BYTES;
                    if let Some(newline) = buf[start..].find('\n') {
                        let cut = start + newline + 1;
                        buf.drain(..cut);
                    }
                }
            }
        }
    });

    let poll_thread = thread::spawn(move || {
        // Give VLC time to start up
        thread::sleep(Duration::from_secs(2));

        let mut consecutive_failures: u32 = 0;
        let mut idle_polls: u32 = 0;
        let mut playback_started = false;

        while !stop.load(Ordering::Relaxed) {
            if let Some(ps) = query_playback() {
                consecutive_failures = 0;

                let is_finished = playback_started && ps.finished;
                if ps.is_playing && ps.duration_secs > 0.0 {
                    playback_started = true;
                    idle_polls = 0;
                } else if !playback_started {
                    idle_polls += 1;
                }

                *state.lock().unwrap_or_else(|e| e.into_inner()) = ps;
                ctx.request_repaint();

                if is_finished {
                    break;
                }

                // VLC is responding but playback never started — check stderr for errors
                if !playback_started && idle_polls >= 15 {
                    if let Some(error_msg) = last_stderr_error(&stderr_buf) {
                        state.lock().unwrap_or_else(|e| e.into_inner()).error =
                            Some(error_msg);
                        ctx.request_repaint();
                        break;
                    }
                    // No stderr yet — recheck in 5 more seconds (threshold is 15)
                    idle_polls = 10;
                }
            } else {
                consecutive_failures += 1;
                if consecutive_failures >= 8 {
                    let error_msg = last_stderr_error(&stderr_buf).unwrap_or_else(|| {
                        "VLC is not responding — it may have crashed or failed to start"
                            .to_string()
                    });
                    state.lock().unwrap_or_else(|e| e.into_inner()).error = Some(error_msg);
                    ctx.request_repaint();
                    break;
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    (stderr_thread, poll_thread)
}

/// Format seconds as M:SS or H:MM:SS.
pub fn format_time(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_contains_file_ip_and_http_interface() {
        let args = build_vlc_args("/home/user/video.mp4", "192.168.1.50");
        assert_eq!(args[0], "/home/user/video.mp4");
        assert_eq!(args[1], "--sout=#chromecast");
        assert_eq!(args[2], "--sout-chromecast-ip=192.168.1.50");
        assert!(args.contains(&"--intf".to_string()));
        assert!(args.contains(&"dummy".to_string()));
        assert!(args.contains(&"--extraintf".to_string()));
        assert!(args.contains(&"http".to_string()));
    }

    #[test]
    fn launch_vlc_rejects_empty_path() {
        let mut prev = None;
        let result = launch_vlc("", "video.mp4", "192.168.1.1", &mut prev);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("VLC path"));
    }

    #[test]
    fn launch_vlc_rejects_empty_file() {
        let mut prev = None;
        let result = launch_vlc("/usr/bin/vlc", "", "192.168.1.1", &mut prev);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("video file"));
    }

    #[test]
    fn launch_vlc_rejects_empty_ip() {
        let mut prev = None;
        let result = launch_vlc("/usr/bin/vlc", "video.mp4", "", &mut prev);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Chromecast IP"));
    }

    #[test]
    fn launch_vlc_rejects_invalid_ip() {
        let mut prev = None;
        let result = launch_vlc("/usr/bin/vlc", "video.mp4", "not-an-ip", &mut prev);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid"));
    }

    #[test]
    fn format_time_minutes() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(65.0), "1:05");
        assert_eq!(format_time(3599.0), "59:59");
    }

    #[test]
    fn format_time_hours() {
        assert_eq!(format_time(3600.0), "1:00:00");
        assert_eq!(format_time(3661.0), "1:01:01");
    }
}
