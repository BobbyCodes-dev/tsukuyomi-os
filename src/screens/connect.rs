use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::launch_external;
use crate::store::connections::{self, Protocol, SavedConnection};
use crate::store::vault::{self, VaultKey};
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 6;
const PROTOCOLS: [Protocol; 2] = [Protocol::Ssh, Protocol::Rdp];

enum Mode {
    List,
    Edit(Option<i64>),
}

pub struct ConnectState {
    user_id: i64,
    vault_key: Option<VaultKey>,
    entries: Vec<SavedConnection>,
    vault_names: Vec<(i64, String)>,
    selected: usize,
    mode: Mode,
    name: widgets::TextField,
    host: widgets::TextField,
    port: widgets::TextField,
    protocol_idx: usize,
    username: widgets::TextField,
    vault_idx: usize,
    focus: usize,
    status: String,
}

impl ConnectState {
    pub fn new(user_id: i64, vault_key: Option<VaultKey>) -> Self {
        let mut state = ConnectState {
            user_id,
            vault_key,
            entries: Vec::new(),
            vault_names: Vec::new(),
            selected: 0,
            mode: Mode::List,
            name: widgets::TextField::new(),
            host: widgets::TextField::new(),
            port: widgets::TextField::with_value(PROTOCOLS[0].default_port().to_string()),
            protocol_idx: 0,
            username: widgets::TextField::new(),
            vault_idx: 0,
            focus: 0,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match connections::list_connections(self.user_id) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading connections: {e}"),
        }
        match vault::list_entry_names(self.user_id) {
            Ok(names) => self.vault_names = names,
            Err(e) => self.status = format!("Error loading vault entries: {e}"),
        }
    }

    fn clear_form(&mut self) {
        self.name = widgets::TextField::new();
        self.host = widgets::TextField::new();
        self.port = widgets::TextField::with_value(PROTOCOLS[0].default_port().to_string());
        self.protocol_idx = 0;
        self.username = widgets::TextField::new();
        self.vault_idx = 0;
        self.focus = 0;
    }

