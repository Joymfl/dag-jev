use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::Text,
    widgets::{Block, Borders, Paragraph},
};

use crate::App;

pub fn ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Min(1)])
        .split(frame.area());

    let chat_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let chat_prefix = Paragraph::new(Text::from("Prompt here".to_string())).block(chat_block);

    frame.render_widget(chat_prefix, chunks[1]);
}
