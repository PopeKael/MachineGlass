use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sample_interval_ms: u64,
    pub history_seconds: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            sample_interval_ms: 1000,
            history_seconds: 1800,
        }
    }
}
impl Config {
    pub fn validate(mut self) -> Self {
        self.sample_interval_ms = self.sample_interval_ms.clamp(500, 10_000);
        self.history_seconds = self.history_seconds.clamp(1800, 7200);
        self
    }
    pub fn history_capacity(&self) -> usize {
        (self.history_seconds * 1000).div_ceil(self.sample_interval_ms) as usize + 1
    }
    pub fn load() -> Self {
        let path = data_dir().join("config.json");
        match fs::read_to_string(&path) {
            Ok(s) => match serde_json::from_str::<Self>(&s) {
                Ok(config) => config.validate(),
                Err(e) => {
                    log(&format!(
                        "Invalid config; defaults used, original preserved: {e}"
                    ));
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                if let Err(e) = fs::create_dir_all(data_dir())
                    .and_then(|_| fs::write(path, serde_json::to_string_pretty(&config).unwrap()))
                {
                    log(&format!("Cannot save default config: {e}"));
                }
                config
            }
            Err(e) => {
                log(&format!("Cannot read config: {e}"));
                Self::default()
            }
        }
    }
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("MachineGlass")
}
/// Small, bounded internal log; no inventory, executable paths or usage data.
pub fn log(message: &str) {
    use std::io::Write;
    let append = |path: &std::path::Path, message: &str| -> std::io::Result<()> {
        let dir = path.parent().expect("Log path has a parent");
        if !dir.is_dir() {
            fs::create_dir_all(dir)?;
        }
        if fs::metadata(path).is_ok_and(|m| m.len() > 1_048_576) {
            fs::rename(path, path.with_extension("previous.log"))?;
        }
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        writeln!(f, "{stamp} pid={} {message}", std::process::id())
    };
    if let Err(error) = append(&data_dir().join("machineglass.log"), message) {
        // A shortcut launch must remain diagnosable if the primary directory
        // or file cannot be written. Unique per-process fallback avoids locks
        // and permission inheritance from the primary log.
        let fallback =
            std::env::temp_dir().join(format!("MachineGlass-{}.log", std::process::id()));
        let _ = append(
            &fallback,
            &format!("Primary log failed: {error}; {message}"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_config_is_bounded_and_retains_thirty_minutes() {
        let c = Config {
            sample_interval_ms: 0,
            history_seconds: u64::MAX,
        }
        .validate();
        assert_eq!(c.sample_interval_ms, 500);
        assert_eq!(c.history_seconds, 7200);
        assert!(c.history_capacity() <= 14401);
        assert_eq!(Config::default().history_capacity(), 1801);
    }
}
