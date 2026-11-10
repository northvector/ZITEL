use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
    Frame,
};

use serde_json::Value;

use crate::app::App;

/// Bordered panel used by every page: rounded corners, muted border, padded title and body.
pub fn panel(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::horizontal(1))
}

/// One-line page header; callers reserve `Constraint::Length(2)` so a blank row follows it.
pub fn draw_header(f: &mut Frame, area: Rect, title: &str) {
    let line = Line::from(vec![
        Span::styled(" Zitel ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {}", title), Style::default().add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

pub fn kv<'a>(label: &str, value: Span<'a>) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{:<12}", label), Style::default().fg(Color::Gray)),
        value,
    ])
}

pub fn add_line<'a>(lines: &mut Vec<Line<'a>>, label: &str, data: &'a Value, key: &str) {
    lines.push(kv(label, Span::raw(data[key].as_str().unwrap_or("-"))));
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

pub fn build_connection_text(data: &Value) -> Text<'_> {
    let mut lines = vec![];

    add_line(&mut lines, "Type", data, "TYPE");
    add_line(&mut lines, "Band", data, "BAND");
    add_line(&mut lines, "CSQ", data, "CSQ");
    add_line(&mut lines, "RSSI", data, "RSSI");

    let public_ip = data["IPV4"]
        .as_str()
        .or_else(|| data["wan_ipaddr"].as_str())
        .unwrap_or("-");

    let internet_status = data["INTERNET"].as_str().unwrap_or("-");

    lines.push(kv("Public IP", Span::raw(public_ip)));

    let status_color = if internet_status.to_lowercase() == "online" {
        Color::Green
    } else {
        Color::Red
    };

    lines.push(kv(
        "Internet",
        Span::styled(internet_status, Style::default().fg(status_color)),
    ));

    Text::from(lines)
}

pub fn build_cell_text(data: &Value) -> Text<'_> {
    let mut lines = vec![];

    add_line(&mut lines, "MCC", data, "MCC");
    add_line(&mut lines, "MNC", data, "MNC");
    add_line(&mut lines, "PCI", data, "PCID");
    add_line(&mut lines, "EARFCN", data, "EARFCN");
    add_line(&mut lines, "TAC", data, "TAC");
    add_line(&mut lines, "eNodeB", data, "ENODE");
    add_line(&mut lines, "Cell ID", data, "CELL");

    Text::from(lines)
}

pub fn build_data_usage_text(app: &App) -> Text<'_> {
    let mut lines = vec![];

    let current_rx = app.index_data["recieve"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let current_tx = app.index_data["sentt"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    lines.push(kv("Received", Span::raw(format_bytes(current_rx))));
    lines.push(kv("Sent", Span::raw(format_bytes(current_tx))));

    if let Some(dl) = app.download_speed {
        lines.push(kv(
            "Download",
            Span::styled(format!("{:.2} Mbps", dl), Style::default().fg(Color::Green)),
        ));
    }

    if let Some(ul) = app.upload_speed {
        lines.push(kv(
            "Upload",
            Span::styled(format!("{:.2} Mbps", ul), Style::default().fg(Color::Magenta)),
        ));
    }

    Text::from(lines)
}

pub fn build_system_text(data: &Value) -> Text<'_> {
    let mut lines = vec![];

    add_line(&mut lines, "Model", data, "model");
    add_line(&mut lines, "Serial", data, "serial");
    add_line(&mut lines, "Hardware", data, "hardv");
    add_line(&mut lines, "Software", data, "sofv");
    add_line(&mut lines, "Uptime", data, "SYSUP");
    add_line(&mut lines, "RAM", data, "ram");
    add_line(&mut lines, "CPU1", data, "cpu1");
    add_line(&mut lines, "CPU2", data, "cpu2");

    Text::from(lines)
}
