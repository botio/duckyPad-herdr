# duckyPad Herdr Bridge

> 繁體中文版 → [README.zh-TW.md](README.zh-TW.md)

Turn a [duckyPad](https://github.com/dekuNukem/duckyPad) (2020 / OG) into a **herdr light board**:

- **Keys 1–14** = up to 14 herdr agents, color = state
- **Press a key** → herdr focuses that agent
- **Key 15** = local shortcut (**F9** by default)
- **OLED** = `H:Herdr` plus a 15-cell grid of agent names (4 chars; empty `-`)
- Herdr is **one profile**. **+ / −** leaves it and restores normal macros

![duckyPad as a herdr light board](img/duckypad-herdr.webp)

---

## You need all five

| # | Need | If missing |
|---|------|------------|
| 1 | duckyPad **plugged in over USB** | 3.1.33+ is a Herdr light board with no SD; a card is only needed for normal macro profiles |
| 2 | **herdr actually running** | Lights clear; log says `herdr.sock: No such file` |
| 3 | Firmware **3.1.33-herdr** | Older firmware will not draw grid names or accept the Bridge |
| 4 | Configurator **5.0.29+** writes a **Herdr profile**, pad **+ / −** selects it | Bridge can connect and still not take over the display |
| 5 | This Bridge (`./install.sh`) | Pad stays dark (key 15 may still be white) |

Current versions: **firmware 3.1.33-herdr**, **Bridge 0.2.8**.

---

## First install (in order, do not skip)

### 1. Flash firmware 3.1.33-herdr

Install [`dfu-util`](http://dfu-util.sourceforge.net/):

```bash
# macOS
brew install dfu-util

# Linux (Debian / Ubuntu)
sudo apt install dfu-util
```

1. **Hold the DFU button** on the back of the pad, then plug in USB; release after it enumerates
2. From **this repo's root**:

```bash
dfu-util --device 0483:df11 -a 0 -D firmware/duckypad_v3.1.33-herdr.dfu
```

3. Press **RESET** on the pad (or unplug/replug)
4. Boot OLED should show **`duckyPad V3.1.33`**

Prebuilt file: [GitHub Release v3.1.32](https://github.com/botio/duckyPad-herdr/releases/tag/v3.1.33).  
Stock recovery and screenshots: [`firmware_updates_and_version_history.md`](../firmware_updates_and_version_history.md).

### 2. Write a Herdr profile

1. Connect the pad with [duckyPad Configurator **5.0.29+**](https://github.com/botio/duckyPad-Configurator)
2. **ADD HERDR PROFILE**, then **SAVE** at the bottom onto the SD card
3. On the pad, **+ / −** until that Herdr profile is selected
4. OLED title should be **`H:Herdr`** (or `H:` plus the name you gave it)

The SD marker must be exactly `HERDR_PROFILE 1`. A renamed profile without that marker **does not** enter Herdr.

### 3. Install the Bridge

You need [Rust](https://rustup.rs/) (`cargo`). `herdr` should be on PATH.

```bash
cd herdr-ducky-pad
./install.sh
```

The script builds the daemon, registers the herdr plugin, and installs a **user service that starts at login**. Re-run after `git pull`; it is idempotent.

| OS | Installed as | Status | Logs |
|----|--------------|--------|------|
| Linux | `~/.config/systemd/user/ducky-pad-bridge.service` | `systemctl --user status ducky-pad-bridge` | `journalctl --user -u ducky-pad-bridge -f` |
| macOS | LaunchAgent `com.botio.ducky-pad-bridge` | `launchctl list \| grep ducky-pad-bridge` | `tail -f /tmp/ducky-pad-bridge.log` |

### 4. macOS required: Input Monitoring

macOS will **not** let a launchd CLI binary open keyboard HID (`0xE00002E2 not permitted`).  
`install.sh` copies the daemon into:

`~/Library/Application Support/ducky-pad-bridge/DuckyPadBridge.app`

1. System Settings → Privacy & Security → **Input Monitoring**
2. **+** add that **DuckyPadBridge.app** (not `target/release/ducky-pad-bridge`)
3. Toggle it off and on
4. Restart:

```bash
launchctl kickstart -k "gui/$(id -u)/com.botio.ducky-pad-bridge"
```

**Every** `./install.sh` needs step 3 again (ad-hoc signature changes).

### 5. Confirm it works

Log **must** contain:

```
duckyPad: opened HID device
herdr: N agent(s) ...
```

Log **must not** contain:

| Line | Meaning |
|------|---------|
| `DRYRUN OUT` | Command never reached the pad (HID not open) |
| `0xE00002E2` / `not permitted` | Input Monitoring not granted to **DuckyPadBridge.app** |
| `herdr.sock: No such file` | herdr is not running |

On the pad:

- OLED: `H:Herdr`, cells like `omp` / `pi`, empty `-`
- Agent keys lit by state; key 15 white at rest
- Press an agent key → herdr focuses that pane

---

## Daily use

1. Open **herdr**
2. **+ / −** to the **Herdr profile** (OLED title `H:…`)
3. Leave the Bridge service running (it starts at login after install)

| Action | Result |
|--------|--------|
| Colors on keys 1–14 | Agent state (table below) |
| Press key 1–14 | Focus that herdr agent |
| Press key 15 | **F9** by default (red while held, white at rest) |
| Write `key15.dsb` in the Configurator | Key 15 runs that macro instead of F9 |
| **+ / −** off Herdr | That profile's own names and macros return |

### Colors

| herdr state | Key color |
|-------------|-----------|
| `blocked` | red |
| `working` | green |
| `done` | blue |
| `unknown` | amber |
| `idle` | dim gray |

More than 14 agents: first 14 in herdr list order; extras wait for a free slot. Slots are sticky across state changes.

Change colors: Configurator **5.0.29+ → HERDR → STATUS COLORS → SAVE COLORS**. No JSON editor.  
Linux: `~/.config/duckyPad/herdr.json`  
macOS: `~/Library/Application Support/duckyPad/herdr.json`

---

## Update / restart

```bash
cd herdr-ducky-pad
git pull
./install.sh
```

Restart only:

```bash
# Linux
systemctl --user restart ducky-pad-bridge

# macOS
launchctl kickstart -k "gui/$(id -u)/com.botio.ducky-pad-bridge"
```

After a macOS reinstall: toggle **Input Monitoring** for DuckyPadBridge.app off/on.

---

## Troubleshooting

| Symptom | Check |
|---------|--------|
| OLED is not `H:…` | Herdr profile not selected (**+ / −**), or Configurator never SAVE'd |
| Grid is all `-`, keys dark | HID not open, or herdr down. Read the log |
| Log has `opened HID` but pad still dark | Firmware is not 3.1.33-herdr; with an SD, select the Herdr profile. No SD auto-enters Herdr |
| `DRYRUN OUT` | No pad, or HID open failed. On Mac, Input Monitoring first |
| `0xE00002E2 not permitted` | Add **DuckyPadBridge.app**, not the CLI path |
| `herdr.sock: No such file` | Start herdr. The Bridge looks in `~/.config/herdr/herdr.sock` |
| herdr has agents, pad does nothing | Log needs **both** `opened HID device` **and** `herdr: N agent(s)` |
| Linux sees USB but cannot open | `lsusb` should show `0483:d11c`; fix hidraw udev / group if needed |
| Flash seemed to do nothing | Must be in DFU (hold DFU, then plug in). Press RESET after flash |

Named herdr session:

```bash
export HERDR_SOCKET_PATH=/path/to/herdr.sock
```

More log: `RUST_LOG=debug` in the Linux unit / macOS plist, then restart the service.

---

## Advanced

### Dry run without a pad

```bash
DUCKY_DRY_RUN=1 ./target/release/ducky-pad-bridge
```

Still talks to herdr and logs HID writes as `DRYRUN OUT`.  
**A working live pad must not print that line.** Unplugged pads also fall back to dry-run.

### Manual install (no script)

`cargo build --release`, then write the systemd unit / LaunchAgent yourself. `install.sh` is the source of truth.  
On macOS the LaunchAgent **must** run the binary inside **DuckyPadBridge.app**, not `target/release/ducky-pad-bridge`.

### Rebuild firmware from source

Not for normal use. Only if you edit the C:

- **Keil µVision** (free MDK for STM32F072): open `../firmware/evo/MDK-ARM/lul.uvprojx`, Rebuild (F7)
- `arm-none-eabi-gcc` cross build (how the prebuilt image is made)

### HID protocol (firmware authors)

OUT (host → pad), report id `5`, 64 bytes:

- `[0]=5, [1]=0, [2]=cmd`
- `cmd 34` RGB: `[3..47]` = 15 × `(R,G,B)`
- `cmd 35` OLED key names: `[3]=len(≤56), [4..]` 15 cells × 4 ASCII (firmware paints the grid; title `H:<profile>` is firmware-owned)
- `cmd 36` Bridge online: `[3]=1` enable, `0` leave
- `cmd 37` read keys: no payload

IN (pad → host), report id `4`, reply to `37`:

- `[0]=4, [1]=0xF1, [2]=0 (OK), [3..7]` = 32-bit little-endian bitfield  
  bit `n` = key `n+1` held. The Bridge uses the low 15 bits for edge detect.

Firmware accepts 34/35 only while a **Herdr profile is selected** and the Bridge has sent `cmd 36`. Key polls are unanswered during SD file access.
