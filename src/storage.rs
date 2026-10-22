//! Progressive, read-only filesystem inventory. This module owns acquisition
//! and the normalized tree; the UI consumes events and never performs I/O.

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
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
    Cached {
        entries: Vec<Entry>,
        last_completed: Option<std::time::SystemTime>,
    },
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
                set_storage_thread_low_priority();
                let started = std::time::Instant::now();
                let (cache_path, cached, last_completed) = load_cache(&drive);
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
                    if !send(&sender, ScanEvent::Cached {
                        entries: chunk.to_vec(),
                        last_completed,
                    }, &thread_cancel) {
                        let _ = sender.try_send(ScanEvent::Finished {
                            cancelled: true,
                            changed: None,
                        });
                        return;
                    }
                }
                scan(
                    ScanPlan {
                        drive,
                        cache_path,
                        cached,
                        cached_completed_at: last_completed,
                        cached_fingerprint,
                        started,
                    },
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
    render_prepared: bool,
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
    pub fn render_prepared(&self) -> bool {
        self.render_prepared
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
    id: u64,
    scan: Option<ScanHandle>,
    tree: StorageTree,
    inventory: StorageTree,
    last_completed: Option<std::time::SystemTime>,
}

impl StorageSession {
    pub fn start(drive: &Drive) -> Self {
        let mut session = Self::empty(drive);
        session.refresh(drive);
        session
    }

    fn empty(drive: &Drive) -> Self {
        Self {
            id: NEXT_STORAGE_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            scan: None,
            tree: StorageTree {
                total_bytes: drive.total_bytes,
                free_bytes: drive.free_bytes,
                ..StorageTree::default()
            },
            inventory: StorageTree::default(),
            last_completed: None,
        }
    }

    fn apply_cached(&mut self, entries: Vec<Entry>) {
        self.inventory.cache_loaded = true;
        for entry in entries {
            self.inventory.insert(entry, true);
        }
    }

    fn apply_snapshot(&mut self, tree: StorageTree) {
        self.tree = tree;
        self.tree.cache_loaded = !self.tree.entries.is_empty();
    }

    fn refresh(&mut self, drive: &Drive) {
        crate::config::log(&format!(
            "Storage refresh start drive={} session_id={} existing_inventory_entries={} render_snapshot_nodes={} cached_render_snapshot={}",
            drive.label,
            self.id,
            self.inventory.discovered_count(),
            self.tree.discovered_count(),
            self.tree.has_cached_snapshot()
        ));
        if let Some(scan) = &self.scan {
            scan.cancel();
        }
        self.tree.finished = None;
        self.tree.changed = None;
        self.tree.error = None;
        self.tree.issues.clear();
        self.tree.inaccessible_count = 0;
        self.tree.total_bytes = drive.total_bytes;
        self.tree.free_bytes = drive.free_bytes;
        self.scan = Some(ScanHandle::start(drive.clone()));
    }

    pub fn poll(&mut self) {
        let Some(scan) = &self.scan else {
            return;
        };
        let mut entries_applied = 0usize;
        while entries_applied < 8192 {
            let Ok(event) = scan.receiver.try_recv() else {
                break;
            };
            match event {
                ScanEvent::Cached {
                    entries,
                    last_completed,
                } => {
                    self.inventory.cache_loaded = true;
                    if last_completed.is_some() {
                        self.last_completed = last_completed;
                    }
                    for entry in entries {
                        self.inventory.insert(entry, true);
                        entries_applied += 1;
                    }
                }
                ScanEvent::Entries(entries) => {
                    for entry in entries {
                        self.inventory.insert(entry.clone(), false);
                        self.tree.insert(entry, false);
                        entries_applied += 1;
                    }
                }
                ScanEvent::DirectoryFinished(parent) => {
                    let stale = self
                        .inventory
                        .cached_by_parent
                        .remove(&parent)
                        .unwrap_or_default();
                    for path in stale {
                        self.inventory.remove_subtree(&path);
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
                    if !cancelled {
                        self.last_completed = Some(std::time::SystemTime::now());
                    }
                }
            }
        }
    }

    pub fn tree(&self) -> &StorageTree {
        &self.tree
    }
    pub fn inventory_count(&self) -> usize {
        self.inventory.discovered_count()
    }
    pub fn cancel(&self) {
        if let Some(scan) = &self.scan {
            scan.cancel();
        }
    }

    pub fn last_completed(&self) -> Option<std::time::SystemTime> {
        self.last_completed
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}

static NEXT_STORAGE_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Keeps each drive's latest tree alive independently of the Storage page and
/// runs at most one filesystem refresh at a time.
pub struct StorageManager {
    drives: Vec<Drive>,
    sessions: HashMap<PathBuf, StorageSession>,
    next_drive: usize,
    active: Option<PathBuf>,
    next_cycle: std::time::Instant,
    last_drive_check: std::time::Instant,
    cache_receiver: Receiver<CacheLoadEvent>,
    cache_worker: Option<JoinHandle<()>>,
    cache_loading: bool,
    started_at: std::time::Instant,
}

enum CacheLoadEvent {
    Snapshot(PathBuf, Box<StorageTree>),
    Entries(PathBuf, Vec<Entry>),
    DriveFinished(PathBuf, Option<std::time::SystemTime>),
    AllFinished,
}

impl StorageManager {
    pub fn start(drives: Vec<Drive>) -> Self {
        let started_at = std::time::Instant::now();
        crate::config::log(&format!("Storage manager starting drives={}", drives.len()));
        let (cache_sender, cache_receiver) = mpsc::sync_channel(2);
        let cache_drives = drives.clone();
        let cache_worker = thread::Builder::new()
            .name("machineglass-storage-cache-load".into())
            .spawn(move || {
                set_storage_thread_low_priority();
                let mut snapshot_roots = HashSet::new();
                for drive in &cache_drives {
                    let snapshot_path = treemap_path(drive);
                    let snapshot_load_started = std::time::Instant::now();
                    if let Some(tree) = load_treemap_snapshot(&snapshot_path, drive) {
                        crate::config::log(&format!("Storage treemap snapshot loaded drive={} nodes={} bytes={} elapsed_ms={}", drive.label, tree.discovered_count(), fs::metadata(&snapshot_path).map(|m| m.len()).unwrap_or(0), snapshot_load_started.elapsed().as_millis()));
                        if cache_sender.send(CacheLoadEvent::Snapshot(drive.root.clone(), Box::new(tree))).is_err() { return; }
                        snapshot_roots.insert(drive.root.clone());
                    }
                }
                for drive in cache_drives {
                    let snapshot_path = treemap_path(&drive);
                    let (path, entries, last_completed) = load_cache(&drive);
                    if !snapshot_roots.contains(&drive.root) && !entries.is_empty() {
                        let snapshot = build_render_tree(&drive, &entries);
                        match save_treemap_snapshot(&snapshot_path, &drive, &snapshot) {
                            Ok(()) => crate::config::log(&format!("Storage treemap snapshot created from inventory drive={} nodes={} bytes={} source={}", drive.label, snapshot.discovered_count(), fs::metadata(&snapshot_path).map(|m| m.len()).unwrap_or(0), path.display())),
                            Err(error) => crate::config::log(&format!("Storage treemap snapshot save failed drive={} error={error}", drive.label)),
                        }
                        if cache_sender.send(CacheLoadEvent::Snapshot(drive.root.clone(), Box::new(snapshot))).is_err() { return; }
                    }
                    crate::config::log(&format!(
                        "Storage cache decoded drive={} entries={}",
                        drive.label,
                        entries.len()
                    ));
                    for chunk in entries.chunks(256) {
                        if cache_sender
                            .send(CacheLoadEvent::Entries(drive.root.clone(), chunk.to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    if cache_sender
                        .send(CacheLoadEvent::DriveFinished(
                            drive.root.clone(),
                            last_completed,
                        ))
                        .is_err()
                    {
                        return;
                    }
                }
                let _ = cache_sender.send(CacheLoadEvent::AllFinished);
            })
            .ok();
        let mut manager = Self {
            sessions: drives
                .iter()
                .map(|drive| (drive.root.clone(), StorageSession::empty(drive)))
                .collect(),
            drives,
            next_drive: 0,
            active: None,
            next_cycle: std::time::Instant::now() + std::time::Duration::from_secs(60 * 60),
            last_drive_check: std::time::Instant::now(),
            cache_receiver,
            cache_worker,
            cache_loading: true,
            started_at,
        };
        if manager.cache_worker.is_none() {
            manager.cache_loading = false;
            manager.start_next();
        }
        manager
    }

    pub fn drives(&self) -> &[Drive] {
        &self.drives
    }

    pub fn poll(&mut self) {
        while let Ok(event) = self.cache_receiver.try_recv() {
            match event {
                CacheLoadEvent::Entries(root, entries) => {
                    if let Some(session) = self.sessions.get_mut(&root) {
                        session.apply_cached(entries);
                    }
                }
                CacheLoadEvent::Snapshot(root, tree) => {
                    if let Some(session) = self.sessions.get_mut(&root) {
                        session.apply_snapshot(*tree);
                    }
                }
                CacheLoadEvent::DriveFinished(root, last_completed) => {
                    if let Some(session) = self.sessions.get_mut(&root) {
                        session.last_completed = last_completed;
                        crate::config::log(&format!(
                            "Storage cache inventory published drive={} session_id={} inventory_entries={} render_snapshot_nodes={} cached_render_snapshot={} elapsed_ms={}",
                            root.display(),
                            session.id(),
                            session.inventory_count(),
                            session.tree().discovered_count(),
                            session.tree().has_cached_snapshot(),
                            self.started_at.elapsed().as_millis()
                        ));
                    }
                }
                CacheLoadEvent::AllFinished => {
                    self.cache_loading = false;
                    self.cache_worker = None;
                }
            }
        }
        if self.last_drive_check.elapsed() >= std::time::Duration::from_secs(60) {
            self.refresh_drive_inventory();
            self.last_drive_check = std::time::Instant::now();
        }
        if let Some(root) = self.active.clone() {
            if let Some(session) = self.sessions.get_mut(&root) {
                session.poll();
                if !session.tree().is_scanning() {
                    self.active = None;
                    if self.next_drive >= self.drives.len() {
                        self.next_cycle =
                            std::time::Instant::now() + std::time::Duration::from_secs(60 * 60);
                    }
                }
            } else {
                self.active = None;
            }
        }
        if !self.cache_loading
            && self.active.is_none()
            && (self.next_drive < self.drives.len() || std::time::Instant::now() >= self.next_cycle)
        {
            if self.next_drive >= self.drives.len() {
                self.next_drive = 0;
            }
            self.start_next();
        }
    }

    fn refresh_drive_inventory(&mut self) {
        let discovered = local_drives();
        let available: HashSet<_> = discovered.iter().map(|drive| drive.root.clone()).collect();
        let known: HashSet<_> = self.drives.iter().map(|drive| drive.root.clone()).collect();
        for drive in discovered {
            if !known.contains(&drive.root) {
                self.sessions
                    .insert(drive.root.clone(), StorageSession::empty(&drive));
                self.drives.push(drive);
                self.next_cycle = std::time::Instant::now();
            } else if let Some(existing) = self.drives.iter_mut().find(|old| old.root == drive.root)
            {
                *existing = drive;
            }
        }
        self.drives.retain(|drive| available.contains(&drive.root));
        self.next_drive = self.next_drive.min(self.drives.len());
    }

    fn start_next(&mut self) {
        if self.next_drive < self.drives.len() {
            let drive = self.drives[self.next_drive].clone();
            self.next_drive += 1;
            let root = drive.root.clone();
            if let Some(session) = self.sessions.get_mut(&root) {
                session.refresh(&drive);
            } else {
                self.sessions
                    .insert(root.clone(), StorageSession::start(&drive));
            }
            self.active = Some(root);
        } else {
            self.next_cycle = std::time::Instant::now() + std::time::Duration::from_secs(60 * 60);
        }
    }

    pub fn session(&self, root: &Path) -> Option<&StorageSession> {
        self.sessions.get(root)
    }

    pub fn is_refreshing(&self, root: &Path) -> bool {
        self.active.as_deref() == Some(root)
    }

    pub fn is_loading_cache(&self) -> bool {
        self.cache_loading
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.started_at.elapsed().as_millis()
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

#[cfg(windows)]
fn set_storage_thread_low_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_LOWEST,
    };
    let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_LOWEST) };
    if ok == 0 {
        crate::config::log(&format!(
            "Could not lower storage worker priority: {}",
            std::io::Error::last_os_error()
        ));
    }
}

#[cfg(not(windows))]
fn set_storage_thread_low_priority() {}

#[derive(Deserialize, Serialize)]
struct StorageCache {
    version: u32,
    root: String,
    volume_id: Option<u32>,
    total_bytes: Option<u64>,
    #[serde(default)]
    completed_at_unix_secs: Option<u64>,
    entries: Vec<Entry>,
}

#[derive(Deserialize, Serialize)]
struct TreemapSnapshot {
    version: u32,
    root: String,
    volume_id: Option<u32>,
    total_bytes: Option<u64>,
    captured_at_unix_secs: u64,
    nodes: Vec<TreemapNode>,
}

#[derive(Deserialize, Serialize)]
struct TreemapNode {
    parent: u32,
    name: String,
    directory: bool,
    size: u64,
    aggregate_count: u64,
    aggregate: bool,
}

fn treemap_path(drive: &Drive) -> PathBuf {
    treemap_path_for_cache(&cache_path(drive))
}

fn treemap_path_for_cache(path: &Path) -> PathBuf {
    path.with_extension("treemap.json")
}

fn build_render_tree(drive: &Drive, entries: &[Entry]) -> StorageTree {
    let mut children: HashMap<PathBuf, Vec<&Entry>> = HashMap::new();
    for entry in entries {
        if entry.path == drive.root && entry.parent == drive.root {
            continue;
        }
        if entry.path != entry.parent {
            children
                .entry(entry.parent.clone())
                .or_default()
                .push(entry);
        }
    }
    let mut directories: Vec<_> = entries.iter().filter(|entry| entry.is_dir).collect();
    directories.sort_by_key(|entry| std::cmp::Reverse(entry.path.components().count()));
    let mut directory_sizes = HashMap::with_capacity(directories.len());
    for directory in directories {
        let children_size = children
            .get(&directory.path)
            .into_iter()
            .flatten()
            .map(|child| {
                if child.is_dir {
                    directory_sizes
                        .get(&child.path)
                        .copied()
                        .unwrap_or(child.size)
                } else {
                    child.size
                }
            })
            .sum::<u64>();
        directory_sizes.insert(directory.path.clone(), directory.size.max(children_size));
    }
    let mut tree = StorageTree {
        total_bytes: drive.total_bytes,
        free_bytes: drive.free_bytes,
        render_prepared: true,
        ..StorageTree::default()
    };
    if let Some(root_entry) = entries
        .iter()
        .find(|entry| entry.path == drive.root && entry.parent == drive.root)
    {
        let mut root_entry = root_entry.clone();
        root_entry.size = directory_sizes
            .get(&drive.root)
            .copied()
            .unwrap_or(root_entry.size);
        tree.insert(root_entry, false);
    }
    let mut pending = vec![(drive.root.clone(), 0usize)];
    while let Some((parent, depth)) = pending.pop() {
        let Some(mut siblings) = children.remove(&parent) else {
            continue;
        };
        siblings.sort_by(|a, b| {
            directory_sizes
                .get(&b.path)
                .copied()
                .unwrap_or(b.size)
                .cmp(&directory_sizes.get(&a.path).copied().unwrap_or(a.size))
                .then_with(|| a.name.cmp(&b.name))
        });
        let total_children = siblings.len();
        let overflow_size = siblings
            .iter()
            .skip(79)
            .map(|entry| {
                directory_sizes
                    .get(&entry.path)
                    .copied()
                    .unwrap_or(entry.size)
            })
            .sum::<u64>();
        let overflow_count = total_children.saturating_sub(79);
        siblings.truncate(79);
        for entry in siblings {
            let mut rendered = (*entry).clone();
            rendered.size = directory_sizes
                .get(&entry.path)
                .copied()
                .unwrap_or(entry.size);
            tree.insert(rendered, false);
            if entry.is_dir && depth < 8 {
                pending.push((entry.path.clone(), depth + 1));
            }
        }
        if overflow_count > 0 {
            let entry = Entry {
                path: parent.join(".machineglass-other-entries"),
                parent: parent.clone(),
                name: "Other discovered items".into(),
                is_dir: false,
                size: overflow_size,
                aggregate_count: overflow_count as u64,
                aggregate: true,
                unmapped: false,
            };
            tree.insert(entry, false);
        }
    }
    tree.cache_loaded = !tree.entries.is_empty();
    tree
}

fn save_treemap_snapshot(path: &Path, drive: &Drive, tree: &StorageTree) -> std::io::Result<()> {
    let mut ordered: Vec<_> = tree.entries.values().collect();
    ordered.sort_by(|a, b| {
        a.path
            .components()
            .count()
            .cmp(&b.path.components().count())
            .then_with(|| a.parent.cmp(&b.parent))
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.name.cmp(&b.name))
    });
    let indexes: HashMap<_, _> = ordered
        .iter()
        .enumerate()
        .map(|(i, entry)| (entry.path.as_path(), i as u32))
        .collect();
    let nodes = ordered
        .iter()
        .map(|entry| TreemapNode {
            parent: if entry.path == drive.root {
                u32::MAX
            } else {
                indexes
                    .get(entry.parent.as_path())
                    .copied()
                    .unwrap_or(u32::MAX)
            },
            name: if entry.path == drive.root {
                String::new()
            } else {
                entry.name.clone()
            },
            directory: entry.is_dir,
            size: entry.size,
            aggregate_count: entry.aggregate_count,
            aggregate: entry.aggregate,
        })
        .collect();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let parent = path.parent().expect("treemap snapshot has parent");
    fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("treemap.json.{}.tmp", std::process::id()));
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer(
        &mut writer,
        &TreemapSnapshot {
            version: 3,
            root: drive.label.clone(),
            volume_id: drive.volume_id,
            total_bytes: drive.total_bytes,
            captured_at_unix_secs: timestamp,
            nodes,
        },
    )
    .map_err(std::io::Error::other)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    replace_file(&temp, path)
}

fn load_treemap_snapshot(path: &Path, drive: &Drive) -> Option<StorageTree> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return None,
    };
    let snapshot: TreemapSnapshot = match serde_json::from_reader(BufReader::new(file)) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            crate::config::log(&format!(
                "Storage treemap snapshot rejected drive={} reason=parse error: {error}",
                drive.label
            ));
            return None;
        }
    };
    if snapshot.version != 3
        || snapshot.root != drive.label
        || snapshot.volume_id != drive.volume_id
    {
        crate::config::log(&format!(
            "Storage treemap snapshot rejected drive={} reason=identity mismatch version={} root={} volume_id={:?} expected_volume_id={:?}",
            drive.label, snapshot.version, snapshot.root, snapshot.volume_id, drive.volume_id
        ));
        return None;
    }
    let node_count = snapshot.nodes.len();
    let mut paths: Vec<PathBuf> = Vec::with_capacity(node_count);
    let mut tree = StorageTree {
        total_bytes: snapshot.total_bytes,
        render_prepared: true,
        ..StorageTree::default()
    };
    for (index, node) in snapshot.nodes.into_iter().enumerate() {
        let parent = if node.parent == u32::MAX {
            drive.root.clone()
        } else {
            let Some(parent) = paths.get(node.parent as usize) else {
                crate::config::log(&format!(
                    "Storage treemap snapshot rejected drive={} reason=parent index {} is not before node {} nodes={}",
                    drive.label, node.parent, index, node_count
                ));
                return None;
            };
            parent.clone()
        };
        let path = if node.parent == u32::MAX && node.name.is_empty() {
            drive.root.clone()
        } else if node.aggregate && node.name == "Other discovered items" {
            parent.join(".machineglass-other-entries")
        } else {
            parent.join(&node.name)
        };
        let entry = Entry {
            path: path.clone(),
            parent,
            name: node.name,
            is_dir: node.directory,
            size: node.size,
            aggregate_count: node.aggregate_count,
            aggregate: node.aggregate,
            unmapped: false,
        };
        tree.insert(entry, false);
        paths.push(path);
        debug_assert!(index < u32::MAX as usize);
    }
    tree.cache_loaded = !tree.entries.is_empty();
    Some(tree)
}

