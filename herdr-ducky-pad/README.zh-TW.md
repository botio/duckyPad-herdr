# duckyPad Herdr Bridge

> 英文版：[README.md](README.md)

把 [duckyPad](https://github.com/dekuNukem/duckyPad)（2020 / OG）當成 **herdr 燈板**：

- **1–14 鍵** = 最多 14 個 herdr agent，顏色 = 狀態
- **按那顆鍵** → herdr 聚焦那個 agent
- **第 15 鍵** = 本機快捷鍵（預設 **F9**）
- **OLED** = `H:Herdr` + 格子裡的 agent 名（最多 4 字，空槽 `-`）
- Herdr 只是 **一個 profile**。用 **+ / −** 切走就恢復原本巨集

![duckyPad 作為 herdr 光板](img/duckypad-herdr.webp)

---

## 使用前要齊這五樣

| # | 要有什麼 | 沒有會怎樣 |
|---|---------|-----------|
| 1 | duckyPad **插著 USB**，microSD **插著** | pad 不亮、或卡在「Please Insert SD card」 |
| 2 | **herdr 視窗已開著** | 燈會被清掉，log 寫 `herdr.sock: No such file` |
| 3 | 韌體 **3.1.32-herdr** | 舊韌體不會畫格子名、也不吃 Bridge |
| 4 | Configurator **5.0.29+** 寫入 **Herdr profile**，pad 用 **+ / − 選到它** | Bridge 連上也不會接管畫面 |
| 5 | 這個 Bridge（`./install.sh`） | pad 停在全暗（第 15 鍵仍可能是白的） |

目前版本：**韌體 3.1.32-herdr**、**Bridge 0.2.8**。

---

## 第一次安裝（照順序，不要跳）

### 1. 刷韌體 3.1.32-herdr

需要 [`dfu-util`](http://dfu-util.sourceforge.net/)：

```bash
# macOS
brew install dfu-util

# Linux（Debian / Ubuntu）
sudo apt install dfu-util
```

1. **按住 pad 背面的 DFU 鍵**，再插上 USB，插上後可以放開
2. 在 **這個 repo 的根目錄** 執行：

```bash
dfu-util --device 0483:df11 -a 0 -D firmware/duckypad_v3.1.32-herdr.dfu
```

3. 按 pad 的 **RESET**（或拔掉重插）
4. OLED 開機應顯示 **`duckyPad V3.1.32`**

預編檔也在 [GitHub Release v3.1.32](https://github.com/botio/duckyPad-herdr/releases/tag/v3.1.32)。  
刷回原廠、截圖逐步說明：[`firmware_updates_and_version_history.md`](../firmware_updates_and_version_history.md)。

### 2. 寫入 Herdr profile

1. 用 [duckyPad Configurator **5.0.29+**](https://github.com/botio/duckyPad-Configurator) 連上 pad
2. 按 **ADD HERDR PROFILE**，再按底部 **SAVE** 寫進 SD
3. 在 pad 上按 **+ / −**，選到那個 Herdr profile
4. OLED 標題應是 **`H:Herdr`**（或 `H:` 加上你取的名字）

SD 裡的標記必須是 `HERDR_PROFILE 1`。只改名稱、沒有這個標記，**不會**進入 Herdr。

### 3. 安裝 Bridge

需要 [Rust](https://rustup.rs/)（`cargo`）。herdr 要在 PATH 上。

```bash
cd herdr-ducky-pad
./install.sh
```

腳本會：建置 daemon、跟 herdr 註冊 plugin、裝成**開機就跑的使用者服務**。  
`git pull` 之後再跑一次即可，可以重複執行。

| 系統 | 裝到哪 | 看狀態 | 看 log |
|------|--------|--------|--------|
| Linux | `~/.config/systemd/user/ducky-pad-bridge.service` | `systemctl --user status ducky-pad-bridge` | `journalctl --user -u ducky-pad-bridge -f` |
| macOS | LaunchAgent `com.botio.ducky-pad-bridge` | `launchctl list \| grep ducky-pad-bridge` | `tail -f /tmp/ducky-pad-bridge.log` |

### 4. macOS 必做：輸入監控

macOS **不會**讓 launchd 直接跑 CLI 開鍵盤 HID（錯誤 `0xE00002E2 not permitted`）。  
`install.sh` 會把程式裝成：

`~/Library/Application Support/ducky-pad-bridge/DuckyPadBridge.app`

1. 系統設定 → 隱私權與安全性 → **輸入監控**
2. **+** 加入上面那個 **DuckyPadBridge.app**（不要加 `target/release/ducky-pad-bridge`）
3. 開關關掉再開一次
4. 重啟服務：

```bash
launchctl kickstart -k "gui/$(id -u)/com.botio.ducky-pad-bridge"
```

**每次** `./install.sh` 之後都要再做一次步驟 3（簽名會變）。

### 5. 確認成功

log 裡**要有**：

```
duckyPad: opened HID device
herdr: N agent(s) ...
```

log 裡**不該有**：

| 看到這個 | 意思 |
|---------|------|
| `DRYRUN OUT` | 指令沒寫進 pad（HID 沒開到） |
| `0xE00002E2` / `not permitted` | macOS 輸入監控沒給 **DuckyPadBridge.app** |
| `herdr.sock: No such file` | herdr 沒開 |

pad 上應看到：

- OLED：`H:Herdr`，底下格子是 `omp` / `pi` 這類短名，空的是 `-`
- 有 agent 的鍵依狀態亮燈；第 15 鍵平常白
- 按 agent 鍵 → herdr 切到那個 pane

---

## 日常使用

1. 開 **herdr**
2. pad 用 **+ / −** 選到 **Herdr profile**（OLED 標題 `H:…`）
3. Bridge 服務保持在跑（裝過就會開機自動跑）

| 操作 | 結果 |
|------|------|
| 看 1–14 鍵顏色 | agent 狀態（見下表） |
| 按 1–14 某一顆 | herdr 聚焦那個 agent |
| 按第 15 鍵 | 預設送 **F9**（按下變紅，放開變白） |
| 在 Configurator 寫 `key15.dsb` | 第 15 鍵改跑你的巨集，不再送 F9 |
| 按 **+ / −** 切走 Herdr | 該 profile 自己的鍵名與巨集回來 |

### 顏色

| herdr 狀態 | 鍵色 |
|-----------|------|
| `blocked` | 紅 |
| `working` | 綠 |
| `done` | 藍 |
| `unknown` | 琥珀 |
| `idle` | 暗灰 |

超過 14 個 agent：只亮 herdr 清單前 14 個，多的等有空槽。鍵位是 sticky 的，狀態變了不會跳鍵。

改顏色：Configurator **5.0.29+ → HERDR → STATUS COLORS → SAVE COLORS**。不必手改 JSON。  
Linux：`~/.config/duckyPad/herdr.json`  
macOS：`~/Library/Application Support/duckyPad/herdr.json`

---

## 更新、重啟

```bash
cd herdr-ducky-pad
git pull
./install.sh
```

只重啟、不重裝：

```bash
# Linux
systemctl --user restart ducky-pad-bridge

# macOS
launchctl kickstart -k "gui/$(id -u)/com.botio.ducky-pad-bridge"
```

macOS 重裝後：**輸入監控**把 DuckyPadBridge.app 關掉再開。

---

## 疑難排解

| 現象 | 先查 |
|------|------|
| OLED 不是 `H:…` | pad 沒選到 Herdr profile（**+ / −**），或 Configurator 沒 SAVE |
| 格子全是 `-`、鍵不亮 | Bridge 沒開到 HID，或 herdr 沒開。看 log |
| log 有 `opened HID` 但仍全暗 | 韌體不是 3.1.32-herdr，或沒選 Herdr profile |
| `DRYRUN OUT` | 當沒插 pad，或 HID 開失敗。Mac 先查輸入監控 |
| `0xE00002E2 not permitted` | 加入 **DuckyPadBridge.app**，不是 CLI 路徑 |
| `herdr.sock: No such file` | 先開 herdr。Bridge 會找 `~/.config/herdr/herdr.sock` |
| herdr 有 agent、pad 沒反應 | log 要同時有 `opened HID device` **和** `herdr: N agent(s)` |
| Linux 找得到 USB 但開不了 | `lsusb` 應有 `0483:d11c`；hidraw 權限不夠就加 udev 或把使用者加入對應群組 |
| 刷完韌體沒變 | 刷的當下要停在 DFU（按住 DFU 再插電），刷完按 RESET |

指定 herdr socket（具名 session）：

```bash
export HERDR_SOCKET_PATH=/path/to/herdr.sock
```

更細的 log：`RUST_LOG=debug`（寫進 Linux unit / macOS plist 的環境變數後重啟服務）。

---

## 進階

### 沒有 pad 時看 Bridge 算什麼顏色

```bash
DUCKY_DRY_RUN=1 ./target/release/ducky-pad-bridge
```

會連 herdr，並把「本來要送給 pad」的 HID 寫入打成 `DRYRUN OUT`。  
**真機正常使用時不該看到這行。** pad 沒插也會自動 dry-run。

### 手動安裝（不用腳本）

`cargo build --release`，再自己寫 systemd unit / LaunchAgent。內容以 `install.sh` 為準。  
macOS 必須跑 **DuckyPadBridge.app 裡的 binary**，不能直接讓 launchd 跑 `target/release/ducky-pad-bridge`。

### 從原始碼重建韌體

一般使用**不要**走這段。只有改 C 才需要。兩選一：

- **Keil µVision**（STM32F072 有免費 MDK）：開 `../firmware/evo/MDK-ARM/lul.uvprojx`，Rebuild（F7）
- `arm-none-eabi-gcc` cross build（repo 裡的預編檔就是這樣產出）

### HID 協定（給改韌體的人）

OUT（電腦 → pad），report id `5`，64 bytes：

- `[0]=5, [1]=0, [2]=cmd`
- `cmd 34` RGB：`[3..47]` = 15 × `(R,G,B)`
- `cmd 35` OLED 鍵名：`[3]=len(≤56), [4..]` 15 格 × 4 ASCII（韌體畫在格子裡；標題 `H:<profile>` 由韌體自己寫）
- `cmd 36` Bridge 在線：`[3]=1` 啟用，`0` 離開
- `cmd 37` 讀鍵：無 payload

IN（pad → 電腦），report id `4`，回應 `37`：

- `[0]=4, [1]=0xF1, [2]=0 (OK), [3..7]` = 32-bit little-endian bitfield  
  bit `n` = 第 `n+1` 鍵正被按住。Bridge 讀低 15 bit 做 edge。

韌體只在**選中 Herdr profile** 且 Bridge 已送 `cmd 36` 時接受 34/35。SD 檔案存取期間不回覆按鍵輪詢。
