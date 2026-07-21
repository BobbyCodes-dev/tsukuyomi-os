use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::aimap::{self, AimapResult, AimapSession};
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 1;

enum Mode {
    List,
    New,
    Detail,
    Running,
}

pub struct AimapState {
    user_id: i64,
    sessions: Vec<AimapSession>,
    results: Vec<AimapResult>,
    selected: usize,
    mode: Mode,
    query: widgets::TextField,
    focus: usize,
    log: widgets::LogPanel,
    running: bool,
    run_rx: Option<Receiver<(i64, Vec<(String, i64, String, String, String)>)>>,
    status: String,
}

impl AimapState {
    pub fn new(user_id: i64) -> Self {
        let mut log = widgets::LogPanel::new(200);
        log.push("AIMap: Discover exposed AI infrastructure.".to_string());
        log.push("Requires Shodan API key and Python 3.10+.".to_string());
        log.push("Press 'n' to start a new scan.".to_string());
        let mut state = AimapState {
            user_id,
            sessions: Vec::new(),
            results: Vec::new(),
            selected: 0,
            mode: Mode::List,
            query: widgets::TextField::new(),
            focus: 0,
            log,
            running: false,
            run_rx: None,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match aimap::list_sessions(self.user_id) {
            Ok(sessions) => {
                self.sessions = sessions;
                if self.selected >= self.sessions.len() {
                    self.selected = self.sessions.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading sessions: {e}"),
        }
    }

    pub fn poll_run(&mut self) {
        if !self.running {
            return;
        }
        let Some(rx) = &self.run_rx else { return };
        match rx.try_recv() {
            Ok((session_id, hosts)) => {
                let count = hosts.len() as i64;
                for (ip, port, service, banner, metadata) in &hosts {
                    let _ = aimap::add_result(
                        self.user_id,
                        session_id,
                        ip,
                        *port,
                        service,
                        banner,
                        metadata,
                    );
                }
                let _ = aimap::finish_session(self.user_id, session_id, count, "complete");
                self.log.push(format!("AIMap scan finished: {count} hosts discovered."));
                self.running = false;
                self.run_rx = None;
                self.mode = Mode::List;
                self.refresh();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.log.push("AIMap scan thread disconnected.".to_string());
                self.running = false;
                self.run_rx = None;
                self.mode = Mode::List;
            }
        }
    }
}

fn start_scan(state: &mut AimapState) {
    let query = state.query.value.trim().to_string();
    if query.is_empty() {
        state.status = "Query is required.".to_string();
        return;
    }
    let session_id = match aimap::create_session(state.user_id, &query) {
        Ok(id) => id,
        Err(e) => {
            state.status = format!("Error creating session: {e}");
            return;
        }
    };
    state.status = "Running AIMap scan...".to_string();
    state.log.push(format!("Starting AIMap scan: {query}"));
    state.running = true;
    state.mode = Mode::Running;

    let (tx, rx) = mpsc::channel();
    state.run_rx = Some(rx);

    std::thread::spawn(move || {
        let hosts = run_aimap_scan(&query);
        let _ = tx.send((session_id, hosts));
    });
}

/// Runs the AIMap scanner. Uses the Shodan API (via Python script or
/// the aimap CLI) to discover exposed AI endpoints.
/// Returns a list of (ip, port, service, banner, metadata) tuples.
fn run_aimap_scan(query: &str) -> Vec<(String, i64, String, String, String)> {
    use std::process::Command;

    // Try aimap CLI first
    let aimap_check = Command::new("aimap").arg("--help").output();
    let cli_installed = match aimap_check {
        Ok(out) => out.status.success(),
        Err(_) => false,
    };

    if cli_installed {
        let output = Command::new("aimap")
            .args(["--query", query, "--format", "json"])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                return parse_aimap_output(&stdout);
            }
        }
    }

    // Fallback: try Python module directly
    let py_output = Command::new("python")
        .args(["-c", &format!(
            "import aimap; import json; results = aimap.scan('{}'); print(json.dumps(results))",
            query.replace('\'', "\\'")
        )])
        .output();

    if let Ok(out) = py_output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            return parse_aimap_output(&stdout);
        }
    }

    // If nothing works, return an informational message
    vec![(
        "203.0.113.0".to_string(),
        0i64,
        "setup_required".to_string(),
        "AIMap not installed. Install: pip install aimap. Set SHODAN_API_KEY env var.".to_string(),
        "{}".to_string(),
    )]
}

/// Parse AIMap JSON output into host records.
/// Expects a JSON array of objects with ip_str, port, _shodan.module, and data fields.
fn parse_aimap_output(stdout: &str) -> Vec<(String, i64, String, String, String)> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(arr) = json.as_array() {
            return arr
                .iter()
                .filter_map(|item| {
                    let ip = item.get("ip_str")?.as_str()?.to_string();
                    let port = item.get("port")?.as_i64()?;
                    let service = item.get("_shodan")
                        .and_then(|s| s.get("module"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let banner = item.get("data")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string();
                    let metadata = item.to_string();
                    Some((ip, port, service, banner, metadata))
                })
                .collect();
        }
    }

    Vec::new()
}

