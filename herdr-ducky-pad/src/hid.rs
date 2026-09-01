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

use anyhow::{Context, Result};

pub const VID: u16 = 0x0483;
pub const PID: u16 = 0xd11c;

const CMD_RGB: u8 = 34;
const CMD_OLED: u8 = 35;
const CMD_MODE: u8 = 36;

const OUT_REPORT: u8 = 5;
const IN_REPORT: u8 = 4;
const IN_KEY_EVENT: u8 = 0xF0;
const OUT_SIZE: usize = 64; // report id (1) + 63 data bytes

pub struct DuckyPad {
    hid: Option<hidapi::HidApi>,
    dev: Option<hidapi::HidDevice>,
    dry_run: bool,
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
            });
        }
        match Self::open_device() {
            Ok((hid, dev)) => {
                log::info!("duckyPad: opened HID device");
                Ok(Self {
                    hid: Some(hid),
                    dev: Some(dev),
                    dry_run: false,
                })
            }
            Err(e) => {
                log::warn!("duckyPad: device unavailable ({e}); falling back to DRY RUN");
                Ok(Self {
                    hid: None,
                    dev: None,
                    dry_run: true,
                })
            }
        }
    }

    fn open_device() -> Result<(hidapi::HidApi, hidapi::HidDevice)> {
        let hid = hidapi::HidApi::new()?;
        let dev = hid
            .open(VID, PID)
            .map_err(|e| anyhow::anyhow!("open ({}:{:04x}): {e}", VID, PID))?;
        Ok((hid, dev))
    }

    /// Re-open the HID device after a disconnect (no-op in dry run).
    pub fn reopen(&mut self) {
        if self.dry_run {
            return;
        }
        self.dev = None;
        self.hid = None;
        match Self::open_device() {
            Ok((hid, dev)) => {
                log::info!("duckyPad: re-opened HID device");
                self.hid = Some(hid);
                self.dev = Some(dev);
            }
            Err(e) => log::warn!("duckyPad: reopen failed ({e}); now dry-run"),
        }
    }

    fn live_dev(&mut self) -> Option<&mut hidapi::HidDevice> {
        self.dev.as_mut()
    }

    fn write_cmd(&mut self, cmd: u8, payload: &[u8]) -> Result<()> {
        let mut buf = vec![0u8; OUT_SIZE];
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
        self.live_dev()
            .unwrap()
            .write(&buf)
            .map_err(|e| anyhow::anyhow!("hid write: {e}"))
            .context("write_cmd")?;
        Ok(())
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

    /// Non-blocking read of a herdr key-press event. Returns the 1-based slot
    /// (1..=15) if a key was pressed, else `None`.
    pub fn read_key_event(&mut self) -> Result<Option<u8>> {
        if self.dry_run || self.dev.is_none() {
            return Ok(None);
        }
        let mut buf = [0u8; 64];
        if let Ok(n) = self.live_dev().unwrap().read_timeout(&mut buf, 0) {
            if n >= 4 && buf[0] == IN_REPORT && buf[1] == IN_KEY_EVENT {
                return Ok(Some(buf[3]));
            }
        }
        Ok(None)
    }
}
