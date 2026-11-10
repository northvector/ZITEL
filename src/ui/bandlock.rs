use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

use crate::{app::App, ui::components::{draw_header, panel}};

pub fn draw_band_lock(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.size());

    draw_header(f, chunks[0], "Band Lock");

    let list = List::new(
        app.band_lock_state
            .items
            .iter()
            .map(|i| ListItem::new(i.as_str()))
            .collect::<Vec<_>>(),
    )
    .block(panel("EARFCN"))
    .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
    .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[1], &mut app.band_lock_state.state);

    let status = app.band_lock_response.clone().unwrap_or_default();

    f.render_widget(
        Paragraph::new(status).block(panel("Status")),
        chunks[2],
    );
}
