use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::ui::{theme, widgets};

#[cfg(windows)]
const MODULE_MISSING_MESSAGE: &str =
    "PSWindowsUpdate module not installed — run `Install-Module PSWindowsUpdate` as admin.";

#[cfg(unix)]
const MODULE_MISSING_MESSAGE: &str =
    "Package manager not detected — install apt, dnf, or pacman for update tracking.";

pub struct PendingUpdate {
    pub kb: String,
    pub size: String,
    pub title: String,
}

pub struct UpdatesState {
    module_installed: bool,
    updates: Vec<PendingUpdate>,
    error: Option<String>,
    status: String,
}

impl UpdatesState {
    pub fn new() -> Self {
        let mut state = UpdatesState {
            module_installed: false,
            updates: Vec::new(),
            error: None,
            status: String::new(),
        };
        state.refresh();
        state
    }

    pub fn refresh(&mut self) {
        self.error = None;
        self.updates.clear();
        self.status.clear();

        #[cfg(windows)]
        {
            match check_module_available() {
                Ok(true) => {
                    self.module_installed = true;
                    match fetch_pending_updates() {
                        Ok(updates) => {
                            self.status = format!("{} pending update(s).", updates.len());
                            self.updates = updates;
                        }
                        Err(e) => self.error = Some(format!("Failed to query updates: {e}")),
                    }
                }
                Ok(false) => {
                    self.module_installed = false;
                }
                Err(e) => self.error = Some(format!("Failed to check PSWindowsUpdate module: {e}")),
            }
        }

        #[cfg(unix)]
        {
            match fetch_pending_updates() {
                Ok(updates) => {
                    self.module_installed = true;
                    self.status = format!("{} pending update(s).", updates.len());
                    self.updates = updates;
                }
                Err(e) => {
                    self.module_installed = false;
                    self.error = Some(e);
                }
            }
        }
    }
}

// ── Windows: PSWindowsUpdate ────────────────────────────────────

#[cfg(windows)]
fn check_module_available() -> Result<bool, String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "if (Get-Module -ListAvailable -Name PSWindowsUpdate) { 'yes' } else { 'no' }"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "yes")
}

#[cfg(windows)]
fn fetch_pending_updates() -> Result<Vec<PendingUpdate>, String> {
    let script = "Import-Module PSWindowsUpdate; Get-WindowsUpdate | ForEach-Object { \"$($_.KB)|$($_.Size)|$($_.Title)\" }";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut updates = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.splitn(3, '|');
        updates.push(PendingUpdate {
            kb: parts.next().unwrap_or("").to_string(),
            size: parts.next().unwrap_or("").to_string(),
            title: parts.next().unwrap_or("").to_string(),
        });
    }
    Ok(updates)
}

// ── Linux: apt / dnf / pacman ───────────────────────────────────

#[cfg(unix)]
fn has_cmd(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn fetch_pending_updates() -> Result<Vec<PendingUpdate>, String> {
    // Try apt first (Debian/Ubuntu)
    if has_cmd("apt") {
        // apt list --upgradable
        let output = Command::new("apt")
            .args(["list", "--upgradable", "-a"])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("apt list --upgradable failed — may need: sudo apt update".to_string());
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut updates = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("Listing...") { continue; }
            // Format: package/version arch [upgradable from: old-version]
            let parts: Vec<&str> = line.splitn(2, '/').collect();
            if parts.len() < 2 { continue; }
            let pkg = parts[0];
            let rest = parts[1];
            // Extract version
            let version = rest.split_whitespace().next().unwrap_or("?");
            updates.push(PendingUpdate {
                kb: pkg.to_string(),
                size: "—".to_string(),
                title: format!("{} → {}", pkg, version),
            });
        }
        return Ok(updates);
    }

    // Try dnf (Fedora/RHEL)
    if has_cmd("dnf") {
        let output = Command::new("dnf")
            .args(["check-update", "--quiet"])
            .output()
            .map_err(|e| e.to_string())?;
        // dnf check-update returns 0 = no updates, 100 = updates available
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut updates = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("Last metadata") { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                updates.push(PendingUpdate {
                    kb: parts[0].to_string(),
                    size: "—".to_string(),
                    title: format!("{} → {}", parts[0], parts[1]),
                });
            }
        }
        return Ok(updates);
    }

    // Try pacman (Arch)
    if has_cmd("pacman") {
        let output = Command::new("pacman")
            .args(["-Qu", "--noconfirm"])
            .output()
            .map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut updates = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            // Format: package old_version -> new_version
            updates.push(PendingUpdate {
                kb: line.split_whitespace().next().unwrap_or("").to_string(),
                size: "—".to_string(),
                title: line.to_string(),
            });
        }
        return Ok(updates);
    }

    Err("No supported package manager found (apt/dnf/pacman)".to_string())
}

pub fn draw(frame: &mut Frame, area: Rect, state: &UpdatesState) {
    let rect = widgets::centered_fixed(94, area.height.min(26), area);
    let block = widgets::form_block("Patch/Update Tracker");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(inner);

    if let Some(err) = &state.error {
        frame.render_widget(
            Paragraph::new(Line::styled(err.clone(), theme::error_style())).wrap(Wrap { trim: false }),
            chunks[0],
        );
    } else if !state.module_installed {
        frame.render_widget(
            Paragraph::new(Line::styled(MODULE_MISSING_MESSAGE, theme::error_style()))
                .wrap(Wrap { trim: false }),
            chunks[0],
        );
    } else if state.updates.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled("No pending updates.", theme::subtitle_style())),
            chunks[0],
        );
    } else {
        let rows: Vec<Row> = state
            .updates
            .iter()
            .map(|u| Row::new(vec![u.kb.clone(), u.size.clone(), u.title.clone()]))
            .collect();
        let table = Table::new(rows, [Constraint::Length(12), Constraint::Length(10), Constraint::Min(30)])
            .header(Row::new(vec!["Package", "Size", "Update"]).style(theme::title_style()))
            .block(widgets::form_block(""));
        frame.render_widget(table, chunks[0]);
    }

    let mut lines = vec![Line::styled("r: refresh  Esc: back", theme::hint_style())];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

pub fn handle_key(state: &mut UpdatesState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Char('r') => {
            state.refresh();
            Action::None
        }
        _ => Action::None,
    }
}