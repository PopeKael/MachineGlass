use serde::{Deserialize, Serialize};

/// Missing, unsupported, warming up, and measured zero are distinct states.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Reading<T> {
    Value(T),
    WarmingUp,
    Unavailable(String),
    Unsupported(String),
}

impl<T> Reading<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }
    pub fn reason(&self) -> &str {
        match self {
            Self::Value(_) => "Measured",
            Self::WarmingUp => "Waiting for second sample",
            Self::Unavailable(s) | Self::Unsupported(s) => s,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessIdentity {
    pub pid: u32,
    /// Windows creation time in 100 ns ticks since 1601; not just a recycled PID.
    pub creation_ticks: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Process {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub identity: Option<ProcessIdentity>,
    pub path: Reading<String>,
    pub cpu_percent: Reading<f64>,
    pub working_set_bytes: Reading<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory {
    pub total_bytes: u64,
    pub available_bytes: u64,
}
impl Memory {
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }
    pub fn percent(&self) -> f64 {
        self.used_bytes() as f64 / self.total_bytes.max(1) as f64 * 100.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub unix_ms: u64,
    pub uptime_seconds: u64,
    pub logical_processors: u32,
    pub cpu_percent: Reading<f64>,
    pub memory: Reading<Memory>,
    pub processes: Reading<Vec<Process>>,
    pub collection_ms: f64,
}

#[derive(Clone, Debug)]
pub struct HistoryPoint {
    pub unix_ms: u64,
    pub cpu_percent: Option<f64>,
    pub memory_percent: Option<f64>,
}
impl From<&Snapshot> for HistoryPoint {
    fn from(s: &Snapshot) -> Self {
        Self {
            unix_ms: s.unix_ms,
            cpu_percent: s.cpu_percent.value().copied(),
            memory_percent: s.memory.value().map(Memory::percent),
        }
    }
}
