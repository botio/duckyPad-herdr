//! duckyPad <-> herdr bridge daemon.
//!
//! The pad is a 15-key custom-HID device. This daemon polls herdr's Unix
//! socket for the live set of agents, lights one NeoPixel key per agent
//! (colored by state: blocked=red, working=green, done=blue, idle=dim-gray,
//! unknown=amber), lists the mapped agents on the OLED, and — when a key is
//! pressed — focuses that agent's pane.
//!
//! The herdr API socket handles one request per connection, so `agent.list` is
//! polled on a short interval and a key press issues a one-shot `agent.focus`.
//! There is no persistent subscription the daemon holds open.

mod herdr;
mod hid;
mod model;

use anyhow::Result;
use herdr::HerdrClient;
use hid::DuckyPad;
use model::{Agent, SLOTS};
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
/// else a named session socket, else the default.
fn socket_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("HERDR_SOCKET_PATH") {
        return std::path::PathBuf::from(p);
    }
    let base = config_dir().join("herdr");
    if let Ok(session) = std::env::var("HERDR_SESSION") {
        return base.join("sessions").join(session).join("herdr.sock");
    }
    base.join("herdr.sock")
}

struct Daemon {
    pad: DuckyPad,
    client: HerdrClient,
    agents: Vec<Agent>,
    last_rgb: [u8; 45],
    last_oled: String,
    last_summary: String,
    last_relist: Instant,
    last_pad_retry: Instant,
    last_pad_sync: Instant,
    need_relist: bool,
}

impl Daemon {
    fn new(pad: DuckyPad, client: HerdrClient) -> Self {
        Self {
            pad,
            client,
            agents: Vec::new(),
            last_rgb: [0; 45],
            last_oled: String::new(),
            last_summary: String::new(),
            last_relist: Instant::now(),
            last_pad_retry: Instant::now(),
            last_pad_sync: Instant::now(),
            need_relist: true,
        }
    }

    /// Push the computed RGB frame + OLED text. Normal polls send only changed
    /// values; reconnect/heartbeat calls force a full replay because a reset
    /// clears the pad's RAM even though the herdr agent list did not change.
    fn push_pad_state(&mut self, force: bool) {
        if force {
            if let Err(e) = self.pad.set_herdr_mode(true) {
                log::warn!("set_herdr_mode: {e:#}");
                return;
            }
        }

        let slots = model::assign_slots(&self.agents);
        let rgb = model::rgb_frame(&slots);
        if force || rgb != self.last_rgb {
            if let Err(e) = self.pad.set_rgb_frame(&rgb) {
                log::warn!("set_rgb_frame: {e:#}");
                return;
            }
            self.last_rgb = rgb;
        }

        let oled = model::oled_text(&slots);
        if force || oled != self.last_oled {
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
            }
            Err(e) => {
                // Keep the last-known agents (a brief herdr hiccup shouldn't
                // darken the pad); the next successful relist refreshes them.
                log::warn!("herdr agent.list: {e:#}");
            }
        }
        self.last_relist = Instant::now();
        self.need_relist = false;
    }

    /// If a key was pressed on the pad, focus the agent in that slot. A read
    /// error marks the stale HID handle as disconnected inside `DuckyPad`;
    /// `tick` will then discover the replacement handle and replay pad state.
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
        if slot < 1 || slot > SLOTS as u8 {
            return;
        }
        let Some(target) = model::assign_slots(&self.agents)
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

    // Enter herdr mode so the pad suppresses keyboard reports and treats the
    // keys as agent-focus inputs. A failure here shouldn't kill the daemon.
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
