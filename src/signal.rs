use ratatui::style::Color;
use serde_json::Value;

#[derive(Clone, Copy)]
pub enum Metric {
    Rsrp,
    Rsrq,
    Sinr,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Grade {
    Excellent,
    Good,
    Fair,
    Poor,
}

impl Grade {
    pub fn label(self) -> &'static str {
        match self {
            Grade::Excellent => "Excellent",
            Grade::Good => "Good",
            Grade::Fair => "Fair",
            Grade::Poor => "Poor",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Grade::Excellent => Color::Green,
            Grade::Good => Color::LightGreen,
            Grade::Fair => Color::Yellow,
            Grade::Poor => Color::Red,
        }
    }
}

impl Metric {
    /// (worst, best, [excellent, good, fair] lower bounds) from common LTE/NR field guidance.
    fn spec(self) -> (f64, f64, [f64; 3]) {
        match self {
            Metric::Rsrp => (-140.0, -44.0, [-80.0, -90.0, -100.0]),
            Metric::Rsrq => (-20.0, -3.0, [-10.0, -15.0, -20.0]),
            Metric::Sinr => (-10.0, 30.0, [20.0, 13.0, 0.0]),
        }
    }

    /// Position of `v` between the metric's worst and best value, 0.0..=1.0.
    pub fn fraction(self, v: f64) -> f64 {
        let (lo, hi, _) = self.spec();
        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
    }

    pub fn grade(self, v: f64) -> Grade {
        let [ex, good, fair] = self.spec().2;
        if v >= ex {
            Grade::Excellent
        } else if v >= good {
            Grade::Good
        } else if v >= fair {
            Grade::Fair
        } else {
            Grade::Poor
        }
    }
}

/// Modem fields arrive as strings like "-95" or "-95 dBm"; take the leading number.
pub fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| {
        let s = v.as_str()?.trim();
        let end = s
            .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
            .unwrap_or(s.len());
        s[..end].parse().ok()
    })
}

/// Reads a signal field from modem data. Zitel firmware reports RSRP/RSRQ in tenths of a dB
/// (-1021 = -102.1 dBm), so values outside the physical range are rescaled; whole-dB firmwares pass through.
pub fn read(data: &Value, key: &str) -> Option<f64> {
    let v = num(&data[key])?;
    let floor = match key {
        "RSRP" => -150.0,
        "RSRQ" => -35.0,
        _ => return Some(v),
    };
    Some(if v < floor { v / 10.0 } else { v })
}

/// 0..=100 quality score averaged over whichever metrics are present.
pub fn score(rsrp: Option<f64>, rsrq: Option<f64>, sinr: Option<f64>) -> Option<f64> {
    let parts: Vec<f64> = [(Metric::Rsrp, rsrp), (Metric::Rsrq, rsrq), (Metric::Sinr, sinr)]
        .into_iter()
        .filter_map(|(m, v)| v.map(|v| m.fraction(v)))
        .collect();
    (!parts.is_empty()).then(|| parts.iter().sum::<f64>() / parts.len() as f64 * 100.0)
}

pub fn score_grade(score: f64) -> Grade {
    if score >= 65.0 {
        Grade::Excellent
    } else if score >= 50.0 {
        Grade::Good
    } else if score >= 30.0 {
        Grade::Fair
    } else {
        Grade::Poor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_grades_and_scores() {
        assert_eq!(num(&json!("-95 dBm")), Some(-95.0));
        assert_eq!(num(&json!(" 12.5")), Some(12.5));
        assert_eq!(num(&json!("")), None);
        let d = json!({"RSRP": "-1021", "RSRQ": "-117", "SINR": "16"});
        assert_eq!(read(&d, "RSRP"), Some(-102.1));
        assert_eq!(read(&d, "RSRQ"), Some(-11.7));
        assert_eq!(read(&d, "SINR"), Some(16.0));
        assert_eq!(read(&json!({"RSRP": "-95"}), "RSRP"), Some(-95.0));
        assert_eq!(Metric::Rsrp.grade(-85.0), Grade::Good);
        assert_eq!(Metric::Sinr.grade(-3.0), Grade::Poor);
        assert_eq!(Metric::Rsrp.fraction(-200.0), 0.0);
        assert_eq!(score(None, None, None), None);
        let strong = score(Some(-75.0), Some(-7.0), Some(25.0)).unwrap();
        let weak = score(Some(-110.0), Some(-18.0), Some(-2.0)).unwrap();
        assert_eq!(score_grade(strong), Grade::Excellent);
        assert_eq!(score_grade(weak), Grade::Poor);
    }
}
