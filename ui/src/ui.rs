use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::Text,
    widgets::{Block, Borders, Paragraph},
};

use crate::{App, CurrentScreen};

pub fn ui(frame: &mut Frame, app: &mut App) {
    if app.current_screen == CurrentScreen::Chat {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(80), Constraint::Min(1)])
            .split(frame.area());

        let chat_block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default());

        let chat_prefix = Paragraph::new(Text::from("Prompt here".to_string())).block(chat_block);

        frame.render_widget(chat_prefix, chunks[1]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());

        let title_block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default());

        let title = Paragraph::new(Text::from("DAG viewer".to_string())).block(title_block);

        frame.render_widget(title, chunks[0]);
    }
}
