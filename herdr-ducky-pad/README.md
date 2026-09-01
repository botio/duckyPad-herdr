# duckyPad Herdr Bridge

> 繁體中文版 → [README.zh-TW.md](README.zh-TW.md)

A [herdr](https://herdr.dev) plugin that drives a **duckyPad** (STM32F072 EVO
macropad) from herdr's agent state.

- **N herdr agents → N lit keys.** Each of the pad's 15 NeoPixel keys represents
  one agent; the key's **color is that agent's state**.
- **Press a key → focus that agent's pane** in herdr.
- The pad's **OLED** shows a short list of the mapped agents.

No hardware changes. Everything on the pad side is firmware (the pad is a
custom-HID device, VID `0x483` / PID `0xd11c`); this plugin is a small Rust
daemon that talks to herdr's socket and to the pad.

## States & colors (locked)

| state     | color          |
|-----------|----------------|
| `blocked` | red            |
| `working` | green          |
| `done`    | blue           |
| `unknown` | amber          |
| `idle`    | dim gray       |

With **more than 15 agents**, slots fill by priority
`blocked > done > working > unknown > idle`; overflow agents are unlit.

## How it works

- **Firmware** (`../firmware/evo`): three custom-HID commands
  (`34` RGB frame, `35` OLED text, `36` herdr-mode on/off) and a key-press
  event on the custom IN report. In "herdr mode" the 15 keys are suppressed
  from the normal keyboard report and instead sent as a key event.
- **This daemon**: polls herdr's Unix socket with a one-shot `agent.list`
  (the socket handles **one request per connection**, so there is no
  persistent push subscription), keeps a picture of the agents, and on any
  change pushes the RGB frame + OLED text to the pad. On a key event it
  issues a one-shot `agent.focus` for that agent's pane.

## Requirements

- Rust toolchain (the `[[build]]` hook runs `cargo build --release`).
- `libhidapi` (the `hidapi` crate builds it; on Linux `libhidapi-hidraw` is
  used).
- herdr `>= 0.8.0` running (with its Unix socket), and the duckyPad flashed
  with the updated firmware and plugged in.

## Build

```bash
cd herdr-ducky-pad
cargo build --release
cargo test            # unit tests for the model (colors, slot priority, frames)
```

## Install as a herdr plugin

```bash
herdr plugin link ./herdr-ducky-pad
herdr plugin list      # check for warnings; the daemon starts after the API is ready
```

`herdr plugin link` runs the `[[build]]` hook (builds the daemon) and then
launches the `[[startup]]` hook (`./target/release/ducky-pad-bridge`) once,
after herdr's API is ready. If the `[[startup]]` section is rejected by your
herdr version (see `plugin list` warnings), run the daemon directly instead:

```bash
./target/release/ducky-pad-bridge
```

## Test without the pad (dry run)

```bash
DUCKY_DRY_RUN=1 ./target/release/ducky-pad-bridge
```

The daemon still connects to herdr and logs every HID write it *would* send
(`DRYRUN OUT cmd=34 ...`), so you can watch the computed colors/OLED without
the physical device. If the pad isn't plugged in, the daemon also falls back
to dry run automatically.

## Building & flashing the firmware

The pad side is the stock duckyPad EVO firmware plus the three herdr
custom-HID commands. Build it with **Keil µVision** (ST ships a free MDK
license for the STM32F072 "F0" parts):

1. Open `../firmware/evo/MDK-ARM/lul.uvprojx`.
2. **Rebuild** (F7).

Then **flash via DFU** — hold the pad's `DFU` button while plugging it in, then
program it with the STM32 **DfuSe** tool or `dfu-util`. The full procedure
(screenshots + the `dfu-util` one-liner, `0483:df11`) is in the main repo:
[`firmware_updates_and_version_history.md`](../firmware_updates_and_version_history.md).

> Flash the **Keil** output. A plain `arm-none-eabi-gcc` build also compiles
> this code, but links a different libc/startup and produces different bytes —
> that build is **validation-only**, not the flashable image.

### No-Keil shortcut: a pre-built `.dfu`

If you don't have Keil, a pre-built **GCC** image of this firmware ships in the
repo and can be flashed directly with `dfu-util` (no Keil, no cross-toolchain):

```bash
# hold the pad's DFU button while plugging in, then (see the main repo doc for
# the exact flags / recovery):
dfu-util --dev 0483:df11 -a 0 -D ../firmware/duckypad_herdr.dfu
```

> **Experimental.** This is a plain `arm-none-eabi-gcc` build — it compiles and
> has a valid vector table, but it is the *validation* build (different
> libc/startup than the blessed Keil image), so it may or may not boot on your
> pad. DFU flashing is **recoverable**: if it doesn't work, re-flash the stock
> `../firmware/duckypad_v3.0.4.dfu`. The guaranteed flashable image remains the
> **Keil** output above.
## End-to-end test (with the pad)

1. **Build & flash the firmware** (see [Building & flashing the firmware](#building--flashing-the-firmware) above).
2. **Plug in** the duckyPad (USB) and start **herdr** in a real session with a
   few agents.
3. **Run the daemon** (or let the plugin start it):
   ```bash
   ./target/release/ducky-pad-bridge
   ```
4. **Observe:**
   - Each agent lights a key in its state color; when an agent moves to
     `blocked` it turns red, `working` green, `done` blue, `idle` dim.
   - The **OLED** lists the mapped agents (`1:name 2:name ...`).
   - **Press a key** → herdr focuses that agent's pane.

## Troubleshooting

- **No lights / no focus:** is the pad in "herdr mode"? The daemon sends
  `cmd 36` on start. Check `dmesg` / `lsusb` for the device (`483:d11c`), and
  that `libhidapi` can see it (permissions: add a udev rule or run as root).
- **`herdr not reachable` in the log:** herdr isn't running, or the socket
  path differs. Set `HERDR_SOCKET_PATH=/path/to/herdr.sock` if herdr uses a
  non-default path (e.g. a named session).
- **Logs:** set `RUST_LOG=debug` for more detail.

## The pad's protocol (reference)

- **OUT** (host → pad), report id `5`, 64-byte buffer:
  - `[0]=5, [1]=0, [2]=cmd`
  - `cmd 34` (RGB): `[3..47]` = 15 × `(R,G,B)`, key order.
  - `cmd 35` (OLED): `[3]=len(≤56), [4..]` = UTF-8 text (`\n` = new line).
  - `cmd 36` (mode): `[3]=1` enter herdr mode, `0` leave.
- **IN** (pad → host), report id `4`: a key press is
  `[0]=4, [1]=0xF0, [2]=0, [3]=slot(1..=15)`.
