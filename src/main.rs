use std::collections::VecDeque;
use std::error::Error;
use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Sparkline, Wrap},
    Frame, Terminal,
};
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

// ---------- constants ----------
const BASE_URL: &str = "http://192.168.0.1";
const DEFAULT_DMZ_IP: &str = "192.168.0.92";
const RSRP_HISTORY_LEN: usize = 100;
const SPEED_HISTORY_LEN: usize = 100;
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

// ---------- application state ----------
#[derive(Copy, Clone)]
enum Page {
    Dashboard,
    NeighborCells,
    BandLock,
    Dmz,
}

impl Page {
    fn index(&self) -> usize {
        match self {
            Page::Dashboard => 0,
            Page::NeighborCells => 1,
            Page::BandLock => 2,
            Page::Dmz => 3,
        }
    }
}

struct BandLockState {
    items: Vec<String>,
    state: ListState,
}

impl BandLockState {
    fn new() -> Self {
        Self {
            items: vec![
                "42490".to_string(),
                "42690".to_string(),
                "42890".to_string(),
            ],
            state: ListState::default().with_selected(Some(0)),
        }
    }
}

struct App {
    token: String,
    auth_header: String,
    page: Page,
    index_data: Value,
    neighbour_data: Value,
    dmz_response: Option<String>,
    band_lock_response: Option<String>,
    rsrp_history: VecDeque<u64>,
    dmz_ip_input: String,
    band_lock_state: BandLockState,
    status_message: String,
    last_refresh_request: Instant,
    request_tx: mpsc::UnboundedSender<(Request, mpsc::UnboundedSender<Response>)>,

    // lazy neighbour fetch
    neighbour_fetched: bool,

    // traffic tracking (per‑dashboard request)
    last_dashboard_time: Option<Instant>,
    prev_receive: Option<u64>,
    prev_sent: Option<u64>,

    // calculated speeds (Mbps)
    download_speed: Option<f64>,
    upload_speed: Option<f64>,

    // sparkline data (Mbps * 10 to fit u64)
    dl_spark_data: VecDeque<u64>,
    ul_spark_data: VecDeque<u64>,
}

// --- communication with background task ---
enum Request {
    RefreshDashboard,
    FetchNeighbors,
    SetBandLock { index: usize, earfcn: String },
    SetDmz { ip: String },
}

enum Response {
    DashboardData { data: Value, error: Option<String> },
    NeighborData { data: Value, error: Option<String> },
    BandLockResult { earfcn: String, result: String },
    DmzResult(String),
}

impl App {
    fn new(
        token: String,
        auth_header: String,
        request_tx: mpsc::UnboundedSender<(Request, mpsc::UnboundedSender<Response>)>,
    ) -> Self {
        Self {
            token,
            auth_header,
            page: Page::Dashboard,
            index_data: Value::Null,
            neighbour_data: Value::Null,
            dmz_response: None,
            band_lock_response: None,
            rsrp_history: VecDeque::with_capacity(RSRP_HISTORY_LEN),
            dmz_ip_input: String::new(),
            band_lock_state: BandLockState::new(),
            status_message: String::new(),
            last_refresh_request: Instant::now(),
            request_tx,
            neighbour_fetched: false,
            last_dashboard_time: None,
            prev_receive: None,
            prev_sent: None,
            download_speed: None,
            upload_speed: None,
            dl_spark_data: VecDeque::with_capacity(SPEED_HISTORY_LEN),
            ul_spark_data: VecDeque::with_capacity(SPEED_HISTORY_LEN),
        }
    }

    fn next_page(&mut self) {
        self.page = match self.page {
            Page::Dashboard => Page::NeighborCells,
            Page::NeighborCells => Page::BandLock,
            Page::BandLock => Page::Dmz,
            Page::Dmz => Page::Dashboard,
        };
    }

    fn previous_page(&mut self) {
        self.page = match self.page {
            Page::Dashboard => Page::Dmz,
            Page::NeighborCells => Page::Dashboard,
            Page::BandLock => Page::NeighborCells,
            Page::Dmz => Page::BandLock,
        };
    }

