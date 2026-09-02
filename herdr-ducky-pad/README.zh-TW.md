# duckyPad Herdr Bridge

> 英文原版：[README.md](README.md)

一個 [herdr](https://herdr.dev) 外掛，用 herdr 的 agent 狀態來驅動 **duckyPad**
（STM32F072 EVO 巨量鍵盤）。

- **N 個 herdr agent → N 顆亮的按鍵。** 鍵盤的 15 顆 NeoPixel 按鍵各代表一個
  agent；按鍵的**顏色就是該 agent 的状態**。
- **按下一顆按鍵 → 在 herdr 中聚焦（focus）那個 agent 的 pane。**
- 鍵盤的 **OLED** 顯示已映射 agent 的簡短清單。

不改动硬體。pad 端全部是韌體（pad 是一個 custom-HID 裝置，VID `0x483` /
PID `0xd11c`）；這個外掛是一個小的 Rust daemon，負責跟 herdr 的 socket 和
pad 對話。

## 狀態與顏色（已固定）

| state     | 顏色     |
|-----------|----------|
| `blocked` | 紅       |
| `working` | 綠       |
| `done`    | 藍       |
| `unknown` | 琥珀     |
| `idle`    | 暗灰     |

**超過 15 個 agent** 時，槽位依優先級填充：`blocked > done > working > unknown > idle`；
多出來的 agent 不亮。

## 運作方式

- **韌體**（`../firmware/evo`）：四個 custom-HID 指令（`34` RGB frame、
  `35` OLED 文字、`36` herdr-mode 開/關、`37` 讀取按鍵狀態）。在「herdr
  mode」下，15 顆按鍵會從正常鍵盤 report 中被抑制；收到 `37` 時 pad 會同步
  掃過所有開關，並在 custom IN report 裡回一個 32-bit little-endian 的
  key-state bitfield。
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

## 作為 herdr 外掛安裝

```bash
herdr plugin link ./herdr-ducky-pad
herdr plugin list      # 檢查有沒有 warning；daemon 會在 API ready 後啟動
```

`herdr plugin link` 會先跑 `[[build]]` hook（編譯 daemon），再在 herdr 的 API
ready 後啟動一次 `[[startup]]` hook（`./target/release/ducky-pad-bridge`）。
如果你的 herdr 版本不接受 `[[startup]]` section（看 `plugin list` 的 warning），
就直接跑 daemon：

```bash
./target/release/ducky-pad-bridge
```

## 不用 pad 測試（dry run）

```bash
DUCKY_DRY_RUN=1 ./target/release/ducky-pad-bridge
```

daemon 仍會連到 herdr，並把牠*原本會*送出的每個 HID write 記進 log
（`DRYRUN OUT cmd=34 ...`），所以不用實體裝置就能看到算出來的顏色/OLED。
如果 pad 沒插上，daemon 也會自動 fallback 到 dry run。

## 建置 & 刷寫韌體

pad 端是原版 duckyPad EVO 韌體，加上那三個 herdr custom-HID 指令。用
**Keil µVision** 編譯（ST 對 STM32F072「F0」系列提供免費的 MDK license）：

1. 開啟 `../firmware/evo/MDK-ARM/lul.uvprojx`。
2. **Rebuild**（F7）。

然後**用 DFU 刷寫**——插上 pad 時按住 `DFU` 鍵，再用 STM32 的 **DfuSe**
工具或 `dfu-util` 寫入。完整步驟（截圖 + `dfu-util` 一行指令、`0483:df11`）
在主 repo：[`firmware_updates_and_version_history.md`](../firmware_updates_and_version_history.md)。

> 要刷的是 **Keil** 的產物。普通的 `arm-none-eabi-gcc` build 也能編過這段
> 程式，但連結的 libc/startup 不同、產出的 bytes 也不同——那個 build 只用於
> **驗證**，不是能刷的 image。

### 免 Keil：現成的 `.dfu`

如果你沒有 Keil，repo 裡附了一份這個韌體（**v3.1.0-herdr** build）的
**GCC** pre-built image，可以直接用 `dfu-util` 刷（不需要 Keil、也不需要
cross-toolchain）。刷好跑起來後，OLED boot 畫面會顯示 `duckyPad V3.1.0`：

```bash
# 插上 pad 時按住 DFU 鍵，然後（完整旗標 / 恢復方式見主 repo 文件）：
dfu-util --device 0483:df11 -a 0 -D ../firmware/duckypad_v3.1.0-herdr.dfu
```

> **實驗性。** 這是普通的 `arm-none-eabi-gcc` build——編得過、vector table 也
> 對，但它是「驗證」用的 build（libc/startup 跟正式 Keil image 不同），所以
> 可能在你的 pad 上能 boot、也可能不能。DFU 刷寫是**可恢復**的：如果不行，
> 就再刷回 stock 的 `../firmware/duckypad_v3.0.4.dfu`。保證能刷的 image 仍是
> 上面的 **Keil** 產物。
## 端到端測試（有 pad）

1. **建置 & 刷寫韌體**（見上方「建置 & 刷寫韌體」）。
2. **插上** duckyPad（USB），並在一個真實 session 裡啟動 **herdr**，裡面放
   幾個 agent。
3. **跑 daemon**（或讓外掛自己啟動它）：
   ```bash
   ./target/release/ducky-pad-bridge
   ```
4. **觀察：**
   - 每個 agent 會按自己的狀態顏色亮一顆按鍵；agent 變 `blocked` 就轉紅、
     `working` 綠、`done` 藍、`idle` 暗。
   - **OLED** 列出已映射的 agent（`1:name 2:name ...`）。
   - **按下一顆按鍵** → herdr 聚焦那個 agent 的 pane。

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
