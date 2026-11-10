use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::{app::App, ui::components::{add_line, draw_header, panel}};

pub fn draw_neighbor_cells(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(f.size());

    draw_header(f, chunks[0], "Neighbor Cells");

    let mut lines = vec![];

    if app.neighbour_fetching {
        lines.push(Line::from("Searching neighbour cells..."));
    } else if !app.neighbour_fetched {
        lines.push(Line::from("Press Enter to search neighbour cells"));
    } else {
        let data = &app.neighbour_data;

        let count = data["lenghtt"]
            .as_str()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        lines.push(Line::from(format!("Found {} neighbour cell(s)", count)));

        for i in 1..=count {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(format!("Cell {}", i), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));

            add_line(&mut lines, "MCC", data, &format!("type{}", i));
            add_line(&mut lines, "MNC", data, &format!("band{}", i));
            add_line(&mut lines, "Band", data, &format!("pcid{}", i));
            add_line(&mut lines, "ARFCN", data, &format!("rsrq{}", i));
            add_line(&mut lines, "PCI", data, &format!("rsrp{}", i));
            add_line(&mut lines, "Signal", data, &format!("rsrppp{}", i));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(
            panel("Results")
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, chunks[1]);
}
