# Tsukuyomi OS — Terminal UI

A terminal-based personal OS shell written in Rust, built with [ratatui](https://ratatui.rs/). Compiles to a single self-contained Windows `.exe` — no Python, no runtime, no installer dependency.

## Features

- Username/password login with local SQLite database (Argon2 password hashing)
- TUI desktop with app launcher
- Top bar: OS name, username, role, live clock with timezone
- Native VM sandbox launcher (Windows Sandbox, Hyper-V, VirtualBox, VMware, QEMU)
- Local settings stored in `%LOCALAPPDATA%\bobbycodes\TsukuyomiOS\settings.json`
- Terminal, browser, and file manager app launchers

## Build

```powershell
git clone <repo>
cd tsukuyomi-os
cargo build --release
```

Produces `target\release\tsukuyomi.exe`, statically linked (no `vcruntime140.dll`/system SQLite dependency).

## Install

```powershell
scripts\Install-Tsukuyomi.ps1
```

Copies the exe into `%LOCALAPPDATA%\TsukuyomiOS` and creates a desktop shortcut. Run `scripts\Nuke-Tsukuyomi.ps1` (or `tsukuyomi.exe uninstall`) to remove it.

## Run

```powershell
tsukuyomi.exe
```

First run walks you through creating a local account (no default credentials).

## VM Sandbox

On Windows 10/11 Pro/Enterprise with Windows Sandbox enabled, selecting **Tsukuyomi Sandbox** will launch an isolated Windows environment for malware analysis. The app also detects VirtualBox, Hyper-V, VMware, and QEMU if installed. VM image building (downloading/provisioning a fresh VirtualBox disk) isn't implemented yet — you'll need a prebuilt disk for the VirtualBox path.

## Settings

Settings and the user database are stored locally in `%LOCALAPPDATA%\bobbycodes\TsukuyomiOS`.
