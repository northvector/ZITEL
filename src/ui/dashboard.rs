use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Sparkline},
    Frame,
};

use crate::{
    app::App,
    ui::components::{
        build_cell_text, build_connection_text, build_data_usage_text, build_system_text,
    },
};

pub fn draw_dashboard(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(f.size());

    let title = Paragraph::new("Zitel Router Manager | Dashboard")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(title, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Percentage(35),
        ])
        .split(main_chunks[0]);

    let conn_block = Block::default().title("Connection").borders(Borders::ALL);

    f.render_widget(
        Paragraph::new(build_connection_text(&app.index_data)).block(conn_block),
        left_chunks[0],
    );

    let cell_block = Block::default().title("Cell Info").borders(Borders::ALL);

    f.render_widget(
        Paragraph::new(build_cell_text(&app.index_data)).block(cell_block),
        left_chunks[1],
    );

    let data_block = Block::default().title("Data Usage").borders(Borders::ALL);

    f.render_widget(
        Paragraph::new(build_data_usage_text(app)).block(data_block),
        left_chunks[2],
    );

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(0),
        ])
        .split(main_chunks[1]);

    let rsrp_data = app.rsrp_history.make_contiguous();

    let rsrp_sparkline = Sparkline::default()
        .block(Block::default().title("RSRP (dBm)").borders(Borders::ALL))
        .data(rsrp_data)
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(rsrp_sparkline, right_chunks[0]);

    let dl_spark = Sparkline::default()
        .block(
            Block::default()
                .title("Download (Mbps)")
                .borders(Borders::ALL),
        )
        .data(app.dl_spark_data.make_contiguous())
        .style(Style::default().fg(Color::Green))
        .max(100);

    f.render_widget(dl_spark, right_chunks[1]);

    let ul_spark = Sparkline::default()
        .block(
            Block::default()
                .title("Upload (Mbps)")
                .borders(Borders::ALL),
        )
        .data(app.ul_spark_data.make_contiguous())
        .style(Style::default().fg(Color::Red))
        .max(50);

    f.render_widget(ul_spark, right_chunks[2]);

    let sys_block = Block::default().title("System").borders(Borders::ALL);

    f.render_widget(
        Paragraph::new(build_system_text(&app.index_data)).block(sys_block),
        right_chunks[3],
    );
}
