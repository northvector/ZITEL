use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Gauge, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::{
    app::App,
    scan::BandResult,
    signal::score_grade,
    ui::components::{draw_header, kv, panel},
};

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn opt(v: Option<f64>, unit: &str, decimals: usize) -> String {
    v.map_or("-".into(), |v| format!("{:.*}{}", decimals, v, unit))
}

fn same(a: Option<&BandResult>, r: &BandResult) -> bool {
    a.map_or(false, |a| a.earfcn == r.earfcn)
}

fn best_line<'a>(label: &str, best: Option<&BandResult>, value: impl Fn(&BandResult) -> String, color: Color) -> Line<'a> {
    match best {
        Some(b) => kv(
            label,
            Span::styled(
                format!("{}  {}  ·  {}", b.earfcn, if b.band.is_empty() { "-" } else { &b.band }, value(b)),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ),
        None => kv(label, Span::styled("-", Style::default().fg(Color::DarkGray))),
    }
}

pub fn draw_band_scan(f: &mut Frame, app: &mut App) {
    let scan = &app.scan;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(7),
        ])
        .split(f.size());

    draw_header(f, chunks[0], "Band Scan");

    // Progress
    let block = panel("Progress");
    let inner = block.inner(chunks[1]);
    f.render_widget(block, chunks[1]);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    if scan.started.is_none() {
        let bands = app.band_lock_state.items.join(", ");
        f.render_widget(
            Paragraph::new(format!("Press Enter to test {} bands: {}", app.band_lock_state.items.len(), bands))
                .style(Style::default().add_modifier(Modifier::BOLD)),
            rows[0],
        );
        f.render_widget(
            Paragraph::new("Each band is locked in turn (≈1 min each); the connection drops briefly while it switches.")
                .style(Style::default().fg(Color::DarkGray)),
            rows[1],
        );
    } else {
        let total = scan.earfcns.len().max(1);
        let done = scan.done_count();
        let secs = scan.elapsed().as_secs();
        f.render_widget(
            Gauge::default()
                .ratio(done as f64 / total as f64)
                .label(format!("{}/{} bands · {:02}:{:02}", done, total, secs / 60, secs % 60))
                .gauge_style(Style::default().fg(if scan.running { Color::Cyan } else { Color::Green }).bg(Color::Black))
                .use_unicode(true),
            rows[0],
        );
        let phase = if scan.running {
            let frame = SPINNER[(scan.elapsed().as_millis() / 100) as usize % SPINNER.len()];
            let earfcn = scan.earfcns.get(scan.current).map(String::as_str).unwrap_or("");
            Line::from(vec![
                Span::styled(format!("{} {}  ", frame, earfcn), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(scan.phase.clone()),
            ])
        } else {
            Line::from(Span::styled(format!("✓ {}", scan.phase), Style::default().fg(Color::Green)))
        };
        f.render_widget(Paragraph::new(phase), rows[1]);
    }

    // Results table
    let (best_speed, best_signal, best_ping) = (scan.best_speed(), scan.best_signal(), scan.best_ping());
    let highlight = |is_best: bool, text: String| {
        if is_best {
            Cell::from(format!("★ {}", text)).style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        } else {
            Cell::from(text)
        }
    };

    let rows: Vec<Row> = scan
        .earfcns
        .iter()
        .enumerate()
        .map(|(i, earfcn)| match scan.results.get(i).and_then(Option::as_ref) {
            Some(r) => {
                let score = r.signal_score();
                let quality = score.map_or("-".into(), |s| format!("{:.0} {}", s, score_grade(s).label()));
                let status = match &r.error {
                    Some(e) => Cell::from(format!("✗ {}", e)).style(Style::default().fg(Color::Red)),
                    None => Cell::from("✓ done").style(Style::default().fg(Color::Green)),
                };
                let ping = match r.ping_lost {
                    0 => opt(r.ping_ms, " ms", 0),
                    lost => format!("{} ({} lost)", opt(r.ping_ms, " ms", 0), lost),
                };
                Row::new(vec![
                    Cell::from(earfcn.clone()),
                    Cell::from(if r.band.is_empty() { "-".into() } else { r.band.clone() }),
                    Cell::from(opt(r.rsrp, "", 1)),
                    Cell::from(opt(r.rsrq, "", 1)),
                    Cell::from(opt(r.sinr, "", 1)),
                    highlight(same(best_signal, r), quality),
                    highlight(same(best_ping, r), ping),
                    highlight(same(best_speed, r), opt(r.speed_mbps, " Mbps", 1)),
                    status,
                ])
            }
            None => {
                let (text, style) = if scan.running && i == scan.current {
                    ("● testing…", Style::default().fg(Color::Yellow))
                } else if scan.running {
                    ("· queued", Style::default().fg(Color::DarkGray))
                } else {
                    ("– skipped", Style::default().fg(Color::DarkGray))
                };
                let mut cells = vec![Cell::from(earfcn.clone())];
                cells.extend((0..7).map(|_| Cell::from("")));
                cells.push(Cell::from(text).style(style));
                Row::new(cells)
            }
        })
        .collect();

    let header = Row::new(["EARFCN", "Band", "RSRP", "RSRQ", "SINR", "Signal", "Ping", "Download", "Status"])
        .style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))
        .bottom_margin(1);
    let widths = [
        Constraint::Length(8),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(15),
        Constraint::Length(16),
        Constraint::Length(15),
        Constraint::Min(10),
    ];
    f.render_widget(
        Table::new(rows).header(header).widths(&widths).column_spacing(1).block(panel("Results")),
        chunks[2],
    );

    // Summary
    let mut lines = vec![
        best_line("Best speed", best_speed, |b| opt(b.speed_mbps, " Mbps", 1), Color::Green),
        best_line(
            "Best signal",
            best_signal,
            |b| b.signal_score().map_or("-".into(), |s| format!("score {:.0} · {}", s, score_grade(s).label())),
            Color::Cyan,
        ),
        best_line("Best ping", best_ping, |b| opt(b.ping_ms, " ms", 0), Color::Magenta),
        Line::from(""),
    ];
    lines.push(match (scan.running, best_speed) {
        (true, _) => Line::from(Span::styled("Esc to cancel; the original band is restored either way.", Style::default().fg(Color::DarkGray))),
        (false, Some(b)) => Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("a", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" to lock the fastest band ({}), Enter to rescan.", b.earfcn), Style::default().fg(Color::DarkGray)),
        ]),
        (false, None) => Line::from(Span::styled("Results appear here as each band finishes.", Style::default().fg(Color::DarkGray))),
    });
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }).block(panel("Summary")), chunks[3]);
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::scan::ScanEvent;

    #[test]
    fn renders_progress_and_marks_winners() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(tx);
        app.scan.start(app.band_lock_state.items.clone());
        app.scan.apply(ScanEvent::Done(0, BandResult {
            earfcn: "42490".into(), band: "43".into(), rsrp: Some(-85.0), rsrq: Some(-9.0), sinr: Some(18.0),
            ping_ms: Some(31.0), speed_mbps: Some(72.4), ..Default::default()
        }));
        app.scan.apply(ScanEvent::Phase(1, "Testing download speed… 40.1 Mbps".into()));

        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        term.draw(|f| draw_band_scan(f, &mut app)).unwrap();
        let screen: String = term.backend().buffer().content.iter().map(|c| c.symbol.as_str()).collect::<Vec<_>>()
            .chunks(120).map(|r| r.concat() + "\n").collect();
        println!("{screen}");

        assert!(screen.contains("1/3 bands"));
        assert!(screen.contains("Testing download speed… 40.1 Mbps"));
        assert!(screen.contains("★ 72.4 Mbps"));
        assert!(screen.contains("testing…"));
        assert!(screen.contains("queued"));
    }
}
