# duckyPad Herdr Bridge

> 英文原版：[README.md](README.md)

一個 [herdr](https://herdr.dev) 外掛，用 herdr 的 agent 狀態來驅動 **duckyPad**
（STM32F072 EVO 巨量鍵盤）。

- **最多 14 個 herdr agent → 14 顆亮的按鍵。** 前 14 顆 NeoPixel 按鍵各代表
  一個 agent；按鍵的**顏色就是該 agent 的狀態**。
- **按 agent 的按鍵 → 在 herdr 中聚焦（focus）那個 agent 的 pane。**
- **第 15 顆是 F9。** 平常白光，按下或按住時是紅光。
- 鍵盤的 **OLED** 顯示已映射 agent 的簡短清單。

不改动硬體。pad 端全部是韌體（pad 是一個 custom-HID 裝置，VID `0x483` /
PID `0xd11c`）；這個外掛是一個小的 Rust daemon，負責跟 herdr 的 socket 和
pad 對話。

duckyPad 裝成 herdr 光板後的樣子：

![duckyPad 作為 herdr 光板](img/duckypad-herdr.webp)

## 狀態與顏色（已固定）

| state     | 顏色     |
|-----------|----------|
| `blocked` | 紅       |
| `working` | 綠       |
| `done`    | 藍       |
| `unknown` | 琥珀     |
| `idle`    | 暗灰     |

**超過 14 個 agent** 時，依 herdr 的 list 順序取前 14 個點燈；
多出來的 agent 先不亮，等 agent 槽位空出來才進場。

## 運作方式

- **韌體**（`../firmware/evo`）：四個 custom-HID 指令（`34` RGB frame、
  `35` OLED 文字、`36` herdr-mode 開/關、`37` 讀取按鍵狀態）。在「herdr
  mode」下，前 14 顆按鍵會從正常鍵盤 report 中被抑制；第 15 顆維持本機
  **F9**（平常白光，按下或按住時紅光）。收到 `37` 時 pad 會同步掃過所有
  開關，並在 custom IN report 裡回一個 32-bit little-endian 的 key-state
  bitfield。
- **這個 daemon**：10ms 主迴圈。每 2 秒用一次性的 `agent.list` 重新輪詢
  herdr 的 Unix socket（這個 socket 是**一條連線只處理一個請求**，所以沒有
  長期的 push 訂閱），維護一份 agent 的快照；有任何變化就把 RGB frame +
  OLED 文字推到 pad。每個 tick 都會輪詢 pad 的 key-state 並做 edge 偵測
  （按住不放會 latch，所以每次按壓只觸發一次）；偵測到新按壓時，對那個
  agent 的 pane 發一個一次性的 `agent.focus`。按鍵對應是 sticky 的：agent
  只要在 list 裡就一直佔同一顆按鍵（新 agent 依 list 順序拿下一個空位），
  所以 state 變化、agent 進出都不會跳位。

## 需求

- Rust toolchain（`[[build]]` hook 會跑 `cargo build --release`）。
- `libhidapi`（`hidapi` crate 會自己把它編出來；Linux 上用的是
  `libhidapi-hidraw`）。
- herdr `>= 0.8.0` 正在運行（要有它的 Unix socket），且 duckyPad 已刷入更新
  後的韌體並插上。

## 建置

```bash
cd herdr-ducky-pad
cargo build --release
```

## 安裝（建置 + 使用者服務）

一個腳本在 **Linux 跟 macOS** 上都能裝——它建置 daemon，並把它跑成
**使用者服務**（Linux 用 systemd user service、macOS 用 launchd
LaunchAgent；herdr 的 `[[startup]]` hook 是「一次性、要結束」的，不適合
監督一個長駐 daemon）：

```bash
./install.sh
```

腳本可以重複執行（idempotent）——`git pull` 之後再跑一次即可。它會：

1. 建置 daemon（`cargo build --release`）；
2. 把 plugin 註冊進 herdr（`herdr plugin link`，前提是 herdr 在 PATH 上）；
3. 安裝並（重）啟服務：
   - **Linux**：`~/.config/systemd/user/ducky-pad-bridge.service`
   - **macOS**：`~/Library/LaunchAgents/com.botio.ducky-pad-bridge.plist`

狀態與 log：

- **Linux**：`systemctl --user status ducky-pad-bridge`、
  `journalctl --user -u ducky-pad-bridge -f`
- **macOS**：`launchctl list | grep ducky-pad-bridge`、
  `tail -f /tmp/ducky-pad-bridge.log`

不用腳本手動裝：`cargo build --release` 之後，自己建立並啟用那個
服務檔——`install.sh` 裡面就是 unit/plist 的完整內容。

## 不用 pad 測試（dry run）

```bash
DUCKY_DRY_RUN=1 ./target/release/ducky-pad-bridge
```

daemon 仍會連到 herdr，並把牠*原本會*送出的每個 HID write 記進 log
（`DRYRUN OUT cmd=34 ...`），所以不用實體裝置就能看到算出來的顏色/OLED。
如果 pad 沒插上，daemon 也會自動 fallback 到 dry run。

## 建置 & 刷寫韌體

pad 端是原版 duckyPad EVO 韌體，加上四個 herdr custom-HID 指令
（`34` RGB、`35` OLED、`36` herdr mode、`37` key state）。

**刷寫——不需要 Keil、不需要 toolchain。** repo 裡附了一份這個韌體的
pre-built image（**v3.1.2-herdr** build）。它是用 `arm-none-eabi-gcc`
產出的；同一套 build 流程已經證實能在真的 duckyPad 上 boot 並運作。
插上 pad 時按住 `DFU` 鍵，然後：

```bash
dfu-util --device 0483:df11 -a 0 -D ../firmware/duckypad_v3.1.2-herdr.dfu
```

跑起來後，OLED boot 畫面會顯示 `duckyPad V3.1.2`。完整步驟（截圖、
刷回 stock `../firmware/duckypad_v3.0.4.dfu` 的恢復方式）在主 repo：
[`firmware_updates_and_version_history.md`](../firmware_updates_and_version_history.md)。

在 herdr mode 下**不需要 SD 卡**——所有顯示資料都走 USB HID；microSD
只給原版的 duckyScript / profile 功能用。

F9 使用標準 USB 鍵盤 report（usage `0x42`），放開時送出 key-up。
從 v3.1.2 起，即使沒有 SD 卡或 profile，也由韌體本機處理：
前景迴圈每 1ms 服務一次，加上 5ms 按鍵去彈跳，不必等 daemon 更新燈色。
USB 忙碌時會保留回報並重送。這些是韌體排程間隔，並非電腦端實測延遲保證。

**要從 source 重建——只有在你改 C code 的時候。** 兩選一：

- **Keil µVision**（ST 對 STM32F072「F0」系列提供免費 MDK license）：
  開啟 `../firmware/evo/MDK-ARM/lul.uvprojx`、Rebuild（F7），用同樣方式
  刷 Keil 產物；或
- `arm-none-eabi-gcc` cross build——repo 裡的 pre-built image 就是這樣
  產出的。
## 端到端測試（有 pad）

1. **建置 & 刷寫韌體**（見上方「建置 & 刷寫韌體」）。
2. **插上** duckyPad（USB），並在一個真實 session 裡啟動 **herdr**，裡面放
   幾個 agent。
3. **啟動 daemon**——在這個目錄跑 `./install.sh`；它會建置並啟動使用者
   服務。
4. **觀察：**
   - 前 14 個 agent 各按自己的狀態顏色亮一顆按鍵；agent 變 `blocked` 就轉紅、
     `working` 綠、`done` 藍、`idle` 暗。
   - 第 15 顆平常是**白光**；按下或按住時會送出 **F9**，並維持**紅光**到放開。
   - **OLED** 列出已映射的 agent（`1:name 2:name ...`）。
   - **按 agent 的按鍵** → herdr 聚焦那個 agent 的 pane。

## 疑難排解

- **不亮 / 不會 focus：** pad 有在「herdr mode」嗎？daemon 啟動時會送
  `cmd 36`。用 `dmesg` / `lsusb` 確認裝置（`483:d11c`），以及 `libhidapi`
  看得到它（權限：加 udev rule 或改用 root 跑）。
- **log 出現 `herdr not reachable`：** herdr 沒在跑，或 socket 路徑不同。
  如果 herdr 用非預設路徑（例如具名 session），設
  `HERDR_SOCKET_PATH=/path/to/herdr.sock`。
- **Log：**設 `RUST_LOG=debug` 看更多細節。

## pad 的 protocol（參考）

- **OUT**（host → pad），report id `5`，64-byte buffer：
  - `[0]=5, [1]=0, [2]=cmd`
  - `cmd 34`（RGB）：`[3..47]` = 15 × `(R,G,B)`，依按鍵順序。
  - `cmd 35`（OLED）：`[3]=len(≤56), [4..]` = UTF-8 文字（`\n` = 換行）。
  - `cmd 36`（mode）：`[3]=1` 進入 herdr mode，`0` 離開。
  - `cmd 37`（key state）：沒有 payload；pad 掃過所有開關後用下面的
    IN report 回覆。
- **IN**（pad → host），report id `4`：對 `37` 的 key-state 回覆是
  `[0]=4, [1]=0xF1, [2]=0 (OK), [3..7]` = 32-bit little-endian bitfield，
  bit `n`（0-based）= 第 `n+1` 顆按鍵正被按住。daemon 讀低 15 bits
  （15 顆 agent 按鍵）並對它做 edge 偵測。
