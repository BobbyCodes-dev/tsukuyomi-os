# Tsukuyomi OS

A self-contained terminal-based OS shell for **Windows and Linux**, written in Rust. Distributed as a single binary — no runtime, no installer overhead, and no build tooling required for end users.

## What it does

Tsukuyomi OS gives security consultants and sysadmins one local interface for day-to-day tools, all behind a single login.

### Core
- Username/password login with a local SQLite database and Argon2 password hashing
- TUI desktop with a categorized app launcher
- Top bar showing OS name, username, role, and live clock/timezone
- Local settings stored in:
  - **Windows:** `%LOCALAPPDATA%\TsukuyomiOS\settings.json`
  - **Linux:** `~/.local/share/TsukuyomiOS/settings.json`
- Terminal, browser, and file manager launchers (cross-platform: `xdg-open`, `$SHELL` on Linux)

### System & Productivity
- **System Health** — CPU/RAM/disk usage and service start/stop/restart
  - Windows: Windows services via PowerShell
  - Linux: systemd services via `systemctl`, CPU from `/proc/stat`, RAM from `/proc/meminfo`
- **Patch Tracker** — pending system updates
  - Windows: `PSWindowsUpdate` PowerShell module
  - Linux: `apt list --upgradable`, `dnf check-update`, or `pacman -Qu`
- **Backup Manager** — named backup jobs with on-demand runs and pass/fail history
  - Windows: `robocopy /MIR`
  - Linux: `rsync -a --delete`
- **Credential Vault** — encrypted local secrets (AES-256-GCM, key derived from your login password via Argon2)

### Network
- **Quick Connect** — saved SSH/RDP bookmarks, optionally backed by the Credential Vault
- **Asset Inventory** — track machines and run on-demand reachability checks
- **Network Diagnostics** — single-host ping, traceroute, port check, and interface stats
- **Remote Support** — launch consensual RustDesk sessions; host mode or connect-by-ID

### Security Workflow

These tools are hidden on the desktop launcher by default. Enable **Show Security Tools** in Settings to reveal them. When hidden, the desktop shows a hint reminding you where to enable them.

- **Tsukuyomi Sandbox** — launch VM environments:
  - Windows: Windows Sandbox, Hyper-V, VirtualBox, VMware
  - Linux: QEMU/KVM, VirtualBox, VMware (automated Alpine VM provisioning)
- **Firewall Rule Manager** — view and manage firewall rules
  - Windows: Windows Defender Firewall via PowerShell (`Get-NetFirewallRule`, etc.)
  - Linux: `ufw` (preferred) or `iptables` fallback
- **Engagement Tracker** — record client, scope, dates, status, and invoice reference
- **Scan Request Log** — log authorized scan requests tied to an engagement
- **OSINT Notes** — simple manual notes per engagement
- **Findings / Report Builder** — engagement-linked findings with severity/status, markdown preview, and file export
- **Evidence Vault** — text-only encrypted evidence entries using the same AES-GCM vault as credentials
- **CVE Lookup** — manual CVE entries with optional NVD API refresh; offline-first
- **AI Agent** — chat with Anthropic, OpenAI-compatible, Gemini, local Ollama, or Ollama Cloud models, with tool calling to open other apps in the OS. Opens in its own window (its own login, since there's no shared session). Provider, model, endpoint, and API key are configured in Settings, where switching providers auto-fills a default endpoint and fetches that provider's available models.

Note: model/endpoint discovery in Settings is a synchronous network call — switching providers or tabbing off the API Key field can briefly freeze the UI for up to ~6s if the endpoint is slow or unreachable before it falls back to a default.

`tsukuyomi.exe` itself stays a single self-contained binary — no bundled runtime or DLLs to ship. The one exception is local Ollama for the AI Agent: it's a separate multi-gigabyte model runtime that can't be embedded in the exe. Tsukuyomi handles it for you instead of making you do it by hand: selecting **Ollama (local)** in Settings auto-starts it if it's already installed but not running, and if it isn't installed at all, pressing `i` on the Provider field downloads the official installer and launches it. Cloud providers (Anthropic, OpenAI-compatible, Gemini, Ollama Cloud) need nothing extra — they're plain HTTPS calls.

## Install — Linux

### Quick install (from source)

```bash
git clone https://github.com/bobbycodes/tsukuyomi-os.git
cd tsukuyomi-os
./install.sh
```

The install script will:
1. Build the binary with `cargo build --release`
2. Install it to `~/.local/bin/tsukuyomi`
3. Create data directories (`~/.local/share/TsukuyomiOS`, `~/.config/TsukuyomiOS`)
4. Add `~/.local/bin` to your PATH if needed
5. Check for optional dependencies (nmap, ufw, rsync, qemu)

### Manual build

```bash
cargo build --release
# Binary is at target/release/tsukuyomi
cp target/release/tsukuyomi ~/.local/bin/
```

### Optional dependencies

| Tool | Purpose | Install |
|------|---------|---------|
| `curl` | Tool downloads (RustDesk) | `apt install curl` |
| `nmap` | Network scanning | `apt install nmap` |
| `ufw` | Firewall management | `apt install ufw` |
| `rsync` | Backup jobs | `apt install rsync` |
| `qemu-system-x86_64` | VM sandbox | `apt install qemu-system-x86` |

## Install — Windows

1. Download `tsukuyomi.exe` from the release page.
2. Place it anywhere you want (for example, a `C:\Tools` folder).
3. Double-click it, or run it from a terminal:

```powershell
tsukuyomi.exe
```

First launch creates your local account and data directory. There are no default credentials.

## Uninstall

### Linux

```bash
./uninstall.sh           # prompts for data removal confirmation
./uninstall.sh --nuke    # removes everything without prompting
./uninstall.sh --keep-data  # removes binary, keeps data
```

Or manually:
```bash
rm ~/.local/bin/tsukuyomi
rm -rf ~/.local/share/TsukuyomiOS ~/.config/TsukuyomiOS ~/.cache/TsukuyomiOS
```

### Windows

Delete `tsukuyomi.exe` and remove `%LOCALAPPDATA%\TsukuyomiOS` if you want to remove all local data, settings, and the database.

## Building from source

### Prerequisites
- Rust toolchain (stable): https://rustup.rs
- Cargo

### Build

```bash
# Debug build
cargo build

# Release build (optimized, stripped)
cargo build --release
```

### Cross-compilation

The `.cargo/config.toml` includes targets for both Windows (MSVC static CRT) and Linux (GNU). Build natively on each platform for best results.

## Security & Privacy Notes

- The Credential Vault encryption key exists only in memory during your session. It is never written to disk.
- Firewall, service, and VM-management actions usually require elevated privileges:
  - Windows: Administrator elevation
  - Linux: root/sudo access
- Security workflow tools are for work you are already authorized to perform. Tsukuyomi OS does not include exploit or attack tooling.

## License

Freeware — see [LICENSE](LICENSE).
