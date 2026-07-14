use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::vault::{self, VaultEntry, VaultKey};
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 4;

enum Mode {
    List,
    Edit(Option<i64>),
}

pub struct VaultState {
    user_id: i64,
    key: VaultKey,
    entries: Vec<VaultEntry>,
    selected: usize,
    mode: Mode,
    name: widgets::TextField,
    username: widgets::TextField,
    password: widgets::TextField,
    notes: widgets::TextField,
    focus: usize,
    reveal: bool,
    status: String,
}

impl VaultState {
    pub fn new(user_id: i64, key: VaultKey) -> Self {
        let mut state = VaultState {
            user_id,
            key,
            entries: Vec::new(),
            selected: 0,
            mode: Mode::List,
            name: widgets::TextField::new(),
            username: widgets::TextField::new(),
            password: widgets::TextField::masked_field(),
            notes: widgets::TextField::new(),
            focus: 0,
            reveal: false,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match vault::list_entries(self.user_id, &self.key) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading vault: {e}"),
        }
    }

    fn clear_form(&mut self) {
        self.name = widgets::TextField::new();
        self.username = widgets::TextField::new();
        self.password = widgets::TextField::masked_field();
        self.notes = widgets::TextField::new();
        self.focus = 0;
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &VaultState) {
    let rect = widgets::centered_fixed(90, area.height.min(26), area);
    let block = widgets::form_block("Credential Vault");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &VaultState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|e| {
            let masked = widgets::TextField { value: e.password.clone(), masked: !state.reveal };
            Row::new(vec![e.name.clone(), e.username.clone(), masked.display(), e.notes.clone()])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(20),
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Min(20),
        ],
    )
    .header(Row::new(vec!["Name", "Username", "Password", "Notes"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "a: add  Enter: edit  d: delete  v: toggle reveal  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &VaultState) {
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit Entry" } else { "New Entry" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Name", state.name.display(), state.focus == 0),
        field_line("Username", state.username.display(), state.focus == 1),
        field_line("Password", state.password.display(), state.focus == 2),
        field_line("Notes", state.notes.display(), state.focus == 3),
        Line::raw(""),
        Line::styled(
            "Tab: move  Ctrl+R: reveal password  Enter: save  Esc: cancel",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn save_entry(state: &mut VaultState) {
    if state.name.value.trim().is_empty() {
        state.status = "Name is required.".to_string();
        return;
    }
    let result = match state.mode {
        Mode::Edit(Some(id)) => vault::update_entry(
            state.user_id,
            &state.key,
            id,
            state.name.value.trim(),
            &state.username.value,
            &state.password.value,
            &state.notes.value,
        ),
        _ => vault::add_entry(
            state.user_id,
            &state.key,
            state.name.value.trim(),
            &state.username.value,
            &state.password.value,
            &state.notes.value,
        ),
    };
    match result {
        Ok(()) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving entry: {e}"),
    }
}

fn handle_list_key(state: &mut VaultState, key: KeyEvent) -> Action {
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
            if let Some(entry) = state.entries.get(state.selected) {
                state.name = widgets::TextField::with_value(entry.name.clone());
                state.username = widgets::TextField::with_value(entry.username.clone());
                state.password = widgets::TextField { value: entry.password.clone(), masked: true };
                state.notes = widgets::TextField::with_value(entry.notes.clone());
                state.focus = 0;
                state.mode = Mode::Edit(Some(entry.id));
                state.status.clear();
            }
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(entry) = state.entries.get(state.selected) {
                let id = entry.id;
                match vault::delete_entry(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Entry deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting entry: {e}"),
                }
            }
            Action::None
        }
        KeyCode::Char('v') => {
            state.reveal = !state.reveal;
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut VaultState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
        state.password.masked = !state.password.masked;
        return Action::None;
    }
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
                1 => state.username.backspace(),
                2 => state.password.backspace(),
                3 => state.notes.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.name.push_char(c),
                1 => state.username.push_char(c),
                2 => state.password.push_char(c),
                3 => state.notes.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut VaultState, key: KeyEvent) -> Action {
    if matches!(state.mode, Mode::Edit(_)) {
        handle_edit_key(state, key)
    } else {
        handle_list_key(state, key)
    }
}
