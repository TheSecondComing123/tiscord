use ratatui::style::{Color, Modifier, Style};

pub const BG: Color = Color::Rgb(30, 31, 34);
pub const BG_SECONDARY: Color = Color::Rgb(43, 45, 49);
pub const BG_TERTIARY: Color = Color::Rgb(35, 36, 40);
pub const TEXT_PRIMARY: Color = Color::Rgb(219, 222, 225);
pub const TEXT_SECONDARY: Color = Color::Rgb(148, 155, 164);
pub const TEXT_MUTED: Color = Color::Rgb(94, 103, 114);
pub const ACCENT: Color = Color::Rgb(88, 101, 242);
pub const ONLINE: Color = Color::Rgb(35, 165, 89);
pub const IDLE: Color = Color::Rgb(240, 178, 50);
pub const DND: Color = Color::Rgb(237, 66, 69);
pub const MENTION: Color = Color::Rgb(250, 168, 26);
pub const LINK: Color = Color::Rgb(0, 168, 252);
pub const BORDER: Color = Color::Rgb(63, 66, 72);

pub fn base() -> Style {
    Style::default().fg(TEXT_PRIMARY).bg(BG)
}
pub fn secondary_text() -> Style {
    Style::default().fg(TEXT_SECONDARY)
}
pub fn muted() -> Style {
    Style::default().fg(TEXT_MUTED)
}
pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}
pub fn selected() -> Style {
    Style::default().bg(BG_SECONDARY).fg(TEXT_PRIMARY)
}
pub fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}
