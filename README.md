# Tsukuyomi OS

A self-contained terminal-based OS shell for Windows, written in Rust. Distributed as a single `tsukuyomi.exe` — no runtime, no installer, and no build tooling required.

## What it does

Tsukuyomi OS gives security consultants and sysadmins one local interface for day-to-day tools, all behind a single login.

### Core
- Username/password login with a local SQLite database and Argon2 password hashing
- TUI desktop with a categorized app launcher
- Top bar showing OS name, username, role, and live clock/timezone
- Local settings stored in `%LOCALAPPDATA%\TsukuyomiOS\settings.json`
- Terminal, browser, and file manager launchers

### System & Productivity
- **System Health** — CPU/RAM/disk usage and Windows service start/stop/restart
- **Patch Tracker** — pending Windows updates via `PSWindowsUpdate` (with a clear fallback if it isn't installed)
- **Backup Manager** — named backup jobs with on-demand `robocopy`-based runs and pass/fail history
- **Credential Vault** — encrypted local secrets (AES-256-GCM, key derived from your login password via Argon2)

### Network
- **Quick Connect** — saved SSH/RDP bookmarks, optionally backed by the Credential Vault
- **Asset Inventory** — track machines and run on-demand reachability checks
- **Network Diagnostics** — single-host ping, traceroute, port check, and interface stats
- **Remote Support** — launch consensual RustDesk sessions; host mode or connect-by-ID

### Security Workflow
- **Tsukuyomi Sandbox** — launch Windows Sandbox, Hyper-V, VirtualBox, VMware, or QEMU environments, with automated Alpine VM provisioning on VirtualBox
- **Firewall Rule Manager** — view and toggle local Windows Defender Firewall rules
- **Engagement Tracker** — record client, scope, dates, status, and invoice reference
- **Scan Request Log** — log authorized scan requests tied to an engagement
- **OSINT Notes** — simple manual notes per engagement
- **Findings / Report Builder** — engagement-linked findings with severity/status, markdown preview, and file export
- **Evidence Vault** — text-only encrypted evidence entries using the same AES-GCM vault as credentials
- **CVE Lookup** — manual CVE entries with optional NVD API refresh; offline-first

## Install

1. Download `tsukuyomi.exe` from the release page.
2. Place it anywhere you want (for example, a `C:\Tools` folder).
3. Double-click it, or run it from a terminal:

```powershell
tsukuyomi.exe
```

First launch creates your local account and data directory. There are no default credentials.

## Uninstall

Delete `tsukuyomi.exe` and remove `%LOCALAPPDATA%\TsukuyomiOS` if you want to remove all local data, settings, and the database.

## Security & Privacy Notes

- The Credential Vault encryption key exists only in memory during your session. It is never written to disk.
- Firewall, service, and VM-management actions usually require Administrator elevation. You will get a clear error if elevation is missing.
- Security workflow tools are for work you are already authorized to perform. Tsukuyomi OS does not include exploit or attack tooling.

## License

See the `LICENSE` file included in the release package.
