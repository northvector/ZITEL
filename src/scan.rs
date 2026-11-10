use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use tokio::{net::TcpStream, sync::mpsc::UnboundedSender, time::sleep};

use crate::{api::api_request, models::requests::Response, signal};

// shortcut: test endpoints are hardcoded, move them into config.json if users need a local server.
const PING_TARGET: &str = "1.1.1.1:443";
// Tried in order; the first that answers is used. Iranian Ookla servers first, Cloudflare (403s at >=100MB) last.
const SPEED_SERVERS: &[(&str, &str)] = &[
    ("Zi-Tel", "http://sp.zi-tel.com:8080/download?size="),
    ("MCI", "http://rhaspd2.mci.ir:8080/download?size="),
    ("Pishgaman", "http://speedtest1.pishgaman.net:8080/download?size="),
    ("Cloudflare", "https://speed.cloudflare.com/__down?bytes="),
];
const SPEED_BYTES: u64 = 99_000_000;
const SPEED_TEST_SECS: f64 = 8.0;
const PING_COUNT: usize = 5;
const SIGNAL_SAMPLES: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Default, Debug)]
pub struct BandResult {
    pub earfcn: String,
    pub band: String,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub sinr: Option<f64>,
    pub ping_ms: Option<f64>,
    pub ping_lost: usize,
    pub speed_mbps: Option<f64>,
    pub error: Option<String>,
}

impl BandResult {
    pub fn signal_score(&self) -> Option<f64> {
        signal::score(self.rsrp, self.rsrq, self.sinr)
    }
}

pub enum ScanEvent {
    Phase(usize, String),
    Done(usize, BandResult),
    Finished(String),
}

#[derive(Default)]
pub struct ScanState {
    pub earfcns: Vec<String>,
    pub results: Vec<Option<BandResult>>,
    pub running: bool,
    pub current: usize,
    pub phase: String,
    pub cancel: Arc<AtomicBool>,
    pub started: Option<Instant>,
    pub ended: Option<Instant>,
}

impl ScanState {
    pub fn start(&mut self, earfcns: Vec<String>) {
        *self = ScanState {
            results: vec![None; earfcns.len()],
            earfcns,
            running: true,
            phase: "Starting…".into(),
            started: Some(Instant::now()),
            ..Default::default()
        };
    }

    pub fn apply(&mut self, event: ScanEvent) {
        match event {
            ScanEvent::Phase(i, text) => {
                self.current = i;
                self.phase = text;
            }
            ScanEvent::Done(i, result) => {
                if let Some(slot) = self.results.get_mut(i) {
                    *slot = Some(result);
                }
            }
            ScanEvent::Finished(msg) => {
                self.running = false;
                self.ended = Some(Instant::now());
                self.phase = msg;
            }
        }
    }

    pub fn elapsed(&self) -> Duration {
        match self.started {
            Some(s) => self.ended.unwrap_or_else(Instant::now) - s,
            None => Duration::ZERO,
        }
    }

    pub fn done_count(&self) -> usize {
        self.results.iter().flatten().count()
    }

    fn best_by(&self, key: impl Fn(&BandResult) -> Option<f64>, higher: bool) -> Option<&BandResult> {
        self.results
            .iter()
            .flatten()
            .filter_map(|r| key(r).map(|v| (r, if higher { v } else { -v })))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(r, _)| r)
    }

    pub fn best_speed(&self) -> Option<&BandResult> {
        self.best_by(|r| r.speed_mbps, true)
    }

    pub fn best_signal(&self) -> Option<&BandResult> {
        self.best_by(|r| r.signal_score(), true)
    }

    pub fn best_ping(&self) -> Option<&BandResult> {
        self.best_by(|r| r.ping_ms, false)
    }
}

fn send(tx: &UnboundedSender<Response>, event: ScanEvent) {
    let _ = tx.send(Response::Scan(event));
}

fn phase(tx: &UnboundedSender<Response>, i: usize, text: impl Into<String>) {
    send(tx, ScanEvent::Phase(i, text.into()));
}

