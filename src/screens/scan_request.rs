use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::scan;
use crate::store::engagements;
use crate::store::scan_requests::{self, ScanRequest};
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 2;

enum Mode {
    List,
    New,
    EditNotes(i64),
}

pub struct ScanRequestState {
    user_id: i64,
    entries: Vec<ScanRequest>,
    engagement_labels: Vec<(i64, String)>,
    selected: usize,
    mode: Mode,
    engagement_idx: usize,
    target: widgets::TextField,
    notes: widgets::TextField,
    focus: usize,
    log: widgets::LogPanel,
    submitting: bool,
    submit_rx: Option<Receiver<(i64, String, Result<String, String>)>>,
    status: String,
}

impl ScanRequestState {
    pub fn new(user_id: i64) -> Self {
        let mut log = widgets::LogPanel::new(200);
        if !scan::nmap_available() {
            log.push(scan::NMAP_MISSING_MESSAGE.to_string());
        }
        let mut state = ScanRequestState {
            user_id,
            entries: Vec::new(),
            engagement_labels: Vec::new(),
            selected: 0,
            mode: Mode::List,
            engagement_idx: 0,
            target: widgets::TextField::new(),
            notes: widgets::TextField::new(),
            focus: 0,
            log,
            submitting: false,
            submit_rx: None,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match scan_requests::list_scan_requests(self.user_id) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading scan requests: {e}"),
        }
        match engagements::list_engagement_labels(self.user_id) {
            Ok(labels) => {
                self.engagement_labels = labels;
                if self.engagement_idx >= self.engagement_labels.len() {
                    self.engagement_idx = 0;
                }
            }
            Err(e) => self.status = format!("Error loading engagements: {e}"),
        }
    }

    fn engagement_label(&self, engagement_id: i64) -> String {
        self.engagement_labels
            .iter()
            .find(|(id, _)| *id == engagement_id)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| "(deleted engagement)".to_string())
    }

    pub fn poll_submit(&mut self) {
        let Some(rx) = &self.submit_rx else { return };
        match rx.try_recv() {
            Ok((id, target, Ok(output))) => {
                let summary = if output.trim().is_empty() { "nmap produced no output.".to_string() } else { output };
                self.log.push(format!("Scan of {target} finished:\n{summary}"));
                let _ = scan_requests::update_notes(self.user_id, id, &summary);
                self.submitting = false;
                self.submit_rx = None;
                self.refresh();
            }
            Ok((id, target, Err(e))) => {
                self.log.push(format!("Scan of {target} failed: {e}"));
                let _ = scan_requests::update_notes(self.user_id, id, &format!("Scan failed: {e}"));
                self.submitting = false;
                self.submit_rx = None;
                self.refresh();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.submitting = false;
                self.submit_rx = None;
            }
        }
    }
}

fn fire_scan(state: &mut ScanRequestState, id: i64, target: String) {
    if state.submitting {
        state.log.push("A previous scan is still running; the new request was logged anyway.".to_string());
    }
    state.submitting = true;
    let (tx, rx) = mpsc::channel();
    state.submit_rx = Some(rx);
    std::thread::spawn(move || {
        let result = scan::run_scan(&target);
        let _ = tx.send((id, target, result));
    });
}

fn submit_request(state: &mut ScanRequestState) {
    if state.engagement_labels.is_empty() {
        state.status = "No engagements found. Add one in Engagement Tracker first.".to_string();
        return;
    }
    if !scan::nmap_available() {
        state.status = scan::NMAP_MISSING_MESSAGE.to_string();
        return;
    }
    let raw_target = state.target.value.trim().to_string();
    if raw_target.is_empty() {
        state.status = "Target is required.".to_string();
        return;
    }
    let target = scan::strip_target(&raw_target);
    let engagement_id = state.engagement_labels[state.engagement_idx].0;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    match scan_requests::add_scan_request(state.user_id, engagement_id, &target, &now) {
        Ok(id) => {
            state.status = "Scan running...".to_string();
            state.log.push(format!("Running nmap against {target}..."));
            state.mode = Mode::List;
            state.refresh();
            fire_scan(state, id, target);
        }
        Err(e) => state.status = format!("Error logging scan request: {e}"),
    }
}

fn load_selected_notes(state: &mut ScanRequestState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    state.notes = widgets::TextField::with_value(entry.notes.clone());
    state.mode = Mode::EditNotes(entry.id);
    state.status.clear();
}

