use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::mpsc;

use crate::models::{
    bandlock::BandLockState,
    page::Page,
    requests::{Request, Response},
};

pub const DEFAULT_DMZ_IP: &str = "192.168.0.92";
pub const RSRP_HISTORY_LEN: usize = 100;
pub const SPEED_HISTORY_LEN: usize = 100;
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub struct App {
    pub page: Page,
    pub index_data: Value,
    pub neighbour_data: Value,
    pub dmz_response: Option<String>,
    pub band_lock_response: Option<String>,
    pub rsrp_history: VecDeque<u64>,
    pub dmz_ip_input: String,
    pub band_lock_state: BandLockState,
    pub status_message: String,
    pub request_tx: mpsc::UnboundedSender<(Request, mpsc::UnboundedSender<Response>)>,

    pub neighbour_fetched: bool,
    pub neighbour_fetching: bool,

    pub last_dashboard_time: Option<Instant>,
    pub prev_receive: Option<u64>,
    pub prev_sent: Option<u64>,

    pub download_speed: Option<f64>,
    pub upload_speed: Option<f64>,

    pub dl_spark_data: VecDeque<u64>,
    pub ul_spark_data: VecDeque<u64>,
}

impl App {
    pub fn new(
        request_tx: mpsc::UnboundedSender<(Request, mpsc::UnboundedSender<Response>)>,
    ) -> Self {
        Self {
            page: Page::Dashboard,
            index_data: Value::Null,
            neighbour_data: Value::Null,
            dmz_response: None,
            band_lock_response: None,
            rsrp_history: VecDeque::with_capacity(RSRP_HISTORY_LEN),
            dmz_ip_input: String::new(),
            band_lock_state: BandLockState::new(),
            status_message: String::new(),
            request_tx,
            neighbour_fetched: false,
            neighbour_fetching: false,
            last_dashboard_time: None,
            prev_receive: None,
            prev_sent: None,
            download_speed: None,
            upload_speed: None,
            dl_spark_data: VecDeque::with_capacity(SPEED_HISTORY_LEN),
            ul_spark_data: VecDeque::with_capacity(SPEED_HISTORY_LEN),
        }
    }

    pub fn next_page(&mut self) {
        self.page = match self.page {
            Page::Dashboard => Page::NeighborCells,
            Page::NeighborCells => Page::BandLock,
            Page::BandLock => Page::Dmz,
            Page::Dmz => Page::Dashboard,
        };
    }

    pub fn previous_page(&mut self) {
        self.page = match self.page {
            Page::Dashboard => Page::Dmz,
            Page::NeighborCells => Page::Dashboard,
            Page::BandLock => Page::NeighborCells,
            Page::Dmz => Page::BandLock,
        };
    }

    pub fn go_to_page(&mut self, idx: usize) {
        self.page = match idx {
            1 => Page::NeighborCells,
            2 => Page::BandLock,
            3 => Page::Dmz,
            _ => Page::Dashboard,
        };
    }

    pub fn update_traffic(&mut self) {
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

                if let Some(dl) = self.download_speed {
                    if self.dl_spark_data.len() >= SPEED_HISTORY_LEN {
                        self.dl_spark_data.pop_front();
                    }

                    self.dl_spark_data.push_back((dl * 10.0) as u64);
                }

                if let Some(ul) = self.upload_speed {
                    if self.ul_spark_data.len() >= SPEED_HISTORY_LEN {
                        self.ul_spark_data.pop_front();
                    }

                    self.ul_spark_data.push_back((ul * 10.0) as u64);
                }
            }
        }

        self.prev_receive = Some(current_rx);
        self.prev_sent = Some(current_tx);
        self.last_dashboard_time = Some(Instant::now());
    }
}