fn current_earfcn(data: &serde_json::Value) -> String {
    data["EARFCN"].as_str().unwrap_or("").trim().to_string()
}

/// Locks each EARFCN in turn, measures it, then restores the EARFCN that was active before the scan.
pub async fn run_scan(
    auth: String,
    earfcns: Vec<String>,
    cancel: Arc<AtomicBool>,
    tx: UnboundedSender<Response>,
) {
    let original = api_request(&auth, "get_index_data")
        .await
        .map(|d| current_earfcn(&d))
        .unwrap_or_default();

    for (i, earfcn) in earfcns.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let result = test_band(&auth, earfcn, i, &cancel, &tx).await;
        send(&tx, ScanEvent::Done(i, result));
    }

    let head = if cancel.load(Ordering::Relaxed) { "Scan cancelled" } else { "Scan complete" };
    let tail = if original.is_empty() {
        "original EARFCN unknown, left on last band".to_string()
    } else {
        let last = earfcns.len().saturating_sub(1);
        phase(&tx, last, format!("Restoring EARFCN {}…", original));
        match api_request(&auth, &format!("set_band_lock {}", original)).await {
            Ok(_) => format!("restored EARFCN {}", original),
            Err(e) => format!("failed to restore EARFCN {}: {}", original, e),
        }
    };
    send(&tx, ScanEvent::Finished(format!("{} · {}", head, tail)));
}