fn save_notes(state: &mut ScanRequestState) {
    if let Mode::EditNotes(id) = state.mode {
        match scan_requests::update_notes(state.user_id, id, state.notes.value.trim()) {
            Ok(()) => {
                state.status = "Notes saved.".to_string();
                state.mode = Mode::List;
                state.refresh();
            }
            Err(e) => state.status = format!("Error saving notes: {e}"),
        }
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &ScanRequestState) {
    let rect = widgets::centered_fixed(100, area.height.min(30), area);
    let block = widgets::form_block("Scan Request");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::New => draw_new(frame, inner, state),
        Mode::EditNotes(_) => draw_notes(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &ScanRequestState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(8), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|e| {
            Row::new(vec![
                state.engagement_label(e.engagement_id),
                e.target.clone(),
                e.submitted_at.clone(),
                if e.notes.is_empty() { "-".to_string() } else { e.notes.clone() },
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Length(24), Constraint::Length(22), Constraint::Length(20), Constraint::Min(20)],
    )
    .header(Row::new(vec!["Engagement", "Target", "Submitted", "Notes"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.log.lines().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let log_widget =
        Paragraph::new(all[start..].to_vec()).block(widgets::log_block("Status")).wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[1]);

    let mut lines = vec![Line::styled("n: new request  Enter/e: edit notes  d: delete  Esc: back", theme::hint_style())];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[2]);
}

fn draw_new(frame: &mut Frame, area: Rect, state: &ScanRequestState) {
    let engagement_display = state
        .engagement_labels
        .get(state.engagement_idx)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| "(none)".to_string());
    let mut lines = vec![
        Line::styled("New Scan Request", theme::title_style()),
        Line::raw(""),
        field_line("Engagement", engagement_display, state.focus == 0),
        field_line("Target (domain or IP)", state.target.display(), state.focus == 1),
        Line::raw(""),
        Line::styled("Tab: move  Left/Right: change engagement  Enter: submit  Esc: cancel", theme::hint_style()),
        Line::styled(
            "Runs nmap locally against this one target and shows the result here when it finishes.",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_notes(frame: &mut Frame, area: Rect, state: &ScanRequestState) {
    let mut lines = vec![
        Line::styled("Edit Notes", theme::title_style()),
        Line::raw(""),
        field_line("Notes", state.notes.display(), true),
        Line::raw(""),
        Line::styled("Enter: save  Esc: cancel", theme::hint_style()),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn handle_list_key(state: &mut ScanRequestState, key: KeyEvent) -> Action {
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
        KeyCode::Char('n') => {
            state.refresh();
            if state.engagement_labels.is_empty() {
                state.status = "No engagements found. Add one in Engagement Tracker first.".to_string();
                return Action::None;
            }
            state.target = widgets::TextField::new();
            state.engagement_idx = 0;
            state.focus = 0;
            state.mode = Mode::New;
            state.status.clear();
            Action::None
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            load_selected_notes(state);
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(entry) = state.entries.get(state.selected) {
                let id = entry.id;
                match scan_requests::delete_scan_request(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Scan request record deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting scan request: {e}"),
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_new_key(state: &mut ScanRequestState, key: KeyEvent) -> Action {
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
        KeyCode::Left if state.focus == 0 => {
            if !state.engagement_labels.is_empty() {
                state.engagement_idx =
                    (state.engagement_idx + state.engagement_labels.len() - 1) % state.engagement_labels.len();
            }
            Action::None
        }
        KeyCode::Right if state.focus == 0 => {
            if !state.engagement_labels.is_empty() {
                state.engagement_idx = (state.engagement_idx + 1) % state.engagement_labels.len();
            }
            Action::None
        }
        KeyCode::Enter => {
            submit_request(state);
            Action::None
        }
        KeyCode::Backspace => {
            if state.focus == 1 {
                state.target.backspace();
            }
            Action::None
        }
        KeyCode::Char(c) => {
            if state.focus == 1 {
                state.target.push_char(c);
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_notes_key(state: &mut ScanRequestState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            save_notes(state);
            Action::None
        }
        KeyCode::Backspace => {
            state.notes.backspace();
            Action::None
        }
        KeyCode::Char(c) => {
            state.notes.push_char(c);
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut ScanRequestState, key: KeyEvent) -> Action {
    match state.mode {
        Mode::List => handle_list_key(state, key),
        Mode::New => handle_new_key(state, key),
        Mode::EditNotes(_) => handle_notes_key(state, key),
    }
}
