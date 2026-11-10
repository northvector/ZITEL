use std::sync::{atomic::AtomicBool, Arc};

use serde_json::Value;

use crate::scan::ScanEvent;

pub enum Request {
    RefreshDashboard,
    FetchNeighbors,
    SetBandLock { earfcn: String },
    SetDmz { ip: String },
    StartScan { earfcns: Vec<String>, cancel: Arc<AtomicBool> },
}

pub enum Response {
    DashboardData {
        data: Value,
        error: Option<String>,
    },
    NeighborData {
        data: Value,
        error: Option<String>,
    },
    BandLockResult {
        result: String,
    },
    DmzResult(String),
    Scan(ScanEvent),
}