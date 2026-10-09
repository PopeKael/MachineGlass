//! Developer-only console probe. Not included in customer distribution.
use machineglass::provider::{TelemetryProvider, WindowsProvider};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let count = args
        .windows(2)
        .find(|w| w[0] == "--samples")
        .and_then(|w| w[1].parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, 600);
    let mut provider = WindowsProvider::default();
    provider.sample();
    for _ in 0..count {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut snapshot = provider.sample();
        // Limit export to this probe; never dump the complete process inventory.
        if let machineglass::model::Reading::Value(rows) = &mut snapshot.processes {
            rows.retain(|p| p.pid == std::process::id());
        }
        if count == 1 {
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        } else {
            println!("{}", serde_json::to_string(&snapshot).unwrap());
        }
    }
}
