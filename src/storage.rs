//! Progressive, read-only filesystem inventory. This module owns acquisition
//! and the normalized tree; the UI consumes events and never performs I/O.

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug)]
pub struct Drive {
    pub root: PathBuf,
    pub label: String,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    pub volume_id: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub parent: PathBuf,
    pub name: String,
    pub is_dir: bool,
    /// File length, or cumulative discovered bytes for a directory.
    pub size: u64,
    pub aggregate_count: u64,
    pub aggregate: bool,
    pub unmapped: bool,
}

#[derive(Clone, Debug)]
pub enum ScanEvent {
    Cached(Vec<Entry>),
    Entries(Vec<Entry>),
    DirectoryFinished(PathBuf),
    Inaccessible(ScanIssue),
    Failed(String),
    Finished {
        cancelled: bool,
        changed: Option<bool>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanIssue {
    pub path: PathBuf,
    pub operation: String,
    pub reason: String,
}

pub fn local_drives() -> Vec<Drive> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{
            GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
        };
        const DRIVE_REMOVABLE: u32 = 2;
        const DRIVE_FIXED: u32 = 3;
        let mask = unsafe { GetLogicalDrives() };
        let mut drives = Vec::new();
        for index in 0..26 {
            if mask & (1 << index) == 0 {
                continue;
            }
            let letter = (b'A' + index as u8) as char;
            let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain([0]).collect();
            let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
            if kind == DRIVE_FIXED || kind == DRIVE_REMOVABLE {
                let (mut total, mut free) = (0u64, 0u64);
                let has_capacity = unsafe {
                    GetDiskFreeSpaceExW(root.as_ptr(), std::ptr::null_mut(), &mut total, &mut free)
                        != 0
                };
                let mut volume_id = 0u32;
                let has_volume_id = unsafe {
                    GetVolumeInformationW(
                        root.as_ptr(),
                        std::ptr::null_mut(),
                        0,
                        &mut volume_id,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        0,
                    ) != 0
                };
                drives.push(Drive {
                    root: PathBuf::from(format!("{letter}:\\")),
                    label: format!("{letter}:\\"),
                    total_bytes: has_capacity.then_some(total),
                    free_bytes: has_capacity.then_some(free),
                    volume_id: has_volume_id.then_some(volume_id),
                });
            }
        }
        drives
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub struct ScanHandle {
    pub receiver: Receiver<ScanEvent>,
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ScanHandle {
    pub fn start(drive: Drive) -> Self {
        let (sender, receiver) = mpsc::sync_channel(32);
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = cancel.clone();
        let failure_sender = sender.clone();
        let worker = thread::Builder::new()
            .name("machineglass-storage-scan".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                let (cache_path, cached) = load_cache(&drive);
                let cached_fingerprint = (!cached.is_empty()).then(|| entries_fingerprint(&cached));
                crate::config::log(&format!(
                    "Storage scan starting drive={} volume_id={:?} total_bytes={:?} cached_entries={} cache={}",
                    drive.label,
                    drive.volume_id,
                    drive.total_bytes,
                    cached.len(),
                    cache_path.display()
                ));
                for chunk in cached.chunks(256) {
                    if !send(&sender, ScanEvent::Cached(chunk.to_vec()), &thread_cancel) {
                        let _ = sender.try_send(ScanEvent::Finished {
                            cancelled: true,
                            changed: None,
                        });
                        return;
                    }
                }
                scan(
                    drive,
                    cache_path,
                    cached,
                    cached_fingerprint,
                    started,
                    sender,
                    thread_cancel,
                )
            })
            .map(Some)
            .unwrap_or_else(|error| {
                let _ = failure_sender.send(ScanEvent::Failed(error.to_string()));
                None
            });
        Self {
            receiver,
            cancel,
            worker,
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Drop for ScanHandle {
    fn drop(&mut self) {
        self.cancel();
        // Do not join on the UI thread: a filesystem call may be blocked by a
        // removable device or a filter driver. The bounded sender observes cancel.
        self.worker.take();
    }
}

/// Normalized presentation model produced from the scanner's progressive events.
#[derive(Default)]
pub struct StorageTree {
    entries: HashMap<PathBuf, Entry>,
    children: HashMap<PathBuf, Vec<PathBuf>>,
    cached_by_parent: HashMap<PathBuf, HashSet<PathBuf>>,
    cache_loaded: bool,
    inaccessible_count: usize,
    issues: Vec<ScanIssue>,
    finished: Option<bool>,
    changed: Option<bool>,
    total_bytes: Option<u64>,
    free_bytes: Option<u64>,
    error: Option<String>,
}

impl StorageTree {
    pub fn entries(&self) -> &HashMap<PathBuf, Entry> {
        &self.entries
    }
    pub fn children(&self) -> &HashMap<PathBuf, Vec<PathBuf>> {
        &self.children
    }
    pub fn has_cached_snapshot(&self) -> bool {
        self.cache_loaded && !self.entries.is_empty()
    }
    pub fn entry(&self, path: &Path) -> Option<&Entry> {
        self.entries.get(path)
    }
    pub fn discovered_count(&self) -> usize {
        self.entries.len()
    }
    pub fn inaccessible_count(&self) -> usize {
        self.inaccessible_count
    }
    pub fn inaccessible_issues(&self) -> &[ScanIssue] {
        &self.issues
    }
    /// None while scanning, Some(false) on completion, Some(true) if cancelled.
    pub fn finished(&self) -> Option<bool> {
        self.finished
    }
    pub fn changed(&self) -> Option<bool> {
        self.changed
    }
    pub fn total_bytes(&self) -> Option<u64> {
        self.total_bytes
    }
    pub fn free_bytes(&self) -> Option<u64> {
        self.free_bytes
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn is_scanning(&self) -> bool {
        self.finished.is_none() && self.error.is_none()
    }
}

/// Owns acquisition and applies worker messages to the normalized tree. The UI
/// only polls this session and renders `tree()`; it does not handle scan events.
pub struct StorageSession {
    scan: ScanHandle,
    tree: StorageTree,
}

impl StorageSession {
    pub fn start(drive: &Drive) -> Self {
        Self {
            scan: ScanHandle::start(drive.clone()),
            tree: StorageTree {
                total_bytes: drive.total_bytes,
                free_bytes: drive.free_bytes,
                ..StorageTree::default()
            },
        }
    }

    pub fn poll(&mut self) {
        let mut entries_applied = 0usize;
        while entries_applied < 8192 {
            let Ok(event) = self.scan.receiver.try_recv() else {
                break;
            };
            match event {
                ScanEvent::Cached(entries) => {
                    self.tree.cache_loaded = true;
                    for entry in entries {
                        self.tree.insert(entry, true);
                        entries_applied += 1;
                    }
                }
                ScanEvent::Entries(entries) => {
                    for entry in entries {
                        self.tree.insert(entry, false);
                        entries_applied += 1;
                    }
                }
                ScanEvent::DirectoryFinished(parent) => {
                    let stale = self
                        .tree
                        .cached_by_parent
                        .remove(&parent)
                        .unwrap_or_default();
                    for path in stale {
                        self.tree.remove_subtree(&path);
                    }
                }
                ScanEvent::Inaccessible(issue) => {
                    if !self.tree.issues.contains(&issue) {
                        self.tree.issues.push(issue);
                        self.tree.inaccessible_count = self.tree.issues.len();
                    }
                }
                ScanEvent::Failed(error) => self.tree.error = Some(error),
                ScanEvent::Finished { cancelled, changed } => {
                    self.tree.finished = Some(cancelled);
                    self.tree.changed = changed;
                }
            }
        }
    }

    pub fn tree(&self) -> &StorageTree {
        &self.tree
    }
    pub fn cancel(&self) {
        self.scan.cancel();
    }
}

impl StorageTree {
    fn insert(&mut self, entry: Entry, cached: bool) {
        let path = entry.path.clone();
        let parent = entry.parent.clone();
        if !self.entries.contains_key(&path) && path != parent {
            self.children
                .entry(parent.clone())
                .or_default()
                .push(path.clone());
        }
        self.entries.insert(path.clone(), entry);
        if cached && path != parent {
            self.cached_by_parent
                .entry(parent)
                .or_default()
                .insert(path);
        } else if let Some(cached) = self.cached_by_parent.get_mut(&parent) {
            cached.remove(&path);
            if cached.is_empty() {
                self.cached_by_parent.remove(&parent);
            }
        }
    }

    fn remove_subtree(&mut self, path: &Path) {
        let mut pending = vec![path.to_path_buf()];
        while let Some(current) = pending.pop() {
            if let Some(children) = self.children.remove(&current) {
                pending.extend(children);
            }
            if let Some(entry) = self.entries.remove(&current) {
                if let Some(siblings) = self.children.get_mut(&entry.parent) {
                    siblings.retain(|child| child != &current);
                }
                if let Some(cached) = self.cached_by_parent.get_mut(&entry.parent) {
                    cached.remove(&current);
                }
            }
            self.cached_by_parent.remove(&current);
        }
    }
}

fn send(sender: &SyncSender<ScanEvent>, event: ScanEvent, cancel: &AtomicBool) -> bool {
    let mut event = event;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        match sender.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(returned)) => {
                event = returned;
                thread::park_timeout(std::time::Duration::from_millis(8));
            }
        }
    }
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[derive(Deserialize, Serialize)]
struct StorageCache {
    version: u32,
    root: String,
    volume_id: Option<u32>,
    total_bytes: Option<u64>,
    entries: Vec<Entry>,
}

fn cache_path(drive: &Drive) -> PathBuf {
    let letter = drive
        .label
        .chars()
        .next()
        .unwrap_or('_')
        .to_ascii_uppercase();
    crate::config::data_dir()
        .join("storage")
        .join(format!("{letter}.json"))
}

fn load_cache(drive: &Drive) -> (PathBuf, Vec<Entry>) {
    let path = cache_path(drive);
    let (entries, outcome) = load_cache_file_diagnostic(&path, drive);
    crate::config::log(&format!(
        "Storage cache drive={} path={} outcome={} entries={}",
        drive.label,
        path.display(),
        outcome,
        entries.len()
    ));
    (path, entries)
}

#[cfg(test)]
fn load_cache_file(path: &Path, drive: &Drive) -> Vec<Entry> {
    load_cache_file_diagnostic(path, drive).0
}

fn load_cache_file_diagnostic(path: &Path, drive: &Drive) -> (Vec<Entry>, String) {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), "not found".into());
        }
        Err(error) => return (Vec::new(), format!("open failed: {error}")),
    };
    // Corrupt or unexpectedly large cache files are ignored; the live scan is
    // still authoritative and will replace the cache when it completes.
    if file
        .metadata()
        .map_or(true, |meta| meta.len() > 64 * 1024 * 1024)
    {
        return (Vec::new(), "metadata failed or cache exceeds 64 MiB".into());
    }
    let cache = match serde_json::from_reader::<_, StorageCache>(BufReader::new(file)) {
        Ok(cache) => cache,
        Err(error) => return (Vec::new(), format!("invalid cache data: {error}")),
    };
    let identity_matches = cache.version == 1
        && drive.volume_id.is_some()
        && cache.root.eq_ignore_ascii_case(&drive.label)
        && cache.volume_id == drive.volume_id
        && cache.total_bytes == drive.total_bytes;
    if identity_matches {
        let count = cache.entries.len();
        (cache.entries, format!("hit (identity matched, {count} entries)"))
    } else {
        (
            Vec::new(),
            format!(
                "identity mismatch (version={}, root_match={}, volume_match={}, capacity_match={})",
                cache.version == 1,
                cache.root.eq_ignore_ascii_case(&drive.label),
                drive.volume_id.is_some() && cache.volume_id == drive.volume_id,
                cache.total_bytes == drive.total_bytes
            ),
        )
    }
}

