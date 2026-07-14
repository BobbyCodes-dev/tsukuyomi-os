use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::launch_external;
use crate::store::{settings, users};
use crate::ui::{theme, widgets};

pub struct AppEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
}

pub const APPS: &[AppEntry] = &[
    AppEntry {
        id: "sandbox",
        name: "Tsukuyomi Sandbox",
        description: "Launch an isolated Windows VM for malware analysis.",
        icon: "\u{1F9EA}",
        category: "Security",
    },
    AppEntry {
        id: "browser",
        name: "Tsukuyomi Browser",
        description: "Open an embedded browser.",
        icon: "\u{1F310}",
        category: "Productivity",
    },
    AppEntry {
        id: "terminal",
        name: "Terminal",
        description: "Local system shell.",
        icon: "\u{1F4BB}",
        category: "System",
    },
    AppEntry {
        id: "files",
        name: "Tsukuyomi Files",
        description: "File manager.",
        icon: "\u{1F4C1}",
        category: "System",
    },
    AppEntry {
        id: "vault",
        name: "Credential Vault",
        description: "Encrypted store for names, usernames, passwords, and notes.",
        icon: "\u{1F510}",
        category: "Security",
    },
    AppEntry {
        id: "connect",
        name: "Quick Connect",
        description: "Saved SSH/RDP bookmarks for one-key connections.",
        icon: "\u{1F5A5}",
        category: "Network",
    },
    AppEntry {
        id: "assets",
        name: "Asset Inventory",
        description: "Track machines you support, with a quick reachability ping.",
        icon: "\u{1F5C3}",
        category: "Network",
    },
    AppEntry {
        id: "updates",
        name: "Patch Tracker",
        description: "View pending Windows updates via PSWindowsUpdate.",
        icon: "\u{1F5D2}",
        category: "System",
    },
    AppEntry {
        id: "health",
        name: "System Health",
        description: "CPU, RAM, disk usage, and Windows services.",
        icon: "\u{1F4CA}",
        category: "System",
    },
    AppEntry {
        id: "network",
        name: "Network Diagnostics",
        description: "Ping, traceroute, port check, and interface stats for a host.",
        icon: "\u{1F6F0}",
        category: "Network",
    },
    AppEntry {
        id: "firewall",
        name: "Firewall Rule Manager",
        description: "View, enable/disable, and add Windows Defender Firewall rules.",
        icon: "\u{1F6E1}",
        category: "Security",
    },
    AppEntry {
        id: "backups",
        name: "Backup Manager",
        description: "Track and run folder backups via robocopy.",
        icon: "\u{1F4BE}",
        category: "System",
    },
    AppEntry {
        id: "remote_support",
        name: "Remote Support",
        description: "Host or connect a mutual-consent remote session via RustDesk.",
        icon: "\u{1F91D}",
        category: "Network",
    },
    AppEntry {
        id: "engagements",
        name: "Engagement Tracker",
        description: "Track client security engagements: scope, dates, status, invoice ref.",
        icon: "\u{1F4CB}",
        category: "Security",
    },
    AppEntry {
        id: "scan_request",
        name: "Scan Request",
        description: "Submit an authorized scan request for an engagement via webhook.",
        icon: "\u{1F3AF}",
        category: "Security",
    },
    AppEntry {
        id: "osint_notes",
        name: "OSINT Notes",
        description: "Manual recon notebook tied to an engagement — no automated data gathering.",
        icon: "\u{1F4DD}",
        category: "Security",
    },
    AppEntry {
        id: "findings",
        name: "Findings / Reports",
        description: "Track findings, build markdown reports, and export client-ready documents.",
        icon: "\u{1F4C4}",
        category: "Security",
    },
    AppEntry {
        id: "evidence",
        name: "Evidence Vault",
        description: "Encrypted text-only evidence entries linked to findings.",
        icon: "\u{1F50F}",
        category: "Security",
    },
    AppEntry {
        id: "cve",
        name: "CVE Lookup",
        description: "Manual CVE notes with optional NVD fetch — offline-first.",
        icon: "\u{1F6A8}",
        category: "Security",
    },
    AppEntry {
        id: "settings",
        name: "Settings",
        description: "Configure Tsukuyomi OS.",
        icon: "\u{2699}",
        category: "System",
    },
];

