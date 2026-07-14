use std::process::Command;
use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::backups::{self, BackupJob};
use crate::ui::{theme, widgets};

pub const FREQUENCIES: &[(&str, &str)] = &[("Manual", "manual"), ("Daily", "daily"), ("Weekly", "weekly")];

const FIELD_COUNT: usize = 4;

enum Mode {
    List,
    Edit(Option<i64>),
}

pub struct BackupsState {
    user_id: i64,
    entries: Vec<BackupJob>,
    selected: usize,
    mode: Mode,
    name: widgets::TextField,
    source: widgets::TextField,
    destination: widgets::TextField,
    frequency_idx: usize,
    focus: usize,
    status: String,
    running_id: Option<i64>,
    run_rx: Option<Receiver<(i64, Result<String, String>)>>,
}

impl BackupsState {
    pub fn new(user_id: i64) -> Self {
        let mut state = BackupsState {
            user_id,
            entries: Vec::new(),
            selected: 0,
            mode: Mode::List,
            name: widgets::TextField::new(),
            source: widgets::TextField::new(),
            destination: widgets::TextField::new(),
            frequency_idx: 0,
            focus: 0,
            status: String::new(),
            running_id: None,
            run_rx: None,
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match backups::list_backups(self.user_id) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading backup jobs: {e}"),
        }
    }

    fn clear_form(&mut self) {
        self.name = widgets::TextField::new();
        self.source = widgets::TextField::new();
        self.destination = widgets::TextField::new();
        self.frequency_idx = 0;
        self.focus = 0;
    }

    pub fn poll_run(&mut self) {
        let Some(rx) = &self.run_rx else { return };
        match rx.try_recv() {
            Ok((id, result)) => {
                let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let status_text = match &result {
                    Ok(msg) => msg.clone(),
                    Err(e) => format!("Failed: {e}"),
                };
                if let Err(e) = backups::record_run(self.user_id, id, &now, &status_text) {
                    self.status = format!("Error recording run result: {e}");
                } else {
                    self.status = match result {
                        Ok(msg) => format!("Backup complete — {msg}"),
                        Err(e) => format!("Backup failed — {e}"),
                    };
                }
                self.running_id = None;
                self.run_rx = None;
                self.refresh();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.running_id = None;
                self.run_rx = None;
            }
        }
    }
}

fn run_robocopy(source: &str, destination: &str) -> Result<String, String> {
    let output = Command::new("robocopy")
        .args([source, destination, "/MIR", "/R:1", "/W:1"])
        .output()
        .map_err(|e| e.to_string())?;
    let code = output.status.code().unwrap_or(-1);
    if code >= 8 || code < 0 {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let tail: String = String::from_utf8_lossy(&output.stdout)
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(if !stderr.is_empty() {
            stderr
        } else if !tail.is_empty() {
            format!("robocopy exit code {code}: {tail}")
        } else {
            format!("robocopy exit code {code}")
        });
    }
    Ok(format!("robocopy exit code {code}"))
}

fn run_selected(state: &mut BackupsState) {
    if state.running_id.is_some() {
        state.status = "A backup is already running.".to_string();
        return;
    }
    let Some(entry) = state.entries.get(state.selected) else { return };
    let id = entry.id;
    let source = entry.source.clone();
    let destination = entry.destination.clone();
    state.status = format!("Running backup for {}...", entry.name);
    state.running_id = Some(id);
    let (tx, rx) = mpsc::channel();
    state.run_rx = Some(rx);
    std::thread::spawn(move || {
        let result = run_robocopy(&source, &destination);
        let _ = tx.send((id, result));
    });
}

fn save_entry(state: &mut BackupsState) {
    if state.name.value.trim().is_empty()
        || state.source.value.trim().is_empty()
        || state.destination.value.trim().is_empty()
    {
        state.status = "Name, source, and destination are required.".to_string();
        return;
    }
    let frequency = FREQUENCIES[state.frequency_idx].1;
    let result = match state.mode {
        Mode::Edit(Some(id)) => backups::update_backup(
            state.user_id,
            id,
            state.name.value.trim(),
            state.source.value.trim(),
            state.destination.value.trim(),
            frequency,
        ),
        _ => backups::add_backup(
            state.user_id,
            state.name.value.trim(),
            state.source.value.trim(),
            state.destination.value.trim(),
            frequency,
        ),
    };
    match result {
        Ok(()) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving backup job: {e}"),
    }
}

