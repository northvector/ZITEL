use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    backend::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};

use tokio::sync::mpsc;

mod api;
mod app;
mod events;
mod models;
mod ui;

use api::{authenticate, run_handlers};
use app::{App, REFRESH_INTERVAL};
use events::{handle_key_event, send_request};
use models::{
    page::Page,
    requests::{Request, Response},
};

use ui::{
    bandlock::draw_band_lock, dashboard::draw_dashboard, dmz::draw_dmz,
    neighbors::draw_neighbor_cells,
};

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    match app.page {
        Page::Dashboard => draw_dashboard(f, app),
        Page::NeighborCells => draw_neighbor_cells(f, app),
        Page::BandLock => draw_band_lock(f, app),
        Page::Dmz => draw_dmz(f, app),
    }

    let footer_rect = ratatui::layout::Rect::new(
        f.size().x,
        f.size().y + f.size().height.saturating_sub(1),
        f.size().width,
        1,
    );

    let tabs = [" Dashboard ", " Neighbors ", " BandLock ", " DMZ "];

    let mut footer_spans = vec![Span::raw(" Tabs: ")];

    let active_index = app.page.index();

    for (i, name) in tabs.iter().enumerate() {
        let style = if i == active_index {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default()
        };

        footer_spans.push(Span::styled(format!("{}({}) ", name, i + 1), style));
    }

    footer_spans.push(Span::raw("| i: refresh ip | q: quit | "));

    footer_spans.push(Span::raw(&app.status_message));

    let footer =
        Paragraph::new(Line::from(footer_spans)).style(Style::default().bg(Color::DarkGray));

    f.render_widget(footer, footer_rect);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (_, auth_header) = authenticate().await?;

    let (worker_tx, request_rx) =
        mpsc::unbounded_channel::<(Request, mpsc::UnboundedSender<Response>)>();

    let (response_tx, mut response_rx) = mpsc::unbounded_channel::<Response>();

    tokio::spawn(run_handlers(auth_header.clone(), request_rx));

    enable_raw_mode()?;

    let mut stdout = io::stdout();

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(worker_tx.clone());

    send_request(&app.request_tx, &response_tx, Request::RefreshDashboard);

    let tick_rate = Duration::from_millis(50);

    let mut last_tick = Instant::now();

    let mut last_refresh = Instant::now();

    loop {
        while let Ok(response) = response_rx.try_recv() {
            match response {
                Response::DashboardData { data, error } => {
                    if let Some(e) = error {
                        app.status_message = format!("Dashboard error: {}", e);
                    } else {
                        if let Some(rsrp_str) = data["RSRP"].as_str() {
                            if let Ok(val) = rsrp_str.parse::<i64>() {
                                let abs_val = val.unsigned_abs().min(140);

                                if app.rsrp_history.len() >= app::RSRP_HISTORY_LEN {
                                    app.rsrp_history.pop_front();
                                }

                                app.rsrp_history.push_back(abs_val);
                            }
                        }

                        app.index_data = data;

                        app.update_traffic();

                        app.status_message = "Dashboard updated".into();
                    }
                }

                Response::NeighborData { data, error } => {
                    app.neighbour_fetching = false;

                    if let Some(e) = error {
                        app.status_message = format!("Neighbour error: {}", e);
                    } else {
                        app.neighbour_data = data;
                        app.neighbour_fetched = true;

                        app.status_message = "Neighbour cells fetched".into();
                    }
                }

                Response::BandLockResult { result } => {
                    app.band_lock_response = Some(result);
                }

                Response::DmzResult(result) => {
                    app.dmz_response = Some(result);

                    app.status_message = "DMZ updated".into();
                }
            }
        }

        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if handle_key_event(key, &mut app, &response_tx) {
                        break;
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        if last_refresh.elapsed() >= REFRESH_INTERVAL {
            send_request(&app.request_tx, &response_tx, Request::RefreshDashboard);

            last_refresh = Instant::now();
        }
    }

    disable_raw_mode()?;

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    terminal.show_cursor()?;

    Ok(())
}