fn load_results(state: &mut AimapState) {
    if let Some(session) = state.sessions.get(state.selected) {
        match aimap::list_results(state.user_id, session.id) {
            Ok(results) => {
                state.results = results;
                state.mode = Mode::Detail;
                state.status.clear();
            }
            Err(e) => state.status = format!("Error loading results: {e}"),
        }
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &AimapState) {
    let rect = widgets::centered_fixed(100, area.height.min(30), area);
    let block = widgets::form_block("AIMap - AI Infrastructure Discovery");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::New => draw_new(frame, inner, state),
        Mode::Detail => draw_detail(frame, inner, state),
        Mode::Running => draw_running(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &AimapState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(8), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .sessions
        .iter()
        .map(|s| {
            Row::new(vec![
                s.query.clone(),
                s.status.clone(),
                s.result_count.to_string(),
                s.created_at.clone(),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Min(30), Constraint::Length(12), Constraint::Length(8), Constraint::Length(20)],
    )
    .header(Row::new(vec!["Query", "Status", "Hosts", "Created"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.sessions.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.log.lines().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let log_widget =
        Paragraph::new(all[start..].to_vec()).block(widgets::log_block("Status")).wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[1]);

    let mut lines = vec![Line::styled(
        "n: new scan  Enter: view hosts  d: delete  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[2]);
}

fn draw_new(frame: &mut Frame, area: Rect, state: &AimapState) {
    let mut lines = vec![
        Line::styled("New AIMap Scan", theme::title_style()),
        Line::raw(""),
        field_line("Shodan query (e.g. product:ollama)", state.query.display(), state.focus == 0),
        Line::raw(""),
        Line::styled("Tab: move  Enter: run  Esc: cancel", theme::hint_style()),
        Line::styled(
            "Discovers exposed AI endpoints via Shodan (requires API key).",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_detail(frame: &mut Frame, area: Rect, state: &AimapState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    if let Some(session) = state.sessions.get(state.selected) {
        let header = vec![
            Line::from(vec![
                Span::styled("Query: ", theme::title_style()),
                Span::raw(session.query.clone()),
            ]),
            Line::from(vec![
                Span::styled("Hosts: ", theme::title_style()),
                Span::raw(session.result_count.to_string()),
                Span::raw(" | "),
                Span::styled("Status: ", theme::title_style()),
                Span::raw(session.status.clone()),
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);
    }

    let rows: Vec<Row> = state
        .results
        .iter()
        .map(|r| Row::new(vec![r.ip.clone(), r.port.to_string(), r.service.clone(), r.banner.clone()]))
        .collect();
    let table = Table::new(rows, [Constraint::Length(18), Constraint::Length(8), Constraint::Length(18), Constraint::Min(30)])
        .header(Row::new(vec!["IP", "Port", "Service", "Banner"]).style(theme::title_style()))
        .row_highlight_style(theme::focused_field_style())
        .highlight_symbol("> ")
        .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.results.is_empty() { None } else { Some(0) });
    frame.render_stateful_widget(table, chunks[1], &mut table_state);

    let mut lines = vec![Line::styled("Esc: back to sessions", theme::hint_style())];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines), chunks[2]);
}

fn draw_running(frame: &mut Frame, area: Rect, state: &AimapState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let header = vec![
        Line::styled("AIMap Scan Running", theme::title_style()),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Query: ", theme::title_style()),
            Span::raw(state.query.value.clone()),
        ]),
    ];
    frame.render_widget(Paragraph::new(header), chunks[0]);

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.log.lines().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let log_widget =
        Paragraph::new(all[start..].to_vec()).block(widgets::log_block("Scan Log")).wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[1]);

    let lines = vec![Line::styled("Press Esc to return (scan continues in background)", theme::hint_style())];
    frame.render_widget(Paragraph::new(lines), chunks[2]);
}

fn handle_list_key(state: &mut AimapState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            if !state.sessions.is_empty() {
                state.selected = if state.selected == 0 { state.sessions.len() - 1 } else { state.selected - 1 };
            }
            Action::None
        }
        KeyCode::Down => {
            if !state.sessions.is_empty() {
                state.selected = (state.selected + 1) % state.sessions.len();
            }
            Action::None
        }
        KeyCode::Char('n') => {
            state.query = widgets::TextField::new();
            state.focus = 0;
            state.mode = Mode::New;
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            if !state.sessions.is_empty() {
                load_results(state);
            }
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(session) = state.sessions.get(state.selected) {
                let id = session.id;
                match aimap::delete_session(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Session deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting session: {e}"),
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_new_key(state: &mut AimapState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status.clear();
            Action::None
        }
        KeyCode::Tab | KeyCode::Down => {
            state.focus = (state.focus + 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.focus = (state.focus + FIELD_COUNT - 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::Enter => {
            start_scan(state);
            Action::None
        }
        KeyCode::Backspace => {
            state.query.backspace();
            Action::None
        }
        KeyCode::Char(c) => {
            state.query.push_char(c);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_running_key(state: &mut AimapState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status = "Scan running in background...".to_string();
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut AimapState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }
    match state.mode {
        Mode::List => handle_list_key(state, key),
        Mode::New => handle_new_key(state, key),
        Mode::Detail => {
            if key.code == KeyCode::Esc {
                state.mode = Mode::List;
            }
            Action::None
        }
        Mode::Running => handle_running_key(state, key),
    }
}