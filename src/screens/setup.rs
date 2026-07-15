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

const FIELD_COUNT: usize = 10;
const AGREE_FIELD: usize = 9;

const TERMS_TEXT: &str = include_str!("../../TERMS.md");
const PRIVACY_TEXT: &str = include_str!("../../PRIVACY.md");

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
    pub agree_terms: bool,
    pub show_legal: bool,
    pub legal_scroll: u16,
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
            agree_terms: false,
            show_legal: false,
            legal_scroll: 0,
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
    if state.show_legal {
        draw_legal(frame, area, state);
        return;
    }

    let rect = widgets::centered_fixed(60, area.height.min(26), area);
    let block = widgets::form_block("");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let agree_style = if state.focus == AGREE_FIELD { theme::focused_field_style() } else { Style::default() };
    let agree_prefix = if state.focus == AGREE_FIELD { "> " } else { "  " };

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
        Line::from(vec![
            Span::styled(
                format!("{agree_prefix}I agree to the Terms of Service & Privacy Policy: "),
                agree_style,
            ),
            Span::raw(if state.agree_terms { "yes" } else { "no" }),
        ]),
        Line::styled(
            "  Press 'v' on that field to read the full text before agreeing.",
            theme::hint_style(),
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

fn draw_legal(frame: &mut Frame, area: Rect, state: &SetupState) {
    let block = widgets::form_block("Terms of Service & Privacy Policy (Esc: back, Up/Down: scroll)");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let full_text = format!("{TERMS_TEXT}\n\n{PRIVACY_TEXT}");
    let lines: Vec<Line> = full_text.lines().map(|l| Line::raw(l.to_string())).collect();
    let para = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((state.legal_scroll, 0));
    frame.render_widget(para, inner);
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

    if !state.agree_terms {
        state.error = "You must agree to the Terms of Service & Privacy Policy to continue.".to_string();
        return Action::None;
    }

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
        Ok(Some(user)) => Action::LoggedIn(user, state.password.value.clone()),
        _ => {
            state.error = "Account created; please log in.".to_string();
            Action::ToLogin
        }
    }
}

pub fn handle_key(state: &mut SetupState, key: KeyEvent) -> Action {
    if state.show_legal {
        match key.code {
            KeyCode::Esc => state.show_legal = false,
            KeyCode::Up => state.legal_scroll = state.legal_scroll.saturating_sub(1),
            KeyCode::Down => state.legal_scroll = state.legal_scroll.saturating_add(1),
            _ => {}
        }
        return Action::None;
    }

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
        KeyCode::Char('v') if state.focus == AGREE_FIELD => {
            state.show_legal = true;
            state.legal_scroll = 0;
            Action::None
        }
        KeyCode::Left | KeyCode::Right if state.focus == AGREE_FIELD => {
            state.agree_terms = !state.agree_terms;
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
