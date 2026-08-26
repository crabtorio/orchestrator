use env_logger::Builder;
use log::{Level, LevelFilter};
use regex::Regex;
use std::io::{IsTerminal, Write};
use std::sync::OnceLock;
use std::{format, write, writeln};

// --- ANSI helpers -----------------------------------------------------

fn color_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED
        .get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal())
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

pub fn init() {
    // To silence all other crates' logging

    Builder::new()
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
        .init();
}
