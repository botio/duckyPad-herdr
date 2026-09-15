//! duckyPad <-> herdr bridge daemon.
//!
//! The pad has 14 agent keys plus a firmware-owned F9 key. This daemon polls
//! herdr's Unix socket for the live set of agents, lights one NeoPixel key per
//! agent (colored by state: blocked=red, working=green, done=blue,
//! idle=dim-gray, unknown=amber), lists mapped agents on the OLED, and — when
//! an agent key is pressed — focuses that agent's pane. Key 15 stays white at
//! rest and is handled entirely by firmware as an F9 shortcut.
//!
//! The herdr API socket handles one request per connection, so `agent.list` is
//! polled on a short interval and a key press issues a one-shot `agent.focus`.
//! There is no persistent subscription the daemon holds open.

mod config;
mod herdr;
mod hid;
mod model;

use anyhow::Result;
use herdr::HerdrClient;
use hid::DuckyPad;
use model::{Agent, SlotMap, AGENT_SLOTS};
use std::time::{Duration, Instant};

/// How often to re-poll herdr for the agent list.
const RELIST_PERIOD: Duration = Duration::from_secs(2);

/// Full pad-state heartbeat. Besides keeping herdr mode asserted, this detects
/// stale HID handles after a hardware reset even when the agent list is stable.
const PAD_SYNC_PERIOD: Duration = Duration::from_secs(2);

/// Main-loop cadence (also the key-press poll interval).
const TICK_PERIOD: Duration = Duration::from_millis(10);

fn config_dir() -> std::path::PathBuf {
    dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Resolve the herdr socket path: `HERDR_SOCKET_PATH` (injected by herdr),
/// else a named session socket, else the newest session socket, else default.
fn socket_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("HERDR_SOCKET_PATH") {
        return std::path::PathBuf::from(p);
    }
    let base = config_dir().join("herdr");
    if let Ok(session) = std::env::var("HERDR_SESSION") {
        return base.join("sessions").join(session).join("herdr.sock");
    }
    let default = base.join("herdr.sock");
    if default.exists() {
        return default;
    }
    // herdr named sessions put the socket under sessions/<id>/herdr.sock
    let sessions = base.join("sessions");
    if let Ok(entries) = std::fs::read_dir(&sessions) {
        let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        for entry in entries.flatten() {
            let sock = entry.path().join("herdr.sock");
            if let Ok(meta) = sock.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                        best = Some((mtime, sock));
                    }
                }
            }
        }
        if let Some((_, path)) = best {
            return path;
        }
    }
    default
}
struct Daemon {
    pad: DuckyPad,
    client: HerdrClient,
    agents: Vec<Agent>,
    /// Sticky pane_id -> key assignment; survives re-lists and state changes.
    slot_map: SlotMap,
    /// User config (color palette + pinned slots); reloaded on every relist.
    config: config::HerdrConfig,
    last_rgb: [u8; 45],
    last_oled: String,
    last_summary: String,
    last_relist: Instant,
    last_pad_retry: Instant,
    last_pad_sync: Instant,
    need_relist: bool,
    /// Rate-limit "herdr.sock missing" warnings.
    herdr_warn_at: Option<Instant>,
}

impl Daemon {
    fn new(pad: DuckyPad, client: HerdrClient) -> Self {
        let config = config::HerdrConfig::load();
        Self {
            pad,
            client,
            agents: Vec::new(),
            slot_map: SlotMap::default(),
            config,
            last_rgb: [0; 45],
            last_oled: String::new(),
            last_summary: String::new(),
            last_relist: Instant::now(),
            last_pad_retry: Instant::now(),
            last_pad_sync: Instant::now(),
            need_relist: true,
            herdr_warn_at: None,
        }
    }

    /// Push RGB + OLED. Heartbeats replay unchanged state so entering a Herdr
    /// profile or reconnecting the pad does not require an agent-state change.
    /// Firmware 3.1.15+ ignores this display data in ordinary macro profiles.
    fn push_pad_state(&mut self, force: bool) {
        if force {
            if let Err(e) = self.pad.set_herdr_mode(true) {
                log::warn!("set_herdr_mode: {e:#}");
                return;
            }
        }
        let slots = self.slot_map.update(&self.agents, &self.config.pinned_slots);
        let rgb = model::rgb_frame(&slots, &self.config.colors);
        if force || rgb != self.last_rgb {
            if let Err(e) = self.pad.set_rgb_frame(&rgb) {
                log::warn!("set_rgb_frame: {e:#}");
                return;
            }
            self.last_rgb = rgb;
        }

        let oled = model::oled_text(&slots);
        // Heartbeats must not force an identical OLED rewrite: firmware OLED
        // I2C is long and used to race NeoPixel timing when every sync redrew it.
        if oled != self.last_oled {
            if let Err(e) = self.pad.set_oled_text(&oled) {
                log::warn!("set_oled_text: {e:#}");
                return;
            }
            self.last_oled = oled;
        }
    }

