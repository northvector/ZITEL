use std::collections::VecDeque;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::{
    app::{App, RSRP_FLOOR_DBM, RSRP_GRAPH_MAX},
    signal::{read, score, score_grade, Grade, Metric},
    ui::components::{
        build_cell_text, build_connection_text, build_data_usage_text, build_system_text,
        draw_header, panel,
    },
};

/// Sparkline of the newest points that fit the panel; ratatui draws from the front, so we slice the tail.
fn draw_graph(f: &mut Frame, area: Rect, title: String, data: &VecDeque<u64>, max: u64, color: Color) {
    let block = panel(&title);
    let inner = block.inner(area);

    if data.is_empty() {
        f.render_widget(
            Paragraph::new("Waiting for data…").style(Style::default().fg(Color::DarkGray)).block(block),
            area,
        );
        return;
    }

    let visible: Vec<u64> = data.iter().skip(data.len().saturating_sub(inner.width as usize)).copied().collect();

    f.render_widget(
        Sparkline::default().block(block).data(&visible).max(max).style(Style::default().fg(color)),
        area,
    );
}

/// Speed history is stored in 0.1 Mbps units; scale to the visible peak with a 1 Mbps floor so idle noise stays flat.
fn draw_speed_graph(f: &mut Frame, area: Rect, label: &str, current: Option<f64>, data: &VecDeque<u64>, color: Color) {
    let peak = data.iter().copied().max().unwrap_or(0);
    let title = match current {
        Some(v) => format!("{}  {:.1} Mbps · peak {:.1}", label, v, peak as f64 / 10.0),
        None => label.to_string(),
    };
    draw_graph(f, area, title, data, peak.max(10), color);
}

fn bar(fraction: f64, width: usize, grade: Grade) -> Span<'static> {
    let filled = (fraction * width as f64).round() as usize;
    Span::styled(
        format!("{}{}", "█".repeat(filled), "░".repeat(width - filled)),
        Style::default().fg(grade.color()),
    )
}

fn graded(grade: Grade) -> Span<'static> {
    Span::styled(format!("  {}", grade.label()), Style::default().fg(grade.color()).add_modifier(Modifier::BOLD))
}

/// Live RSRP/RSRQ/SINR gauges with quality grades, a combined score and RSRP stability over the history window.
fn draw_signal_analyzer(f: &mut Frame, area: Rect, app: &App) {
    let block = panel("Signal Analyzer");
    let width = (block.inner(area).width as usize).saturating_sub(30).clamp(5, 40);
    let label = |s: &str| Span::styled(format!("{:<7}", s), Style::default().fg(Color::Gray));
    let mut lines = vec![];

    let read = |key: &str| read(&app.index_data, key);
    for (name, key, unit, metric) in [
        ("RSRP", "RSRP", "dBm", Metric::Rsrp),
        ("RSRQ", "RSRQ", "dB", Metric::Rsrq),
        ("SINR", "SINR", "dB", Metric::Sinr),
    ] {
        lines.push(match read(key) {
            Some(v) => Line::from(vec![
                label(name),
                Span::raw(format!("{:>6.1} {:<4}", v, unit)),
                bar(metric.fraction(v), width, metric.grade(v)),
                graded(metric.grade(v)),
            ]),
            None => Line::from(vec![label(name), Span::styled("-", Style::default().fg(Color::DarkGray))]),
        });
    }

    if let Some(s) = score(read("RSRP"), read("RSRQ"), read("SINR")) {
        let grade = score_grade(s);
        lines.push(Line::from(vec![
            label("Score"),
            Span::styled(format!("{:>6.0}/100 ", s), Style::default().add_modifier(Modifier::BOLD)),
            bar(s / 100.0, width, grade),
            graded(grade),
        ]));
    }

    let h = &app.rsrp_history;
    if h.len() >= 2 {
        let dbm: Vec<f64> = h.iter().map(|&l| l as f64 + RSRP_FLOOR_DBM as f64).collect();
        let avg = dbm.iter().sum::<f64>() / dbm.len() as f64;
        let sd = (dbm.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / dbm.len() as f64).sqrt();
        let (min, max) = dbm.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        let stability = if sd < 2.0 { ("stable", Color::Green) } else if sd < 4.0 { ("variable", Color::Yellow) } else { ("unstable", Color::Red) };
        lines.push(Line::from(vec![
            label("Range"),
            Span::raw(format!("{:.0}…{:.0} dBm · avg {:.1} · σ {:.1} dB  ", min, max, avg, sd)),
            Span::styled(stability.0, Style::default().fg(stability.1)),
        ]));
    }

    f.render_widget(Paragraph::new(lines).block(block), area);
}

pub fn draw_dashboard(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(f.size());

    draw_header(f, chunks[0], "Dashboard");

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    // Heights = content lines + 2 border rows, so panels hug their text instead of stretching.
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Length(9), Constraint::Min(6)])
        .split(main_chunks[0]);

    f.render_widget(
        Paragraph::new(build_connection_text(&app.index_data)).block(panel("Connection")),
        left_chunks[0],
    );

    f.render_widget(
        Paragraph::new(build_cell_text(&app.index_data)).block(panel("Cell Info")),
        left_chunks[1],
    );

    f.render_widget(
        Paragraph::new(build_data_usage_text(app)).block(panel("Data Usage")),
        left_chunks[2],
    );

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0), Constraint::Length(10)])
        .split(main_chunks[1]);

    draw_signal_analyzer(f, right_chunks[0], app);

    let graph_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 3), Constraint::Ratio(1, 3), Constraint::Ratio(1, 3)])
        .split(right_chunks[1]);

    let rsrp_title = match read(&app.index_data, "RSRP") {
        Some(v) => format!("RSRP  {:.1} dBm", v),
        None => "RSRP".to_string(),
    };
    draw_graph(f, graph_chunks[0], rsrp_title, &app.rsrp_history, RSRP_GRAPH_MAX, Color::Yellow);
    draw_speed_graph(f, graph_chunks[1], "Download", app.download_speed, &app.dl_spark_data, Color::Green);
    draw_speed_graph(f, graph_chunks[2], "Upload", app.upload_speed, &app.ul_spark_data, Color::Magenta);

    f.render_widget(
        Paragraph::new(build_system_text(&app.index_data)).block(panel("System")),
        right_chunks[2],
    );
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};
    use serde_json::json;

    use super::*;

    #[test]
    fn graphs_show_newest_points_and_live_titles() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(tx);
        app.index_data = json!({"RSRP": "-950", "RSRQ": "-110", "SINR": "14", "TYPE": "LTE", "INTERNET": "online"});
        app.rsrp_history.extend([45; 150]);
        app.dl_spark_data.extend((0..150).map(|i| if i < 140 { 0 } else { 123 }));
        app.download_speed = Some(12.3);

        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| draw_dashboard(f, &mut app)).unwrap();
        let screen: String = term.backend().buffer().content.iter().map(|c| c.symbol.as_str()).collect::<Vec<_>>()
            .chunks(120).map(|r| r.concat() + "\n").collect();
        println!("{screen}");

        assert!(screen.contains("RSRP  -95.0 dBm"));
        assert!(screen.contains("Download  12.3 Mbps · peak 12.3"));
        // newest (non-zero) download samples must be on screen: full-height bars appear
        assert!(screen.contains('█'));
        assert!(screen.contains("Signal Analyzer"));
        assert!(screen.contains("RSRP    -95.0 dBm"));
        assert!(screen.contains("Fair"));
        assert!(screen.contains("stable"));
    }
}