async fn test_band(
    auth: &str,
    earfcn: &str,
    i: usize,
    cancel: &AtomicBool,
    tx: &UnboundedSender<Response>,
) -> BandResult {
    let mut r = BandResult { earfcn: earfcn.to_string(), ..Default::default() };

    phase(tx, i, format!("Locking EARFCN {}", earfcn));
    if let Err(e) = api_request(auth, &format!("set_band_lock {}", earfcn)).await {
        r.error = Some(format!("lock failed: {}", e));
        return r;
    }

    // The modem keeps reporting the old cell for a moment after the lock command.
    sleep(Duration::from_secs(3)).await;
    let start = Instant::now();
    let mut seen = String::new();
    let first = loop {
        if cancel.load(Ordering::Relaxed) {
            r.error = Some("cancelled".into());
            return r;
        }
        if start.elapsed() > CONNECT_TIMEOUT {
            r.error = Some(if seen.is_empty() || seen == earfcn {
                "no internet (timeout)".into()
            } else {
                format!("modem stayed on {}", seen)
            });
            return r;
        }
        phase(tx, i, format!("Waiting for network… {}s", start.elapsed().as_secs() + 3));
        if let Ok(d) = api_request(auth, "get_index_data").await {
            seen = current_earfcn(&d);
            let online = d["INTERNET"].as_str().map_or(false, |s| s.eq_ignore_ascii_case("online"));
            if online && seen == earfcn {
                break d;
            }
        }
        sleep(Duration::from_secs(1)).await;
    };
    r.band = first["BAND"].as_str().unwrap_or("").trim().to_string();

    let mut samples = vec![first];
    for n in 1..SIGNAL_SAMPLES {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        phase(tx, i, format!("Sampling signal {}/{}", n + 1, SIGNAL_SAMPLES));
        sleep(Duration::from_secs(1)).await;
        if let Ok(d) = api_request(auth, "get_index_data").await {
            samples.push(d);
        }
    }
    let avg = |key: &str| {
        let v: Vec<f64> = samples.iter().filter_map(|d| signal::read(d, key)).collect();
        (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
    };
    r.rsrp = avg("RSRP");
    r.rsrq = avg("RSRQ");
    r.sinr = avg("SINR");

    // The modem says "online" before traffic actually routes, so probe the real speed host (DNS + TLS + HTTP).
    let client = reqwest::Client::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            r.error = Some("cancelled".into());
            return r;
        }
        if start.elapsed() > CONNECT_TIMEOUT {
            r.error = Some("modem online but no internet (timeout)".into());
            return r;
        }
        phase(tx, i, format!("Waiting for internet… {}s", start.elapsed().as_secs() + 3));
        let mut ok = false;
        for (_, base) in SPEED_SERVERS {
            if let Ok(resp) = client.get(format!("{}1", base)).timeout(Duration::from_secs(3)).send().await {
                if resp.status().is_success() {
                    ok = true;
                    break;
                }
            }
        }
        if ok {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let mut times = Vec::new();
    for n in 0..PING_COUNT {
        phase(tx, i, format!("Measuring ping {}/{}", n + 1, PING_COUNT));
        let t = Instant::now();
        if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(PING_TARGET)).await {
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        sleep(Duration::from_millis(200)).await;
    }
    r.ping_lost = PING_COUNT - times.len();
    r.ping_ms = (!times.is_empty()).then(|| times.iter().sum::<f64>() / times.len() as f64);

    if cancel.load(Ordering::Relaxed) {
        r.error = Some("cancelled".into());
        return r;
    }
    match download_speed(i, cancel, tx).await {
        Ok(v) => r.speed_mbps = Some(v),
        Err(e) => r.error = Some(format!("speed test failed: {}", e)),
    }
    r
}

fn mbps(bytes: u64, secs: f64) -> f64 {
    bytes as f64 * 8.0 / secs.max(0.001) / 1_000_000.0
}

/// Tries each server in SPEED_SERVERS until one gives a result.
async fn download_speed(
    i: usize,
    cancel: &AtomicBool,
    tx: &UnboundedSender<Response>,
) -> Result<f64, Box<dyn Error>> {
    let mut errors = Vec::new();
    for (name, base) in SPEED_SERVERS {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        match download_from(name, &format!("{}{}", base, SPEED_BYTES), i, cancel, tx).await {
            Ok(v) => return Ok(v),
            Err(e) => errors.push(format!("{}: {}", name, e)),
        }
    }
    Err(errors.join("; ").into())
}

/// Streams a large file for SPEED_TEST_SECS and reports the average throughput.
async fn download_from(
    name: &str,
    url: &str,
    i: usize,
    cancel: &AtomicBool,
    tx: &UnboundedSender<Response>,
) -> Result<f64, Box<dyn Error>> {
    phase(tx, i, format!("Testing download speed ({})…", name));
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(SPEED_TEST_SECS as u64 + 20))
        .build()?;
    let mut resp = client.get(url).send().await?.error_for_status()?;

    let start = Instant::now();
    let mut last_report = start;
    let mut bytes = 0u64;
    while let Some(chunk) = resp.chunk().await? {
        bytes += chunk.len() as u64;
        let secs = start.elapsed().as_secs_f64();
        if last_report.elapsed() >= Duration::from_millis(400) {
            phase(tx, i, format!("Testing download speed ({})… {:.1} Mbps", name, mbps(bytes, secs)));
            last_report = Instant::now();
        }
        if secs >= SPEED_TEST_SECS || cancel.load(Ordering::Relaxed) {
            break;
        }
    }
    if bytes == 0 {
        return Err("no data received".into());
    }
    Ok(mbps(bytes, start.elapsed().as_secs_f64()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(earfcn: &str, speed: Option<f64>, ping: Option<f64>, rsrp: f64) -> BandResult {
        BandResult { earfcn: earfcn.into(), speed_mbps: speed, ping_ms: ping, rsrp: Some(rsrp), ..Default::default() }
    }

    #[test]
    fn picks_best_per_category_and_skips_failures() {
        let mut s = ScanState::default();
        s.start(vec!["a".into(), "b".into(), "c".into()]);
        s.apply(ScanEvent::Done(0, res("a", Some(50.0), Some(40.0), -100.0)));
        s.apply(ScanEvent::Done(1, res("b", Some(80.0), Some(60.0), -90.0)));
        s.apply(ScanEvent::Done(2, res("c", None, Some(20.0), -80.0)));
        s.apply(ScanEvent::Finished("done".into()));

        assert!(!s.running);
        assert_eq!(s.done_count(), 3);
        assert_eq!(s.best_speed().unwrap().earfcn, "b");
        assert_eq!(s.best_ping().unwrap().earfcn, "c");
        assert_eq!(s.best_signal().unwrap().earfcn, "c");
    }
}
