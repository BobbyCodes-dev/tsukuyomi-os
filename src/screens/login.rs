use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::users;
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 2;

#[derive(Default)]
pub struct LoginState {
    pub username: widgets::TextField,
    pub password: widgets::TextField,
    pub focus: usize,
    pub error: String,
}

impl LoginState {
    pub fn new() -> Self {
        Self { password: widgets::TextField::masked_field(), ..Default::default() }
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &LoginState) {
    let rect = widgets::centered_fixed(50, 12, area);
    let block = widgets::form_block("");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines = vec![
        Line::styled("Tsukuyomi OS", theme::title_style()),
        Line::styled("Terminal-based personal OS shell", theme::subtitle_style()),
        Line::raw(""),
        field_line("Username", state.username.display(), state.focus == 0),
        field_line("Password", state.password.display(), state.focus == 1),
        Line::raw(""),
        Line::styled("Enter: sign in  Ctrl+R: reset setup  Esc: quit", theme::hint_style()),
    ];
    if !state.error.is_empty() {
        lines.push(Line::styled(state.error.clone(), theme::error_style()));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn try_submit(state: &mut LoginState) -> Action {
    match users::authenticate(&state.username.value, &state.password.value) {
        Ok(Some(user)) => Action::LoggedIn(user),
        Ok(None) => {
            state.error = "Invalid username or password.".to_string();
            Action::None
        }
        Err(e) => {
            state.error = format!("Login error: {e}");
            Action::None
        }
    }
}

pub fn handle_key(state: &mut LoginState, key: KeyEvent) -> Action {
    // Ctrl+R replaces Python's bare 'r' keybinding: a plain 'r' is a valid
    // username/password character here, so the reset shortcut needs a modifier
    // to avoid colliding with typing into a focused text field.
    if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::ToSetup;
    }
    match key.code {
        KeyCode::Esc => Action::Quit,
        KeyCode::Tab | KeyCode::Down | KeyCode::BackTab | KeyCode::Up => {
            state.focus = (state.focus + 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::Enter => try_submit(state),
        KeyCode::Backspace => {
            match state.focus {
                0 => state.username.backspace(),
                1 => state.password.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.username.push_char(c),
                1 => state.password.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}
