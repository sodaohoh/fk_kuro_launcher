# fk_kuro_launcher

A lightweight, non-intrusive Steam Wrapper & Auto-Restart tool for **Wuthering Waves (鳴潮)** written in 100% Rust.

[English](#-english) | [繁體中文](#-繁體中文)

---

## 📖 English

### Overview
In *Wuthering Waves*, in-game hotfixes download `.pak` patch files and exit the client with return code `0` (indistinguishable from a manual game exit). When launched via Steam, this causes the game to stop running and requires manually clicking "Play" again to apply the patch.

`fk_kuro_launcher` solves this problem by acting as a **Steam Wrapper**:
1. **Log Decryption**: Reverses Kuro Games' custom byte substitution cipher (LUT) in `Client.log` in real time.
2. **Shared File Access**: Opens `Client.log` using Win32 `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE` so ACE Anti-Cheat and the game client are never locked.
3. **Steam Wrapper Lifecycle**: Wraps `%command%`, keeping Steam status **"Playing"**, **Steam Overlay active**, and **playtime tracking continuous** across hotfix restarts.
4. **Auto Target Resolution**: Automatically redirects launcher shims (`Wuthering Waves.exe`) to `Client-Win64-Shipping.exe`, eliminating Administrator Elevation errors (`OS Error 740`).
5. **Zero False Positives**: Differentiates manual game exits (menu exit / Alt+F4) from in-game hotfix restart requests (`Engine exit requested`). Normal quits terminate the monitor cleanly.

---

### 🚀 1-Click Automated Setup (Recommended)

Run the 1-click installer command in PowerShell:
```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/install.ps1 | iex
```
*(Or `iwr -useb https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/install.ps1 | iex`)*

The installer automatically:
- Installs and updates `fk_kuro_launcher.exe` to `%LOCALAPPDATA%\fk_kuro_launcher\fk_kuro_launcher.exe` (downloading the latest release from GitHub Releases, or using a custom local path via `-ExePath`).
- Auto-detects Steam installation path via Windows Registry.
- Safely creates a `localconfig.vdf.bak` backup, closes Steam, injects Steam Launch Options, and restarts Steam.

You can also run `install.ps1` locally from a cloned directory:
```powershell
.\install.ps1
```
Or specify a custom executable path:
```powershell
.\install.ps1 -ExePath "C:\path\to\fk_kuro_launcher.exe"
```

---

### 🔄 Auto-Update

`fk_kuro_launcher` includes full automatic update functionality:
- **Installer Auto-Update (`install.ps1`)**: Running `install.ps1` automatically downloads and updates `%LOCALAPPDATA%\fk_kuro_launcher\fk_kuro_launcher.exe` to the latest GitHub Release asset. If offline or network download fails, existing installations are preserved.
- **Runtime Self Auto-Update**: When running in the background, `fk_kuro_launcher` automatically checks GitHub Releases for new tags. When a newer version is detected, it downloads the release in the background, safely swaps the executable on Windows (`.old` / `.new`), and seamlessly applies the update for the next launch.

---

### 🎮 Manual Steam Setup

1. Open **Steam**, right-click **Wuthering Waves** ➔ **Properties**.
2. Under **General**, find **Launch Options**.
3. Paste the following command (replace `%LOCALAPPDATA%` with your actual AppData Local path if needed):

```cmd
"%LOCALAPPDATA%\fk_kuro_launcher\fk_kuro_launcher.exe" %command%
```

---

### 🛠️ Building from Source (Rust)

```bash
cd C:\path\to\fk_kuro_launcher
cargo build --release
```
The compiled executable is located at `target/release/fk_kuro_launcher.exe` (or `target/x86_64-pc-windows-gnu/release/fk_kuro_launcher.exe`). Running `.\install.ps1` afterwards will automatically detect and install this release build.

---

### 🗑️ Uninstallation

To uninstall `fk_kuro_launcher` and automatically clear Steam Launch Options:

**1-Click Web Command:**
```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/uninstall.ps1 | iex
```

**Local Command:**
```powershell
.\install.ps1 -Uninstall
```
*(or run `.\uninstall.ps1` directly)*

---

<br>

---

## 📖 繁體中文

### 專案簡介
在《鳴潮》進行遊戲內熱更新時，遊戲主程式會下載 `.pak` 補丁檔並回傳 `0` 號結束碼退出（外觀與玩家手動關閉遊戲無異）。透過 Steam 啟動時，Steam 會判定遊戲已停止，必須手動再次點擊「開始遊戲」才能載入補丁。

`fk_kuro_launcher` 透過 100% Rust 撰寫的 **Steam 原生 Wrapper（包裝器）架構** 徹底解決此問題：
1. **即時日誌解密**：實時破譯與解密庫洛遊戲在 `Client.log` 中使用的自訂單位元組替換加密表 (LUT)。
2. **非侵入式共享讀取**：採用 Win32 `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE` 共享模式，完全不影響 ACE 反作弊與遊戲檔案鎖定。
3. **Steam 無縫整合**：接管 Steam 的 `%command%`，在熱更新重啟時**維護「在遊戲中」狀態**、**Steam 覆蓋介面 (Overlay)** 與 **遊戲時數累計不中斷**。
4. **自動目標解析**：自動將 Steam 傳入的 `Wuthering Waves.exe` 啟動器檔定向至 `Client-Win64-Shipping.exe`，徹底解決 Windows 系統管理員權限提權錯誤 (`OS Error 740`)。
5. **零誤觸發**：精準識別玩家手動離開遊戲與引擎熱更新請求（`Engine exit requested`）。手動關閉遊戲時監控程式隨之乾淨退出。

---

### 🚀 一鍵全自動 Steam 設定（推薦）

在 PowerShell 中直接執行一鍵安裝指令：
```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/install.ps1 | iex
```
*(或使用 `iwr -useb https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/install.ps1 | iex`)*

一鍵安裝腳本會自動處理：
- 自動將 `fk_kuro_launcher.exe` 安裝並更新至 `%LOCALAPPDATA%\fk_kuro_launcher\fk_kuro_launcher.exe`（從 GitHub Releases 下載最新版本，或可透過 `-ExePath` 指定自訂路徑）。
- 透過系統登錄檔自動檢測 Steam 安裝目錄。
- 安全建立 `localconfig.vdf.bak` 備份、關閉 Steam、注入啟動選項並自動重啟 Steam！

您也可以在複製的專案目錄中本地執行：
```powershell
.\install.ps1
```
若可執行檔位於自訂路徑，可指定 `-ExePath` 參數：
```powershell
.\install.ps1 -ExePath "C:\path\to\fk_kuro_launcher.exe"
```

---

### 🔄 自動更新 (Auto-Update)

`fk_kuro_launcher` 支援全自動更新功能：
- **安裝檔自動更新 (`install.ps1`)**: 執行 `install.ps1` 時會自動從 GitHub Releases 下載並更新 `%LOCALAPPDATA%\fk_kuro_launcher\fk_kuro_launcher.exe` 至最新版本（若網路下載失敗，將保留現有版本）。
- **運行時自動更新**: 於背景運行時，`fk_kuro_launcher` 會自動檢查 GitHub Releases。當檢測到新版本時，會在背景自動下載最新檔，透過 Windows 安全檔名重命名機制（`.old` / `.new`）完成無縫替換，於下次啟動時生效。

---

### 🎮 手動 Steam 啟動選項設定

1. 開啟 **Steam** 收藏庫，右鍵點擊 **《鳴潮》 ➔ 內容**。
2. 在 **一般 ➔ 啟動選項** 輸入框貼入以下指令：

```cmd
"%LOCALAPPDATA%\fk_kuro_launcher\fk_kuro_launcher.exe" %command%
```

---

### 🛠️ 編譯說明 (Rust)

```bash
cd C:\path\to\fk_kuro_launcher
cargo build --release
```
編譯產生的可執行檔位於 `target/release/fk_kuro_launcher.exe`（或 `target/x86_64-pc-windows-gnu/release/fk_kuro_launcher.exe`）。完成編譯後執行 `.\install.ps1` 將會自動檢測並安裝此版本。

---

### 🗑️ 卸載說明

若要卸載 `fk_kuro_launcher` 並自動清除 Steam 啟動選項設定：

**一鍵線上卸載指令：**
```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/uninstall.ps1 | iex
```

**本地卸載指令：**
```powershell
.\install.ps1 -Uninstall
```
*(或直接執行 `.\uninstall.ps1`)*

---

### 📄 授權條款 (License)
本專案採用 [MIT License](LICENSE) 授權。
