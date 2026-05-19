use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw_band_lock(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.size());

    let title = Paragraph::new("Band Lock")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(title, chunks[0]);

    let list = List::new(
        app.band_lock_state
            .items
            .iter()
            .map(|i| ListItem::new(i.as_str()))
            .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::ALL).title("EARFCN"))
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[1], &mut app.band_lock_state.state);

    let status = app.band_lock_response.clone().unwrap_or_default();

    f.render_widget(
        Paragraph::new(status).block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );
}
