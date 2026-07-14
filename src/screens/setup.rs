use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::{settings, users};
use crate::ui::{theme, widgets};

pub const TIMEZONES: &[&str] = &[
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "Europe/London",
    "Europe/Paris",
    "Europe/Berlin",
    "Asia/Tokyo",
    "Asia/Shanghai",
    "Australia/Sydney",
    "UTC",
];

pub const DATE_FORMATS: &[(&str, &str)] = &[
    ("YYYY-MM-DD", "%Y-%m-%d"),
    ("MM/DD/YYYY", "%m/%d/%Y"),
    ("DD/MM/YYYY", "%d/%m/%Y"),
    ("Mon DD, YYYY", "%b %d, %Y"),
];

const FIELD_COUNT: usize = 9;

pub struct SetupState {
    pub username: widgets::TextField,
    pub password: widgets::TextField,
    pub password2: widgets::TextField,
    pub display_name: widgets::TextField,
    pub timezone_idx: usize,
    pub region: widgets::TextField,
    pub language: widgets::TextField,
    pub time_format_idx: usize,
    pub date_format_idx: usize,
    pub focus: usize,
    pub error: String,
}

impl Default for SetupState {
    fn default() -> Self {
        let s = settings::load_settings();
        let timezone_idx = TIMEZONES.iter().position(|&t| t == s.timezone).unwrap_or(1);
        let date_format_idx =
            DATE_FORMATS.iter().position(|(_, f)| *f == s.date_format).unwrap_or(0);
        let time_format_idx = if s.use_24h { 0 } else { 1 };
        SetupState {
            username: widgets::TextField::new(),
            password: widgets::TextField::masked_field(),
            password2: widgets::TextField::masked_field(),
            display_name: widgets::TextField::new(),
            timezone_idx,
            region: widgets::TextField::with_value(s.region),
            language: widgets::TextField::with_value(s.language),
            time_format_idx,
            date_format_idx,
            focus: 0,
            error: String::new(),
        }
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &SetupState) {
    let rect = widgets::centered_fixed(60, area.height.min(24), area);
    let block = widgets::form_block("");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines = vec![
        Line::styled("Tsukuyomi OS Setup", theme::title_style()),
        Line::styled("Welcome. Create your local account to continue.", theme::subtitle_style()),
        Line::raw(""),
        field_line("Username", state.username.display(), state.focus == 0),
        field_line("Password", state.password.display(), state.focus == 1),
        field_line("Confirm Password", state.password2.display(), state.focus == 2),
        field_line("Display Name", state.display_name.display(), state.focus == 3),
        field_line("Timezone", TIMEZONES[state.timezone_idx].to_string(), state.focus == 4),
        field_line("Region", state.region.display(), state.focus == 5),
        field_line("Language", state.language.display(), state.focus == 6),
        field_line(
            "Time Format",
            if state.time_format_idx == 0 { "24-hour".to_string() } else { "12-hour".to_string() },
            state.focus == 7,
        ),
        field_line(
            "Date Format",
            DATE_FORMATS[state.date_format_idx].0.to_string(),
            state.focus == 8,
        ),
        Line::raw(""),
        Line::styled(
            "Tab/Shift+Tab: move  Left/Right: change  Enter: create account  Esc: quit",
            theme::hint_style(),
        ),
    ];
    if !state.error.is_empty() {
        lines.push(Line::styled(state.error.clone(), theme::error_style()));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn try_submit(state: &mut SetupState) -> Action {
    if state.username.value.trim().is_empty() || state.password.value.is_empty() {
        state.error = "Username and password are required.".to_string();
        return Action::None;
    }
    if state.password.value != state.password2.value {
        state.error = "Passwords do not match.".to_string();
        return Action::None;
    }
    if state.password.value.len() < 6 {
        state.error = "Password must be at least 6 characters.".to_string();
        return Action::None;
    }

    let username = state.username.value.trim().to_string();
    let display = if state.display_name.value.trim().is_empty() {
        username.clone()
    } else {
        state.display_name.value.trim().to_string()
    };

    match users::create_user(&username, &state.password.value, &display, "admin") {
        Ok(true) => {}
        Ok(false) => {
            state.error = "Username already exists.".to_string();
            return Action::None;
        }
        Err(e) => {
            state.error = format!("Error creating account: {e}");
            return Action::None;
        }
    }

    let mut s = settings::load_settings();
    s.timezone = TIMEZONES[state.timezone_idx].to_string();
    s.region = state.region.value.clone();
    s.language = state.language.value.clone();
    s.use_24h = state.time_format_idx == 0;
    s.time_format = if s.use_24h { "%H:%M:%S".to_string() } else { "%I:%M:%S %p".to_string() };
    s.date_format = DATE_FORMATS[state.date_format_idx].1.to_string();
    s.onboarded = true;
    if let Err(e) = settings::save_settings(&s) {
        state.error = format!("Error saving settings: {e}");
        return Action::None;
    }

    match users::authenticate(&username, &state.password.value) {
        Ok(Some(user)) => Action::LoggedIn(user),
        _ => {
            state.error = "Account created; please log in.".to_string();
            Action::ToLogin
        }
    }
}

pub fn handle_key(state: &mut SetupState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Quit,
        KeyCode::Tab | KeyCode::Down => {
            state.focus = (state.focus + 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.focus = (state.focus + FIELD_COUNT - 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::Left if state.focus == 4 => {
            state.timezone_idx = (state.timezone_idx + TIMEZONES.len() - 1) % TIMEZONES.len();
            Action::None
        }
        KeyCode::Right if state.focus == 4 => {
            state.timezone_idx = (state.timezone_idx + 1) % TIMEZONES.len();
            Action::None
        }
        KeyCode::Left | KeyCode::Right if state.focus == 7 => {
            state.time_format_idx = 1 - state.time_format_idx;
            Action::None
        }
        KeyCode::Left if state.focus == 8 => {
            state.date_format_idx =
                (state.date_format_idx + DATE_FORMATS.len() - 1) % DATE_FORMATS.len();
            Action::None
        }
        KeyCode::Right if state.focus == 8 => {
            state.date_format_idx = (state.date_format_idx + 1) % DATE_FORMATS.len();
            Action::None
        }
        KeyCode::Enter => try_submit(state),
        KeyCode::Backspace => {
            match state.focus {
                0 => state.username.backspace(),
                1 => state.password.backspace(),
                2 => state.password2.backspace(),
                3 => state.display_name.backspace(),
                5 => state.region.backspace(),
                6 => state.language.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.username.push_char(c),
                1 => state.password.push_char(c),
                2 => state.password2.push_char(c),
                3 => state.display_name.push_char(c),
                5 => state.region.push_char(c),
                6 => state.language.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}
