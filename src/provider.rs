//! Direct user-mode Windows collector. Every acquired handle is RAII-owned.
use crate::model::*;
use std::{
    collections::HashMap,
    mem::{size_of, zeroed},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_NO_MORE_FILES, FILETIME, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        SystemInformation::{GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::{
            GetActiveProcessorCount, GetActiveProcessorGroupCount, GetProcessTimes, GetSystemTimes,
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
    },
};

pub trait TelemetryProvider: Send {
    fn sample(&mut self) -> Snapshot;
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: only successfully acquired handles are wrapped; ownership is unique.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn ticks(t: FILETIME) -> u64 {
    ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64
}
fn error() -> String {
    std::io::Error::last_os_error().to_string()
}

pub fn cpu_delta(previous: (u64, u64), current: (u64, u64)) -> Option<f64> {
    let total = current.1.checked_sub(previous.1)?;
    let idle = current.0.checked_sub(previous.0)?;
    if total == 0 || idle > total {
        return None;
    }
    Some((total - idle) as f64 / total as f64 * 100.0)
}

#[derive(Default)]
pub struct WindowsProvider {
    gpu: crate::gpu::WindowsGpuProvider,
    previous_cpu: Option<(u64, u64)>,
    previous_processes: HashMap<ProcessIdentity, (u64, Instant)>,
}

impl WindowsProvider {
    fn cpu(&mut self) -> Reading<f64> {
        // GetSystemTimes only covers the caller's processor group on >64 CPUs.
        // SAFETY: these Windows APIs require no pointer arguments.
        if unsafe { GetActiveProcessorGroupCount() } > 1 {
            return Reading::Unsupported(
                "Multiple processor groups require a future collector".into(),
            );
        }
        // SAFETY: initialized FILETIME buffers are valid writable output pointers.
        unsafe {
            let (mut idle, mut kernel, mut user): (FILETIME, FILETIME, FILETIME) =
                (zeroed(), zeroed(), zeroed());
            if GetSystemTimes(&mut idle, &mut kernel, &mut user) == 0 {
                self.previous_cpu = None;
                return Reading::Unavailable(error());
            }
            let current = (ticks(idle), ticks(kernel) + ticks(user));
            let previous = self.previous_cpu.replace(current);
            match previous {
                None => Reading::WarmingUp,
                Some(p) => cpu_delta(p, current)
                    .map(Reading::Value)
                    .unwrap_or_else(|| {
                        Reading::Unavailable("CPU counter reset or no elapsed ticks".into())
                    }),
            }
        }
    }

    fn memory(&self) -> Reading<Memory> {
        // SAFETY: the structure is sized and initialized as required by Windows.
        unsafe {
            let mut m: MEMORYSTATUSEX = zeroed();
            m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut m) == 0 {
                return Reading::Unavailable(error());
            }
            Reading::Value(Memory {
                total_bytes: m.ullTotalPhys,
                available_bytes: m.ullAvailPhys,
            })
        }
    }

    fn processes(&mut self, logical_processors: u32) -> Reading<Vec<Process>> {
        // SAFETY: snapshot, structures, and buffers follow the Toolhelp/PSAPI contracts.
        // Process disappearance/access denial is represented as unavailable per field.
        unsafe {
            let raw = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if raw == INVALID_HANDLE_VALUE {
                self.previous_processes.clear();
                return Reading::Unavailable(error());
            }
            let snapshot = OwnedHandle(raw);
            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot.0, &mut entry) == 0 {
                self.previous_processes.clear();
                return Reading::Unavailable(error());
            }
            let mut rows = Vec::new();
            let mut next = HashMap::new();
            loop {
                let name_len = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let mut row = Process {
                    pid: entry.th32ProcessID,
                    parent_pid: entry.th32ParentProcessID,
                    name: String::from_utf16_lossy(&entry.szExeFile[..name_len]),
                    identity: None,
                    path: Reading::Unavailable("Not accessible".into()),
                    cpu_percent: Reading::Unavailable("Not accessible".into()),
                    working_set_bytes: Reading::Unavailable("Not accessible".into()),
                };
                let raw_process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, row.pid);
                if !raw_process.is_null() {
                    let handle = OwnedHandle(raw_process);
                    let mut path = vec![0u16; 32768];
                    let mut len = path.len() as u32;
                    if QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut len) != 0 {
                        row.path = Reading::Value(String::from_utf16_lossy(&path[..len as usize]));
                    } else {
                        row.path = Reading::Unavailable(error());
                    }
                    let (mut created, mut exited, mut kernel, mut user): (
                        FILETIME,
                        FILETIME,
                        FILETIME,
                        FILETIME,
                    ) = (zeroed(), zeroed(), zeroed(), zeroed());
                    if GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user)
                        != 0
                    {
                        let id = ProcessIdentity {
                            pid: row.pid,
                            creation_ticks: ticks(created),
                        };
                        row.identity = Some(id);
                        let total = ticks(kernel) + ticks(user);
                        let now = Instant::now();
                        row.cpu_percent = match self.previous_processes.get(&id) {
                            Some((old, then)) => match total.checked_sub(*old) {
                                Some(delta) if now.duration_since(*then).as_secs_f64() > 0.0 => {
                                    Reading::Value(
                                        (delta as f64
                                            / 10_000_000.0
                                            / now.duration_since(*then).as_secs_f64()
                                            / logical_processors.max(1) as f64
                                            * 100.0)
                                            .clamp(0.0, 100.0),
                                    )
                                }
                                _ => Reading::Unavailable("Process CPU counter reset".into()),
                            },
                            None => Reading::WarmingUp,
                        };
                        next.insert(id, (total, now));
                    } else {
                        row.cpu_percent = Reading::Unavailable(error());
                    }
                    let mut counters: PROCESS_MEMORY_COUNTERS = zeroed();
                    counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
                    if K32GetProcessMemoryInfo(
                        handle.0,
                        &mut counters,
                        size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                    ) != 0
                    {
                        row.working_set_bytes = Reading::Value(counters.WorkingSetSize as u64);
                    } else {
                        row.working_set_bytes = Reading::Unavailable(error());
                    }
                } else {
                    let reason = error();
                    row.path = Reading::Unavailable(reason.clone());
                    row.cpu_percent = Reading::Unavailable(reason.clone());
                    row.working_set_bytes = Reading::Unavailable(reason);
                }
                rows.push(row);
                if Process32NextW(snapshot.0, &mut entry) == 0 {
                    if windows_sys::Win32::Foundation::GetLastError() != ERROR_NO_MORE_FILES {
                        self.previous_processes.clear();
                        return Reading::Unavailable(error());
                    }
                    break;
                }
            }
            self.previous_processes = next;
            Reading::Value(rows)
        }
    }
}

