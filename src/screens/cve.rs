use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::cve::{self, CveEntry};
use crate::ui::{theme, widgets};

enum Mode {
    List,
    Add,
    Detail,
    Fetching,
}

pub struct CveState {
    user_id: i64,
    entries: Vec<CveEntry>,
    selected: usize,
    mode: Mode,
    cve_id: widgets::TextField,
    description: widgets::TextField,
    cvss: widgets::TextField,
    severity: widgets::TextField,
    references: widgets::TextField,
    focus: usize,
    fetch_target: widgets::TextField,
    status: String,
    fetch_handle: Option<std::thread::JoinHandle<()>>,
    fetch_result: Option<anyhow::Result<()>>,
}

impl CveState {
    pub fn new(user_id: i64) -> Self {
        let mut state = CveState {
            user_id,
            entries: Vec::new(),
            selected: 0,
            mode: Mode::List,
            cve_id: widgets::TextField::new(),
            description: widgets::TextField::new(),
            cvss: widgets::TextField::new(),
            severity: widgets::TextField::new(),
            references: widgets::TextField::new(),
            focus: 0,
            fetch_target: widgets::TextField::new(),
            status: String::new(),
            fetch_handle: None,
            fetch_result: None,
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match cve::list_cves(self.user_id) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading CVEs: {e}"),
        }
    }

    fn clear_add(&mut self) {
        self.cve_id = widgets::TextField::new();
        self.description = widgets::TextField::new();
        self.cvss = widgets::TextField::new();
        self.severity = widgets::TextField::new();
        self.references = widgets::TextField::new();
        self.focus = 0;
    }

    fn save_add(&mut self) {
        if self.cve_id.value.trim().is_empty() {
            self.status = "CVE ID is required.".to_string();
            return;
        }
        let result = cve::add_cve(
            self.user_id,
            self.cve_id.value.trim(),
            &self.description.value,
            &self.cvss.value,
            &self.severity.value,
            &self.references.value,
            &today(),
        );
        match result {
            Ok(_) => {
                self.status = "CVE saved.".to_string();
                self.mode = Mode::List;
                self.refresh();
            }
            Err(e) => self.status = format!("Error saving CVE: {e}"),
        }
    }

    pub fn poll_fetch(&mut self) {
        if let Some(handle) = self.fetch_handle.take() {
            if handle.is_finished() {
                if let Some(result) = self.fetch_result.take() {
                    match result {
                        Ok(()) => {
                            self.status = "Fetch complete.".to_string();
                            self.refresh();
                        }
                        Err(e) => self.status = format!("Fetch failed: {e}"),
                    }
                }
                self.mode = Mode::List;
            } else {
                self.fetch_handle = Some(handle);
            }
        }
    }
}

