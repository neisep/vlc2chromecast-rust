use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub const VLC_HTTP_PORT: u16 = 54212;
const VLC_HTTP_PASSWORD: &str = "vlc2cc";
/// Base64 of ":vlc2cc"
const VLC_AUTH_HEADER: &str = "OnZsYzJjYw==";

#[derive(Clone, Default, Debug)]
pub struct PlaybackState {
    pub position_secs: f64,
    pub duration_secs: f64,
    pub is_playing: bool,
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

/// Send a command to VLC's HTTP interface.
fn send_vlc_command(command: &str) {
    let addr: std::net::SocketAddr = ([127, 0, 0, 1], VLC_HTTP_PORT).into();
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else {
        return;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let request = format!(
        "GET /requests/status.json?command={command} HTTP/1.0\r\nAuthorization: Basic {VLC_AUTH_HEADER}\r\n\r\n"
    );
    let _ = stream.write_all(request.as_bytes());
    let mut buf = [0u8; 512];
    let _ = stream.read(&mut buf);
}

/// Toggle pause/resume on the current stream.
pub fn toggle_pause() {
    send_vlc_command("pl_pause");
}

/// Seek to a specific position in seconds.
pub fn seek(seconds: f64) {
    send_vlc_command(&format!("seek&val={}", seconds as u64));
}

/// Stop playback, then kill VLC process and its entire process group.
pub fn kill_previous(child: &mut Option<Child>) {
    if let Some(proc) = child {
        if proc.try_wait().ok().flatten().is_none() {
            // First, gracefully stop the stream via HTTP
            send_vlc_command("pl_stop");
            std::thread::sleep(Duration::from_millis(300));

            // Kill the entire process group (VLC + any child processes)
            #[cfg(unix)]
            {
                let pgid = proc.id();
                let _ = Command::new("kill")
                    .args(["-TERM", &format!("-{pgid}")])
                    .status();
                std::thread::sleep(Duration::from_millis(500));
            }
            // Force kill if still running
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
pub fn launch_vlc(
    vlc_path: &str,
    video_file: &str,
    chromecast_ip: &str,
    previous: &mut Option<Child>,
) -> Result<(), String> {
    if vlc_path.is_empty() {
        return Err("VLC path is not set".to_string());
    }
    if video_file.is_empty() {
        return Err("No video file selected".to_string());
    }
    if chromecast_ip.is_empty() {
        return Err("Chromecast IP is not set".to_string());
    }

    kill_previous(previous);

    let args = build_vlc_args(video_file, chromecast_ip);

    let mut cmd = Command::new(vlc_path);
    cmd.args(&args);

    // Put VLC in its own process group so we can kill all child processes together
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch VLC: {e}"))?;

    *previous = Some(child);
    Ok(())
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

    Some(PlaybackState {
        position_secs: json["time"].as_f64().unwrap_or(0.0),
        duration_secs: json["length"].as_f64().unwrap_or(0.0),
        is_playing: json["state"].as_str() == Some("playing"),
    })
}

/// Start a background thread that polls VLC for playback status every second.
pub fn start_playback_monitor(
    state: Arc<Mutex<PlaybackState>>,
    stop: Arc<AtomicBool>,
    ctx: eframe::egui::Context,
) -> JoinHandle<()> {
    thread::spawn(move || {
        // Give VLC time to start up
        thread::sleep(Duration::from_secs(2));

        while !stop.load(Ordering::Relaxed) {
            if let Some(ps) = query_playback() {
                *state.lock().unwrap() = ps;
                ctx.request_repaint();
            }
            thread::sleep(Duration::from_secs(1));
        }
    })
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
