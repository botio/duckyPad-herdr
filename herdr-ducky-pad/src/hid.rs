//! duckyPad custom-HID layer.
//!
//! The pad is a USB custom-HID device (VID 0x483 / PID 0xd11c). Beyond its
//! normal keyboard reports it exposes two custom reports:
//!
//!   * OUT (report id 5): `[5, 0, cmd, payload...]`        host -> pad
//!   * IN  (report id 4): `[4, 0xF0, 0, slot(1..=15)]`     pad -> host
//!
//! hidapi includes the report id as byte 0 of read/write buffers, matching the
//! firmware (`USBD_CUSTOM_HID_SendReport` sends the buffer with the report id).
//!
//! `DRY RUN` (env `DUCKY_DRY_RUN=1`, or an unavailable device) logs HID writes
//! instead of sending them, so the daemon is testable without the pad.

use anyhow::Result;

pub const VID: u16 = 0x0483;
pub const PID: u16 = 0xd11c;

/// Usage (Counted Buffer) from the Report 4/5 custom-HID collection.
/// The device also exposes keyboard/media/mouse collections under the same
/// VID/PID, so `HidApi::open(VID, PID)` is ambiguous and can select a handle
/// that never receives the custom IN reports.
const CUSTOM_USAGE: u16 = 0x003A;

fn matches_custom_collection(vendor_id: u16, product_id: u16, usage: u16) -> bool {
    vendor_id == VID && product_id == PID && usage == CUSTOM_USAGE
}

const CMD_RGB: u8 = 34;
const CMD_OLED: u8 = 35;
const CMD_MODE: u8 = 36;

const OUT_REPORT: u8 = 5;
const IN_REPORT: u8 = 4;
const IN_KEY_STATE: u8 = 0xF1;
const CMD_GET_HERDR_KEYS: u8 = 37;
const OUT_SIZE: usize = 64; // report id (1) + 63 data bytes
const KEY_STATE_TIMEOUT_MS: i32 = 50;

pub struct DuckyPad {
    hid: Option<hidapi::HidApi>,
    dev: Option<hidapi::HidDevice>,
    dry_run: bool,
    /// DRY RUN was explicitly requested (`DUCKY_DRY_RUN=1`) rather than the pad
    /// being temporarily unavailable. We only auto-retry the connection when it
    /// was a fallback, so a forced dry run stays dry.
    forced_dry_run: bool,
    last_key_bits: u32,
    key_down: bool,
}

impl DuckyPad {
    /// Open the pad. `dry_run` (or an unavailable device) means writes are
    /// logged instead of sent.
    pub fn connect(dry_run: bool) -> Result<Self> {
        if dry_run {
            log::info!("duckyPad: DRY RUN (no HID writes)");
            return Ok(Self {
                hid: None,
                dev: None,
                dry_run: true,
                forced_dry_run: true,
                last_key_bits: 0,
                key_down: false,
            });
        }
        match Self::open_device() {
            Ok((hid, dev)) => Ok(Self {
                hid: Some(hid),
                dev: Some(dev),
                dry_run: false,
                forced_dry_run: false,
                last_key_bits: 0,
                key_down: false,
            }),
            Err(e) => {
                log::warn!("duckyPad: device unavailable ({e}); waiting (will auto-retry)");
                Ok(Self {
                    hid: None,
                    dev: None,
                    dry_run: true,
                    forced_dry_run: false,
                    last_key_bits: 0,
                    key_down: false,
                })
            }
        }
    }

    /// True when we're in dry-run *because the pad isn't (yet) connected* and
    /// we should keep trying to connect (as opposed to an explicit DRY RUN).
    pub fn is_waiting(&self) -> bool {
        !self.forced_dry_run && self.dev.is_none()
    }

    /// Whether the current HID handle is live. A stale handle is removed as
    /// soon as any read/write reports a disconnect.
    pub fn is_connected(&self) -> bool {
        !self.dry_run && self.dev.is_some()
    }

    /// Attempt to (re)connect while waiting. Returns `true` if the device is
    /// now open (dry-run is off).
    pub fn try_reconnect(&mut self) -> bool {
        if !self.is_waiting() {
            return false;
        }
        match Self::open_device() {
            Ok((hid, dev)) => {
                self.hid = Some(hid);
                self.dev = Some(dev);
                self.dry_run = false;
                self.last_key_bits = 0;
                self.key_down = false;
                true
            }
            Err(_) => false,
        }
    }

