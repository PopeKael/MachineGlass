use crate::model::{Gpu, GpuEngine, Reading};
use std::collections::BTreeMap;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
};
use windows_sys::Win32::System::Performance::*;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn status(code: u32) -> String {
    format!("Windows GPU performance status 0x{code:08x}")
}

struct Query(usize);
impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.0 as _);
        }
    }
}
struct Counters {
    query: Query,
    engines: Result<usize, String>,
    dedicated: Result<usize, String>,
    shared: Result<usize, String>,
    primed: bool,
}
impl Counters {
    fn new() -> Result<Self, String> {
        let mut handle = std::ptr::null_mut();
        let code = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut handle) };
        if code != 0 {
            return Err(status(code));
        }
        let query = Query(handle as usize);
        let add = |path: &str| {
            let mut counter = std::ptr::null_mut();
            let code = unsafe {
                PdhAddEnglishCounterW(query.0 as _, wide(path).as_ptr(), 0, &mut counter)
            };
            if code == 0 {
                Ok(counter as usize)
            } else {
                Err(status(code))
            }
        };
        Ok(Self {
            engines: add(r"\GPU Engine(*)\Utilization Percentage"),
            dedicated: add(r"\GPU Adapter Memory(*)\Dedicated Usage"),
            shared: add(r"\GPU Adapter Memory(*)\Shared Usage"),
            query,
            primed: false,
        })
    }
}