impl TelemetryProvider for WindowsProvider {
    fn sample(&mut self) -> Snapshot {
        let began = Instant::now();
        // SAFETY: no pointers; ALL_PROCESSOR_GROUPS is the documented 0xffff value.
        let logical_processors = unsafe { GetActiveProcessorCount(0xffff) };
        let cpu_percent = self.cpu();
        let memory = self.memory();
        let processes = self.processes(logical_processors);
        let gpus = self.gpu.sample();
        Snapshot {
            unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            uptime_seconds: unsafe { GetTickCount64() } / 1000,
            logical_processors,
            cpu_percent,
            memory,
            processes,
            gpus,
            collection_ms: began.elapsed().as_secs_f64() * 1000.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kernel_includes_idle_and_counter_reset_is_not_zero() {
        assert_eq!(cpu_delta((10, 100), (40, 200)), Some(70.0));
        assert_eq!(cpu_delta((10, 100), (110, 200)), Some(0.0));
        assert_eq!(cpu_delta((10, 100), (1, 5)), None);
        assert_eq!(cpu_delta((10, 100), (10, 100)), None);
    }
    #[test]
    fn pid_reuse_does_not_reuse_cpu_baseline() {
        let old = ProcessIdentity {
            pid: 42,
            creation_ticks: 100,
        };
        let new = ProcessIdentity {
            pid: 42,
            creation_ticks: 200,
        };
        let map = HashMap::from([(old, 50)]);
        assert!(!map.contains_key(&new));
    }
}
