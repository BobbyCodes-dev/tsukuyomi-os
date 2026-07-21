use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::paramspider::{self, ParamSpiderResult, ParamSpiderSession};
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 1;

enum Mode {
    List,
    New,
    Detail,
    Running,
}

pub struct ParamSpiderState {
    user_id: i64,
    sessions: Vec<ParamSpiderSession>,
    results: Vec<ParamSpiderResult>,
    selected: usize,
    mode: Mode,
    query: widgets::TextField,
    focus: usize,
    log: widgets::LogPanel,
    running: bool,
    run_rx: Option<Receiver<(i64, Vec<(String, String, String)>)>>,
    status: String,
}

impl ParamSpiderState {
    pub fn new(user_id: i64) -> Self {
        let mut log = widgets::LogPanel::new(200);
        log.push("ParamSpider - URL Mining from Web Archives".to_string());
        log.push("Requires paramspider (pip install .)".to_string());
        log.push("Press 'n' to start a new query.".to_string());
        let mut state = ParamSpiderState {
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
        match paramspider::list_sessions(self.user_id) {
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
            Ok((session_id, findings)) => {
                let count = findings.len() as i64;
                for (step, finding, source_url) in &findings {
                    let _ = paramspider::add_result(
                        self.user_id,
                        session_id,
                        &self.query.value,
                        step,
                        finding,
                        source_url,
                    );
                }
                let _ = paramspider::finish_session(self.user_id, session_id, count, "complete");
                self.log.push(format!("ParamSpider mining finished: {} URLs found.", count));
                self.running = false;
                self.run_rx = None;
                self.mode = Mode::List;
                self.refresh();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.log.push("ParamSpider thread disconnected.".to_string());
                self.running = false;
                self.run_rx = None;
                self.mode = Mode::List;
            }
        }
    }
}

fn start_query(state: &mut ParamSpiderState) {
    let query = state.query.value.trim().to_string();
    if query.is_empty() {
        state.status = "Query is required.".to_string();
        return;
    }
    let session_id = match paramspider::create_session(state.user_id, &query) {
        Ok(id) => id,
        Err(e) => {
            state.status = format!("Error creating session: {e}");
            return;
        }
    };
    state.status = "Running ParamSpider...".to_string();
    state.log.push(format!("Starting ParamSpider: {query}"));
    state.running = true;
    state.mode = Mode::Running;

    let (tx, rx) = mpsc::channel();
    state.run_rx = Some(rx);

    std::thread::spawn(move || {
        let findings = run_tool_pipeline(&query);
        let _ = tx.send((session_id, findings));
    });
}

/// Runs the tool pipeline in a subprocess.
/// Returns a list of (step, finding, source_url) tuples.
fn run_tool_pipeline(query: &str) -> Vec<(String, String, String)> {
    use std::process::Command;

    // Check if CLI is installed
    let cli_check = Command::new("paramspider").arg("--version").output();
    let installed = match cli_check {
        Ok(out) => out.status.success(),
        Err(_) => false,
    };

    if !installed {
        return vec![(
            "system_check".to_string(),
            "ParamSpider CLI not found. CLI_INSTALL_PLACEHOLDER".to_string(),
            "https://github.com/devanshbatham/ParamSpider".to_string(),
        )];
    }

    // Run the tool
    let output = Command::new("paramspider")
        .arg("--query")
        .arg(query)
        .arg("--format")
        .arg("json")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            parse_tool_output(&stdout)
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            vec![("pipeline_error".to_string(), format!("ParamSpider error: {stderr}"), "".to_string())]
        }
        Err(e) => {
            vec![("pipeline_error".to_string(), format!("Failed to execute ParamSpider: {e}"), "".to_string())]
        }
    }
}

/// Parse tool output into findings.
fn parse_tool_output(stdout: &str) -> Vec<(String, String, String)> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return vec![("pipeline".to_string(), "No output from ParamSpider.".to_string(), "".to_string())];
    }

    // Try JSON parsing first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(arr) = json.as_array() {
            return arr
                .iter()
                .filter_map(|item| {
                    let step = item.get("step")?.as_str()?.to_string();
                    let finding = item.get("finding")?.as_str()?.to_string();
                    let url = item.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
                    Some((step, finding, url))
                })
                .collect();
        }
    }

    // Fallback: treat as plain text output
    vec![("pipeline".to_string(), trimmed.to_string(), "".to_string())]
}

fn load_results(state: &mut ParamSpiderState) {
    if let Some(session) = state.sessions.get(state.selected) {
        match paramspider::list_results(state.user_id, session.id) {
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

pub fn draw(frame: &mut Frame, area: Rect, state: &ParamSpiderState) {
    let rect = widgets::centered_fixed(100, area.height.min(30), area);
    let block = widgets::form_block("ParamSpider - URL Mining from Web Archives");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::New => draw_new(frame, inner, state),
        Mode::Detail => draw_detail(frame, inner, state),
        Mode::Running => draw_running(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &ParamSpiderState) {
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
    .header(Row::new(vec!["Query", "Status", "Results", "Created"]).style(theme::title_style()))
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
        "n: new query  Enter: view results  d: delete  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[2]);
}

fn draw_new(frame: &mut Frame, area: Rect, state: &ParamSpiderState) {
    let mut lines = vec![
        Line::styled(format!("New ParamSpider Query"), theme::title_style()),
        Line::raw(""),
        field_line("Domain (e.g. example.com)", state.query.display(), state.focus == 0),
        Line::raw(""),
        Line::styled("Tab: move  Enter: run  Esc: cancel", theme::hint_style()),
        Line::styled(
            "Mines URLs from web archives, filtering boring URLs to focus on parameter-rich ones.",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_detail(frame: &mut Frame, area: Rect, state: &ParamSpiderState) {
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
                Span::styled("Results: ", theme::title_style()),
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
        .map(|r| Row::new(vec![r.step.clone(), r.finding.clone(), r.source_url.clone()]))
        .collect();
    let table = Table::new(rows, [Constraint::Length(20), Constraint::Min(40), Constraint::Min(20)])
        .header(Row::new(vec!["Step", "Finding", "Source"]).style(theme::title_style()))
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

fn draw_running(frame: &mut Frame, area: Rect, state: &ParamSpiderState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let header = vec![
        Line::styled("ParamSpider Running", theme::title_style()),
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
        Paragraph::new(all[start..].to_vec()).block(widgets::log_block("Pipeline Log")).wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[1]);

    let lines = vec![Line::styled("Press Esc to cancel (results will still be saved)", theme::hint_style())];
    frame.render_widget(Paragraph::new(lines), chunks[2]);
}

fn handle_list_key(state: &mut ParamSpiderState, key: KeyEvent) -> Action {
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
                match paramspider::delete_session(state.user_id, id) {
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

fn handle_new_key(state: &mut ParamSpiderState, key: KeyEvent) -> Action {
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
            start_query(state);
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

fn handle_running_key(state: &mut ParamSpiderState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status = "Pipeline running in background...".to_string();
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut ParamSpiderState, key: KeyEvent) -> Action {
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
