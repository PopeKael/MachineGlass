use crate::{
    config::{Config, log},
    model::{HistoryPoint, Snapshot},
    provider::{TelemetryProvider, WindowsProvider},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct State {
    pub latest: Option<Arc<Snapshot>>,
    pub history: VecDeque<HistoryPoint>,
    pub last_arrival: Option<Instant>,
    pub error: Option<String>,
}
impl State {
    fn record(&mut self, snapshot: Snapshot, config: &Config) {
        self.history.push_back(HistoryPoint::from(&snapshot));
        while self.history.len() > config.history_capacity() {
            self.history.pop_front();
        }
        let cutoff = snapshot
            .unix_ms
            .saturating_sub(config.history_seconds * 1000);
        while self.history.front().is_some_and(|p| p.unix_ms < cutoff) {
            self.history.pop_front();
        }
        self.latest = Some(Arc::new(snapshot));
        self.last_arrival = Some(Instant::now());
    }
}
pub struct Engine {
    pub state: Arc<Mutex<State>>,
    stop: mpsc::Sender<()>,
    worker: Option<JoinHandle<()>>,
}
impl Engine {
    pub fn start(config: Config, ctx: eframe::egui::Context) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        let (stop, receive) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("windows-telemetry".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut provider = WindowsProvider::default();
                    let mut last_errors = String::new();
                    loop {
                        let began = Instant::now();
                        let snapshot = provider.sample();
                        let errors = [
                            ("CPU", snapshot.cpu_percent.reason()),
                            ("Memory", snapshot.memory.reason()),
                            ("Processes", snapshot.processes.reason()),
                        ]
                        .into_iter()
                        .filter(|(_, reason)| {
                            *reason != "Measured" && *reason != "Waiting for second sample"
                        })
                        .map(|(name, reason)| format!("{name}: {reason}"))
                        .collect::<Vec<_>>()
                        .join("; ");
                        if errors != last_errors {
                            if errors.is_empty() {
                                log("Core providers recovered");
                            } else {
                                log(&format!("Core provider status: {errors}"));
                            }
                            last_errors = errors;
                        }
                        if let Ok(mut state) = shared.lock() {
                            state.record(snapshot, &config);
                        }
                        ctx.request_repaint();
                        let remaining = Duration::from_millis(config.sample_interval_ms)
                            .saturating_sub(began.elapsed());
                        match receive.recv_timeout(remaining) {
                            Err(mpsc::RecvTimeoutError::Timeout) => (),
                            _ => break,
                        }
                    }
                }));
                if result.is_err() {
                    log("Windows telemetry worker panicked; UI remains available");
                    if let Ok(mut state) = shared.lock() {
                        state.error = Some(
                            "Telemetry worker stopped. Restart MachineGlass; see internal log."
                                .into(),
                        );
                    }
                    ctx.request_repaint();
                }
            })
            .expect("Unable to start telemetry thread");
        Self {
            state,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        log("Clean shutdown");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Reading;
    fn sample(ms: u64, cpu: Reading<f64>) -> Snapshot {
        Snapshot {
            unix_ms: ms,
            uptime_seconds: 0,
            logical_processors: 1,
            cpu_percent: cpu,
            memory: Reading::Unavailable("test".into()),
            processes: Reading::Value(Vec::new()),
            collection_ms: 0.0,
        }
    }
    #[test]
    fn history_evicts_by_age_preserves_gaps_and_bounds_clock_reversal() {
        let config = Config::default();
        let mut state = State::default();
        for second in 0..=1900 {
            state.record(sample(second * 1000, Reading::Value(0.0)), &config);
        }
        assert_eq!(state.history.len(), 1801);
        assert_eq!(state.history.front().unwrap().unix_ms, 100_000);
        state.record(
            sample(1_901_000, Reading::Unavailable("provider failed".into())),
            &config,
        );
        assert_eq!(state.history.back().unwrap().cpu_percent, None);
        assert_eq!(state.history.back().unwrap().memory_percent, None);
        for _ in 0..10_000 {
            state.record(sample(1000, Reading::Value(1.0)), &config);
        }
        assert!(state.history.len() <= config.history_capacity());
        state.record(sample(10_000_000, Reading::Value(1.0)), &config);
        assert_eq!(state.history.len(), 1);
    }
}