fn entries_fingerprint(entries: &[Entry]) -> u64 {
    let mut ordered: Vec<_> = entries.iter().collect();
    ordered.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    let mut hasher = DefaultHasher::new();
    for entry in ordered {
        entry.hash(&mut hasher);
    }
    hasher.finish()
}

fn save_cache(path: &Path, drive: &Drive, entries: HashMap<PathBuf, Entry>) -> std::io::Result<()> {
    let parent = path.parent().expect("storage cache has a parent");
    fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer(
        &mut writer,
        &StorageCache {
            version: 1,
            root: drive.label.clone(),
            volume_id: drive.volume_id,
            total_bytes: drive.total_bytes,
            entries: entries.into_values().collect(),
        },
    )
    .map_err(std::io::Error::other)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    replace_file(&temp, path)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain([0]).collect();
    let destination: Vec<u16> = destination.as_os_str().encode_wide().chain([0]).collect();
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

fn entry_for_directory(path: &Path, size: u64) -> Entry {
    Entry {
        path: path.to_path_buf(),
        parent: path.parent().unwrap_or(path).to_path_buf(),
        name: path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        ),
        is_dir: true,
        size,
        aggregate_count: 0,
        aggregate: false,
        unmapped: false,
    }
}

fn inaccessible_issue(path: &Path, operation: &str, error: &std::io::Error) -> ScanEvent {
    let reason = error.to_string();
    crate::config::log(&format!(
        "Storage unreadable path={} operation={} reason={reason}",
        path.display(),
        operation
    ));
    ScanEvent::Inaccessible(ScanIssue {
        path: path.to_path_buf(),
        operation: operation.into(),
        reason,
    })
}

