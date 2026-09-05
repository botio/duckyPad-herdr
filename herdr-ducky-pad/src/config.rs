//! User configuration for the duckyPad bridge.
//!
//! Reads `duckyPad/herdr.json` (XDG config dir) written by the duckyPad
//! Configurator. The file is optional: when absent or invalid the daemon
//! keeps its built-in palette and sticky (pane_id-based) slot assignment.
//!
//! Schema (v1):
//! ```json
//! {
//!   "schema_version": 1,
//!   "colors": { "working": [0, 255, 0], "blocked": [255, 0, 0] },
//!   "pinned_slots": { "1": "pane-abc", "2": "pane-def" }
//! }
//! ```
//!
//! - `colors`: state name -> `[r, g, b]`. Only present keys override the
//!   built-in palette; missing keys keep the built-in color.
//! - `pinned_slots`: agent slot (1..14) -> pane_id. Key 15 is the fixed F9
//!   shortcut and cannot be pinned. When a pane_id is pinned, the agent stays
//!   on that slot (overriding the sticky lowest-free-slot rule). Unpinned slots
//!   use the sticky rule.

use std::collections::HashMap;
use crate::model::AGENT_SLOTS;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default)]
pub struct HerdrConfig {
    /// state name -> [r, g, b] override.
    pub colors: HashMap<String, [u8; 3]>,
    /// Agent slot (1..14) -> pane_id pin; key 15 is the fixed F9 shortcut.
    pub pinned_slots: HashMap<usize, String>,
}

impl HerdrConfig {
    /// Load from the standard config path. Returns a default (empty) config
    /// when the file is absent; logs a warning and returns a default config
    /// when the file exists but is invalid.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match Self::parse(&text) {
                Ok(cfg) => cfg,
                Err(e) => {
                    log::warn!("herdr config {path:?} invalid: {e}; using defaults");
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                log::warn!("herdr config {path:?} unreadable: {e}; using defaults");
                Self::default()
            }
        }
    }

    /// Parse a config file's JSON text.
    pub fn parse(text: &str) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Raw {
            #[serde(default = "default_version")]
            schema_version: u32,
            #[serde(default)]
            colors: HashMap<String, Vec<u32>>,
            #[serde(default)]
            pinned_slots: HashMap<String, String>,
        }
        fn default_version() -> u32 {
            SCHEMA_VERSION
        }

        let raw: Raw = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if raw.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema_version {} (expected {SCHEMA_VERSION})",
                raw.schema_version
            ));
        }

        let mut colors = HashMap::new();
        for (name, rgb) in raw.colors {
            if rgb.len() != 3 {
                return Err(format!("color for {name:?} must be [r, g, b]"));
            }
            let [r, g, b] = [rgb[0], rgb[1], rgb[2]].map(|v| v as u8);
            colors.insert(name, [r, g, b]);
        }

        let mut pinned_slots = HashMap::new();
        for (slot, pane) in raw.pinned_slots {
            let slot: usize = slot
                .parse()
                .map_err(|_| format!("slot {slot:?} not an integer"))?;
            if !(1..=AGENT_SLOTS).contains(&slot) {
                return Err(format!("slot {slot} out of range 1..{AGENT_SLOTS}"));
            }
            if pane.is_empty() {
                return Err(format!("slot {slot} pane_id must be a non-empty string"));
            }
            pinned_slots.insert(slot, pane);
        }

        Ok(Self {
            colors,
            pinned_slots,
        })
    }
}

/// Standard config file location: `$XDG_CONFIG_HOME/duckyPad/herdr.json`
/// (Linux) or the platform config dir otherwise.
pub fn config_path() -> std::path::PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("duckyPad").join("herdr.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let cfg = HerdrConfig::parse("{}").unwrap();
        assert!(cfg.colors.is_empty());
        assert!(cfg.pinned_slots.is_empty());
    }

    #[test]
    fn parse_colors_and_pins() {
        let text = r#"{
            "schema_version": 1,
            "colors": {"working": [10, 20, 30], "blocked": [255, 0, 0]},
            "pinned_slots": {"1": "pane-a", "14": "pane-z"}
        }"#;
        let cfg = HerdrConfig::parse(text).unwrap();
        assert_eq!(cfg.colors.get("working").copied(), Some([10, 20, 30]));
        assert_eq!(cfg.colors.get("blocked").copied(), Some([255, 0, 0]));
        assert_eq!(cfg.pinned_slots.get(&1), Some(&"pane-a".to_string()));
        assert_eq!(cfg.pinned_slots.get(&14), Some(&"pane-z".to_string()));
    }

    #[test]
    fn reject_bad_version() {
        let err = HerdrConfig::parse(r#"{"schema_version": 99}"#)
            .unwrap_err();
        assert!(err.contains("schema_version"));
    }

    #[test]
    fn reject_short_rgb() {
        let err = HerdrConfig::parse(r#"{"colors": {"working": [1, 2]}}"#)
            .unwrap_err();
        assert!(err.contains("working"));
    }

    #[test]
    fn reject_f9_slot_pin() {
        let err = HerdrConfig::parse(r#"{"pinned_slots": {"15": "x"}}"#)
            .unwrap_err();
        assert!(err.contains("15"));
    }

    #[test]
    fn reject_empty_pane() {
        let err = HerdrConfig::parse(r#"{"pinned_slots": {"3": ""}}"#)
            .unwrap_err();
        assert!(err.contains("pane_id"));
    }
}
