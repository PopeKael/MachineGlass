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
    pub gpus: Reading<Vec<Gpu>>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GpuEngine {
    pub id: String,
    /// Exact engtype label exposed by Windows, never a vendor-specific mapping.
    pub label: String,
    pub percent: Reading<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gpu {
    pub id: String,
    pub name: String,
    pub engines: Reading<Vec<GpuEngine>>,
    pub dedicated_used_bytes: Reading<u64>,
    pub dedicated_total_bytes: Reading<u64>,
    pub shared_used_bytes: Reading<u64>,
    pub shared_limit_bytes: Reading<u64>,
    pub temperature_c: Reading<f64>,
}
pub fn busiest_engine(
    engines: &[GpuEngine],
    filter: impl Fn(&GpuEngine) -> bool,
) -> Option<&GpuEngine> {
    engines
        .iter()
        .filter(|e| filter(e) && e.percent.value().is_some())
        .max_by(|a, b| {
            a.percent
                .value()
                .unwrap()
                .total_cmp(b.percent.value().unwrap())
                .then_with(|| b.id.cmp(&a.id))
        })
}
impl Gpu {
    pub fn activity(&self, filter: impl Fn(&GpuEngine) -> bool) -> Reading<f64> {
        match &self.engines {
            Reading::Value(engines) => {
                let candidates: Vec<_> = engines.iter().filter(|e| filter(e)).collect();
                if candidates.is_empty() {
                    return Reading::Unsupported("Engine class not exposed".into());
                }
                if let Some(e) = candidates.iter().find(|e| e.percent.value().is_none()) {
                    return e.percent.clone();
                }
                busiest_engine(engines, filter)
                    .map(|e| e.percent.clone())
                    .unwrap_or_else(|| Reading::Unavailable("No engine readings".into()))
            }
            Reading::WarmingUp => Reading::WarmingUp,
            Reading::Unavailable(e) => Reading::Unavailable(e.clone()),
            Reading::Unsupported(e) => Reading::Unsupported(e.clone()),
        }
    }
}
pub fn compute_engine(e: &GpuEngine) -> bool {
    e.label.to_ascii_lowercase().contains("compute")
}
pub fn three_d_engine(e: &GpuEngine) -> bool {
    e.label.eq_ignore_ascii_case("3d")
}

#[derive(Clone, Debug)]
pub struct GpuHistoryPoint {
    pub unix_ms: u64,
    pub adapters: Vec<Gpu>,
}

#[cfg(test)]
mod gpu_model_tests {
    use super::*;
    #[test]
    fn busiest_compute_and_overall_preserve_missing_and_zero() {
        let engine = |id: &str, label: &str, percent| GpuEngine {
            id: id.into(),
            label: label.into(),
            percent,
        };
        let mut gpu = Gpu {
            id: "adapter".into(),
            name: "test".into(),
            engines: Reading::Value(vec![
                engine("1", "Compute 7", Reading::Value(25.0)),
                engine("2", "Compute 8", Reading::Value(60.0)),
                engine("3", "3D", Reading::Value(90.0)),
            ]),
            dedicated_used_bytes: Reading::Value(100),
            dedicated_total_bytes: Reading::Value(200),
            shared_used_bytes: Reading::Value(0),
            shared_limit_bytes: Reading::Value(200),
            temperature_c: Reading::Unsupported("test".into()),
        };
        assert_eq!(gpu.activity(compute_engine).value(), Some(&60.0));
        assert_eq!(gpu.activity(|_| true).value(), Some(&90.0));
        gpu.engines = Reading::Value(vec![engine("1", "Compute 7", Reading::Value(0.0))]);
        assert_eq!(gpu.activity(compute_engine).value(), Some(&0.0));
        assert!(matches!(
            gpu.activity(three_d_engine),
            Reading::Unsupported(_)
        ));
        gpu.engines = Reading::Value(vec![
            engine("1", "Compute 7", Reading::Value(60.0)),
            engine("2", "Compute 8", Reading::Unavailable("lost".into())),
        ]);
        assert!(gpu.activity(compute_engine).value().is_none());
        assert!(gpu.activity(|_| true).value().is_none());
    }
}
