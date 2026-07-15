# Privacy Policy

Last updated: 2026-07-15

Tsukuyomi OS is a local, offline-first application. This policy explains what data it handles and where it goes.

## Data stored locally

All data Tsukuyomi OS creates for you — your account, password hash, settings, Credential Vault entries, engagement/finding/evidence records, CVE notes, and backup job configs — is stored only on your own machine, in `%LOCALAPPDATA%\TsukuyomiOS`. None of it is transmitted to the developer of Tsukuyomi OS. There is no telemetry, analytics, or crash reporting built into the app.

## Data that leaves your device

Tsukuyomi OS only makes outbound network calls when you explicitly use a feature that requires one:

- **AI Agent** — if you configure an AI provider (Anthropic, OpenAI-compatible, Gemini, Ollama Cloud, or local Ollama), your chat messages are sent to that provider's API to generate a response. That provider's own privacy policy governs how they handle that data. Local Ollama keeps everything on your machine. Your API key is encrypted at rest in the Credential Vault and only decrypted in memory to make a request.
- **CVE Lookup** — optionally queries the public NVD (National Vulnerability Database) API for CVE details you request. No account data is sent.
- **Quick Connect / Remote Support** — SSH, RDP, and RustDesk sessions you initiate connect directly (or via RustDesk's own relay infrastructure, depending on your configuration) to the host you specify.
- **Patch Tracker** — checks pending Windows updates locally via `PSWindowsUpdate`; this talks to Microsoft's Windows Update service, not Tsukuyomi OS's developer.
- **Ollama auto-install** — if you choose to install local Ollama through Settings, the official installer is downloaded directly from ollama.com.

## What Tsukuyomi OS does not do

- No usage analytics or telemetry.
- No "phone home" on startup or at any other time.
- No developer-operated server that the app talks to.

## Your control over your data

Because everything is stored locally, you control it entirely. Uninstalling (see README) removes it. Settings also includes a "type NUKE to erase all data" option that wipes all local Tsukuyomi OS data from within the app, while leaving the exe itself in place.

## Changes

This policy may be updated between releases. Changes will be reflected in this file and in the copy shown during account setup.