fn load_selected_into_form(state: &mut BackupsState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    state.name = widgets::TextField::with_value(entry.name.clone());
    state.source = widgets::TextField::with_value(entry.source.clone());
    state.destination = widgets::TextField::with_value(entry.destination.clone());
    state.frequency_idx = FREQUENCIES.iter().position(|(_, v)| *v == entry.frequency).unwrap_or(0);
    state.focus = 0;
    state.mode = Mode::Edit(Some(entry.id));
    state.status.clear();
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &BackupsState) {
    let rect = widgets::centered_fixed(100, area.height.min(28), area);
    let block = widgets::form_block("Backup Manager");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &BackupsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|e| {
            let freq_label = FREQUENCIES.iter().find(|(_, v)| *v == e.frequency).map(|(l, _)| *l).unwrap_or(&e.frequency);
            let last_run = if e.last_run.is_empty() { "-".to_string() } else { e.last_run.clone() };
            let last_status = if state.running_id == Some(e.id) {
                "Running...".to_string()
            } else if e.last_status.is_empty() {
                "-".to_string()
            } else {
                e.last_status.clone()
            };
            Row::new(vec![
                e.name.clone(),
                e.source.clone(),
                e.destination.clone(),
                freq_label.to_string(),
                last_run,
                last_status,
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Length(9),
            Constraint::Length(20),
            Constraint::Min(16),
        ],
    )
    .header(
        Row::new(vec!["Name", "Source", "Destination", "Freq", "Last Run", "Last Status"])
            .style(theme::title_style()),
    )
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "a: add  Enter/e: edit  d: delete  n: run now  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &BackupsState) {
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit Backup Job" } else { "New Backup Job" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Name", state.name.display(), state.focus == 0),
        field_line("Source Path", state.source.display(), state.focus == 1),
        field_line("Destination Path", state.destination.display(), state.focus == 2),
        field_line("Frequency", FREQUENCIES[state.frequency_idx].0.to_string(), state.focus == 3),
        Line::raw(""),
        Line::styled(
            "Tab: move  Left/Right: change frequency  Enter: save  Esc: cancel",
            theme::hint_style(),
        ),
        Line::styled(
            "Frequency is metadata only — \"run now\" is the only action that executes a backup.",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn handle_list_key(state: &mut BackupsState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            if !state.entries.is_empty() {
                state.selected =
                    if state.selected == 0 { state.entries.len() - 1 } else { state.selected - 1 };
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
            state.clear_form();
            state.mode = Mode::Edit(None);
            state.status.clear();
            Action::None
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            load_selected_into_form(state);
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(entry) = state.entries.get(state.selected) {
                let id = entry.id;
                if state.running_id == Some(id) {
                    state.status = "Cannot delete a job that is currently running.".to_string();
                    return Action::None;
                }
                match backups::delete_backup(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Backup job deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting backup job: {e}"),
                }
            }
            Action::None
        }
        KeyCode::Char('n') => {
            run_selected(state);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut BackupsState, key: KeyEvent) -> Action {
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
        KeyCode::Left if state.focus == 3 => {
            state.frequency_idx = (state.frequency_idx + FREQUENCIES.len() - 1) % FREQUENCIES.len();
            Action::None
        }
        KeyCode::Right if state.focus == 3 => {
            state.frequency_idx = (state.frequency_idx + 1) % FREQUENCIES.len();
            Action::None
        }
        KeyCode::Enter => {
            save_entry(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.name.backspace(),
                1 => state.source.backspace(),
                2 => state.destination.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.name.push_char(c),
                1 => state.source.push_char(c),
                2 => state.destination.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut BackupsState, key: KeyEvent) -> Action {
    if matches!(state.mode, Mode::Edit(_)) {
        handle_edit_key(state, key)
    } else {
        handle_list_key(state, key)
    }
}
