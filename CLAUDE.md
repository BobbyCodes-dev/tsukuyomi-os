# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

Tsukuyomi OS is a terminal-based "personal OS" shell written in Rust, built with [ratatui](https://ratatui.rs/)/[crossterm](https://github.com/crossterm-rs/crossterm), compiled to a single self-contained Windows `.exe` (no runtime, no installer dependency). It presents a fake desktop with an app launcher (sandbox VM, browser, terminal, file manager, settings) backed by a local SQLite user store.

This is a from-scratch rewrite of an earlier Python/Textual TUI (plus a legacy FastAPI web UI) — both were deleted once the Rust version reached functional parity. There is no Python in this repo anymore; don't reintroduce it or assume `pip`/`pyproject.toml` workflows.

## Commands

```powershell
cargo build --release        # produces target/release/tsukuyomi.exe
cargo run                    # build + launch the TUI directly
```

`.cargo/config.toml` sets `-C target-feature=+crt-static` for the MSVC target, so `cargo build --release` alone statically links the CRT — the resulting exe has no `vcruntime140.dll`/system-SQLite/Python dependency. Verify self-containment after touching build config by checking the binary's loaded modules only include base Windows OS DLLs (`ntdll`, `kernel32`, `advapi32`, `ucrtbase` — the latter is an OS component since Windows 10, not something you need to ship).

Subcommands (via `clap`, see `src/main.rs`):
- `tsukuyomi.exe` (no args) — launches the TUI.
- `tsukuyomi.exe uninstall [--keep-vms] [--yes]` — deletes all local Tsukuyomi OS data (destructive; prompts for `NUKE` confirmation unless `--yes`).

There is no test suite, linter config, or CI pipeline in this repo.

## Architecture

### State machine, not a screen stack

ratatui has no Textual-style `Screen` push/pop system, so `src/app.rs` hand-rolls one: a `Screen` enum (`Setup`/`Login`/`Desktop`/`Sandbox`/`Settings`), a `DesktopState` kept alive separately in `App.desktop` (rather than as `Screen` payload) so it survives navigating away to Sandbox/Settings and back, and an `Action` enum returned by each screen's `handle_key` that `App::apply` matches on to drive transitions. The main loop (`App::run`) is `terminal.draw` → `event::poll(250ms)` → dispatch key → tick the desktop clock — see `src/app.rs`.

### Screens (`src/screens/`)

One module per screen (`setup.rs`, `login.rs`, `desktop.rs`, `sandbox.rs`, `settings.rs`), each exposing a `draw(frame, area, state)` and a `handle_key(state, key) -> Action`. Forms are rendered as a single styled `Paragraph` inside a bordered `Block` (not per-field sub-widgets) with a `> ` prefix marking the focused field — ratatui has no `Input`/`Select` widgets, so `ui/widgets.rs::TextField` hand-rolls text entry (with a `masked` flag for passwords) and select-style fields are just an index into a constant list (e.g. `setup::TIMEZONES`) cycled with Left/Right.

**Keybinding note**: letter shortcuts (`q` quit, `r` refresh, `s` settings) are only safe on `Desktop`, which has no text fields to collide with. `Setup`/`Login` have text fields that accept any character, so `Esc` is the universal quit/back key there, and Login's Python-original bare-`r` "reset to setup" became **Ctrl+R** to avoid swallowing a literal `r` typed into a field — a deliberate adaptation, not an oversight, if you're diffing behavior against the original.

### Store (`src/store/`)

`data_dir()` (in `mod.rs`) hand-builds `%LOCALAPPDATA%\bobbycodes\TsukuyomiOS` directly from the `LOCALAPPDATA` env var rather than depending on the `directories` crate, specifically to guarantee byte-identical parity with the original Python app's `platformdirs.user_data_dir` path (the `directories` crate's own path-construction scheme isn't guaranteed to match). `users.rs` is `rusqlite` (bundled/statically-linked SQLite) with **Argon2** password hashing (per-user random salt) — this is a deliberate upgrade from the original Python version's fixed-salt SHA-256, decided when the rewrite happened; it means old Python-created `users.db` files won't authenticate and accounts must be recreated. `settings.rs` is `serde_json` with `#[serde(default)]` at the struct level, which replicates the original's "defaults merged with whatever the file has" load semantics for free.

### VM module (`src/vm/`)

`detect.rs` ports Windows Sandbox/Hyper-V/VirtualBox/VMware/QEMU backend detection (PowerShell `Get-WindowsEdition`/`Get-WindowsOptionalFeature` calls, PATH+suffix search for `VBoxManage`/`vmrun`/`qemu-system-x86_64`) and the `choose_backend`/`suggest_action` preference logic. `launch.rs` handles actually starting a backend (writing the `.wsb` config for Windows Sandbox, `VBoxManage`/PowerShell `Start-VM` invocations). `builder.rs` is a **stub** — VM image building (downloading/building an Alpine VirtualBox disk) was deliberately deferred out of scope for this rewrite and always returns an informative "not implemented" error; don't be surprised the VirtualBox path in `SandboxScreen` can't actually provision a fresh disk yet.

### Windows-only by design

External app launching (`src/launch_external.rs`: browser via `cmd /C start`, terminal via `powershell.exe`, file manager via `explorer.exe`) has no cross-platform fallback — this was a deliberate scope decision (the whole point of the rewrite is a Windows `.exe`), not an oversight. Don't add `xdg-open`/`open`/`$SHELL` branches without checking whether that's actually wanted.

### Install/uninstall scripts (`scripts/`)

`Install-Tsukuyomi.ps1` copies a prebuilt `tsukuyomi.exe` (looked for next to the script first, then at `../target/release/tsukuyomi.exe` for developers building from source) into `%LOCALAPPDATA%\TsukuyomiOS` and creates a desktop shortcut — no Python bootstrap. `Nuke-Tsukuyomi.ps1` delegates data cleanup to `tsukuyomi.exe uninstall --yes` (the single source of truth for which directories hold app data) and only then removes the install directory itself, in that order deliberately — deleting the install directory (which contains the running exe) before the exe has exited would try to delete a locked file. The `.bat` files are thin wrappers that just invoke the corresponding `.ps1`.
