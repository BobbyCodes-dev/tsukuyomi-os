# Tsukuyomi OS — Terminal UI

A terminal-based personal OS shell built with [Textual](https://textual.textualize.io/). Runs on Windows, macOS, and Linux.

## Features

- Username/password login with local SQLite database
- TUI desktop with app launcher
- Top bar: OS name, username, role, live clock with timezone
- Native VM sandbox launcher (Windows Sandbox, Hyper-V, VirtualBox, VMware, QEMU)
- Local settings stored in `~/.local/share/TsukuyomiOS/settings.json` (or OS equivalent)
- Terminal, browser, and file manager app launchers

## Install

```bash
git clone <repo>
cd tsukuyomi-os
python -m venv .venv
source .venv/bin/activate  # Windows: .venv\Scripts\activate
pip install -r requirements.txt
```

## Run

```bash
tsukuyomi
```

Default login:
- **Username:** `admin`
- **Password:** `changeme`

## VM Sandbox

On Windows 10/11 Pro/Enterprise with Windows Sandbox enabled, selecting **Tsukuyomi Sandbox** will launch an isolated Windows environment for malware analysis. The app also detects VirtualBox, Hyper-V, VMware, and QEMU if installed.

## Web Mode (legacy)

The original FastAPI web UI is still available as `tsukuyomi-web` on port 8765.

## Settings

Settings are stored locally in the user's data directory. The server/Tsukuyomi OS only stores username and password hashes.
