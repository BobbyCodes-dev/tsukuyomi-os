use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::evidence::{self, EvidenceItem};
use crate::store::vault::VaultKey;
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 3;

enum Mode {
    List,
    Edit(Option<i64>),
    View,
}

pub struct EvidenceState {
    user_id: i64,
    key: VaultKey,
    items: Vec<EvidenceItem>,
    selected: usize,
    mode: Mode,
    name: widgets::TextField,
    description: widgets::TextField,
    content: widgets::TextField,
    focus: usize,
    reveal: bool,
    status: String,
}

impl EvidenceState {
    pub fn new(user_id: i64, key: VaultKey) -> Self {
        let mut state = EvidenceState {
            user_id,
            key,
            items: Vec::new(),
            selected: 0,
            mode: Mode::List,
            name: widgets::TextField::new(),
            description: widgets::TextField::new(),
            content: widgets::TextField::new(),
            focus: 0,
            reveal: false,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match evidence::list_evidence(self.user_id, &self.key) {
            Ok(items) => {
                self.items = items;
                if self.selected >= self.items.len() {
                    self.selected = self.items.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading evidence: {e}"),
        }
    }

    fn clear_form(&mut self) {
        self.name = widgets::TextField::new();
        self.description = widgets::TextField::new();
        self.content = widgets::TextField::new();
        self.focus = 0;
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &EvidenceState) {
    let rect = widgets::centered_fixed(90, area.height.min(26), area);
    let block = widgets::form_block("Evidence Vault");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
        Mode::View => draw_view(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &EvidenceState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .items
        .iter()
        .map(|e| Row::new(vec![e.name.clone(), e.description.clone().lines().next().unwrap_or_default().to_string()]))
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Length(28), Constraint::Min(40)],
    )
    .header(Row::new(vec!["Name", "Description"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.items.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "a: add  Enter: view  e: edit  d: delete  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &EvidenceState) {
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit Evidence" } else { "New Evidence" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Name", state.name.display(), state.focus == 0),
        field_line("Description", state.description.display(), state.focus == 1),
        field_line("Content", state.content.display(), state.focus == 2),
        Line::raw(""),
        Line::styled(
            "Tab: move  Enter: save  Esc: cancel",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_view(frame: &mut Frame, area: Rect, state: &EvidenceState) {
    let item = state.items.get(state.selected);
    let mut lines = vec![
        Line::styled("Evidence Entry", theme::title_style()),
        Line::raw(""),
    ];
    if let Some(item) = item {
        lines.push(Line::from(vec![Span::styled("Name: ", theme::title_style()), Span::raw(item.name.clone())]));
        lines.push(Line::from(vec![Span::styled("Description: ", theme::title_style()), Span::raw(item.description.clone())]));
        lines.push(Line::raw(""));
        lines.push(Line::styled("Content:", theme::title_style()));
        for line in item.content.lines() {
            lines.push(Line::raw(line.to_string()));
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("Esc: back  v: toggle reveal  e: edit  d: delete", theme::hint_style()));
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn save_item(state: &mut EvidenceState) {
    if state.name.value.trim().is_empty() {
        state.status = "Name is required.".to_string();
        return;
    }
    let result = match state.mode {
        Mode::Edit(Some(id)) => evidence::update_evidence(
            state.user_id,
            &state.key,
            id,
            state.name.value.trim(),
            &state.description.value,
            &state.content.value,
        )
        .map(|_| ()),
        _ => evidence::add_evidence(
            state.user_id,
            &state.key,
            state.name.value.trim(),
            &state.description.value,
            &state.content.value,
        )
        .map(|_| ()),
    };
    match result {
        Ok(_) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving evidence: {e}"),
    }
}

fn handle_list_key(state: &mut EvidenceState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            if !state.items.is_empty() {
                state.selected = if state.selected == 0 { state.items.len() - 1 } else { state.selected - 1 };
            }
            Action::None
        }
        KeyCode::Down => {
            if !state.items.is_empty() {
                state.selected = (state.selected + 1) % state.items.len();
            }
            Action::None
        }
        KeyCode::Char('a') => {
            state.clear_form();
            state.mode = Mode::Edit(None);
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            if !state.items.is_empty() {
                state.mode = Mode::View;
                state.status.clear();
            }
            Action::None
        }
        KeyCode::Char('e') => {
            if let Some(item) = state.items.get(state.selected) {
                state.name = widgets::TextField::with_value(item.name.clone());
                state.description = widgets::TextField::with_value(item.description.clone());
                state.content = widgets::TextField::with_value(item.content.clone());
                state.focus = 0;
                state.mode = Mode::Edit(Some(item.id));
                state.status.clear();
            }
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(item) = state.items.get(state.selected) {
                let id = item.id;
                match evidence::delete_evidence(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Evidence deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting evidence: {e}"),
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut EvidenceState, key: KeyEvent) -> Action {
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
            save_item(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.name.backspace(),
                1 => state.description.backspace(),
                2 => state.content.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.name.push_char(c),
                1 => state.description.push_char(c),
                2 => state.content.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_view_key(state: &mut EvidenceState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            Action::None
        }
        KeyCode::Char('v') => {
            state.reveal = !state.reveal;
            Action::None
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            if let Some(item) = state.items.get(state.selected) {
                state.name = widgets::TextField::with_value(item.name.clone());
                state.description = widgets::TextField::with_value(item.description.clone());
                state.content = widgets::TextField::with_value(item.content.clone());
                state.focus = 0;
                state.mode = Mode::Edit(Some(item.id));
                state.status.clear();
            }
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(item) = state.items.get(state.selected) {
                let id = item.id;
                match evidence::delete_evidence(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Evidence deleted.".to_string();
                        state.mode = Mode::List;
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting evidence: {e}"),
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut EvidenceState, key: KeyEvent) -> Action {
    match state.mode {
        Mode::List => handle_list_key(state, key),
        Mode::Edit(_) => handle_edit_key(state, key),
        Mode::View => handle_view_key(state, key),
    }
}