fn apply_directory_delta(
    directory: &Path,
    delta: i128,
    root: &Path,
    totals: &mut HashMap<PathBuf, u64>,
    records: &mut HashMap<PathBuf, Entry>,
    batch: &mut HashMap<PathBuf, Entry>,
) {
    if delta == 0 {
        return;
    }
    let mut ancestor = Some(directory.to_path_buf());
    while let Some(path) = ancestor {
        let cached_size = records.get(&path).map_or(0, |entry| entry.size);
        let total = totals.entry(path.clone()).or_insert(cached_size);
        *total = if delta > 0 {
            total.saturating_add(delta as u64)
        } else {
            total.saturating_sub(delta.unsigned_abs() as u64)
        };
        let entry = entry_for_directory(&path, *total);
        records.insert(path.clone(), entry.clone());
        batch.insert(path.clone(), entry);
        ancestor = if path == root {
            None
        } else {
            path.parent().map(Path::to_path_buf)
        };
    }
}

fn prune_cache_subtree(
    path: &Path,
    records: &mut HashMap<PathBuf, Entry>,
    children: &mut HashMap<PathBuf, HashSet<PathBuf>>,
) {
    let mut pending = vec![path.to_path_buf()];
    while let Some(current) = pending.pop() {
        if let Some(descendants) = children.remove(&current) {
            pending.extend(descendants);
        }
        if let Some(entry) = records.remove(&current)
            && let Some(siblings) = children.get_mut(&entry.parent)
        {
            siblings.remove(&current);
        }
    }
}

