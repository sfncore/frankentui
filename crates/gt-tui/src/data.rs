use serde::Deserialize;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use ftui_runtime::subscription::{StopSignal, SubId, Subscription};

use crate::msg::Msg;

// --- Data types matching `gt status --json` output ---

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TownStatus {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub overseer: Overseer,
    #[serde(default)]
    pub agents: Vec<AgentInfo>,
    #[serde(default)]
    pub rigs: Vec<RigStatus>,
    #[serde(default)]
    pub summary: StatusSummary,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Overseer {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub unread_mail: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatusSummary {
    #[serde(default)]
    pub rig_count: u32,
    #[serde(default)]
    pub polecat_count: u32,
    #[serde(default)]
    pub crew_count: u32,
    #[serde(default)]
    pub active_hooks: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RigStatus {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub polecat_count: u32,
    #[serde(default)]
    pub crew_count: u32,
    #[serde(default)]
    pub has_witness: bool,
    #[serde(default)]
    pub has_refinery: bool,
    #[serde(default)]
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub has_work: bool,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub unread_mail: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct ConvoyItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub progress: String,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub done: u32,
    #[serde(default)]
    pub landed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GtEvent {
    pub timestamp: String,
    pub event_type: String,
    pub actor: String,
    pub message: String,
}

// --- CLI helpers ---

fn run_gt(args: &[&str]) -> Option<String> {
    let result = Command::new("gt")
        .args(args)
        .output()
        .ok()?;
    if result.status.success() {
        Some(String::from_utf8_lossy(&result.stdout).to_string())
    } else {
        // Some gt commands write JSON to stderr for non-zero exits
        let stdout = String::from_utf8_lossy(&result.stdout);
        if !stdout.trim().is_empty() {
            Some(stdout.to_string())
        } else {
            None
        }
    }
}

pub fn fetch_status() -> TownStatus {
    if let Some(output) = run_gt(&["status", "--json"]) {
        // Skip warning lines (start with WARNING or whitespace before JSON)
        let json_start = output.find('{').unwrap_or(0);
        let json = &output[json_start..];
        if let Ok(status) = serde_json::from_str::<TownStatus>(json) {
            return status;
        }
    }

    TownStatus {
        name: "Gas Town".to_string(),
        ..Default::default()
    }
}

pub fn fetch_convoys() -> Vec<ConvoyItem> {
    if let Some(output) = run_gt(&["convoy", "list", "--json"]) {
        let json_start = output.find('[').or_else(|| output.find('{')).unwrap_or(0);
        let json = &output[json_start..];
        if let Ok(items) = serde_json::from_str::<Vec<ConvoyItem>>(json) {
            return items;
        }
    }
    Vec::new()
}

fn parse_event_line(line: &str) -> Option<GtEvent> {
    let val: serde_json::Value = serde_json::from_str(line).ok()?;
    Some(GtEvent {
        timestamp: val.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        event_type: val.get("type").and_then(|v| v.as_str()).unwrap_or("event").to_string(),
        actor: val.get("actor").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        message: val.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    })
}

// --- Subscriptions ---

pub struct StatusPoller;

impl Subscription<Msg> for StatusPoller {
    fn id(&self) -> SubId {
        0x5354_4154 // "STAT"
    }

    fn run(&self, sender: mpsc::Sender<Msg>, stop: StopSignal) {
        loop {
            let status = fetch_status();
            if sender.send(Msg::StatusRefresh(status)).is_err() {
                break;
            }
            if stop.wait_timeout(Duration::from_secs(5)) {
                break;
            }
        }
    }
}

pub struct ConvoyPoller;

impl Subscription<Msg> for ConvoyPoller {
    fn id(&self) -> SubId {
        0x434F_4E56 // "CONV"
    }

    fn run(&self, sender: mpsc::Sender<Msg>, stop: StopSignal) {
        loop {
            let convoys = fetch_convoys();
            if sender.send(Msg::ConvoyRefresh(convoys)).is_err() {
                break;
            }
            if stop.wait_timeout(Duration::from_secs(10)) {
                break;
            }
        }
    }
}

pub struct EventTailer;

impl Subscription<Msg> for EventTailer {
    fn id(&self) -> SubId {
        0x4556_4E54 // "EVNT"
    }

    fn run(&self, sender: mpsc::Sender<Msg>, stop: StopSignal) {
        let path = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/home/ubuntu".to_string()))
            .join(".events.jsonl");

        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => {
                // No events file yet, poll until it appears
                loop {
                    if stop.wait_timeout(Duration::from_secs(2)) {
                        return;
                    }
                    if path.exists() {
                        break;
                    }
                }
                match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(_) => return,
                }
            }
        };

        let mut reader = BufReader::new(file);
        // Seek to end to only get new events
        let _ = reader.seek(SeekFrom::End(0));

        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    // No new data, wait briefly
                    if stop.wait_timeout(Duration::from_millis(500)) {
                        break;
                    }
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        if let Some(event) = parse_event_line(trimmed) {
                            if sender.send(Msg::NewEvent(event)).is_err() {
                                break;
                            }
                        }
                    }
                }
                Err(_) => {
                    if stop.wait_timeout(Duration::from_secs(1)) {
                        break;
                    }
                }
            }
        }
    }
}