    fn vault_label(&self) -> String {
        if self.vault_idx == 0 {
            "(none)".to_string()
        } else {
            self.vault_names
                .get(self.vault_idx - 1)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| "(none)".to_string())
        }
    }

    fn vault_entry_id(&self) -> Option<i64> {
        if self.vault_idx == 0 {
            None
        } else {
            self.vault_names.get(self.vault_idx - 1).map(|(id, _)| *id)
        }
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &ConnectState) {
    let rect = widgets::centered_fixed(90, area.height.min(26), area);
    let block = widgets::form_block("Quick Connect");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &ConnectState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|c| {
            Row::new(vec![
                c.name.clone(),
                c.protocol.as_str().to_uppercase(),
                c.host.clone(),
                c.port.to_string(),
                c.username.clone(),
                if c.vault_entry_id.is_some() { "Yes".to_string() } else { "No".to_string() },
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(6),
            Constraint::Length(20),
            Constraint::Length(6),
            Constraint::Length(14),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Name", "Proto", "Host", "Port", "Username", "Vault"]).style(theme::title_style()),
    )
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "Enter: connect  a: add  e: edit  d: delete  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &ConnectState) {
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit Connection" } else { "New Connection" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Name", state.name.display(), state.focus == 0),
        field_line("Host", state.host.display(), state.focus == 1),
        field_line("Port", state.port.display(), state.focus == 2),
        field_line(
            "Protocol",
            PROTOCOLS[state.protocol_idx].as_str().to_uppercase(),
            state.focus == 3,
        ),
        field_line("Username", state.username.display(), state.focus == 4),
        field_line("Vault Credential", state.vault_label(), state.focus == 5),
        Line::raw(""),
        Line::styled(
            "Tab: move  Left/Right: change  Enter: save  Esc: cancel",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn save_entry(state: &mut ConnectState) {
    if state.name.value.trim().is_empty() || state.host.value.trim().is_empty() {
        state.status = "Name and host are required.".to_string();
        return;
    }
    let protocol = PROTOCOLS[state.protocol_idx];
    let port: u16 = state.port.value.trim().parse().unwrap_or_else(|_| protocol.default_port());
    let vault_entry_id = state.vault_entry_id();
    let result = match state.mode {
        Mode::Edit(Some(id)) => connections::update_connection(
            state.user_id,
            id,
            state.name.value.trim(),
            state.host.value.trim(),
            port,
            protocol,
            &state.username.value,
            vault_entry_id,
        ),
        _ => connections::add_connection(
            state.user_id,
            state.name.value.trim(),
            state.host.value.trim(),
            port,
            protocol,
            &state.username.value,
            vault_entry_id,
        ),
    };
    match result {
        Ok(()) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving connection: {e}"),
    }
}

fn connect_selected(state: &mut ConnectState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    match entry.protocol {
        Protocol::Ssh => {
            launch_external::open_ssh(&entry.host, entry.port, &entry.username);
            state.status = format!("Opening SSH session to {}...", entry.host);
        }
        Protocol::Rdp => {
            let password = match (entry.vault_entry_id, state.vault_key) {
                (Some(id), Some(key)) => vault::get_password(state.user_id, &key, id).ok().flatten(),
                _ => None,
            };
            launch_external::open_rdp(&entry.host, entry.port, &entry.username, password.as_deref());
            state.status = format!("Launching RDP session to {}...", entry.host);
        }
    }
}

fn load_selected_into_form(state: &mut ConnectState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    state.name = widgets::TextField::with_value(entry.name.clone());
    state.host = widgets::TextField::with_value(entry.host.clone());
    state.port = widgets::TextField::with_value(entry.port.to_string());
    state.protocol_idx = PROTOCOLS.iter().position(|p| *p == entry.protocol).unwrap_or(0);
    state.username = widgets::TextField::with_value(entry.username.clone());
    state.vault_idx = match entry.vault_entry_id {
        Some(id) => state.vault_names.iter().position(|(vid, _)| *vid == id).map(|i| i + 1).unwrap_or(0),
        None => 0,
    };
    state.focus = 0;
    state.mode = Mode::Edit(Some(entry.id));
    state.status.clear();
}

fn handle_list_key(state: &mut ConnectState, key: KeyEvent) -> Action {
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
        KeyCode::Char('e') => {
            load_selected_into_form(state);
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(entry) = state.entries.get(state.selected) {
                let id = entry.id;
                match connections::delete_connection(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Connection deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting connection: {e}"),
                }
            }
            Action::None
        }
        KeyCode::Enter => {
            connect_selected(state);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut ConnectState, key: KeyEvent) -> Action {
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
            state.protocol_idx = (state.protocol_idx + PROTOCOLS.len() - 1) % PROTOCOLS.len();
            Action::None
        }
        KeyCode::Right if state.focus == 3 => {
            state.protocol_idx = (state.protocol_idx + 1) % PROTOCOLS.len();
            Action::None
        }
        KeyCode::Left if state.focus == 5 => {
            let total = state.vault_names.len() + 1;
            state.vault_idx = (state.vault_idx + total - 1) % total;
            Action::None
        }
        KeyCode::Right if state.focus == 5 => {
            let total = state.vault_names.len() + 1;
            state.vault_idx = (state.vault_idx + 1) % total;
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
                2 => state.port.backspace(),
                4 => state.username.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.name.push_char(c),
                1 => state.host.push_char(c),
                2 if c.is_ascii_digit() => state.port.push_char(c),
                4 => state.username.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut ConnectState, key: KeyEvent) -> Action {
    if matches!(state.mode, Mode::Edit(_)) {
        handle_edit_key(state, key)
    } else {
        handle_list_key(state, key)
    }
}
