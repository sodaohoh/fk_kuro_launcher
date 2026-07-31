# Release vX.Y.Z

## Features & Highlights

- **Real-Time Log Decryption**: Decrypts Kuro Games' byte substitution cipher (LUT) in `Client.log` in real time to detect hotfix restart signals (`Engine exit requested`).
- **Seamless Steam Wrapper Lifecycle**: Keeps Steam status **"Playing"**, **Steam Overlay active**, and **playtime tracking continuous** across hotfix restarts.
- **Shared File Access**: Opens log files with `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE` so ACE Anti-Cheat and game client are never locked.
- **Auto Target Resolution**: Automatically redirects launcher shims (`Wuthering Waves.exe`) to `Client-Win64-Shipping.exe`, eliminating Administrator Elevation errors (`OS Error 740`).
- **Auto-Update Support**: Integrated runtime auto-update mechanism from GitHub Releases.

---

## PowerShell 1-Click Install

Run in PowerShell to automatically install or update `fk_kuro_launcher` and inject Steam launch options:

```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/master/install.ps1 | iex
```

---

## PowerShell 1-Click Uninstall

Run in PowerShell to clean up Steam launch options and remove installed files:

```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/master/uninstall.ps1 | iex
```
