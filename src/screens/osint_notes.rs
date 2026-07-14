use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::engagements;
use crate::store::osint_notes::{self, OsintNote};
use crate::ui::{theme, widgets};

pub const CATEGORIES: &[&str] = &["Personnel", "Infrastructure", "Social Media", "Other"];

const FIELD_COUNT: usize = 5;

enum Mode {
    List,
    Edit(Option<i64>),
}

pub struct OsintNotesState {
    user_id: i64,
    entries: Vec<OsintNote>,
    engagement_labels: Vec<(i64, String)>,
    selected: usize,
    mode: Mode,
    engagement_idx: usize,
    title: widgets::TextField,
    category_idx: usize,
    content: widgets::TextField,
    source_url: widgets::TextField,
    focus: usize,
    status: String,
}

impl OsintNotesState {
    pub fn new(user_id: i64) -> Self {
        let mut state = OsintNotesState {
            user_id,
            entries: Vec::new(),
            engagement_labels: Vec::new(),
            selected: 0,
            mode: Mode::List,
            engagement_idx: 0,
            title: widgets::TextField::new(),
            category_idx: 0,
            content: widgets::TextField::new(),
            source_url: widgets::TextField::new(),
            focus: 0,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match osint_notes::list_notes(self.user_id) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading OSINT notes: {e}"),
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

    fn clear_form(&mut self) {
        self.engagement_idx = 0;
        self.title = widgets::TextField::new();
        self.category_idx = 0;
        self.content = widgets::TextField::new();
        self.source_url = widgets::TextField::new();
        self.focus = 0;
    }

    fn engagement_label(&self, engagement_id: i64) -> String {
        self.engagement_labels
            .iter()
            .find(|(id, _)| *id == engagement_id)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| "(deleted engagement)".to_string())
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

fn content_lines(content: &str, focused: bool) -> Vec<Line<'static>> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    let mut lines = vec![Line::styled(format!("{prefix}Content:"), style)];
    if content.is_empty() {
        lines.push(Line::raw("    (empty)".to_string()));
    } else {
        for line in content.split('\n') {
            lines.push(Line::raw(format!("    {line}")));
        }
    }
    lines
}

pub fn draw(frame: &mut Frame, area: Rect, state: &OsintNotesState) {
    let rect = widgets::centered_fixed(100, area.height.min(30), area);
    let block = widgets::form_block("OSINT Notes");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &OsintNotesState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|e| {
            let preview: String = e.content.chars().take(40).collect();
            let preview = if e.content.chars().count() > 40 { format!("{preview}...") } else { preview };
            Row::new(vec![
                state.engagement_label(e.engagement_id),
                e.title.clone(),
                e.category.clone(),
                preview.replace('\n', " "),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Length(22), Constraint::Length(20), Constraint::Length(14), Constraint::Min(20)],
    )
    .header(Row::new(vec!["Engagement", "Title", "Category", "Content"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled("a: add  Enter/e: edit  d: delete  Esc: back", theme::hint_style())];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &OsintNotesState) {
    let engagement_display = state
        .engagement_labels
        .get(state.engagement_idx)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| "(none)".to_string());
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit OSINT Note" } else { "New OSINT Note" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Engagement", engagement_display, state.focus == 0),
        field_line("Title", state.title.display(), state.focus == 1),
        field_line("Category", CATEGORIES[state.category_idx].to_string(), state.focus == 2),
    ];
    lines.extend(content_lines(&state.content.value, state.focus == 3));
    lines.push(Line::raw(""));
    lines.push(field_line("Source URL", state.source_url.display(), state.focus == 4));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Tab: move  Left/Right: change Engagement/Category  Enter: newline in Content, save elsewhere  Ctrl+S: save  Esc: cancel",
        theme::hint_style(),
    ));
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn save_entry(state: &mut OsintNotesState) {
    if state.engagement_labels.is_empty() {
        state.status = "No engagements found. Add one in Engagement Tracker first.".to_string();
        return;
    }
    if state.title.value.trim().is_empty() {
        state.status = "Title is required.".to_string();
        return;
    }
    let engagement_id = state.engagement_labels[state.engagement_idx].0;
    let category = CATEGORIES[state.category_idx];
    let result = match state.mode {
        Mode::Edit(Some(id)) => osint_notes::update_note(
            state.user_id,
            id,
            engagement_id,
            state.title.value.trim(),
            category,
            &state.content.value,
            state.source_url.value.trim(),
        ),
        _ => osint_notes::add_note(
            state.user_id,
            engagement_id,
            state.title.value.trim(),
            category,
            &state.content.value,
            state.source_url.value.trim(),
        ),
    };
    match result {
        Ok(()) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving OSINT note: {e}"),
    }
}

fn load_selected_into_form(state: &mut OsintNotesState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    state.engagement_idx =
        state.engagement_labels.iter().position(|(id, _)| *id == entry.engagement_id).unwrap_or(0);
    state.title = widgets::TextField::with_value(entry.title.clone());
    state.category_idx = CATEGORIES.iter().position(|&c| c == entry.category).unwrap_or(0);
    state.content = widgets::TextField::with_value(entry.content.clone());
    state.source_url = widgets::TextField::with_value(entry.source_url.clone());
    state.focus = 0;
    state.mode = Mode::Edit(Some(entry.id));
    state.status.clear();
}

fn handle_list_key(state: &mut OsintNotesState, key: KeyEvent) -> Action {
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
            state.refresh();
            if state.engagement_labels.is_empty() {
                state.status = "No engagements found. Add one in Engagement Tracker first.".to_string();
                return Action::None;
            }
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
                match osint_notes::delete_note(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Note deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting note: {e}"),
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut OsintNotesState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        save_entry(state);
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
        KeyCode::Left if state.focus == 2 => {
            state.category_idx = (state.category_idx + CATEGORIES.len() - 1) % CATEGORIES.len();
            Action::None
        }
        KeyCode::Right if state.focus == 2 => {
            state.category_idx = (state.category_idx + 1) % CATEGORIES.len();
            Action::None
        }
        KeyCode::Enter if state.focus == 3 => {
            state.content.push_char('\n');
            Action::None
        }
        KeyCode::Enter => {
            save_entry(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                1 => state.title.backspace(),
                3 => state.content.backspace(),
                4 => state.source_url.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                1 => state.title.push_char(c),
                3 => state.content.push_char(c),
                4 => state.source_url.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut OsintNotesState, key: KeyEvent) -> Action {
    if matches!(state.mode, Mode::Edit(_)) {
        handle_edit_key(state, key)
    } else {
        handle_list_key(state, key)
    }
}