    fn go_to_page(&mut self, idx: usize) {
        self.page = match idx {
            1 => Page::NeighborCells,
            2 => Page::BandLock,
            3 => Page::Dmz,
            _ => Page::Dashboard,
        };
    }

    fn update_traffic(&mut self) {
        let current_rx = self.index_data["recieve"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let current_tx = self.index_data["sentt"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        if let (Some(prev_rx), Some(prev_tx), Some(prev_time)) =
            (self.prev_receive, self.prev_sent, self.last_dashboard_time)
        {
            let elapsed = prev_time.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let dl_bytes = current_rx.saturating_sub(prev_rx) as f64;
                let ul_bytes = current_tx.saturating_sub(prev_tx) as f64;
                self.download_speed = Some((dl_bytes * 8.0) / (elapsed * 1_000_000.0));
                self.upload_speed = Some((ul_bytes * 8.0) / (elapsed * 1_000_000.0));

                // scale to u64 for sparkline (Mbps * 10)
                if let Some(dl) = self.download_speed {
                    let scaled = (dl * 10.0) as u64;
                    if self.dl_spark_data.len() >= SPEED_HISTORY_LEN {
                        self.dl_spark_data.pop_front();
                    }
                    self.dl_spark_data.push_back(scaled);
                }
                if let Some(ul) = self.upload_speed {
                    let scaled = (ul * 10.0) as u64;
                    if self.ul_spark_data.len() >= SPEED_HISTORY_LEN {
                        self.ul_spark_data.pop_front();
                    }
                    self.ul_spark_data.push_back(scaled);
                }
            }
        }

        self.prev_receive = Some(current_rx);
        self.prev_sent = Some(current_tx);
        self.last_dashboard_time = Some(Instant::now());
    }
}

// ---------- API helpers ----------
async fn authenticate() -> Result<(String, String), Box<dyn Error>> {
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let url = format!("{}/authenticate.leano", BASE_URL);
    let xml_data = "authenticate admin admin";

    let response = client
        .post(&url)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .body(xml_data)
        .send()
        .await?;

    let json: Value = response.json().await?;

    if json["status"] == "success" {
        let token = json["token"].as_str().unwrap_or("").to_string();
        let auth_header = token.clone();
        Ok((token, auth_header))
    } else {
        Err("Authentication failed".into())
    }
}

async fn api_request(auth_header: &str, command: &str) -> Result<Value, Box<dyn Error>> {
    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

    let url = format!("{}/api.leano", BASE_URL);

    let response = client
        .post(&url)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .header("Leano_Auth", auth_header)
        .header("Accept", "*/*")
        .header("X-Requested-With", "XMLHttpRequest")
        .body(command.to_string())
        .send()
        .await?;

    let json: Value = response.json().await?;
    Ok(json)
}

// --- background task runner ---
async fn run_handlers(
    auth_header: String,
    mut rx: mpsc::UnboundedReceiver<(Request, mpsc::UnboundedSender<Response>)>,
) {
    while let Some((request, resp_tx)) = rx.recv().await {
        match request {
            Request::RefreshDashboard => {
                let result = api_request(&auth_header, "get_index_data").await;
                let (data, error) = match result {
                    Ok(d) => (d, None),
                    Err(e) => (Value::Null, Some(e.to_string())),
                };
                let _ = resp_tx.send(Response::DashboardData { data, error });
            }
            Request::FetchNeighbors => {
                let result = api_request(&auth_header, "get_neighbour_cell").await;
                let (data, error) = match result {
                    Ok(d) => (d, None),
                    Err(e) => (Value::Null, Some(e.to_string())),
                };
                let _ = resp_tx.send(Response::NeighborData { data, error });
            }
            Request::SetBandLock { index: _, earfcn } => {
                let command = format!("set_band_lock {}", earfcn);
                let result = api_request(&auth_header, &command).await;
                let msg = match result {
                    Ok(resp) => format!(
                        "Set to {}: {}",
                        earfcn,
                        serde_json::to_string_pretty(&resp).unwrap_or_default()
                    ),
                    Err(e) => format!("Error: {}", e),
                };
                let _ = resp_tx.send(Response::BandLockResult {
                    earfcn,
                    result: msg,
                });
            }
            Request::SetDmz { ip } => {
                let command = format!("set_dmz 1 tcpudp {}", ip);
                let result = api_request(&auth_header, &command).await;
                let msg = match result {
                    Ok(resp) => serde_json::to_string_pretty(&resp).unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                };
                let _ = resp_tx.send(Response::DmzResult(msg));
            }
        }
    }
}

// ---------- TUI drawing ----------
fn draw_dashboard(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
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
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);