fn today() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let days = now.duration_since(UNIX_EPOCH).unwrap().as_secs() / 86_400;
    let (y, m, d) = unix_days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn unix_days_to_ymd(mut days: u64) -> (i32, u32, u32) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year as u64 { break; }
        days -= days_in_year as u64;
        year += 1;
    }
    let mut month = 1;
    let days_in_months = [31, if is_leap_year(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for dim in days_in_months {
        if days < dim as u64 { break; }
        days -= dim as u64;
        month += 1;
    }
    (year, month, days as u32 + 1)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &CveState) {
    let rect = widgets::centered_fixed(90, area.height.min(28), area);
    let block = widgets::form_block("CVE Lookup");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Add => draw_add(frame, inner, state),
        Mode::Detail => draw_detail(frame, inner, state),
        Mode::Fetching => draw_fetching(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &CveState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|c| Row::new(vec![c.cve_id.clone(), c.severity.clone(), c.cvss_score.clone()]))
        .collect();
    let table = Table::new(rows, [Constraint::Min(20), Constraint::Length(12), Constraint::Length(10)])
        .header(Row::new(vec!["CVE", "Severity", "CVSS"]).style(theme::title_style()))
        .row_highlight_style(theme::focused_field_style())
        .highlight_symbol("> ")
        .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "a: add  Enter: detail  f: fetch from NVD  d: delete  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_add(frame: &mut Frame, area: Rect, state: &CveState) {
    let mut lines = vec![
        Line::styled("Add CVE (manual)", theme::title_style()),
        Line::raw(""),
        field_line("CVE ID", state.cve_id.display(), state.focus == 0),
        field_line("Severity", state.severity.display(), state.focus == 1),
        field_line("CVSS", state.cvss.display(), state.focus == 2),
        field_line("References", state.references.display(), state.focus == 3),
        Line::raw(""),
        field_line("Description", state.description.display(), state.focus == 4),
        Line::raw(""),
        Line::styled("Tab: move  Enter: save  Esc: cancel", theme::hint_style()),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_detail(frame: &mut Frame, area: Rect, state: &CveState) {
    let item = state.entries.get(state.selected);
    let mut lines = vec![Line::styled("CVE Detail", theme::title_style()), Line::raw("")];
    if let Some(c) = item {
        lines.push(Line::from(vec![Span::styled("ID: ", theme::title_style()), Span::raw(c.cve_id.clone())]));
        lines.push(Line::from(vec![Span::styled("Severity: ", theme::title_style()), Span::raw(c.severity.clone())]));
        lines.push(Line::from(vec![Span::styled("CVSS: ", theme::title_style()), Span::raw(c.cvss_score.clone())]));
        lines.push(Line::from(vec![Span::styled("Fetched: ", theme::title_style()), Span::raw(c.fetched_at.clone())]));
        if !c.description.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::styled("Description:", theme::title_style()));
            for line in c.description.lines() {
                lines.push(Line::raw(line.to_string()));
            }
        }
        if !c.refs.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::styled("References:", theme::title_style()));
            for line in c.refs.lines() {
                lines.push(Line::raw(line.to_string()));
            }
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("Esc: back", theme::hint_style()));
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_fetching(frame: &mut Frame, area: Rect, state: &CveState) {
    let mut lines = vec![
        Line::styled("Fetch from NVD", theme::title_style()),
        Line::raw(""),
        field_line("CVE ID", state.fetch_target.display(), true),
        Line::raw(""),
        Line::styled("Enter: fetch  Esc: cancel", theme::hint_style()),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn start_fetch(state: &mut CveState) {
    let cve_id = state.fetch_target.value.trim().to_string();
    if cve_id.is_empty() {
        state.status = "CVE ID is required.".to_string();
        return;
    }
    state.status = "Fetching from NVD...".to_string();
    let user_id = state.user_id;
    let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<()>>();
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            match cve::fetch_nvd(&cve_id).await {
                Ok(data) => cve::upsert_from_nvd(user_id, &cve_id, &data).map(|_| ()),
                Err(e) => Err(e),
            }
        });
        let _ = tx.send(result);
    });
    state.fetch_handle = Some(handle);
    state.fetch_result = rx.recv().ok();
}

fn handle_list_key(state: &mut CveState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            if !state.entries.is_empty() {
                state.selected = if state.selected == 0 { state.entries.len() - 1 } else { state.selected - 1 };
            }
            Action::None
        }
        KeyCode::Down => {
            if !state.entries.is_empty() {
                state.selected = (state.selected + 1) % state.entries.len();
            }
            Action::None
        }
        KeyCode::Char('a') => {
            state.clear_add();
            state.mode = Mode::Add;
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            if !state.entries.is_empty() {
                state.mode = Mode::Detail;
                state.status.clear();
            }
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(c) = state.entries.get(state.selected) {
                let id = c.id;
                match cve::delete_cve(state.user_id, id) {
                    Ok(()) => {
                        state.status = "CVE deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting CVE: {e}"),
                }
            }
            Action::None
        }
        KeyCode::Char('f') => {
            state.fetch_target = widgets::TextField::new();
            state.mode = Mode::Fetching;
            state.status.clear();
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_add_key(state: &mut CveState, key: KeyEvent) -> Action {
    const FIELD_COUNT: usize = 5;
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
            state.save_add();
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.cve_id.backspace(),
                1 => state.severity.backspace(),
                2 => state.cvss.backspace(),
                3 => state.references.backspace(),
                4 => state.description.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.cve_id.push_char(c),
                1 => state.severity.push_char(c),
                2 => state.cvss.push_char(c),
                3 => state.references.push_char(c),
                4 => state.description.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_fetching_key(state: &mut CveState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            start_fetch(state);
            Action::None
        }
        KeyCode::Backspace => {
            state.fetch_target.backspace();
            Action::None
        }
        KeyCode::Char(c) => {
            state.fetch_target.push_char(c);
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut CveState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }
    match state.mode {
        Mode::List => handle_list_key(state, key),
        Mode::Add => handle_add_key(state, key),
        Mode::Detail => {
            if key.code == KeyCode::Esc {
                state.mode = Mode::List;
            }
            Action::None
        }
        Mode::Fetching => handle_fetching_key(state, key),
    }
}
