use env_logger::Builder;
use log::{Level, LevelFilter};
use regex::Regex;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::{format, write, writeln};

// --- ANSI helpers -----------------------------------------------------

fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

const RESET: &str = "\x1b[0m";

fn paint(code: &str, bold: bool, text: &str) -> String {
    if !color_enabled() {
        return text.to_string();
    }
    if bold {
        format!("\x1b[1;{code}m{text}{RESET}")
    } else {
        format!("\x1b[{code}m{text}{RESET}")
    }
}

// 30-37 standard ANSI foreground colors
const RED: &str = "31";
const GREEN: &str = "32";
const YELLOW: &str = "33";
const BLUE: &str = "34";
const MAGENTA: &str = "35";
const CYAN: &str = "36";
const WHITE: &str = "37";
const GRAY: &str = "90"; // bright black

// --- Regexes ------------------------------------------------------------

fn sender_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"sender: (?:Some\(Participant \{ actor_type: (\w+), id: (\d+) \}\)|None)")
            .unwrap()
    })
}
fn receiver_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"receiver: (?:Some\(Participant \{ actor_type: (\w+), id: (\d+) \}\)|None)")
            .unwrap()
    })
}
fn event_type_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"event_type: (\w+)").unwrap())
}
fn channel_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"channel: (\w+)").unwrap())
}
fn payload_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"payload: (.+) \}\s*$").unwrap())
}
fn message_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"^\{"Message": "(.*)"\}$"#).unwrap())
}

struct ParsedEvent<'a> {
    sender_type: Option<&'a str>,
    sender_id: Option<&'a str>,
    receiver_type: Option<&'a str>,
    receiver_id: Option<&'a str>,
    event_type: &'a str,
    channel: &'a str,
    payload: String,
}

fn parse_log_event(msg: &str) -> Option<ParsedEvent<'_>> {
    if !msg.starts_with("LogEvent") {
        return None;
    }
    let sc = sender_re().captures(msg)?;
    let rc = receiver_re().captures(msg)?;
    let event_type = event_type_re().captures(msg)?.get(1)?.as_str();
    let channel = channel_re().captures(msg)?.get(1)?.as_str();
    let payload_raw = payload_re()
        .captures(msg)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .unwrap_or("");
    let payload = message_re()
        .captures(payload_raw)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| payload_raw.to_string());

    Some(ParsedEvent {
        sender_type: sc.get(1).map(|m| m.as_str()),
        sender_id: sc.get(2).map(|m| m.as_str()),
        receiver_type: rc.get(1).map(|m| m.as_str()),
        receiver_id: rc.get(2).map(|m| m.as_str()),
        event_type,
        channel,
        payload,
    })
}

fn actor_code(actor_type: Option<&str>) -> &'static str {
    match actor_type {
        Some("Orchestrator") => MAGENTA,
        Some("Explorer") => CYAN,
        _ => WHITE,
    }
}

fn channel_code(channel: &str) -> &'static str {
    match channel {
        "Error" => RED,
        "Warn" => YELLOW,
        "Debug" => BLUE,
        "Info" => GREEN,
        _ => WHITE,
    }
}

fn level_code(level: Level) -> &'static str {
    match level {
        Level::Error => RED,
        Level::Warn => YELLOW,
        Level::Info => GREEN,
        Level::Debug => BLUE,
        Level::Trace => CYAN,
    }
}
fn spawn_log_viewer(path_str: String) {
    let tail_cmd = format!("tail -n +1 -f '{}'", path_str);

    let launched = if std::env::var_os("TMUX").is_some() {
        Command::new("tmux")
            .args(["split-window", "-d", "-h", &tail_cmd])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        Command::new("kitty")
            .args(["@", "launch", "--type=os-window", "sh", "-c", &tail_cmd])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    } else if std::env::consts::OS == "macos" {
        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            tail_cmd
        );
        Command::new("osascript")
            .args(["-e", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        // Try other Linux terminals
        [
            "x-terminal-emulator",
            "gnome-terminal",
            "konsole",
            "xterm",
            "alacritty",
        ]
        .iter()
        .any(|term| {
            let mut cmd = Command::new(term);
            match *term {
                "gnome-terminal" => {
                    cmd.args(["--", "sh", "-c", &tail_cmd]);
                }
                "x-terminal-emulator" | "xterm" | "alacritty" => {
                    cmd.args(["-e", "sh", "-c", &tail_cmd]);
                }
                "konsole" => {
                    cmd.args(["-e", "sh", "-c", &tail_cmd]);
                }
                _ => unreachable!(),
            }
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        })
    } else if std::env::consts::OS == "windows" {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_CONSOLE: u32 = 0x00000010;
            let ps_cmd = format!(
                "while ($true) {{ Get-Content -Path '{}' -Tail 1000 -Wait -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 500 }}",
                path_str.replace('\'', "''")
            );
            Command::new("powershell")
                .args(["-NoExit", "-Command", &ps_cmd])
                .creation_flags(CREATE_NEW_CONSOLE)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        }
        #[cfg(not(windows))]
        {
            false
        }
    } else {
        false
    };

    if !launched {
        // If it fails, run anyway but advise the user
        eprintln!(
            "(couldn't open a live log view, run `tail -f {}` in another terminal to see logs)",
            path_str
        );
    }
}

pub fn init() {
    let log_path = "./orchestrator.log";
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)
        .expect("failed to open log file");

    let _ = writeln!(file, "--- orchestrator log started ---");

    Builder::new()
        .target(env_logger::Target::Pipe(Box::new(file)))
        .format(|buf, record| {
            let msg = record.args().to_string();

            write!(
                buf,
                "[{} {} {}] ",
                paint(GRAY, false, &buf.timestamp().to_string()),
                paint(
                    level_code(record.level()),
                    true,
                    &format!("{:5}", record.level())
                ),
                paint(GRAY, false, record.target()),
            )?;

            match parse_log_event(&msg) {
                Some(ev) => {
                    let sender = match (ev.sender_type, ev.sender_id) {
                        (Some(t), Some(i)) => format!("{t}#{i}"),
                        _ => "-".to_string(),
                    };
                    let receiver = match (ev.receiver_type, ev.receiver_id) {
                        (Some(t), Some(i)) => format!("{t}#{i}"),
                        _ => "-".to_string(),
                    };

                    writeln!(
                        buf,
                        "{} {} {} {} {}",
                        paint(actor_code(ev.sender_type), true, &sender),
                        paint(GRAY, false, "->"),
                        paint(actor_code(ev.receiver_type), true, &receiver),
                        paint(
                            channel_code(ev.channel),
                            false,
                            &format!("[{}]", ev.event_type)
                        ),
                        ev.payload,
                    )
                }
                None => writeln!(buf, "{}", msg),
            }
        })
        .filter(None, LevelFilter::Off)
        .filter(Some("orchestrator"), LevelFilter::Debug)
        .filter(Some("explorer_common"), LevelFilter::Trace)
        .filter(Some("common_game"), LevelFilter::Trace)
        .filter(Some("ml-explorer"), LevelFilter::Debug)
        .filter(Some("az-explorer"), LevelFilter::Debug)
        .init();

    spawn_log_viewer(log_path.into());
}
