use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::engagements::{self, Engagement};
use crate::ui::{theme, widgets};

pub const ENGAGEMENT_TYPES: &[&str] =
    &["WiFi Audit", "Network Scan", "Physical Security", "Web Assessment", "Other"];

pub const STATUSES: &[&str] = &["Scheduled", "Active", "Completed", "Cancelled"];

const FIELD_COUNT: usize = 8;

enum Mode {
    List,
    Edit(Option<i64>),
}

pub struct EngagementsState {
    user_id: i64,
    entries: Vec<Engagement>,
    selected: usize,
    mode: Mode,
    client_name: widgets::TextField,
    engagement_type_idx: usize,
    scope: widgets::TextField,
    start_date: widgets::TextField,
    end_date: widgets::TextField,
    status_idx: usize,
    invoice_ref: widgets::TextField,
    notes: widgets::TextField,
    focus: usize,
    status: String,
}

impl EngagementsState {
    pub fn new(user_id: i64) -> Self {
        let mut state = EngagementsState {
            user_id,
            entries: Vec::new(),
            selected: 0,
            mode: Mode::List,
            client_name: widgets::TextField::new(),
            engagement_type_idx: 0,
            scope: widgets::TextField::new(),
            start_date: widgets::TextField::new(),
            end_date: widgets::TextField::new(),
            status_idx: 0,
            invoice_ref: widgets::TextField::new(),
            notes: widgets::TextField::new(),
            focus: 0,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match engagements::list_engagements(self.user_id) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading engagements: {e}"),
        }
    }

    fn clear_form(&mut self) {
        self.client_name = widgets::TextField::new();
        self.engagement_type_idx = 0;
        self.scope = widgets::TextField::new();
        self.start_date = widgets::TextField::new();
        self.end_date = widgets::TextField::new();
        self.status_idx = 0;
        self.invoice_ref = widgets::TextField::new();
        self.notes = widgets::TextField::new();
        self.focus = 0;
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &EngagementsState) {
    let rect = widgets::centered_fixed(100, area.height.min(28), area);
    let block = widgets::form_block("Engagement Tracker");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &EngagementsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|e| {
            Row::new(vec![
                e.client_name.clone(),
                e.engagement_type.clone(),
                e.status.clone(),
                e.start_date.clone(),
                e.end_date.clone(),
                e.invoice_ref.clone(),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(11),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Min(14),
        ],
    )
    .header(
        Row::new(vec!["Client", "Type", "Status", "Start", "End", "Invoice Ref"])
            .style(theme::title_style()),
    )
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "a: add  Enter/e: edit  d: delete  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &EngagementsState) {
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit Engagement" } else { "New Engagement" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Client", state.client_name.display(), state.focus == 0),
        field_line("Type", ENGAGEMENT_TYPES[state.engagement_type_idx].to_string(), state.focus == 1),
        field_line("Authorized Scope", state.scope.display(), state.focus == 2),
        field_line("Start Date", state.start_date.display(), state.focus == 3),
        field_line("End Date", state.end_date.display(), state.focus == 4),
        field_line("Status", STATUSES[state.status_idx].to_string(), state.focus == 5),
        field_line("Invoice Ref", state.invoice_ref.display(), state.focus == 6),
        field_line("Notes", state.notes.display(), state.focus == 7),
        Line::raw(""),
        Line::styled(
            "Tab: move  Left/Right: change Type/Status  Enter: save  Esc: cancel",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn save_entry(state: &mut EngagementsState) {
    if state.client_name.value.trim().is_empty() {
        state.status = "Client name is required.".to_string();
        return;
    }
    let engagement_type = ENGAGEMENT_TYPES[state.engagement_type_idx];
    let status_value = STATUSES[state.status_idx];
    let result = match state.mode {
        Mode::Edit(Some(id)) => engagements::update_engagement(
            state.user_id,
            id,
            state.client_name.value.trim(),
            engagement_type,
            state.scope.value.trim(),
            state.start_date.value.trim(),
            state.end_date.value.trim(),
            status_value,
            state.invoice_ref.value.trim(),
            state.notes.value.trim(),
        ),
        _ => engagements::add_engagement(
            state.user_id,
            state.client_name.value.trim(),
            engagement_type,
            state.scope.value.trim(),
            state.start_date.value.trim(),
            state.end_date.value.trim(),
            status_value,
            state.invoice_ref.value.trim(),
            state.notes.value.trim(),
        ),
    };
    match result {
        Ok(()) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving engagement: {e}"),
    }
}

fn load_selected_into_form(state: &mut EngagementsState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    state.client_name = widgets::TextField::with_value(entry.client_name.clone());
    state.engagement_type_idx = ENGAGEMENT_TYPES
        .iter()
        .position(|&t| t == entry.engagement_type)
        .unwrap_or(0);
    state.scope = widgets::TextField::with_value(entry.scope.clone());
    state.start_date = widgets::TextField::with_value(entry.start_date.clone());
    state.end_date = widgets::TextField::with_value(entry.end_date.clone());
    state.status_idx = STATUSES.iter().position(|&s| s == entry.status).unwrap_or(0);
    state.invoice_ref = widgets::TextField::with_value(entry.invoice_ref.clone());
    state.notes = widgets::TextField::with_value(entry.notes.clone());
    state.focus = 0;
    state.mode = Mode::Edit(Some(entry.id));
    state.status.clear();
}

fn handle_list_key(state: &mut EngagementsState, key: KeyEvent) -> Action {
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
                match engagements::delete_engagement(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Engagement deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting engagement: {e}"),
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut EngagementsState, key: KeyEvent) -> Action {
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
        KeyCode::Left if state.focus == 1 => {
            state.engagement_type_idx =
                (state.engagement_type_idx + ENGAGEMENT_TYPES.len() - 1) % ENGAGEMENT_TYPES.len();
            Action::None
        }
        KeyCode::Right if state.focus == 1 => {
            state.engagement_type_idx = (state.engagement_type_idx + 1) % ENGAGEMENT_TYPES.len();
            Action::None
        }
        KeyCode::Left if state.focus == 5 => {
            state.status_idx = (state.status_idx + STATUSES.len() - 1) % STATUSES.len();
            Action::None
        }
        KeyCode::Right if state.focus == 5 => {
            state.status_idx = (state.status_idx + 1) % STATUSES.len();
            Action::None
        }
        KeyCode::Enter => {
            save_entry(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.client_name.backspace(),
                2 => state.scope.backspace(),
                3 => state.start_date.backspace(),
                4 => state.end_date.backspace(),
                6 => state.invoice_ref.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.client_name.push_char(c),
                2 => state.scope.push_char(c),
                3 => state.start_date.push_char(c),
                4 => state.end_date.push_char(c),
                6 => state.invoice_ref.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut EngagementsState, key: KeyEvent) -> Action {
    if matches!(state.mode, Mode::Edit(_)) {
        handle_edit_key(state, key)
    } else {
        handle_list_key(state, key)
    }
}
