use ratatui::style::{Color, Modifier, Style};

/// Translated from `tsukuyomi.tcss`'s `$primary` / `$text-muted` / `$success` /
/// `$primary-darken-2` theme variables (Textual's default dark theme palette).
pub const PRIMARY: Color = Color::Cyan;
pub const PRIMARY_DARK: Color = Color::DarkGray;
pub const MUTED: Color = Color::Gray;
pub const SUCCESS: Color = Color::Green;

pub fn title_style() -> Style {
    Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
}

pub fn subtitle_style() -> Style {
    Style::default().fg(MUTED)
}

pub fn hint_style() -> Style {
    Style::default().fg(MUTED)
}

pub fn clock_style() -> Style {
    Style::default().fg(SUCCESS)
}

pub fn form_border_style() -> Style {
    Style::default().fg(PRIMARY)
}

pub fn log_border_style() -> Style {
    Style::default().fg(PRIMARY_DARK)
}

pub fn focused_field_style() -> Style {
    Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
}

pub fn error_style() -> Style {
    Style::default().fg(Color::Red)
}