fn scan(
    drive: Drive,
    cache_path: PathBuf,
    cached: Vec<Entry>,
    cached_fingerprint: Option<u64>,
    started: std::time::Instant,
    sender: SyncSender<ScanEvent>,
    cancel: Arc<AtomicBool>,
) {
    let root = drive.root.clone();
    let mut cache_records: HashMap<PathBuf, Entry> = cached
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut cache_children: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
    for entry in cache_records.values() {
        if entry.path != entry.parent {
            cache_children
                .entry(entry.parent.clone())
                .or_default()
                .insert(entry.path.clone());
        }
    }
    let root_entry = entry_for_directory(
        &root,
        cache_records.get(&root).map_or(0, |entry| entry.size),
    );
    let mut batch = HashMap::<PathBuf, Entry>::new();
    batch.insert(root.clone(), root_entry.clone());
    cache_records.insert(root.clone(), root_entry);
    // Keep cached directory totals as the initial estimate. Each discovered
    // child applies only its size delta, so an unchanged refresh does not
    // collapse large boxes to zero while the worker descends through them.
    let mut directory_totals: HashMap<PathBuf, u64> = HashMap::new();
    let mut small_files: HashMap<PathBuf, (u64, u64)> = HashMap::new();
    let mut pending = vec![root.clone()];
    let mut work_since_flush = 0usize;
    let mut last_flush = std::time::Instant::now();
    let mut stopped = false;
    let mut issue_count = 0usize;

    while let Some(directory) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            stopped = true;
            break;
        }
        let read_dir = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                if !send(
                    &sender,
                    inaccessible_issue(&directory, "enumerate directory", &error),
                    &cancel,
                ) {
                    stopped = true;
                    break;
                }
                issue_count += 1;
                crate::config::log(&format!(
                    "Storage scan retaining cached estimate for unreadable directory={}",
                    directory.display()
                ));
                continue;
            }
        };
        directory_totals.entry(directory.clone()).or_insert_with(|| {
            cache_records
                .get(&directory)
                .map_or(0, |entry| entry.size)
        });
        let mut seen_children = HashSet::new();
        let mut directory_complete = true;
        for item in read_dir {
            if cancel.load(Ordering::Relaxed) {
                stopped = true;
                break;
            }
            let item = match item {
                Ok(item) => item,
                Err(error) => {
                    directory_complete = false;
                    issue_count += 1;
                    let _ = send(
                        &sender,
                        inaccessible_issue(&directory, "read directory entry", &error),
                        &cancel,
                    );
                    continue;
                }
            };
            let path = item.path();
            let file_name = item.file_name();
            // On Windows, DirEntry::metadata reuses enumeration metadata and
            // avoids a separate path-based metadata lookup. It also preserves
            // the reparse-point entry rather than following its target.
            let metadata = match item.metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    let _ = send(
                        &sender,
                        inaccessible_issue(&path, "read entry metadata", &error),
                        &cancel,
                    );
                    issue_count += 1;
                    directory_complete = false;
                    seen_children.insert(path.clone());
                    if let Some(old) = cache_records.get(&path).cloned() {
                        batch.insert(path.clone(), old.clone());
                    }
                    continue;
                }
            };
            let is_dir = metadata.is_dir();
            let length = if is_dir { 0 } else { metadata.len() };
            let display_entry = if !is_dir && length < 512 * 1024 {
                let aggregate = small_files.entry(directory.clone()).or_default();
                aggregate.0 = aggregate.0.saturating_add(length);
                aggregate.1 += 1;
                Entry {
                    path: directory.join(".machineglass-small-files"),
                    parent: directory.clone(),
                    name: "Small files".into(),
                    is_dir: false,
                    size: aggregate.0,
                    aggregate_count: aggregate.1,
                    aggregate: true,
                    unmapped: false,
                }
            } else {
                Entry {
                    path: path.clone(),
                    parent: directory.clone(),
                    name: file_name.to_string_lossy().into_owned(),
                    is_dir,
                    size: if is_dir {
                        cache_records.get(&path).map_or(0, |entry| entry.size)
                    } else {
                        length
                    },
                    aggregate_count: 0,
                    aggregate: false,
                    unmapped: false,
                }
            };
            seen_children.insert(display_entry.path.clone());
            // Small-file totals are committed once the directory enumeration
            // completes. This avoids shrinking a cached aggregate to the
            // partial sum of files encountered so far.
            let is_small_file_group = display_entry.aggregate;
            if !is_small_file_group {
                let old_size = cache_records.get(&display_entry.path).map_or(0, |old| old.size);
                cache_records.insert(display_entry.path.clone(), display_entry.clone());
                if !is_dir {
                    apply_directory_delta(
                        &directory,
                        length as i128 - old_size as i128,
                        &root,
                        &mut directory_totals,
                        &mut cache_records,
                        &mut batch,
                    );
                }
            }
            if display_entry.path != display_entry.parent {
                cache_children
                    .entry(display_entry.parent.clone())
                    .or_default()
                    .insert(display_entry.path.clone());
            }
            if !is_small_file_group {
                batch.insert(display_entry.path.clone(), display_entry);
            }
            if is_dir && !is_reparse_point(&metadata) {
                pending.push(path);
            }
            work_since_flush += 1;
            if work_since_flush >= 128
                || last_flush.elapsed() >= std::time::Duration::from_millis(75)
            {
                if !flush_batch(&sender, &mut batch, &cancel) {
                    stopped = true;
                    break;
                }
                work_since_flush = 0;
                last_flush = std::time::Instant::now();
            }
        }
        if stopped {
            break;
        }
        if let Some((size, count)) = small_files.remove(&directory)
            && count > 0
        {
            let aggregate_path = directory.join(".machineglass-small-files");
            let old_size = cache_records
                .get(&aggregate_path)
                .map_or(0, |entry| entry.size);
            let aggregate = Entry {
                path: aggregate_path.clone(),
                parent: directory.clone(),
                name: "Small files".into(),
                is_dir: false,
                size,
                aggregate_count: count,
                aggregate: true,
                unmapped: false,
            };
            cache_records.insert(aggregate_path.clone(), aggregate.clone());
            seen_children.insert(aggregate_path.clone());
            cache_children
                .entry(directory.clone())
                .or_default()
                .insert(aggregate_path);
            batch.insert(aggregate.path.clone(), aggregate);
            apply_directory_delta(
                &directory,
                size as i128 - old_size as i128,
                &root,
                &mut directory_totals,
                &mut cache_records,
                &mut batch,
            );
        }
        let directory_entry = entry_for_directory(
            &directory,
            *directory_totals
                .entry(directory.clone())
                .or_insert_with(|| cache_records.get(&directory).map_or(0, |entry| entry.size)),
        );
        cache_records.insert(directory.clone(), directory_entry.clone());
        batch.insert(directory.clone(), directory_entry);
        if !flush_batch(&sender, &mut batch, &cancel) {
            stopped = true;
            break;
        }
        if directory_complete
            && !send(
                &sender,
                ScanEvent::DirectoryFinished(directory.clone()),
                &cancel,
            )
        {
            stopped = true;
            break;
        }
        if !directory_complete {
            continue;
        }
        if let Some(stale) = cache_children.get(&directory).cloned() {
            for stale_path in stale {
                if !seen_children.contains(&stale_path) {
                    let removed_size = cache_records.get(&stale_path).map_or(0, |entry| entry.size);
                    apply_directory_delta(
                        &directory,
                        -(removed_size as i128),
                        &root,
                        &mut directory_totals,
                        &mut cache_records,
                        &mut batch,
                    );
                    prune_cache_subtree(&stale_path, &mut cache_records, &mut cache_children);
                }
            }
        }
        cache_children.remove(&directory);
    }
    let cancelled = stopped || cancel.load(Ordering::Relaxed);
    // An incomplete pass cannot claim the map is unchanged: retained cache
    // data may conceal changes below an unreadable location.
    let changed = if cancelled || issue_count > 0 {
        None
    } else {
        cached_fingerprint.map(|previous| {
            let current: Vec<_> = cache_records.values().cloned().collect();
            previous != entries_fingerprint(&current)
        })
    };
    let final_entry_count = cache_records.len();
    if !cancelled {
        match save_cache(&cache_path, &drive, cache_records) {
            Ok(()) => crate::config::log(&format!(
                "Storage cache saved drive={} entries={} path={}",
                drive.label,
                final_entry_count,
                cache_path.display()
            )),
            Err(error) => crate::config::log(&format!(
                "Storage cache save failed drive={} path={} error={error}",
                drive.label,
                cache_path.display()
            )),
        }
    }
    crate::config::log(&format!(
        "Storage scan finished drive={} cancelled={} changed={changed:?} entries={} unreadable={} elapsed_ms={}",
        drive.label,
        cancelled,
        final_entry_count,
        issue_count,
        started.elapsed().as_millis()
    ));
    let _ = sender.send(ScanEvent::Finished { cancelled, changed });
}

