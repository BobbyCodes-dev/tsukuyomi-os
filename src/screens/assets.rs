use std::collections::HashMap;
use std::process::Command;
use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::assets::{self, Asset};
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 5;

enum Mode {
    List,
    Edit(Option<i64>),
}

pub struct AssetsState {
    user_id: i64,
    entries: Vec<Asset>,
    selected: usize,
    mode: Mode,
    name: widgets::TextField,
    host: widgets::TextField,
    os: widgets::TextField,
    tags: widgets::TextField,
    notes: widgets::TextField,
    focus: usize,
    status: String,
    ping_results: HashMap<i64, String>,
    ping_rx: Option<Receiver<(i64, bool)>>,
}

impl AssetsState {
    pub fn new(user_id: i64) -> Self {
        let mut state = AssetsState {
            user_id,
            entries: Vec::new(),
            selected: 0,
            mode: Mode::List,
            name: widgets::TextField::new(),
            host: widgets::TextField::new(),
            os: widgets::TextField::new(),
            tags: widgets::TextField::new(),
            notes: widgets::TextField::new(),
            focus: 0,
            status: String::new(),
            ping_results: HashMap::new(),
            ping_rx: None,
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match assets::list_assets(self.user_id) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading assets: {e}"),
        }
    }

    fn clear_form(&mut self) {
        self.name = widgets::TextField::new();
        self.host = widgets::TextField::new();
        self.os = widgets::TextField::new();
        self.tags = widgets::TextField::new();
        self.notes = widgets::TextField::new();
        self.focus = 0;
    }

    pub fn poll_ping(&mut self) {
        let Some(rx) = &self.ping_rx else { return };
        match rx.try_recv() {
            Ok((id, reachable)) => {
                let label = if reachable { "Reachable" } else { "Unreachable" };
                self.ping_results.insert(id, label.to_string());
                self.ping_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.ping_rx = None,
        }
    }
}

fn ping_host(host: &str) -> bool {
    Command::new("ping")
        .args(["-n", "1", host])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &AssetsState) {
    let rect = widgets::centered_fixed(94, area.height.min(26), area);
    let block = widgets::form_block("Asset Inventory");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &AssetsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|e| {
            let status = state.ping_results.get(&e.id).cloned().unwrap_or_else(|| "-".to_string());
            Row::new(vec![
                e.name.clone(),
                e.host.clone(),
                e.os.clone(),
                e.tags.clone(),
                e.notes.clone(),
                status,
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(16),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Min(16),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec!["Name", "Host/IP", "OS", "Tags", "Notes", "Status"]).style(theme::title_style()),
    )
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "a: add  Enter/e: edit  d: delete  p: ping  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &AssetsState) {
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit Asset" } else { "New Asset" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Name", state.name.display(), state.focus == 0),
        field_line("Host/IP", state.host.display(), state.focus == 1),
        field_line("OS", state.os.display(), state.focus == 2),
        field_line("Tags", state.tags.display(), state.focus == 3),
        field_line("Notes", state.notes.display(), state.focus == 4),
        Line::raw(""),
        Line::styled("Tab: move  Enter: save  Esc: cancel", theme::hint_style()),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn save_entry(state: &mut AssetsState) {
    if state.name.value.trim().is_empty() || state.host.value.trim().is_empty() {
        state.status = "Name and host are required.".to_string();
        return;
    }
    let result = match state.mode {
        Mode::Edit(Some(id)) => assets::update_asset(
            state.user_id,
            id,
            state.name.value.trim(),
            state.host.value.trim(),
            state.os.value.trim(),
            state.tags.value.trim(),
            &state.notes.value,
        ),
        _ => assets::add_asset(
            state.user_id,
            state.name.value.trim(),
            state.host.value.trim(),
            state.os.value.trim(),
            state.tags.value.trim(),
            &state.notes.value,
        ),
    };
    match result {
        Ok(()) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving asset: {e}"),
    }
}

fn load_selected_into_form(state: &mut AssetsState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    state.name = widgets::TextField::with_value(entry.name.clone());
    state.host = widgets::TextField::with_value(entry.host.clone());
    state.os = widgets::TextField::with_value(entry.os.clone());
    state.tags = widgets::TextField::with_value(entry.tags.clone());
    state.notes = widgets::TextField::with_value(entry.notes.clone());
    state.focus = 0;
    state.mode = Mode::Edit(Some(entry.id));
    state.status.clear();
}

fn ping_selected(state: &mut AssetsState) {
    if state.ping_rx.is_some() {
        state.status = "A ping check is already in progress.".to_string();
        return;
    }
    let Some(entry) = state.entries.get(state.selected) else { return };
    let id = entry.id;
    let host = entry.host.clone();
    state.ping_results.insert(id, "Pinging...".to_string());
    let (tx, rx) = mpsc::channel();
    state.ping_rx = Some(rx);
    std::thread::spawn(move || {
        let reachable = ping_host(&host);
        let _ = tx.send((id, reachable));
    });
}

fn handle_list_key(state: &mut AssetsState, key: KeyEvent) -> Action {
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
                match assets::delete_asset(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Asset deleted.".to_string();
                        state.ping_results.remove(&id);
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting asset: {e}"),
                }
            }
            Action::None
        }
        KeyCode::Char('p') => {
            ping_selected(state);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut AssetsState, key: KeyEvent) -> Action {
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
            save_entry(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.name.backspace(),
                1 => state.host.backspace(),
                2 => state.os.backspace(),
                3 => state.tags.backspace(),
                4 => state.notes.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.name.push_char(c),
                1 => state.host.push_char(c),
                2 => state.os.push_char(c),
                3 => state.tags.push_char(c),
                4 => state.notes.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut AssetsState, key: KeyEvent) -> Action {
    if matches!(state.mode, Mode::Edit(_)) {
        handle_edit_key(state, key)
    } else {
        handle_list_key(state, key)
    }
}