    // left column
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(35),
                Constraint::Percentage(30),
                Constraint::Percentage(35),
            ]
            .as_ref(),
        )
        .split(main_chunks[0]);

    let conn_block = Block::default().title("Connection").borders(Borders::ALL);
    let conn_text = build_connection_text(&app.index_data);
    f.render_widget(Paragraph::new(conn_text).block(conn_block), left_chunks[0]);

    let cell_block = Block::default().title("Cell Info").borders(Borders::ALL);
    let cell_text = build_cell_text(&app.index_data);
    f.render_widget(Paragraph::new(cell_text).block(cell_block), left_chunks[1]);

    let data_block = Block::default().title("Data Usage").borders(Borders::ALL);
    let data_text = build_data_usage_text(app);
    f.render_widget(Paragraph::new(data_text).block(data_block), left_chunks[2]);

    // right column
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(6), // RSRP sparkline
                Constraint::Length(6), // DL speed sparkline
                Constraint::Length(6), // UL speed sparkline
                Constraint::Min(0),    // System info
            ]
            .as_ref(),
        )
        .split(main_chunks[1]);

    let binding = app.rsrp_history.make_contiguous();
    let rsrp_sparkline = Sparkline::default()
        .block(Block::default().title("RSRP (dBm)").borders(Borders::ALL))
        .data(&binding)
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
        .max(100); // 10.0 Mbps * 10 = 100
    f.render_widget(dl_spark, right_chunks[1]);

    let ul_spark = Sparkline::default()
        .block(
            Block::default()
                .title("Upload (Mbps)")
                .borders(Borders::ALL),
        )
        .data(app.ul_spark_data.make_contiguous())
        .style(Style::default().fg(Color::Red))
        .max(50); // 5.0 Mbps * 10 = 50
    f.render_widget(ul_spark, right_chunks[2]);

    let sys_block = Block::default().title("System").borders(Borders::ALL);
    let sys_text = build_system_text(&app.index_data);
    f.render_widget(Paragraph::new(sys_text).block(sys_block), right_chunks[3]);
}

