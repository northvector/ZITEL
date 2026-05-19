use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, DEFAULT_DMZ_IP};

pub fn draw_dmz(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(f.size());

    let title = Paragraph::new("Set DMZ")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(title, chunks[0]);

    let input = Paragraph::new(format!(
        "DMZ IP (default {}): {}",
        DEFAULT_DMZ_IP, app.dmz_ip_input
    ))
    .block(Block::default().borders(Borders::ALL));

    f.render_widget(input, chunks[1]);

    let status = app.dmz_response.clone().unwrap_or_default();

    f.render_widget(
        Paragraph::new(status).block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );
}