    /// Log a compact agent summary, but only when it actually changes (so a
    /// steady state doesn't spam a line every relist).
    fn note_agents(&mut self) {
        let mut keys: Vec<String> = self
            .agents
            .iter()
            .map(|a| format!("{}={:?}", a.name, a.state))
            .collect();
        keys.sort();
        let line = format!("herdr: {} agent(s) {}", self.agents.len(), keys.join(" "));
        if line != self.last_summary {
            log::info!("{line}");
            self.last_summary = line;
        }
    }

    /// One-shot `agent.list`; refresh the tracked agents and push to the pad.
    fn poll_agents(&mut self) {
        // Reload user config each relist so palette/pin edits apply without
        // a daemon restart.
        self.config = config::HerdrConfig::load();
        // Re-resolve the socket each poll: herdr may start later, or move to a
        // session path after the bridge launched.
        self.client = HerdrClient::new(socket_path());
        match self.client.agent_list() {
            Ok(v) => {
                self.agents = v
                    .get("result")
                    .and_then(|r| r.get("agents"))
                    .and_then(|a| a.as_array())
                    .map(|arr| arr.iter().filter_map(Agent::from_value).collect())
                    .unwrap_or_default();
                self.note_agents();
                self.push_pad_state(false);
                self.herdr_warn_at = None;
            }
            Err(e) => {
                // Keep the last-known agents (a brief herdr hiccup shouldn't
                // darken the pad); the next successful relist refreshes them.
                // Rate-limit the warning — a missing herdr.sock used to spam
                // once every 2s forever.
                let now = Instant::now();
                let should_log = self
                    .herdr_warn_at
                    .map(|t| now.duration_since(t) >= Duration::from_secs(30))
                    .unwrap_or(true);
                if should_log {
                    log::warn!(
                        "herdr agent.list: {e:#} (is herdr running? socket={})",
                        socket_path().display()
                    );
                    self.herdr_warn_at = Some(now);
                }
            }
        }
        self.last_relist = Instant::now();
        self.need_relist = false;
    }

    /// If an agent key was pressed on the pad, focus the agent in that slot.
    /// Key 15 is masked out of the agent key state by the firmware: it is the
    /// local shortcut key (F9 by default, or a user-assigned duckyScript), never
    /// an agent focus. A read error marks the stale HID handle as disconnected
    /// inside `DuckyPad`; `tick` will then discover the replacement handle and
    /// replay pad state.
    fn poll_key(&mut self) {
        if let Err(e) = self.pad.release_key() {
            log::warn!("release_key: {e:#}");
            return;
        }
        let slot = match self.pad.poll_key() {
            Ok(Some(slot)) => slot,
            Ok(None) => return,
            Err(e) => {
                log::warn!("poll_key: {e:#}");
                return;
            }
        };
        if !(1..=AGENT_SLOTS as u8).contains(&slot) {
            return;
        }
        let Some(target) = self
            .slot_map
            .update(&self.agents, &self.config.pinned_slots)
            .get((slot - 1) as usize)
            .and_then(|s| *s)
            .map(|a| a.pane_id.clone())
        else {
            log::info!("key {slot}: no agent in this slot");
            return;
        };
        log::info!("key {slot} -> focus {target}");
        if let Err(e) = self.client.focus(&target) {
            log::warn!("focus({target}): {e:#}");
        }
    }

    /// One loop tick: reconnect or heartbeat the pad, maybe re-poll herdr, and
    /// service any key press.
    fn tick(&mut self) {
        let now = Instant::now();
        if self.pad.is_waiting() && now.duration_since(self.last_pad_retry) >= RELIST_PERIOD {
            self.last_pad_retry = now;
            if self.pad.try_reconnect() {
                self.last_pad_sync = now;
                self.push_pad_state(true);
            }
        } else if self.pad.is_connected()
            && now.duration_since(self.last_pad_sync) >= PAD_SYNC_PERIOD
        {
            // This full replay is also the connection probe. A stale handle
            // fails the first write and moves the pad into the waiting state.
            self.last_pad_sync = now;
            self.push_pad_state(true);
        }

        if self.need_relist || now.duration_since(self.last_relist) >= RELIST_PERIOD {
            self.poll_agents();
        }
        self.poll_key();
    }
}

fn run() -> Result<()> {
    let dry_run = std::env::var("DUCKY_DRY_RUN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let pad = DuckyPad::connect(dry_run)?;
    let client = HerdrClient::new(socket_path());
    let mut d = Daemon::new(pad, client);

    // Advertise availability; selecting the Herdr profile is a local action.
    // A failure here should not kill the daemon.
    if let Err(e) = d.pad.set_herdr_mode(true) {
        log::warn!("set_herdr_mode: {e:#}");
    }

    loop {
        d.tick();
        std::thread::sleep(TICK_PERIOD);
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!(
        "ducky-pad-bridge starting (socket={})",
        socket_path().display()
    );
    if let Err(e) = run() {
        log::error!("fatal: {e:#}");
        std::process::exit(1);
    }
}
