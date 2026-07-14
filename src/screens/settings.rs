use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::settings;
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 4;

pub struct SettingsState {
    pub theme: widgets::TextField,
    pub timezone: widgets::TextField,
    pub language: widgets::TextField,
    pub notifications: widgets::TextField,
    pub focus: usize,
    pub status: String,
}

impl Default for SettingsState {
    fn default() -> Self {
        let s = settings::load_settings();
        SettingsState {
            theme: widgets::TextField::with_value(s.theme),
            timezone: widgets::TextField::with_value(s.timezone),
            language: widgets::TextField::with_value(s.language),
            notifications: widgets::TextField::with_value(if s.notifications { "true" } else { "false" }),
            focus: 0,
            status: String::new(),
        }
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let rect = widgets::centered_fixed(80, 16, area);
    let block = widgets::form_block("Settings");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines = vec![
        field_line("Theme", state.theme.display(), state.focus == 0),
        field_line("Timezone", state.timezone.display(), state.focus == 1),
        field_line("Language", state.language.display(), state.focus == 2),
        field_line("Notifications", state.notifications.display(), state.focus == 3),
        Line::raw(""),
        Line::styled("Tab: move  Enter: save  Esc: back", theme::hint_style()),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn save(state: &mut SettingsState) {
    let mut s = settings::load_settings();
    s.theme = state.theme.value.clone();
    s.timezone = state.timezone.value.clone();
    s.language = state.language.value.clone();
    s.notifications = matches!(state.notifications.value.to_lowercase().as_str(), "true" | "1" | "yes" | "on");
    match settings::save_settings(&s) {
        Ok(()) => state.status = "Settings saved locally.".to_string(),
        Err(e) => state.status = format!("Error saving settings: {e}"),
    }
}

pub fn handle_key(state: &mut SettingsState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Tab | KeyCode::Down => {
            state.focus = (state.focus + 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.focus = (state.focus + FIELD_COUNT - 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::Enter => {
            save(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.theme.backspace(),
                1 => state.timezone.backspace(),
                2 => state.language.backspace(),
                3 => state.notifications.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.theme.push_char(c),
                1 => state.timezone.push_char(c),
                2 => state.language.push_char(c),
                3 => state.notifications.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}
