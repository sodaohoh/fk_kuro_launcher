# fk_kuro_launcher

[![CI & Release Build](https://github.com/sodaohoh/fk_kuro_launcher/actions/workflows/ci.yml/badge.svg)](https://github.com/sodaohoh/fk_kuro_launcher/actions/workflows/ci.yml)
[![GitHub release](https://img.shields.io/github/v/release/sodaohoh/fk_kuro_launcher)](https://github.com/sodaohoh/fk_kuro_launcher/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A lightweight Steam Wrapper & Auto-Restart tool for **Wuthering Waves (鳴潮)** written in Rust.

[English](README.md) | [繁體中文](README_zh-TW.md)

---

## Overview

In *Wuthering Waves*, in-game hotfix updates exit the client to apply patches. When launched via Steam, this causes the game to stop running and requires manually clicking "Play" again.

`fk_kuro_launcher` acts as a Steam Wrapper that automatically detects hotfix restart signals, respawns the game client, and maintains continuous **Steam "Playing" status**, **Steam Overlay**, and **playtime tracking**.

---

## Quick Start (1-Click Install)

Run this command in PowerShell to automatically install and set up Steam Launch Options:

```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/install.ps1 | iex
```

---

## Key Features

- **Continuous Steam Status**: Maintains Steam Overlay, playtime tracking, and "Playing" status across hotfix restarts.
- **Auto Target Resolution**: Automatically redirects launcher shims (`Wuthering Waves.exe`) to `Client-Win64-Shipping.exe`, eliminating Administrator Elevation errors (`OS Error 740`).
- **Non-Intrusive Monitoring**: Monitors `Client.log` in real time with shared file access (`FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`) so anti-cheat is never locked.
- **Seamless Auto-Update**: Background version check automatically updates the binary when a new release is published.

---

## Uninstallation

To uninstall `fk_kuro_launcher` and automatically restore Steam Launch Options, run:

```powershell
irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/uninstall.ps1 | iex
```

Or locally:
```powershell
.\install.ps1 -Uninstall
```

---

## Building from Source

```bash
cargo build --release
```
The compiled executable will be located at `target/release/fk_kuro_launcher.exe`.

---

## License

This project is licensed under the [MIT License](LICENSE).