fn flush_batch(
    sender: &SyncSender<ScanEvent>,
    batch: &mut HashMap<PathBuf, Entry>,
    cancel: &AtomicBool,
) -> bool {
    if batch.is_empty() {
        return true;
    }
    let entries = std::mem::take(batch).into_values().collect();
    send(sender, ScanEvent::Entries(entries), cancel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_reports_top_level_before_descending_and_finishes() {
        let root =
            std::env::temp_dir().join(format!("machineglass-storage-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("child")).unwrap();
        fs::File::create(root.join("first.bin"))
            .unwrap()
            .set_len(600_000)
            .unwrap();
        fs::File::create(root.join("child").join("later.bin"))
            .unwrap()
            .set_len(600_000)
            .unwrap();
        let (tx, rx) = mpsc::sync_channel(2048);
        scan(
            Drive {
                root: root.clone(),
                label: root.display().to_string(),
                total_bytes: Some(2_000_000),
                free_bytes: Some(800_000),
                volume_id: Some(1),
            },
            root.join("test-cache.json"),
            Vec::new(),
            None,
            std::time::Instant::now(),
            tx,
            Arc::new(AtomicBool::new(false)),
        );
        let events: Vec<_> = rx.try_iter().collect();
        let top_file = events
            .iter()
            .position(|e| matches!(e, ScanEvent::Entries(batch) if batch.iter().any(|v| v.path.ends_with("first.bin"))))
            .unwrap();
        let nested_file = events
            .iter()
            .position(|e| matches!(e, ScanEvent::Entries(batch) if batch.iter().any(|v| v.path.ends_with("later.bin"))))
            .unwrap();
        assert!(top_file < nested_file);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ScanEvent::Entries(batch) if batch.iter().any(|v| v.path == root && v.size == 1_200_000)))
        );
        assert!(matches!(
            events.last(),
            Some(ScanEvent::Finished { cancelled: false, .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_before_first_entry_is_reported() {
        let root = std::env::temp_dir().join(format!("machineglass-cancel-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let (tx, rx) = mpsc::sync_channel(8);
        let cancelled = Arc::new(AtomicBool::new(true));
        scan(
            Drive {
                root: root.clone(),
                label: root.display().to_string(),
                total_bytes: None,
                free_bytes: None,
                volume_id: None,
            },
            root.join("test-cache.json"),
            Vec::new(),
            None,
            std::time::Instant::now(),
            tx,
            cancelled,
        );
        assert!(matches!(
            rx.try_iter().last(),
            Some(ScanEvent::Finished { cancelled: true, .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cache_reconciliation_removes_stale_subtrees_and_keeps_live_items() {
        let root = PathBuf::from("F:\\");
        let old_dir = root.join("Old");
        let old_file = old_dir.join("gone.bin");
        let live_file = root.join("still-here.bin");
        let mut tree = StorageTree::default();
        tree.insert(entry_for_directory(&root, 900), true);
        tree.insert(entry_for_directory(&old_dir, 500), true);
        tree.insert(
            Entry {
                path: old_file.clone(),
                parent: old_dir.clone(),
                name: "gone.bin".into(),
                is_dir: false,
                size: 500,
                aggregate_count: 0,
                aggregate: false,
                unmapped: false,
            },
            true,
        );
        tree.insert(
            Entry {
                path: live_file.clone(),
                parent: root.clone(),
                name: "still-here.bin".into(),
                is_dir: false,
                size: 100,
                aggregate_count: 0,
                aggregate: false,
                unmapped: false,
            },
            false,
        );
        let stale = tree.cached_by_parent.remove(&root).unwrap();
        for path in stale {
            tree.remove_subtree(&path);
        }
        assert!(tree.entry(&old_dir).is_none());
        assert!(tree.entry(&old_file).is_none());
        assert!(tree.entry(&live_file).is_some());
    }

    #[test]
    fn saved_cache_round_trips_and_rejects_another_volume() {
        let temp = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("machineglass-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        let path = temp.join("volume.json");
        let drive = Drive {
            root: PathBuf::from("F:\\"),
            label: "F:\\".into(),
            total_bytes: Some(1000),
            free_bytes: Some(400),
            volume_id: Some(12),
        };
        let root_entry = entry_for_directory(&drive.root, 600);
        let mut entries = HashMap::new();
        entries.insert(root_entry.path.clone(), root_entry);
        save_cache(&path, &drive, entries).unwrap();

        let cached = load_cache_file(&path, &drive);
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].size, 600);
        let other_volume = Drive {
            volume_id: Some(13),
            ..drive
        };
        assert!(load_cache_file(&path, &other_volume).is_empty());
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn unchanged_cached_scan_preserves_sizes_and_reports_no_changes() {
        let root = std::env::temp_dir().join(format!("machineglass-repeat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("child")).unwrap();
        fs::write(root.join("small-a.txt"), b"small file A").unwrap();
        fs::write(root.join("child").join("small-b.txt"), b"small file B").unwrap();
        fs::File::create(root.join("large.bin"))
            .unwrap()
            .set_len(600_000)
            .unwrap();
        let drive = Drive {
            root: root.clone(),
            label: root.display().to_string(),
            total_bytes: Some(2_000_000),
            free_bytes: Some(800_000),
            volume_id: Some(1),
        };
        let cache = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("machineglass-repeat-cache-{}.json", std::process::id()));
        let (tx, rx) = mpsc::sync_channel(2048);
        scan(
            drive.clone(),
            cache.clone(),
            Vec::new(),
            None,
            std::time::Instant::now(),
            tx,
            Arc::new(AtomicBool::new(false)),
        );
        let _first_events: Vec<_> = rx.try_iter().collect();
        let (first, outcome) = load_cache_file_diagnostic(&cache, &drive);
        assert!(!first.is_empty(), "initial scan cache outcome: {outcome}");
        let expected_bytes = 600_024;
        assert_eq!(
            first.iter().find(|entry| entry.path == root).unwrap().size,
            expected_bytes
        );
        let first_fingerprint = entries_fingerprint(&first);

        let (tx, rx) = mpsc::sync_channel(2048);
        scan(
            drive.clone(),
            cache.clone(),
            first,
            Some(first_fingerprint),
            std::time::Instant::now(),
            tx,
            Arc::new(AtomicBool::new(false)),
        );
        let second_events: Vec<_> = rx.try_iter().collect();
        let second = load_cache_file(&cache, &drive);
        assert_eq!(entries_fingerprint(&second), first_fingerprint);
        assert!(matches!(
            second_events.last(),
            Some(ScanEvent::Finished {
                cancelled: false,
                changed: Some(false)
            })
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