fn build_connection_text(data: &Value) -> Text {
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

fn build_cell_text(data: &Value) -> Text {
    let mut lines = vec![];
    add_line(&mut lines, "Modem Call Control", data, "MCC");
    add_line(&mut lines, "MNC", data, "MNC");
    add_line(&mut lines, "PCI", data, "PCID");
    add_line(&mut lines, "EARFCN", data, "EARFCN");
    add_line(&mut lines, "Technical Assistance Center (TAC)", data, "TAC");
    add_line(&mut lines, "eNodeB", data, "ENODE");
    add_line(&mut lines, "Cell ID", data, "CELL");
    Text::from(lines)
}

fn build_data_usage_text(app: &App) -> Text {
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

fn build_system_text(data: &Value) -> Text {
    let mut lines = vec![];
    add_line(&mut lines, "Model", data, "model");
    add_line(&mut lines, "Serial", data, "serial");
    add_line(&mut lines, "Hardware", data, "hardv");
    add_line(&mut lines, "Software", data, "sofv");
    add_line(&mut lines, "Uptime (s)", data, "SYSUP");
    add_line(&mut lines, "RAM (MB)", data, "ram");
    add_line(&mut lines, "CPU1 %", data, "cpu1");
    add_line(&mut lines, "CPU2 %", data, "cpu2");

    if let (Some(c1), Some(c2)) = (
        data["cpu1"].as_str().and_then(|v| v.parse::<f64>().ok()),
        data["cpu2"].as_str().and_then(|v| v.parse::<f64>().ok()),
    ) {
        let avg = (c1 + c2) / 2.0;
        lines.push(Line::from(format!("CPU Avg %   {:.1}", avg)));
    }

    Text::from(lines)
}

fn add_line<'a>(lines: &mut Vec<Line<'a>>, label: &str, data: &'a Value, key: &str) {
    let val = data[key].as_str().unwrap_or("-");
    lines.push(Line::from(vec![
        Span::styled(format!("{:12}", label), Style::default().fg(Color::Gray)),
        Span::raw(val),
    ]));
}

fn format_bytes(bytes: u64) -> String {
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

fn draw_neighbor_cells(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(f.size());

    let title = Paragraph::new("Neighbor Cells")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let data = &app.neighbour_data;
    let count = data["lenghtt"]
        .as_str()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    let mut lines = vec![];
    lines.push(Line::from(format!("Found {} neighbor cell(s)", count)));
    for i in 1..=count {
        lines.push(Line::from(""));
        lines.push(Line::from(format!(" Cell {} ", i)));
        add_line(&mut lines, "MCC", data, &format!("type{}", i));
        add_line(&mut lines, "MNC", data, &format!("band{}", i));
        add_line(&mut lines, "Band", data, &format!("pcid{}", i));
        add_line(&mut lines, "ARFCN", data, &format!("rsrq{}", i));
        add_line(&mut lines, "PCI", data, &format!("rsrp{}", i));
        add_line(&mut lines, "Signal(dBm)", data, &format!("rsrppp{}", i));
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Neighbour Cells"),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, chunks[1]);
}

fn draw_band_lock(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.size());

    let title = Paragraph::new("Band Lock – Select EARFCN")
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
    let status_para = Paragraph::new(status).block(Block::default().borders(Borders::ALL));
    f.render_widget(status_para, chunks[2]);
}

fn draw_dmz(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(f.size());

    let title = Paragraph::new("Set DMZ")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let input = Paragraph::new(format!(
        "DMZ IP (default {}): {}",
        DEFAULT_DMZ_IP, app.dmz_ip_input
    ))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(input, chunks[1]);

    let status = app.dmz_response.clone().unwrap_or_default();
    let status_para = Paragraph::new(status).block(Block::default().borders(Borders::ALL));
    f.render_widget(status_para, chunks[2]);
}

fn ui(f: &mut Frame, app: &mut App) {
    match app.page {
        Page::Dashboard => draw_dashboard(f, app),
        Page::NeighborCells => draw_neighbor_cells(f, app),
        Page::BandLock => draw_band_lock(f, app),
        Page::Dmz => draw_dmz(f, app),
    }

    let footer_rect = Rect::new(
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
    footer_spans.push(Span::raw("| q: quit | "));
    footer_spans.push(Span::raw(&app.status_message));

    let footer =
        Paragraph::new(Line::from(footer_spans)).style(Style::default().bg(Color::DarkGray));
    f.render_widget(footer, footer_rect);
}

// ---------- main TUI loop ----------
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (token, auth_header) = authenticate().await?;

    let (worker_tx, request_rx) =
        mpsc::unbounded_channel::<(Request, mpsc::UnboundedSender<Response>)>();
    let (response_tx, mut response_rx) = mpsc::unbounded_channel::<Response>();

    tokio::spawn(run_handlers(auth_header.clone(), request_rx));

    fn send_request(
        worker_tx: &mpsc::UnboundedSender<(Request, mpsc::UnboundedSender<Response>)>,
        response_tx: &mpsc::UnboundedSender<Response>,
        request: Request,
    ) {
        let _ = worker_tx.send((request, response_tx.clone()));
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(token, auth_header.clone(), worker_tx.clone());

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
                                if app.rsrp_history.len() >= RSRP_HISTORY_LEN {
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
                    if let Some(e) = error {
                        app.status_message = format!("Neighbour error: {}", e);
                    } else {
                        app.neighbour_data = data;
                        app.status_message = "Neighbour cells fetched".into();
                    }
                }
                Response::BandLockResult { result, .. } => {
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
                    // --- Global quit ---
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('Q') {
                        break;
                    }

                    // --- DMZ input handling (always takes precedence over tab switching) ---
                    if matches!(app.page, Page::Dmz) {
                        match key.code {
                            KeyCode::Enter => {
                                let ip = if app.dmz_ip_input.is_empty() {
                                    DEFAULT_DMZ_IP.to_string()
                                } else {
                                    app.dmz_ip_input.clone()
                                };
                                send_request(&app.request_tx, &response_tx, Request::SetDmz { ip });
                                app.dmz_response = Some("Sending...".to_string());
                                app.dmz_ip_input.clear();
                            }
                            KeyCode::Backspace | KeyCode::Delete => {
                                app.dmz_ip_input.pop();
                            }
                            KeyCode::Tab => {
                                app.next_page();
                                if matches!(app.page, Page::NeighborCells) && !app.neighbour_fetched {
                                    app.neighbour_fetched = true;
                                    send_request(
                                        &app.request_tx,
                                        &response_tx,
                                        Request::FetchNeighbors,
                                    );  
                                }
                            }
                            KeyCode::Char(c) => {
                                // Allow digits and dots only (simple IP input)
                                if c.is_ascii_digit() || c == '.' {
                                    app.dmz_ip_input.push(c);
                                }
                            }
                            _ => {}
                        }
                        continue; // skip any tab‑switching keys while in DMZ
                    }

                    // --- Tab switching (only when NOT in DMZ) ---
                    match key.code {
                        KeyCode::Tab => {
                            app.next_page();
                            if matches!(app.page, Page::NeighborCells) && !app.neighbour_fetched {
                                app.neighbour_fetched = true;
                                send_request(
                                    &app.request_tx,
                                    &response_tx,
                                    Request::FetchNeighbors,
                                );
                            }
                        }
                        KeyCode::BackTab => {
                            app.previous_page();
                            if matches!(app.page, Page::NeighborCells) && !app.neighbour_fetched {
                                app.neighbour_fetched = true;
                                send_request(
                                    &app.request_tx,
                                    &response_tx,
                                    Request::FetchNeighbors,
                                );
                            }
                        }
                        KeyCode::Char('1') => {
                            app.go_to_page(0);
                            if matches!(app.page, Page::NeighborCells) && !app.neighbour_fetched {
                                app.neighbour_fetched = true;
                                send_request(
                                    &app.request_tx,
                                    &response_tx,
                                    Request::FetchNeighbors,
                                );
                            }
                        }
                        KeyCode::Char('2') => app.go_to_page(1),
                        KeyCode::Char('3') => app.go_to_page(2),
                        KeyCode::Char('4') => app.go_to_page(3),
                        KeyCode::Char('5') => app.go_to_page(4),
                        KeyCode::Up | KeyCode::Down => {
                            if let Page::BandLock = app.page {
                                let i = match key.code {
                                    KeyCode::Up => app
                                        .band_lock_state
                                        .state
                                        .selected()
                                        .map_or(0, |s| s.saturating_sub(1)),
                                    KeyCode::Down => {
                                        let last =
                                            app.band_lock_state.items.len().saturating_sub(1);
                                        app.band_lock_state
                                            .state
                                            .selected()
                                            .map_or(0, |s| (s + 1).min(last))
                                    }
                                    _ => 0,
                                };
                                app.band_lock_state.state.select(Some(i));
                            }
                        }
                        KeyCode::Enter => {
                            if let Page::BandLock = app.page {
                                let selected = app.band_lock_state.state.selected().unwrap_or(0);
                                let earfcn = app.band_lock_state.items[selected].clone();
                                send_request(
                                    &app.request_tx,
                                    &response_tx,
                                    Request::SetBandLock {
                                        index: selected,
                                        earfcn,
                                    },
                                );
                                app.band_lock_response = Some("Sending...".to_string());
                            }
                        }
                        _ => {}
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