fn persist_render_snapshot(path: &Path, drive: &Drive, entries: &HashMap<PathBuf, Entry>) {
    let tree = build_render_tree(drive, &entries.values().cloned().collect::<Vec<_>>());
    if let Err(error) = save_treemap_snapshot(path, drive, &tree) {
        crate::config::log(&format!(
            "Storage treemap snapshot save failed drive={} error={error}",
            drive.label
        ));
    }
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

fn load_cache(drive: &Drive) -> (PathBuf, Vec<Entry>, Option<std::time::SystemTime>) {
    let started = std::time::Instant::now();
    let path = cache_path(drive);
    let (entries, outcome, last_completed) = load_cache_file_diagnostic(&path, drive);
    let bytes = fs::metadata(&path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if outcome.starts_with("hit") {
        crate::config::log(&format!(
            "Storage cache found and identity validated drive={} entries={} bytes={}",
            drive.label,
            entries.len(),
            bytes
        ));
    }
    crate::config::log(&format!(
        "Storage cache validation drive={} path={} bytes={} outcome={} entries={} elapsed_ms={}",
        drive.label,
        path.display(),
        bytes,
        outcome,
        entries.len(),
        started.elapsed().as_millis()
    ));
    (path, entries, last_completed)
}

#[cfg(test)]
fn load_cache_file(path: &Path, drive: &Drive) -> Vec<Entry> {
    load_cache_file_diagnostic(path, drive).0
}

fn load_cache_file_diagnostic(
    path: &Path,
    drive: &Drive,
) -> (Vec<Entry>, String, Option<std::time::SystemTime>) {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), "not found".into(), None);
        }
        Err(error) => return (Vec::new(), format!("open failed: {error}"), None),
    };
    let legacy_completion_time = file.metadata().ok().and_then(|meta| meta.modified().ok());
    let cache = match serde_json::from_reader::<_, StorageCache>(BufReader::new(file)) {
        Ok(cache) => cache,
        Err(error) => return (Vec::new(), format!("invalid cache data: {error}"), None),
    };
    let root_matches = cache
        .root
        .eq_ignore_ascii_case(&drive.root.to_string_lossy());
    let identity_matches = cache.version == 1
        && drive.volume_id.is_some()
        && root_matches
        && cache.volume_id == drive.volume_id
        && cache.total_bytes == drive.total_bytes;
    if identity_matches {
        let valid_entries = cache.entries.iter().all(|entry| {
            entry.path.starts_with(&drive.root)
                && ((entry.path == drive.root
                    && entry.parent == drive.root.parent().unwrap_or(&drive.root))
                    || (entry.parent.starts_with(&drive.root)
                        && entry.path.parent() == Some(entry.parent.as_path())))
        });
        if !valid_entries {
            return (
                Vec::new(),
                "invalid cache paths (outside root or parent mismatch)".into(),
                None,
            );
        }
        let count = cache.entries.len();
        let completed_at = cache
            .completed_at_unix_secs
            .map(|seconds| std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .or(legacy_completion_time);
        (
            cache.entries,
            format!("hit (identity matched, {count} entries)"),
            completed_at,
        )
    } else {
        (
            Vec::new(),
            format!(
                "identity mismatch (version={}, root_match={}, volume_match={}, capacity_match={})",
                cache.version == 1,
                root_matches,
                drive.volume_id.is_some() && cache.volume_id == drive.volume_id,
                cache.total_bytes == drive.total_bytes
            ),
            None,
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

fn save_cache(
    path: &Path,
    drive: &Drive,
    entries: HashMap<PathBuf, Entry>,
    completed_at: Option<std::time::SystemTime>,
) -> std::io::Result<()> {
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
            completed_at_unix_secs: completed_at.and_then(|time| {
                time.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|age| age.as_secs())
            }),
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

struct ScanPlan {
    drive: Drive,
    cache_path: PathBuf,
    cached: Vec<Entry>,
    cached_completed_at: Option<std::time::SystemTime>,
    cached_fingerprint: Option<u64>,
    started: std::time::Instant,
}

fn scan(plan: ScanPlan, sender: SyncSender<ScanEvent>, cancel: Arc<AtomicBool>) {
    let ScanPlan {
        drive,
        cache_path,
        cached,
        cached_completed_at,
        cached_fingerprint,
        started,
    } = plan;
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
    let mut last_checkpoint = std::time::Instant::now();
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
        directory_totals
            .entry(directory.clone())
            .or_insert_with(|| cache_records.get(&directory).map_or(0, |entry| entry.size));
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
                let old_size = cache_records
                    .get(&display_entry.path)
                    .map_or(0, |old| old.size);
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
                if last_checkpoint.elapsed() >= std::time::Duration::from_secs(5 * 60) {
                    checkpoint_cache(&cache_path, &drive, &cache_records, cached_completed_at);
                    last_checkpoint = std::time::Instant::now();
                }
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
    let completed_at = if cancelled {
        cached_completed_at
    } else {
        Some(std::time::SystemTime::now())
    };
    match save_cache(&cache_path, &drive, cache_records.clone(), completed_at) {
        Ok(()) => crate::config::log(&format!(
            "Storage cache {} drive={} entries={} path={}",
            if cancelled {
                "interrupted progress saved"
            } else {
                "saved"
            },
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
    let snapshot_path = treemap_path_for_cache(&cache_path);
    persist_render_snapshot(&snapshot_path, &drive, &cache_records);
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

fn checkpoint_cache(
    path: &Path,
    drive: &Drive,
    records: &HashMap<PathBuf, Entry>,
    completed_at: Option<std::time::SystemTime>,
) {
    match save_cache(path, drive, records.clone(), completed_at) {
        Ok(()) => {
            persist_render_snapshot(&treemap_path_for_cache(path), drive, records);
            crate::config::log(&format!(
                "Storage cache checkpoint drive={} entries={} path={}",
                drive.label,
                records.len(),
                path.display()
            ))
        }
        Err(error) => crate::config::log(&format!(
            "Storage cache checkpoint failed drive={} path={} error={error}",
            drive.label,
            path.display()
        )),
    }
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
    fn render_snapshot_round_trips_prepared_top_children_and_aggregates_overflow() {
        let temp = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("machineglass-treemap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        let drive = Drive {
            root: PathBuf::from("F:\\"),
            label: "F:\\".into(),
            total_bytes: Some(10_000),
            free_bytes: Some(1_000),
            volume_id: Some(12),
        };
        let mut entries = vec![entry_for_directory(&drive.root, 5_050)];
        for i in 0..100 {
            let path = drive.root.join(format!("file-{i:03}.bin"));
            entries.push(Entry {
                parent: drive.root.clone(),
                name: format!("file-{i:03}.bin"),
                path,
                is_dir: false,
                size: 100 - i,
                aggregate_count: 0,
                aggregate: false,
                unmapped: false,
            });
        }
        let tree = build_render_tree(&drive, &entries);
        assert!(tree.render_prepared());
        assert_eq!(tree.children().get(&drive.root).unwrap().len(), 80);
        assert!(
            tree.entries()
                .values()
                .any(|entry| entry.name == "Other discovered items" && entry.aggregate_count == 21)
        );
        let path = temp.join("F.treemap.json");
        save_treemap_snapshot(&path, &drive, &tree).unwrap();
        let loaded = load_treemap_snapshot(&path, &drive).unwrap();
        assert!(loaded.render_prepared());
        assert_eq!(loaded.entry(&drive.root).unwrap().size, 5_050);
        assert_eq!(loaded.children().get(&drive.root).unwrap().len(), 80);
        assert_eq!(
            loaded.entry(&drive.root.join("file-000.bin")).unwrap().size,
            100
        );
        assert_eq!(
            loaded
                .entries()
                .values()
                .find(|entry| entry.name == "Other discovered items")
                .unwrap()
                .aggregate_count,
            21
        );
        assert!(
            load_treemap_snapshot(
                &path,
                &Drive {
                    volume_id: Some(13),
                    ..drive
                }
            )
            .is_none()
        );
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn render_snapshot_recovers_directory_totals_from_cached_children() {
        let drive = Drive {
            root: PathBuf::from("F:\\"),
            label: "F:\\".into(),
            total_bytes: Some(10_000),
            free_bytes: Some(9_500),
            volume_id: Some(12),
        };
        let directory = drive.root.join("SteamLibrary");
        let file = directory.join("large.vhdx");
        let entries = vec![
            entry_for_directory(&drive.root, 0),
            entry_for_directory(&directory, 0),
            Entry {
                path: file,
                parent: directory.clone(),
                name: "large.vhdx".into(),
                is_dir: false,
                size: 8_000,
                aggregate_count: 0,
                aggregate: false,
                unmapped: false,
            },
        ];
        let tree = build_render_tree(&drive, &entries);
        assert_eq!(tree.entry(&directory).unwrap().size, 8_000);
        assert_eq!(tree.entry(&drive.root).unwrap().size, 8_000);
    }

    #[test]
    fn sidecar_path_follows_the_cache_path_not_the_drive_letter() {
        let cache = PathBuf::from("target/test-cache.json");
        assert_eq!(
            treemap_path_for_cache(&cache),
            PathBuf::from("target/test-cache.treemap.json")
        );
    }

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
            ScanPlan {
                drive: Drive {
                    root: root.clone(),
                    label: root.display().to_string(),
                    total_bytes: Some(2_000_000),
                    free_bytes: Some(800_000),
                    volume_id: Some(1),
                },
                cache_path: root.join("test-cache.json"),
                cached: Vec::new(),
                cached_completed_at: None,
                cached_fingerprint: None,
                started: std::time::Instant::now(),
            },
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
            Some(ScanEvent::Finished {
                cancelled: false,
                ..
            })
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
            ScanPlan {
                drive: Drive {
                    root: root.clone(),
                    label: root.display().to_string(),
                    total_bytes: None,
                    free_bytes: None,
                    volume_id: None,
                },
                cache_path: root.join("test-cache.json"),
                cached: Vec::new(),
                cached_completed_at: None,
                cached_fingerprint: None,
                started: std::time::Instant::now(),
            },
            tx,
            cancelled,
        );
        assert!(matches!(
            rx.try_iter().last(),
            Some(ScanEvent::Finished {
                cancelled: true,
                ..
            })
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
        save_cache(&path, &drive, entries, None).unwrap();

        let (cached, outcome, completed_at) = load_cache_file_diagnostic(&path, &drive);
        assert!(outcome.starts_with("hit"));
        assert!(completed_at.is_some(), "legacy cache mtime is retained");
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
    fn cache_larger_than_64_mib_is_loaded_when_valid() {
        let temp = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("machineglass-large-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        let path = temp.join("large.json");
        let drive = Drive {
            root: PathBuf::from("F:\\"),
            label: "F:\\".into(),
            total_bytes: Some(1000),
            free_bytes: Some(400),
            volume_id: Some(12),
        };
        let mut entries = HashMap::new();
        let root_entry = entry_for_directory(&drive.root, 600);
        entries.insert(root_entry.path.clone(), root_entry);
        save_cache(&path, &drive, entries, None).unwrap();
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            let spaces = [b' '; 1024 * 1024];
            for _ in 0..65 {
                file.write_all(&spaces).unwrap();
            }
        }
        assert!(fs::metadata(&path).unwrap().len() > 64 * 1024 * 1024);
        let (cached, outcome, _) = load_cache_file_diagnostic(&path, &drive);
        assert_eq!(cached.len(), 1, "cache outcome: {outcome}");
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
            .join(format!(
                "machineglass-repeat-cache-{}.json",
                std::process::id()
            ));
        let (tx, rx) = mpsc::sync_channel(2048);
        scan(
            ScanPlan {
                drive: drive.clone(),
                cache_path: cache.clone(),
                cached: Vec::new(),
                cached_completed_at: None,
                cached_fingerprint: None,
                started: std::time::Instant::now(),
            },
            tx,
            Arc::new(AtomicBool::new(false)),
        );
        let _first_events: Vec<_> = rx.try_iter().collect();
        let (first, outcome, _) = load_cache_file_diagnostic(&cache, &drive);
        assert!(!first.is_empty(), "initial scan cache outcome: {outcome}");
        let expected_bytes = 600_024;
        assert_eq!(
            first.iter().find(|entry| entry.path == root).unwrap().size,
            expected_bytes
        );
        let first_fingerprint = entries_fingerprint(&first);

        let (tx, rx) = mpsc::sync_channel(2048);
        scan(
            ScanPlan {
                drive: drive.clone(),
                cache_path: cache.clone(),
                cached: first,
                cached_completed_at: None,
                cached_fingerprint: Some(first_fingerprint),
                started: std::time::Instant::now(),
            },
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