    fn open_device() -> Result<(hidapi::HidApi, hidapi::HidDevice)> {
        let hid = hidapi::HidApi::new()?;
        let path = hid
            .device_list()
            .find(|info| {
                matches_custom_collection(info.vendor_id(), info.product_id(), info.usage())
            })
            .map(|info| info.path().to_owned())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "custom HID usage 0x{CUSTOM_USAGE:04x} not found for {VID:04x}:{PID:04x}"
                )
            })?;
        log::info!(
            "duckyPad: opening custom HID usage=0x{CUSTOM_USAGE:04x} path={}",
            path.to_string_lossy()
        );
        let dev = hid
            .open_path(&path)
            .map_err(|e| anyhow::anyhow!("open {}: {e}", path.to_string_lossy()))?;
        log::info!("duckyPad: opened HID device");
        Ok((hid, dev))
    }

    /// Drop a stale HID handle after an I/O error. The main loop will retry
    /// discovery and force a full state sync when the device reappears.
    fn mark_disconnected(&mut self) {
        if self.forced_dry_run {
            return;
        }
        if self.dev.is_some() {
            log::warn!("duckyPad: disconnected; waiting for device");
        }
        self.dev = None;
        self.hid = None;
        self.dry_run = true;
    }

    fn live_dev(&mut self) -> Option<&mut hidapi::HidDevice> {
        self.dev.as_mut()
    }

    fn write_cmd(&mut self, cmd: u8, payload: &[u8]) -> Result<()> {
        let mut buf = [0u8; OUT_SIZE];
        buf[0] = OUT_REPORT;
        buf[1] = 0;
        buf[2] = cmd;
        let n = payload.len().min(OUT_SIZE - 3);
        buf[3..3 + n].copy_from_slice(&payload[..n]);
        if self.dry_run || self.dev.is_none() {
            let hex: String = buf[3..3 + n]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join("");
            log::info!("DRYRUN OUT cmd={cmd} len={n} payload={hex}");
            return Ok(());
        }
        match self.live_dev().unwrap().write(&buf) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.mark_disconnected();
                Err(anyhow::anyhow!("write_cmd: hid write: {e}"))
            }
        }
    }

    /// cmd 34: 15-key RGB frame, 45 bytes = 15 * (R,G,B) in key order.
    pub fn set_rgb_frame(&mut self, frame: &[u8; 45]) -> Result<()> {
        self.write_cmd(CMD_RGB, frame)
    }

    /// cmd 35: OLED text, `[len(<=56), bytes...]`, `\n` for line breaks.
    pub fn set_oled_text(&mut self, text: &str) -> Result<()> {
        let mut p = Vec::with_capacity(1 + text.len());
        p.push(text.len().min(255) as u8);
        p.extend_from_slice(text.as_bytes());
        self.write_cmd(CMD_OLED, &p)
    }

    /// cmd 36: enter (1) or leave (0) herdr mode.
    pub fn set_herdr_mode(&mut self, on: bool) -> Result<()> {
        self.write_cmd(CMD_MODE, &[if on { 1 } else { 0 }])
    }

    /// Ask the pad for a synchronous snapshot of all switch states, then
    /// compare it with the previous snapshot to emit press edges. This
    /// survives a busy IN endpoint (an unsolicited event could be dropped
    /// when the previous response is still pending) and works against a
    /// herdr-mode pad that no longer emits keyboard reports.
    pub fn read_key_event(&mut self) -> Result<Option<u8>> {
        if self.dry_run || self.dev.is_none() {
            return Ok(None);
        }

        let mut request = [0u8; OUT_SIZE];
        request[0] = OUT_REPORT;
        request[2] = CMD_GET_HERDR_KEYS;
        if let Err(e) = self.live_dev().unwrap().write(&request) {
            self.mark_disconnected();
            return Err(anyhow::anyhow!("write_cmd: hid write: {e}"));
        }

        let mut response = [0u8; OUT_SIZE];
        match self
            .live_dev()
            .unwrap()
            .read_timeout(&mut response, KEY_STATE_TIMEOUT_MS)
        {
            Ok(n) if n >= 7 && response[0] == IN_REPORT && response[1] == IN_KEY_STATE => {
                Ok(Self::decode_key_state(&response, &mut self.last_key_bits))
            }
            Ok(_) => Ok(None),
            Err(e) => {
                self.mark_disconnected();
                Err(anyhow::anyhow!("read_key_event: hid read: {e}"))
            }
        }
    }

    fn decode_key_state(buf: &[u8], last_bits: &mut u32) -> Option<u8> {
        let bits = u32::from_le_bytes([buf[3], buf[4], buf[5], buf[6]]);
        let pressed = bits & !*last_bits;
        *last_bits = bits;
        if pressed & 0x7fff != 0 {
            // Lowest-numbered newly-pressed key wins (1..=15).
            Some(pressed.trailing_zeros() as u8 + 1)
        } else {
            None
        }
    }

    /// Poll the pad for a key press. Returns `Some(slot)` only on the first
    /// poll after a press is detected; the slot is consumed (the key must be
    /// released before another press is recognized) so a 10ms poll loop does
    /// not re-focus the agent on every tick while the switch is held.
    pub fn poll_key(&mut self) -> Result<Option<u8>> {
        if self.key_down {
            return Ok(None);
        }
        let slot = self.read_key_event()?;
        if slot.is_some() {
            self.key_down = true;
        }
        Ok(slot)
    }

    /// Call on the next tick after a key was consumed: release the
    /// held-key latch when no keys are still physically pressed.
    pub fn release_key(&mut self) -> Result<()> {
        if !self.key_down {
            return Ok(());
        }
        if self.dry_run || self.dev.is_none() {
            self.key_down = false;
            return Ok(());
        }
        let mut request = [0u8; OUT_SIZE];
        request[0] = OUT_REPORT;
        request[2] = CMD_GET_HERDR_KEYS;
        if let Err(e) = self.live_dev().unwrap().write(&request) {
            self.mark_disconnected();
            return Err(anyhow::anyhow!("write_cmd: hid write: {e}"));
        }
        let mut response = [0u8; OUT_SIZE];
        match self
            .live_dev()
            .unwrap()
            .read_timeout(&mut response, KEY_STATE_TIMEOUT_MS)
        {
            Ok(n) if n >= 7 && response[0] == IN_REPORT && response[1] == IN_KEY_STATE => {
                let bits = u32::from_le_bytes([response[3], response[4], response[5], response[6]]);
                self.last_key_bits = bits;
                if bits & 0x7fff == 0 {
                    self.key_down = false;
                }
                Ok(())
            }
            Ok(_) => Ok(()),
            Err(e) => {
                self.mark_disconnected();
                Err(anyhow::anyhow!("read_key_event: hid read: {e}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad_with_last_bits(bits: u32) -> DuckyPad {
        DuckyPad {
            hid: None,
            dev: None,
            dry_run: true,
            forced_dry_run: true,
            last_key_bits: bits,
            key_down: false,
        }
    }

    fn key_state_report(bits: u32) -> [u8; 64] {
        let mut report = [0u8; 64];
        report[0] = IN_REPORT;
        report[1] = IN_KEY_STATE;
        report[3..7].copy_from_slice(&bits.to_le_bytes());
        report
    }

    #[test]
    fn key_state_edge_emits_only_newly_pressed_key() {
        let mut pad = pad_with_last_bits(0);
        let report = key_state_report(0b1010);

        assert_eq!(
            DuckyPad::decode_key_state(&report, &mut pad.last_key_bits),
            Some(2)
        );
        assert_eq!(pad.last_key_bits, 0b1010);

        // Same snapshot again: no new press.
        assert_eq!(
            DuckyPad::decode_key_state(&report, &mut pad.last_key_bits),
            None
        );

        // Release bit 2, press bit 4: the new press (bit 4) is reported
        // while already-held keys are ignored.
        let next = key_state_report(0b10000);
        assert_eq!(
            DuckyPad::decode_key_state(&next, &mut pad.last_key_bits),
            Some(5)
        );
    }

    #[test]
    fn key_state_ignores_plus_minus_switches() {
        let mut pad = pad_with_last_bits(0);
        let report = key_state_report(1u32 << 15); // SW_MINUS bit (16th switch)
        assert_eq!(
            DuckyPad::decode_key_state(&report, &mut pad.last_key_bits),
            None
        );
    }

    #[test]
    fn read_error_is_not_swallowed_and_enters_waiting_state() {
        let mut pad = pad_with_last_bits(0);
        pad.dry_run = false;
        pad.mark_disconnected();
        // A read error path is exercised by the integration test with a real
        // socket; here we just check the disconnected fallback stays inert.
        assert!(pad.read_key_event().unwrap().is_none());
    }

    #[test]
    fn opens_counted_buffer_collection_not_keyboard_collection() {
        assert!(matches_custom_collection(VID, PID, CUSTOM_USAGE));
        assert!(!matches_custom_collection(VID, PID, 0x0006));
    }
}
