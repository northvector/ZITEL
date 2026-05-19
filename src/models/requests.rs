use serde_json::Value;

pub enum Request {
    RefreshDashboard,
    FetchNeighbors,
    SetBandLock { earfcn: String },
    SetDmz { ip: String },
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
}