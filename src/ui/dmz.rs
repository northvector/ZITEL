use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Paragraph},
    Frame,
};

use crate::{
    app::{App, DEFAULT_DMZ_IP},
    ui::components::{draw_header, panel},
};

pub fn draw_dmz(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(f.size());

    draw_header(f, chunks[0], "Set DMZ");

    let input = Paragraph::new(format!(
        "Default {}  ›  {}▏",
        DEFAULT_DMZ_IP, app.dmz_ip_input
    ))
    .block(panel("DMZ IP"));

    f.render_widget(input, chunks[1]);

    let status = app.dmz_response.clone().unwrap_or_default();

    f.render_widget(
        Paragraph::new(status).block(panel("Status")),
        chunks[2],
    );
}