pub fn now_string() -> String {
    let s = settings::load_settings();
    let tz: chrono_tz::Tz = s.timezone.parse().unwrap_or(chrono_tz::America::Chicago);
    let now = chrono::Utc::now().with_timezone(&tz);
    now.format("%a, %b %d, %Y  %I:%M:%S %p").to_string()
}

pub struct DesktopState {
    pub selected: usize,
    pub log: widgets::LogPanel,
    pub clock_text: String,
    last_tick: Instant,
}

impl DesktopState {
    pub fn new() -> Self {
        let mut log = widgets::LogPanel::new(200);
        log.push(format!(
            "[{}] Welcome to Tsukuyomi OS. Select an app and press Enter.",
            now_string()
        ));
        DesktopState { selected: 0, log, clock_text: now_string(), last_tick: Instant::now() }
    }

    pub fn log_status(&mut self, message: impl Into<String>) {
        self.log.push(format!("[{}] {}", now_string(), message.into()));
    }

    pub fn tick(&mut self) {
        if self.last_tick.elapsed() >= Duration::from_secs(1) {
            self.clock_text = now_string();
            self.last_tick = Instant::now();
        }
    }
}

pub fn draw(frame: &mut Frame, area: Rect, state: &DesktopState, user: &users::User) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(15),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::styled("Tsukuyomi OS", theme::title_style())),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!("User: {} ({})  |  Role: {}", user.display_name, user.username, user.role),
            theme::subtitle_style(),
        )),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(Line::styled(state.clock_text.clone(), theme::clock_style())),
        chunks[2],
    );
    frame.render_widget(
        Paragraph::new(Line::styled(
            "Use Up/Down to navigate, Enter to launch, 's' for settings, 'q' to quit.",
            theme::hint_style(),
        )),
        chunks[3],
    );

    let rows: Vec<Row> = APPS
        .iter()
        .map(|a| Row::new(vec![a.icon.to_string(), a.name.to_string(), a.description.to_string(), a.category.to_string()]))
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Length(4), Constraint::Length(22), Constraint::Min(20), Constraint::Length(14)],
    )
    .header(Row::new(vec!["Icon", "App", "Description", "Category"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block("Apps"));
    let mut table_state = TableState::default().with_selected(Some(state.selected));
    frame.render_stateful_widget(table, chunks[4], &mut table_state);

    let visible = chunks[5].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.log.lines().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let log_widget = Paragraph::new(all[start..].to_vec())
        .block(widgets::log_block("Status"))
        .wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[5]);
}

fn launch_selected(state: &mut DesktopState) -> Action {
    let app = &APPS[state.selected];
    state.log_status(format!("Launching {}...", app.name));
    match app.id {
        "sandbox" => Action::ToSandbox,
        "settings" => Action::ToSettings,
        "vault" => Action::ToVault,
        "connect" => Action::ToConnect,
        "assets" => Action::ToAssets,
        "updates" => Action::ToUpdates,
        "health" => Action::ToHealth,
        "network" => Action::ToNetwork,
        "firewall" => Action::ToFirewall,
        "backups" => Action::ToBackups,
        "remote_support" => Action::ToRemoteSupport,
        "engagements" => Action::ToEngagements,
        "scan_request" => Action::ToScanRequest,
        "osint_notes" => Action::ToOsintNotes,
        "findings" => Action::ToFindings,
        "evidence" => Action::ToEvidence,
        "cve" => Action::ToCve,
        "browser" => {
            launch_external::open_browser();
            state.log_status("Browser opened externally.");
            Action::None
        }
        "terminal" => {
            launch_external::open_terminal();
            state.log_status("Terminal opened externally.");
            Action::None
        }
        "files" => {
            launch_external::open_files();
            state.log_status("File manager opened externally.");
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut DesktopState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('r') => {
            state.clock_text = now_string();
            state.log_status("Refreshed.");
            Action::None
        }
        KeyCode::Char('s') => Action::ToSettings,
        KeyCode::Up => {
            state.selected = if state.selected == 0 { APPS.len() - 1 } else { state.selected - 1 };
            Action::None
        }
        KeyCode::Down => {
            state.selected = (state.selected + 1) % APPS.len();
            Action::None
        }
        KeyCode::Enter => launch_selected(state),
        _ => Action::None,
    }
}