/// Owned, aligned array storage: PDH writes structures followed by UTF-16 names.
/// All names/values are copied before releasing the buffer. Retry size races.
fn values(counter: &Result<usize, String>) -> Result<Vec<(String, Reading<f64>)>, String> {
    let counter = *counter.as_ref().map_err(Clone::clone)?;
    for _ in 0..3 {
        let (mut bytes, mut count) = (0, 0);
        let code = unsafe {
            PdhGetFormattedCounterArrayW(
                counter as _,
                PDH_FMT_DOUBLE,
                &mut bytes,
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if code == 0 && bytes == 0 {
            return Ok(Vec::new());
        }
        if code != PDH_MORE_DATA {
            return Err(status(code));
        }
        if bytes > 32 * 1024 * 1024 {
            return Err("GPU counter array exceeded safety bound".into());
        }
        let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
        let ptr = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        let code = unsafe {
            PdhGetFormattedCounterArrayW(counter as _, PDH_FMT_DOUBLE, &mut bytes, &mut count, ptr)
        };
        if code == PDH_MORE_DATA {
            continue;
        }
        if code != 0 {
            return Err(status(code));
        }
        let mut result = Vec::new();
        // SAFETY: successful PDH call populated count structures and NUL-terminated
        // names in the aligned allocation, which remains alive throughout copying.
        unsafe {
            for item in std::slice::from_raw_parts(ptr, count as usize) {
                let mut len = 0;
                while *item.szName.add(len) != 0 {
                    len += 1;
                }
                let name = String::from_utf16_lossy(std::slice::from_raw_parts(item.szName, len));
                let v = item.FmtValue.Anonymous.doubleValue;
                let reading = if item.FmtValue.CStatus <= 1 && v.is_finite() && v >= 0.0 {
                    Reading::Value(v)
                } else {
                    Reading::Unavailable(status(item.FmtValue.CStatus))
                };
                result.push((name, reading));
            }
        }
        return Ok(result);
    }
    Err("GPU instances changed during enumeration; retry next sample".into())
}

fn adapter_key(name: &str) -> Option<String> {
    let suffix = name.split_once("luid_")?.1;
    let words: Vec<_> = suffix.split('_').take(2).collect();
    if words.len() != 2 {
        return None;
    }
    Some(format!("luid_{}_{}", words[0], words[1]).to_lowercase())
}
fn engine_key(name: &str) -> Option<(String, String, String)> {
    let adapter = adapter_key(name)?;
    let physical = name.split_once("_phys_")?.1.split('_').next()?;
    let (number, label) = name.split_once("_eng_")?.1.split_once("_engtype_")?;
    Some((
        adapter,
        format!("phys_{physical}_eng_{number}"),
        label.to_string(),
    ))
}

/// Counters contain per-process slices. Sum slices for the SAME physical engine,
/// never across engines; then overall = max(engine). Invalid slices stay invalid.
fn aggregate(rows: &[(String, Reading<f64>)], adapter: &str) -> Vec<GpuEngine> {
    let mut engines: BTreeMap<String, GpuEngine> = BTreeMap::new();
    for (name, reading) in rows {
        let Some((key, id, label)) = engine_key(name) else {
            continue;
        };
        if key != adapter {
            continue;
        }
        let entry = engines.entry(id.clone()).or_insert_with(|| GpuEngine {
            id,
            label,
            percent: Reading::Value(0.0),
        });
        entry.percent = match (&entry.percent, reading) {
            (Reading::Value(a), Reading::Value(b)) => Reading::Value(a + b),
            (_, Reading::Value(_)) => entry.percent.clone(),
            (_, missing) => missing.clone(),
        };
    }
    for e in engines.values_mut() {
        if let Reading::Value(v) = &mut e.percent {
            *v = v.clamp(0.0, 100.0);
        }
    }
    engines.into_values().collect()
}
fn memory_reading(
    rows: &Result<Vec<(String, Reading<f64>)>, String>,
    adapter: &str,
) -> Reading<u64> {
    match rows {
        Err(e) => Reading::Unavailable(e.clone()),
        Ok(rows) => {
            let mut found = false;
            let mut sum = 0u64;
            for (name, value) in rows
                .iter()
                .filter(|(name, _)| adapter_key(name).as_deref() == Some(adapter))
            {
                let _ = name;
                found = true;
                match value {
                    Reading::Value(v) => sum = sum.saturating_add(*v as u64),
                    _ => return Reading::Unavailable(value.reason().into()),
                }
            }
            if found {
                Reading::Value(sum)
            } else {
                Reading::Unavailable("No adapter memory counter instance".into())
            }
        }
    }
}

#[derive(Default)]
pub struct WindowsGpuProvider {
    counters: Option<Counters>,
    retry: u32,
}
impl WindowsGpuProvider {
    pub fn sample(&mut self) -> Reading<Vec<Gpu>> {
        // A fresh DXGI factory discovers adapter removal/addition without stale handles.
        let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
            Ok(f) => f,
            Err(e) => return Reading::Unavailable(format!("DXGI: {e}")),
        };
        let mut adapters = Vec::new();
        let mut index = 0;
        loop {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(a) => a,
                Err(e) if e.code() == windows::Win32::Graphics::Dxgi::DXGI_ERROR_NOT_FOUND => break,
                Err(e) => return Reading::Unavailable(format!("DXGI enumeration: {e}")),
            };
            index += 1;
            let desc = match unsafe { adapter.GetDesc1() } {
                Ok(d) => d,
                Err(e) => return Reading::Unavailable(format!("DXGI descriptor: {e}")),
            };
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            adapters.push(desc);
        }
        if self.counters.is_none() && self.retry == 0 {
            self.counters = Counters::new().ok();
            self.retry = 30;
        }
        self.retry = self.retry.saturating_sub(1);
        let mut warm = false;
        let (engine_rows, dedicated, shared) = if let Some(c) = &mut self.counters {
            let code = unsafe { PdhCollectQueryData(c.query.0 as _) };
            if code == 0 {
                warm = !c.primed;
                c.primed = true;
                (values(&c.engines), values(&c.dedicated), values(&c.shared))
            } else {
                c.primed = false;
                let e = status(code);
                (Err(e.clone()), Err(e.clone()), Err(e))
            }
        } else {
            let e = "Windows GPU performance query unavailable".to_string();
            (Err(e.clone()), Err(e.clone()), Err(e))
        };
        // Retry unavailable counter registration periodically (driver/provider recovery).
        if self.retry == 0
            && self.counters.as_ref().is_some_and(|c| {
                !c.primed || c.engines.is_err() || c.dedicated.is_err() || c.shared.is_err()
            })
        {
            self.counters = None;
        }
        Reading::Value(
            adapters
                .into_iter()
                .map(|d| {
                    let id = format!(
                        "luid_0x{:08x}_0x{:08x}",
                        d.AdapterLuid.HighPart as u32, d.AdapterLuid.LowPart
                    );
                    let engines = match &engine_rows {
                        Ok(rows) => {
                            let mut engines = aggregate(rows, &id);
                            if warm {
                                for e in &mut engines {
                                    e.percent = Reading::WarmingUp;
                                }
                            }
                            if engines.is_empty() {
                                Reading::Unavailable("No WDDM engine instances exposed".into())
                            } else {
                                Reading::Value(engines)
                            }
                        }
                        Err(e) => Reading::Unavailable(e.clone()),
                    };
                    Gpu {
                        id: id.clone(),
                        name: String::from_utf16_lossy(
                            &d.Description[..d
                                .Description
                                .iter()
                                .position(|c| *c == 0)
                                .unwrap_or(d.Description.len())],
                        ),
                        engines,
                        dedicated_used_bytes: memory_reading(&dedicated, &id),
                        dedicated_total_bytes: Reading::Value(d.DedicatedVideoMemory as u64),
                        shared_used_bytes: memory_reading(&shared, &id),
                        shared_limit_bytes: Reading::Value(d.SharedSystemMemory as u64),
                        temperature_c: Reading::Unsupported(
                            "No reliable native temperature provider".into(),
                        ),
                    }
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combines_process_slices_but_never_engines_or_adapters() {
        let rows = vec![
            (
                "pid_1_luid_0x00000000_0x00000001_phys_0_eng_2_engtype_Compute_0".into(),
                Reading::Value(30.0),
            ),
            (
                "pid_2_luid_0x00000000_0x00000001_phys_0_eng_2_engtype_Compute_0".into(),
                Reading::Value(40.0),
            ),
            (
                "pid_1_luid_0x00000000_0x00000001_phys_0_eng_3_engtype_Compute_1".into(),
                Reading::Value(20.0),
            ),
            (
                "pid_1_luid_0x00000000_0x00000002_phys_0_eng_2_engtype_Compute_0".into(),
                Reading::Value(99.0),
            ),
        ];
        let engines = aggregate(&rows, "luid_0x00000000_0x00000001");
        assert_eq!(engines.len(), 2);
        assert_eq!(engines[0].percent.value(), Some(&70.0));
        assert_eq!(engines[0].label, "Compute_0");
        assert_eq!(
            crate::model::busiest_engine(&engines, |_| true)
                .unwrap()
                .percent
                .value(),
            Some(&70.0)
        );
    }
    #[test]
    fn invalid_slice_does_not_become_zero() {
        let rows = vec![(
            "pid_1_luid_0x00000000_0x00000001_phys_0_eng_0_engtype_3D".into(),
            Reading::Unavailable("reset".into()),
        )];
        assert!(
            aggregate(&rows, "luid_0x00000000_0x00000001")[0]
                .percent
                .value()
                .is_none()
        );
        assert!(engine_key("bad instance").is_none());
    }
}
