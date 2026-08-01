# fk_kuro_launcher

[![CI & Release Build](https://github.com/sodaohoh/fk_kuro_launcher/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/sodaohoh/fk_kuro_launcher/actions/workflows/ci.yml)
[![GitHub release](https://img.shields.io/github/v/release/sodaohoh/fk_kuro_launcher)](https://github.com/sodaohoh/fk_kuro_launcher/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

專為 **《鳴潮》 (Wuthering Waves)** 打造的輕量、非侵入式 Steam 原生 Wrapper 與熱更新自動重啟工具（100% Rust 撰寫）。

[English](README.md) | [繁體中文](README_zh-TW.md)

---

## 專案簡介

在《鳴潮》進行遊戲內熱更新時，遊戲主程式會下載補丁並自動退出。透過 Steam 啟動時，這會導致 Steam 認定遊戲已關閉，必須手動點擊「開始遊戲」才能載入補丁。

`fk_kuro_launcher` 透過接管 Steam `%command%` 啟動器，實時監控遊戲熱更新請求，自動重啟遊戲主程式，並保持 **Steam「在遊戲中」狀態**、**Steam 覆蓋介面 (Overlay)** 與 **遊戲時數累計不中斷**。

---

## 一鍵快速安裝

在 PowerShell 中複製並執行以下指令，系統會自動下載安裝並設定 Steam 啟動選項：

```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/master/install.ps1 | iex
```

安裝程式會刻意將 Steam 啟動選項寫成 `"fk_kuro_launcher.exe" "%command%"`。`%command%` 的引號不可刪除；如果省略，Steam 會把 `C:\Program Files\...` 這類含空格的遊戲路徑拆成多個參數，Wrapper 只會收到 `C:\Program`，因此遊戲不會啟動。若先前已安裝舊版，請重新執行安裝指令。

Release 版本會以 Windows GUI 應用程式執行，不會開啟 terminal 視窗。執行期間的診斷資訊會寫入 `%LOCALAPPDATA%\fk_kuro_launcher\launcher.log`；嚴重的啟動錯誤仍可能顯示 Windows 錯誤對話框。

---

## 主要特點

- **Steam 狀態無縫維護**：在熱更新重啟過程中保持 Steam 覆蓋介面、在線狀態與時數累計不中斷。
- **自動目標解析**：自動將 Steam 傳入的 `Wuthering Waves.exe` 定向至 `Client-Win64-Shipping.exe`，解決系統管理員權限提權錯誤 (`OS Error 740`)。
- **非侵入式日誌監控**：採用 Win32 共享讀取模式實時解密 `Client.log`，完全不鎖定檔案、不影響 ACE 反作弊。
- **全自動背景更新**：內建非同步背景更新檢測，當有新版本時自動完成無縫替換。

---

## 一鍵卸載說明

若要卸載 `fk_kuro_launcher` 並自動清除 Steam 啟動選項設定，請在 PowerShell 執行：

```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/master/uninstall.ps1 | iex
```

或在本地專案目錄執行：
```powershell
.\install.ps1 -Uninstall
```

---

## 本地編譯說明

```bash
cargo build --release
```
編譯產生的可執行檔位於 `target/release/fk_kuro_launcher.exe`。

---

## 授權條款 (License)

本專案採用 [MIT License](LICENSE) 授權。
