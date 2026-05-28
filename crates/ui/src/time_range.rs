use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TimeRange {
    #[default]
    Last30m,
    Last5m,
    Last1h,
    Last6h,
    Last24h,
    Last7d,
    All,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct TimeRangeQuery {
    pub(crate) range: Option<String>,
}

impl TimeRange {
    pub(crate) fn from_query(q: &TimeRangeQuery) -> Self {
        Self::from_param(q.range.as_deref())
    }

    pub(crate) fn from_param(range: Option<&str>) -> Self {
        match range.unwrap_or("30m").trim().to_ascii_lowercase().as_str() {
            "5m" | "last5m" => Self::Last5m,
            "30m" | "last30m" => Self::Last30m,
            "1h" | "last1h" => Self::Last1h,
            "6h" | "last6h" => Self::Last6h,
            "24h" | "1d" | "last24h" | "last1d" => Self::Last24h,
            "7d" | "last7d" => Self::Last7d,
            "all" | "all_time" | "alltime" => Self::All,
            _ => Self::Last30m,
        }
    }

    pub(crate) fn as_param_value(&self) -> &'static str {
        match self {
            TimeRange::Last5m => "5m",
            TimeRange::Last30m => "30m",
            TimeRange::Last1h => "1h",
            TimeRange::Last6h => "6h",
            TimeRange::Last24h => "24h",
            TimeRange::Last7d => "7d",
            TimeRange::All => "all",
        }
    }

    pub(crate) fn seconds(&self) -> Option<u64> {
        match self {
            TimeRange::Last5m => Some(5 * 60),
            TimeRange::Last30m => Some(30 * 60),
            TimeRange::Last1h => Some(60 * 60),
            TimeRange::Last6h => Some(6 * 60 * 60),
            TimeRange::Last24h => Some(24 * 60 * 60),
            TimeRange::Last7d => Some(7 * 24 * 60 * 60),
            TimeRange::All => None,
        }
    }

    pub(crate) fn promql_range(&self) -> Option<&'static str> {
        match self {
            TimeRange::All => None,
            _ => Some(self.as_param_value()),
        }
    }

    /// Used to keep "last known" values from going stale in instant queries.
    ///
    /// Prometheus uses a 5m lookback by default. We override it so "all time"
    /// doesn't fall back to 0 when a series becomes stale.
    pub(crate) fn prom_lookback_delta(&self) -> &'static str {
        match self {
            TimeRange::All => "365d",
            _ => self.as_param_value(),
        }
    }

    pub(crate) fn unix_now_seconds() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return 0;
        };
        dur.as_secs()
    }
}

