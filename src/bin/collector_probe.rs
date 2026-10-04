//! Developer-only console probe. Not included in customer distribution.
use machineglass::provider::{TelemetryProvider, WindowsProvider};
fn main() {
    let mut provider = WindowsProvider::default();
    provider.sample();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut snapshot = provider.sample();
    // Limit the export to our process; avoid dumping a user's complete inventory.
    if let machineglass::model::Reading::Value(rows) = &mut snapshot.processes {
        rows.retain(|p| p.pid == std::process::id());
    }
    println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
}
