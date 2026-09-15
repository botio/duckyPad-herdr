# duckyPad Herdr Bridge

> 英文原版：[README.md](README.md)

一個 [herdr](https://herdr.dev) 外掛，用 herdr 的 agent 狀態來驅動 **duckyPad**
（STM32F072 EVO 巨量鍵盤）。

- **最多 14 個 herdr agent → 14 顆亮的按鍵。** 前 14 顆 NeoPixel 按鍵各代表
  一個 agent；按鍵的**顏色就是該 agent 的狀態**。
- **按 agent 的按鍵 → 在 herdr 中聚焦（focus）那個 agent 的 pane。**
- **第 15 顆是你的本機快捷鍵。** 預設送出 **F9**（平常白光，按下或按住時紅光），也可改成你自己指定的 duckyScript。
- 鍵盤的 **OLED** 顯示已映射 agent 的簡短清單。
- **Herdr 是可選的 profile，不再自動接管整台 pad。** 韌體 3.1.20+ 只在選到 Herdr profile 時接受 Bridge 的畫面與燈色；**+ / −** 照常切換 profile。

不改动硬體。pad 端全部是韌體（pad 是一個 custom-HID 裝置，VID `0x483` /
PID `0xd11c`）；這個外掛是一個小的 Rust daemon，負責跟 herdr 的 socket 和
pad 對話。

duckyPad 裝成 herdr 光板後的樣子：

![duckyPad 作為 herdr 光板](img/duckypad-herdr.webp)

## 預設狀態與顏色

| state     | 顏色     |
|-----------|----------|
| `blocked` | 紅       |
| `working` | 綠       |
| `done`    | 藍       |
| `unknown` | 琥珀     |
| `idle`    | 暗灰     |

在 Configurator **5.0.29+ → HERDR → STATUS COLORS → SAVE COLORS** 可修改五種狀態色，不必手改 JSON。色票為這台電腦共用，Bridge 每約兩秒重新載入；既有 agent 綁定不會被色票儲存覆蓋。Linux 使用 `$XDG_CONFIG_HOME/duckyPad/herdr.json`（預設 `~/.config`），macOS 使用 `~/Library/Application Support/duckyPad/herdr.json`。

按 **ADD HERDR PROFILE** 建立，再按底部 **SAVE** 寫入 pad。用實體 **+ / −** 選到它才會啟用 Herdr；切回其他 profile 就恢復原本巨集。SD 的 `config.txt` 標記是 `HERDR_PROFILE 1`，名稱本身不會啟用模式。即使 agent 狀態沒變，Bridge 的下一次 heartbeat 也會更新切入後的畫面。

**超過 14 個 agent** 時，依 herdr 的 list 順序取前 14 個點燈；
多出來的 agent 先不亮，等 agent 槽位空出來才進場。

## 運作方式

- **韌體**（`../firmware/evo`）：四個 custom-HID 指令（`34` RGB frame、
  `35` OLED 文字、`36` Bridge 啟用狀態、`37` 讀取 agent 按鍵）。
  選中的 Herdr profile 把前 14 鍵保留給 agent；第 15 鍵是本機快捷鍵——
  沒有 script 就是 **F9**，有 `key15.dsb` 就跑那個巨集。
  `37` 只回傳 agent 按鍵的 bitfield；其他 profile 回傳零。
  SD 檔案存取期間不回覆按鍵輪詢，避免與 Configurator 的資料回覆混在一起。
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

macOS 的 `hidutil list` 可能把 duckyPad `0483:d11c` 列為主要
Usage Page `1`／Usage `6`（Keyboard）。Bridge 在 macOS 依 VID/PID
選擇裝置，因為開啟的是整個 HID 裝置，不是 Windows 的獨立 collection。
啟用 hidapi 的 `macos-shared-device`，以共享模式開啟，不獨占鍵盤。
共享模式不會略過 macOS 隱私權控管：請授予 launchd 實際執行的
`ducky-pad-bridge` 輸入監控權限，再重新啟動服務。
`herdr: ... agent(s)` 只證明 socket 已連線；`DRYRUN OUT` 不會送到 pad。
請確認出現 `duckyPad: opened HID device`，再以實體 pad 的畫面及 agent
按鍵反應確認通訊。

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
（`34` RGB、`35` OLED、`36` Bridge 啟用狀態、`37` agent 按鍵）。

**刷寫——不需要 Keil、不需要 toolchain。** repo 內附 **v3.1.20-herdr**，
用固定的 ARM GNU Toolchain 13.2.1 建置。主機回歸測試涵蓋 profile 控制權、
RGB/OLED 前景繪製、連續 NeoPixel 輸出、SD 排他、切換、第 15 鍵
script-vs-F9 及 F9 釋放；這不等於實機 LED 波形驗證。

```bash
dfu-util --device 0483:df11 -a 0 -D ../firmware/duckypad_v3.1.20-herdr.dfu
```

跑起來後，OLED boot 畫面會顯示 `duckyPad V3.1.20`。完整步驟（截圖、
刷回 stock `../firmware/duckypad_v3.0.4.dfu` 的恢復方式）在主 repo：
[`firmware_updates_and_version_history.md`](../firmware_updates_and_version_history.md)。

韌體 3.1.20+ **需要 microSD 上有 Herdr profile，並且選中它**。
只啟動 Bridge、沒有 SD 或沒有 profile，都不會接管 pad。
既有 Bridge 使用相同的 HID 指令，不必為 profile 改動或可自訂的第 15 鍵重新安裝。

第 15 鍵沒有 script 時，用標準 USB 鍵盤 report（usage `0x42`）送出 F9，
平常白光、按住時紅光；寫入 `key15.dsb` 後就改跑那個巨集，跟一般巨集鍵一樣。
切離 Herdr 或開始 SD 檔案存取時會釋放 F9，並取消尚未送出的按下事件。
本機前景迴圈每 1ms 服務一次，加上 5ms 去彈跳；USB 忙碌時重試回報。
這些是排程間隔，不是電腦端實測延遲保證。

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
