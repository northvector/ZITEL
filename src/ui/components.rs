use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
};

use serde_json::Value;

use crate::app::App;

pub fn add_line<'a>(lines: &mut Vec<Line<'a>>, label: &str, data: &'a Value, key: &str) {
    let val = data[key].as_str().unwrap_or("-");

    lines.push(Line::from(vec![
        Span::styled(format!("{:12}", label), Style::default().fg(Color::Gray)),
        Span::raw(val),
    ]));
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
    add_line(&mut lines, "RSRP", data, "RSRP");
    add_line(&mut lines, "RSRQ", data, "RSRQ");
    add_line(&mut lines, "SINR", data, "SINR");
    add_line(&mut lines, "RSSI", data, "RSSI");

    let public_ip = data["IPV4"]
        .as_str()
        .or_else(|| data["wan_ipaddr"].as_str())
        .unwrap_or("-");

    let internet_status = data["INTERNET"].as_str().unwrap_or("-");

    lines.push(Line::from(format!("Public IP:    {}", public_ip)));

    let status_color = if internet_status.to_lowercase() == "online" {
        Color::Green
    } else {
        Color::Red
    };

    lines.push(Line::from(vec![
        Span::raw("Internet:     "),
        Span::styled(internet_status, Style::default().fg(status_color)),
    ]));

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

    lines.push(Line::from(format!(
        "Received: {}",
        format_bytes(current_rx)
    )));

    lines.push(Line::from(format!(
        "Sent:     {}",
        format_bytes(current_tx)
    )));

    if let Some(dl) = app.download_speed {
        lines.push(Line::from(vec![
            Span::raw("Download:  "),
            Span::styled(format!("{:.2} Mbps", dl), Style::default().fg(Color::Green)),
        ]));
    }

    if let Some(ul) = app.upload_speed {
        lines.push(Line::from(vec![
            Span::raw("Upload:    "),
            Span::styled(format!("{:.2} Mbps", ul), Style::default().fg(Color::Red)),
        ]));
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
