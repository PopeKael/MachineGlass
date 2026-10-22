# Local verification — 4 October 2026

This is a Milestone 1 proof of concept, verified on the current Windows desktop.

## 22 October 2026 — Storage-map checkpoint acceptance

Ran `cargo fmt`, `cargo test --locked` (18 library tests and one native layout
test), `cargo clippy --all-targets --locked -- -D warnings`, and
`cargo build --release --locked`; all passed. `scripts/Package.ps1` refreshed
the authoritative shortcut package. SHA-256 for both
`target/release/MachineGlass.exe` and
`dist/MachineGlass/MachineGlass.exe` is
`BA85D588A3C383B1FEB1E3C357F36594EE2D8C8ED8A5FDBAECA0612DD11578DA`.

Launched the exact `dist` executable with the real `%LOCALAPPDATA%`, Storage
page, and F: selected. The inspected 944×720 native framebuffer shows the
persisted F: map with 116,298 inventory records, the `AI/WSL/Ubuntu-24.04`
directory, and its 56.97 GiB `ext4.vhdx` tile. The map status was “Saved map ·
refresh queued”; this short capture verifies native rendering and cache display,
not a completed full-volume refresh. Private capture:
`target/checkpoint-native-F-final.bmp` (kept out of Git).

## 17 October 2026 — Shortcut executable and native Storage rendering

Refreshed `dist\\MachineGlass\\MachineGlass.exe`, the executable used by the
user's shortcut. The previous packaged binary differed from the current
`target\\release\\MachineGlass.exe`; after packaging, both files had SHA-256
`BA85D588A3C383B1FEB1E3C357F36594EE2D8C8ED8A5FDBAECA0612DD11578DA`.

Launched that exact `dist` path with `--view storage --storage-drive C:` and
`LOCALAPPDATA` set to the real user data directory because the verification
environment normally redirects it. The rendered 944×720 native framebuffer
shows the full cached C: treemap, 391,026 inventory records, `Cached map ·
verifying…`, and `Refreshing saved map`; the sample was updated 0.5 seconds
before capture. The application closed cleanly after capture. The C.json
inventory remained 144,705,145 bytes and its treemap sidecar remained
7,703,968 bytes with their existing timestamps; the run ended before the
five-minute checkpoint. Capture:
`C:\Users\Warren\.codex\visualizations\2026\10\17\01a1488e-75e7-76c3-abe0-7fac4fc91d47\machineglass-shortcut-cached-native.bmp`.

The first capture used the sandbox's redirected `LOCALAPPDATA`, which has no
user caches and therefore displayed only its partial initial scan. It was not
used as acceptance evidence. The subsequent capture used the actual local
cache and displayed the persisted tree while refresh was active.

## 17 October 2026 — Cold-start Storage cache lifecycle

Cold-launched the release executable twice against the staged benchmark
caches, then against the user's actual `%LOCALAPPDATA%` caches. The actual C:
file was 144,736,179 bytes with 391,065 entries. Its SHA-256 was
`82CB4EF469978EFB9A88DB0D645B310728DE18DDA86264C3AA706C5057402CFB` both
before and after the actual-cache run. The process was stopped after about 10
seconds, before the five-minute checkpoint could write a cache.

Actual-cache cold-start timeline (release build):

| Event | Time from manager start | Evidence |
|---|---:|---|
| C.json found and identity validated | 644 ms | 144,736,179 bytes; identity matched; 391,065 decoded entries |
| C: map published to manager | 2.208 s | session 1; 391,065 tree entries; cached snapshot true |
| C: cached map rendered by Storage UI | 2.210 s | session 1; 391,065 entries; selected path `C:\` |
| D: map published | 3.206 s | session 2; 69,665 entries |
| E: map published | 3.228 s | session 3; 2,846 entries |
| F: map published | 4.390 s | session 4; 116,298 entries |
| G: map published | 5.243 s | session 5; 86,261 entries |
| All caches complete; C: refresh starts | 5.262 s | session 1; 391,065 existing entries; cached snapshot true |
| Scanner's second C.json load validates | 5.883 s | 619 ms; 391,065 entries |
| Scanner begins C: traversal | 8.076 s | cached_entries=391065 |

Against the actual local caches, the native UI logged D: (session 2, 69,665
entries), E: (session 3, 2,846), F: (session 4, 116,298), and G: (session 5,
86,261), each with `cached_snapshot=true`. It then logged C: again on session
1 with 391,065 entries and `refreshing=true`. The actual native C: capture
during refresh shows the full treemap, “Cached map · verifying…”, and
“Refreshing saved map”. A separate native capture run against staged copies
also displayed the distinct D:, E:, F:, and G: treemaps. Opening the page or
changing the drive only reads `StorageManager::session(root)`; the combo
handler changes the UI root/path and does not construct or start a session.

Code trace and runtime logs agree: cache events insert into the existing
session tree; `refresh` resets scan status/issue fields but does not clear or
replace entries, children, cache state, or session; scanner events reconcile
into that same tree; the UI renders `session.tree()`. C's apparent fresh build
is the initial progressive publication interval: the large cache is decoded
off-thread, then entry chunks are applied with an 8,192-entry per-frame
budget. It takes about 2.2 seconds until all 391,065 are visible. D:/E:/F:/G:
look ready when visited later because their per-drive cache events have already
been applied. Background refresh does not start until all these maps are
published. During the actual run, the UI's telemetry footer updated 0.2
seconds before capture. Input-latency instrumentation was not added.

The second deserialization is confirmed and intentionally unchanged. After the
manager has loaded and published C.json, `start_next` calls `refresh` on that
existing session. `ScanHandle::start` receives the drive, not the decoded
manager entries, so its worker calls `load_cache` again to construct the
scanner's cached vector and fingerprint for reconciliation. This duplicate
read is not the cause of a cleared map; refresh already began with the full
manager tree and the UI continued to render it.

Lifecycle logging now records cache validation outcome/bytes/entries and
duration, decoded count, manager publication count/session/time, refresh
session/count/snapshot state, and one-time UI display plus selection. Example
actual-run sequence: `Storage cache found and identity validated drive=C:`;
`Storage map published to manager ... session_id=1 entries=391065`;
`Storage refresh start ... session_id=1 existing_entries=391065`;
`Storage UI selected drive=C: session_id=1 entries=391065 ... refreshing=true`.
The duplicate scanner load is visible as the second C: cache-validation line.

Native captures and staged-selection logs are kept outside Git under
`C:\Users\Warren\.codex\visualizations\2026\10\16\01a14723-c9b1-75a1-adf4-c9f87f4ac24f`.
No functional fix was needed: no map replacement or clearing occurred. The
smallest next change, if the 2.2-second first-paint interval proves unacceptable,
is to explore reducing UI insertion work after cache validation; do not replace
JSON or the `StorageTree` based on this correctness incident. The duplicate
scanner cache load is a separate, already measured optimization candidate.

## 17 October 2026 — Persistent Storage maps

- Cache files remain JSON under `%LOCALAPPDATA%\MachineGlass\storage\`, one
  file per drive letter (for example `C.json`). This was confirmed against the
  existing user cache without modifying it.
- C: cache size: 144,736,179 bytes. It deserialized with matching version,
  root, volume serial, capacity and entry paths: 391,065 entries. D:, E:, F:,
  and G: also loaded as identity-matched snapshots with 69,665; 2,846;
  116,298; and 86,261 entries. H: had no cache. Logs report cache hit/miss,
  invalid JSON, identity mismatch fields, and invalid path relationships.
- Startup cache loading runs on a bounded-channel worker. It publishes cached
  entries for every detected drive before the scanner begins. One low-priority
  scan worker then refreshes drives sequentially; the manager polls it while
  any page is open. It waits one hour after a complete drive cycle and checks
  local-drive inventory once per minute. Dropping the manager signals worker
  cancellation; filesystem calls already blocked in Windows or a filter driver
  are not joined on the UI thread.
- Each scan atomically checkpoints its current map every five minutes and at
  exit, including interrupted progress. Cache metadata preserves the last
  completed refresh time; legacy caches derive that time from their file mtime.
  Temporary writes are flushed before the existing Windows replace operation.
- Regression: valid JSON larger than 64 MiB is loaded, with no fixed size
  ceiling. Runtime check loaded the actual 144,736,179-byte C: snapshot, showed
  its 391,065-entry map in the native Storage page, and logged the background
  refresh start. The release capture is outside the repository at
  `C:\Users\Warren\.codex\visualizations\2026\10\16\01a14723-c9b1-75a1-adf4-c9f87f4ac24f\storage-manager-refresh-native.bmp`.
- The release refresh ran for 34 seconds and wrote no checkpoint under the
  final five-minute cadence. A full C: scan, full-cycle timing, and complete
  map reconciliation were not observed. The initial manager cache pass and
  scanner currently deserialize the selected cache independently; JSON parse
  and checkpoint performance for the 145 MB snapshot have not been benchmarked.
- Validation: `cargo fmt --all`; `cargo test --locked` (15 library tests, one
  UI layout test); `cargo clippy --all-targets --locked -- -D warnings`; and
  `cargo build --release --locked` all passed. Build artifacts used the
  writable Codex verification directory because the repository's existing
  `target` linker PDB was denied by the sandbox. Clippy required the permitted
  user-level configuration access.

## 14 October 2026 — Cached refresh stability and unreadable diagnostics

- Root cause: during a cached refresh, directory size totals were rebuilt from
  only currently encountered files. As the worker descended, cached totals
  could drop and then grow again even with no filesystem changes. The scan now
  retains cached totals as estimates and applies size deltas for changed,
  newly found, or removed children. Per-directory small-file aggregates update
  once that directory has been enumerated.
- A complete scan compares a deterministic fingerprint of its accepted cache
  snapshot and resulting tree. The UI says `Scan complete · no changes` when
  they match, and `Scan complete · map updated` when they differ. The cache is
  still atomically rewritten after a successful non-cancelled pass. A pass with
  unreadable locations does not claim “no changes,” because cached data may
  have been retained below those locations.
- Cache logs include drive, volume identity/capacity, cache path, hit/miss or
  mismatch reason, entry count and save result. Scan logs include duration,
  completion/cancellation, unreadable count and whether the map changed.
  Unreadable logs capture path, operation, and Windows I/O error text. The UI
  hover popup shows up to eight unique path/reason pairs and notes any remainder.
- Regression test: first scan a temporary tree with both small and large files,
  load its cache, scan it again unchanged, then verify root total is stable,
  cache fingerprints match, and completion reports no changes.
- `cargo fmt --all -- --check` — passed. `cargo test --locked` — passed (14
  library tests and one UI layout test). `cargo clippy --all-targets --locked
  -- -D warnings` — passed after running with permission to read Clippy's
  user-level configuration. `cargo build --release --locked` — passed.
- The packaged release executable launched with the Storage page selected.
  `verification/storage-diagnostics-native.bmp` is the captured and inspected
  native 944x720 initial Storage page. The unreadable hover popup was not
  visually captured; its path/reason data is covered by the scanner model and
  its egui hover construction compiled, but native hover interaction remains
  unverified in this session.
- `dist\MachineGlass\MachineGlass.exe` was refreshed after the in-use copy
  closed. SHA256 matches `target\release\MachineGlass.exe`:
  `1997E313513D5F77DF12BA4694B1830D40789B63B9DFE100E64D9A0184343F55`.

## 14 October 2026 — Progressive Storage view

### Implemented

- `src/storage.rs` enumerates logical fixed/removable drives with
  `GetLogicalDrives` / `GetDriveTypeW`, and captures total/free volume bytes
  with `GetDiskFreeSpaceExW`. Each selected volume is scanned on a named Rust
  worker. A bounded sync channel streams top-level entries, then descendants
  in batches, with coalesced cumulative directory-size updates.
  `StorageSession` applies events to `StorageTree`; the egui renderer reads
  only the tree model and its maintained parent index.
- Cancellation is cooperative and does not join a potentially blocked
  filesystem call on the UI thread. Unreadable directory/metadata entries are
  counted, and the scan continues. `DirEntry::metadata()` preserves link
  metadata and reuses the directory enumeration data on Windows. The Windows
  reparse-point attribute prevents descending into junctions, symlinks, mount
  points, or other reparse entries. Worker-start failures are surfaced in the
  UI; cancellation before the worker's first entry is still reported as
  cancelled.
- The root treemap uses the fixed volume total as its area denominator. The
  remaining tile shows free space plus bytes not yet mapped; its label is
  backed by the separate real free-space figure shown in the page header.
  Directory rectangles update and subdivide as cumulative logical sizes arrive.
  Files smaller than 512 KiB are grouped per directory; more than 80 visible
  children are represented by the largest 79 plus a remainder tile. Treemap
  descent draws at most eight nested levels per view; directories can still be
  selected and opened as the new view root.
- Open in Explorer targets the hovered real file/directory, or the selected
  directory when there is no hovered real item. Synthetic aggregate/remainder
  tiles are not sent to Explorer.
- Directories now use squarified area placement, so sibling size differences
  form proportional blocks rather than the long strips in the user screenshots.
  Stable directory colors and same-color child files make folder containment
  easier to read. The renderer uses a maintained parent index and only sorts
  the largest visible children for each drawn folder.
- Per-volume JSON snapshots live in `%LOCALAPPDATA%\MachineGlass\storage`.
  The worker accepts a snapshot only when schema, root, volume serial and total
  capacity match. It streams cached entries before live results, prunes cached
  children after successful directory enumeration, and retains cached children
  for inaccessible/incomplete directories. A complete, non-cancelled scan
  writes a temporary file and atomically replaces the snapshot. Snapshot files
  over 64 MiB are ignored. This is stale-while-refresh display; it does not
  skip the full filesystem walk or use USN Journal data.
- The Windows scan hot path now batches up to 128 discoveries or about 75 ms,
  coalesces directory-total updates, and uses `DirEntry::metadata()` to reuse
  enumeration metadata. This removes avoidable per-item channel, path-lookup,
  and repeated-index work. No before/after full-volume timing is available yet.

### Native observations

- Release framebuffer `verification/storage-capacity-native.bmp` was captured
  with C: selected while scanning continued. It showed a 464.34 GiB total,
  199.13 GiB free, 31.65 GiB discovered, and a 432.69 GiB “Free + not yet
  mapped” area. The pagefile rectangle occupied approximately 6.8% of the full
  drive frame (31.65 / 464.34), instead of filling the discovered-only map.
  The capture reported 5,554 discovered entries and one unreadable path while
  the scan was still active.
- `verification/storage-final.bmp` is the native 640x480 client capture after
  shrinking the window. Navigation, Storage heading, drive picker, controls,
  explanatory text, and map viewport fit. The rail is resizable from 96 to
  180 px; the 640 px minimum client width leaves a responsive central region.
- A prior native C: scan of the pre-capacity prototype reached 33,167 entries
  without blocking the UI. It is not used as proof of the final map scale; the
  fixed-capacity C: capture above is the relevant rendering evidence.
- After the user's F: screenshots, launched the rebuilt portable package and
  inspected its native F: rendering. At 13,229 discovered entries and two
  unreadable paths, it showed nested, proportional folder blocks rather than
  the previous full-width bands. It reported 607.01 GiB discovered of 931.50
  GiB while scanning continued. This confirms progressive rendering, not a
  complete-volume timing measurement.

### Validation

- `cargo fmt --all` — passed.
- `cargo test --target-dir target\storage-tests --locked` — passed, 14 tests. The storage tests verify
  top-level-before-descendant event ordering, cumulative root size, and normal
  completion on a temporary tree with large files, cancellation, cache
  round-tripping and volume identity, and stale-subtree reconciliation. A UI
  layout test verifies squarified area ratios and rectangle bounds.
- `cargo clippy --all-targets --locked -- -D warnings` — passed.
- `cargo build --release --locked` — passed. The standard generated release
  executable was rebuilt after closing the temporary native verification
  instance that held its previous image open. A separate fresh-target build
  also passed.

### Limits and presentation debt

- The observed C: scan was still running; a complete whole-volume pass and
  long-duration responsiveness were not measured. Cancelling is cooperative;
  a worker inside a blocking filesystem operation may finish that call before
  it observes cancellation, but it never blocks the UI thread.
- Sizes are logical file lengths, not allocated clusters. Sparse/compressed
  files and hard links can make the sum differ from physically occupied disk
  space. Hard links are currently counted once per encountered path. The
  remainder tile combines actual free bytes with unscanned file bytes, so it
  must not be read as a single measured category.
- Reparse points are deliberately not followed. Their target contents are not
  included through that path. Access denied and disappearing entries are
  counted as unreadable and omitted, not reported as zero-size measurements.
- If the volume capacity API fails, the root map clearly reports that its scale
  is unavailable rather than filling the view with discovered-only sizes.
  Header/status wrapping at still smaller widths, DPI variants, and scan cost
  on very large volumes need additional presentation/runtime work.
- The cache improves first display on repeat scans; scan-time reduction from
  reusing cache entries is not yet implemented. It does not skip directories.
  USN Journal optimization, deletion, cleanup, and filesystem-changing actions
  remain excluded. Explorer opening is the only external process launch.
- The native F: pass was still active at the 13,229-entry rendering check, and
  no completed F: snapshot was written during this verification. Cache file
  round-trip and volume-identity behavior passed in the isolated storage test;
  a second live F: launch showing cached-first refresh remains unverified.

## 10 October 2026 — Public preview preparation

Owner authorized public source and portable distribution. Documentation-only
publication changes reuse the native release verified below; no application
code or dependency changed. Source history comprised 14 commits before the
publication commit. Reviewed historical file paths and scanned all reachable
commits for common GitHub/AWS token, private-key and API-key assignment patterns;
no matches. This is a targeted review, not a guarantee from a dedicated secret
scanner. No raw inventories, logs or captures are tracked.

The portable folder contains 322 files: the verified executable, README, MIT
licence, project documentation, dependency inventory and third-party/font
licence texts. Checked for unexpected runtime/config/probe files; none found.
The executable SHA256 remains
`CA1F277F544CB5D92153EBFB099C0096C86C95DE62A5BD36023A5C20ED94346E`.
Distribute as `MachineGlass-0.1.0-windows-x64.zip` with a separate SHA256 file,
marked as an early prerelease. ZIP entry/content verification and anonymous
GitHub repository/release/download checks are required at publication; raw
results remain local under `verification/`.

Publication completed and independently checked without authentication:
repository and release pages returned HTTP 200; the GitHub API reported public
visibility and Issues enabled. v0.1.0 is a published prerelease with the ZIP and
checksum assets, targeting commit `57eda7fd772fbe6207b6f30c016e5c12e96732fd`.
All 322 ZIP file entries matched the prepared folder byte-for-byte. The anonymous
download matched local archive SHA256
`49D2FEF055EC2A0E261836FD3B283C7B51C4FC47A3F44981286A9BE51D0CA1BB`.
Local/GitHub main parity and clean status passed after publication; checked again
after committing this completion record. Raw result: local
`verification/publication-check.json`.

## 10 October 2026 — Always-on-top option

- `cargo fmt`, `cargo test --locked` (nine tests),
  `cargo clippy --all-targets --locked -- -D warnings` and
  `cargo build --release --locked` passed.
- Updated and launched `dist/MachineGlass/MachineGlass.exe`; actual native
  rendering showed the new checkbox below the sidebar navigation and all seven
  graphlets at the existing default size.
- Initial state: topmost false, Opera foreground above MachineGlass. After
  checking the option: `WS_EX_TOPMOST` true, foreground HWND 197170 (Opera),
  MachineGlass HWND 134068 unfocused and above the foreground window in native
  z-order. Screenshot also showed its inactive title bar and checked option.
- Clearing the checkbox removed `WS_EX_TOPMOST`; checked it again for use.
  Read-only window-state evidence is local under `verification/pin-*.json`.
  Browser activation via Computer Use timed out on app approval; the observed
  foreground and z-order comparison confirms the result independently.
- Option uses `ViewportCommand::WindowLevel` on change only, defaults off on
  relaunch, and adds no collector or polling loop. Tray round-trip retention was
  not separately exercised in this check. No new overhead benchmark was run.
- Portable and release executable SHA256:
  `CA1F277F544CB5D92153EBFB099C0096C86C95DE62A5BD36023A5C20ED94346E`.

## Build and correctness

- `cargo test --locked`: four tests passed. CPU deltas, missing/reset counters,
  PID reuse, configuration bounds, history eviction and unavailable graph points.
- `cargo clippy --all-targets --locked -- -D warnings`: passed.
- `cargo build --release --locked`: passed, native MSVC release.
- PE import inspection: Windows system DLLs and OpenGL only; no external
  Visual C++ runtime DLL, browser, Python or Node runtime.
- Native dashboard, filtered process list and self-observation view captured
  from egui's actual framebuffer and visually inspected. The inspection caught
  and corrected process-table column alignment. No browser preview was used.
- Graceful native window close exited in approximately 108 ms and wrote the
  clean-shutdown log entry.

Cargo intermittently reported access-denied errors for incremental/build lock
files in the restricted execution environment. Retrying the release build
outside that restriction succeeded. The test assertions passed; no runtime
dependency was installed to work around this.

## Measurement cross-check

The direct Windows collector and independent Windows CIM reading agreed on
physical memory capacity: **34,228,498,432 bytes**. Available memory differed by
5,373,952 bytes (about 5.1 MiB), with the CIM reading taken after the probe.
Sixteen logical processors were reported. The probe successfully obtained its
own executable path, creation identity, CPU delta and working set.

The probe reported 17.15% total CPU while sequential Windows performance-counter
samples reported 12.70% and 9.49%. These are different measurement intervals;
this verifies live, non-placeholder readings but **does not establish exact
time-aligned CPU agreement**. A synchronized workload comparison remains future
collector validation. Idle-time CPU percentage also need not equal Task Manager's
processor utility metric; see [Microsoft's explanation](https://learn.microsoft.com/en-us/troubleshoot/windows-client/performance/cpu-usage-exceeds-100).

## Observed footprint

One dashboard instance, after startup, observed for approximately 15 seconds:

| Metric | Observation |
|---|---:|
| CPU, normalized to all 16 logical processors | 0.163% |
| Working set at end | 105.2 MiB |
| Private bytes at end | 195.2 MiB |
| Handles at end | 304 |
| Release executable | approximately 5.15 MiB |
| Typical observed acquisition time | 16–19 ms |

These are short observations, not a performance guarantee, a leak test or proof
that the minimum-footprint goal is complete. The renderer's allocated memory
still deserves profiling. The self-monitoring view visibly showed its own
process and acquisition timing.

## Packaging and limits

The portable package contains the executable, documentation, an inventory of
126 resolved Windows dependency packages (including build dependencies), and
provided licence / font notice files. A later package audit found omitted
upstream licence texts in several registry archives; the tray increment below
fixes their inclusion. The registry lockfile includes additional-platform packages
that are not part of the resolved Windows graph.

GPU / VRAM, temperatures, storage, network, services, process trees, historical
process correlation and trace persistence remain unimplemented. No installer,
signing, public release or Steam integration is claimed. Protected-process
access, disappearing processes, unsupported CPU groups and source errors are
handled in code; a comprehensive permission / hardware / fault matrix is not
yet tested. Sorting controls and process-detail interactions still need broader
hands-on acceptance beyond the rendered filtered-list check.

Local raw probe output, comparisons and native framebuffer captures are under
`verification/` and excluded from Git. The source was initially checkpointed
locally; on 4 October 2026 the upload scope was corrected to include the source
and development history in the private GitHub repository.

## Stethoscope icon and system tray increment

- Original teal / blue stethoscope artwork added as SVG, PNG, raw RGBA and a
  multi-resolution Windows ICO (16, 32, 48, 64, 128 and 256 pixels).
- The icon is embedded in the executable and used by the native window and tray.
- Native screenshot inspection confirmed the window icon and live dashboard.
  The runtime log confirmed successful native tray registration.
- All four existing collector/history tests pass, Clippy passes with warnings
  denied, and the release build succeeds with the icon resource required.
- Show / Hide / Quit and close-to-tray paths are implemented. Interactive tray
  menu and close/restore/quit verification remains incomplete: the Computer Use
  helper detected active user input and blocked the attempted close action.
  No user shell restart was performed to test Explorer recovery.
- The new release is installed in the existing portable package and running.
  These observations do not supersede the earlier overhead measurements; the
  tray build has not yet received a separate overhead benchmark.
- The package audit found several crate archives without their standalone
  licence texts. Notices were retrieved from the exact commits recorded in
  those crates, stored locally with provenance, and added as packaging fallbacks.
  Packaging now rejects unrecognized missing notices instead of silently
  leaving an incomplete redistribution bundle.

## Tray shutdown investigation — 4 October 2026

The user reported that tray Quit did not finish and Task Manager was needed.
The earlier seconds-only log contains an eleven-second gap between a Quit
request and collector cleanup, but lacks process IDs and stage markers. That
entry cannot establish the cause or be attributed confidently to this report.

Added diagnostic markers without changing the Quit/close-to-tray policy:
millisecond Unix timestamps and PID on log entries, UI observation of Quit and
close events, framework on_exit, app destruction, collector stop/join timings,
actual tray-icon destruction timing, and return from the native event loop.
No process inventory is included. Old log entries retain their earlier format.

Five tests pass, including a real Windows collector check that shutdown
interrupts a ten-second sampling wait in under one second. Formatting, Clippy
with warnings denied, and the release build pass. A native tray-menu reproduction
was subsequently performed by the user, as recorded below. This diagnostic
increment is not a confirmed fix for the earlier intermittent failure.

The user reported a quick successful tray Quit on the diagnostic build. For
PID 15644, its log records 996 ms from the tray callback to event-loop return:
959 ms before the UI observed Quit, 16 ms from that observation to on_exit,
1.1 ms for collector shutdown, and 11.2 ms for tray-icon removal. No MachineGlass
process remained afterward. This verifies one complete graceful exit; the
earlier hang was not reproduced and its cause remains unresolved.

## Hidden-window tray recovery — 4 October 2026

A later shortcut-launched instance reproduced the user's inaccessible-window
report after X hid the window. Its primary log contained no entries for that
PID, despite the shortcut targeting the current binary and the process using
the expected LOCALAPPDATA. The reason for the failed logging remains unknown.

Tray restore now shows the native HWND directly with `ShowWindow(SW_RESTORE)`
before queued viewport commands; Quit uses that wake-up before graceful close.
A real native-window regression verifies visibility without an egui frame.
Primary-log failures fall back to `%TEMP%\MachineGlass-<PID>.log`, recording
the primary error. Both paths remain bounded and contain no process inventory.

Formatting, six release tests, Clippy with warnings denied, release build and
packaging passed. The native dashboard framebuffer was inspected. Computer Use
clicked X on the replacement window; the visible window disappeared, the app
remained alive, and the log recorded close-to-tray. The user subsequently ran
the manual lifecycle, including exit/restart, and reported success. This supplies
manual acceptance in addition to the isolated native-window regression.

For the replacement instance, the log records 77 ms from tray Quit to native
event-loop return, with native visibility confirmed before the UI observed Quit.
Collector shutdown took 1.2 ms and tray-icon removal 20.2 ms. No MachineGlass
process remained at the subsequent check. This verifies the reported workflow;
broader shell-restart and Windows-session coverage remains future work.

A test-process fallback log records Access denied on primary-log writes and
successful fallback entries. This verifies the fallback path, but does not
attribute the earlier shortcut-launch logging failure to a proven cause.

## Basic WDDM GPU validation — 9 October 2026

Source: Windows PDH `GPU Engine(*)\\Utilization Percentage` (per-process slices)
and `GPU Adapter Memory(*)` Dedicated/Shared Usage; DXGI hardware adapter
enumeration, LUID identity and memory capacity. References:

- [Microsoft GPU/Task Manager measurement explanation](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/)
- [PDH wildcard formatted arrays](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhgetformattedcounterarrayw)
- [DXGI adapter description](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ns-dxgi-dxgi_adapter_desc)

The native provider detected AMD Radeon RX 7800 XT and eight engine identities:
3D, Copy (two separate IDs), Compute 0, Timer 0, Security 1, Video JPEG 0 and
Video Codec Engine. Labels were obtained from Windows, not configured mappings.
Runtime acquisition and native launch were medium integrity (S-1-16-8192), with
no elevation request. The tool sandbox could not see GPU performance counters;
ordinary desktop execution could. This is a verification-environment boundary,
not a requirement for an elevated MachineGlass process.

| Required check | Evidence |
|---|---|
| Idle GPU behaviour | Compute 0 measured 0.0% before and after compute; low desktop activity remained. No fake zero or average. Background video prevented claiming a fully quiescent GPU. |
| Normal desktop / 3D | Native probe measured 3D around 1.8–2.8%; Task Manager showed about 3%. Video engine activity correctly drove overall utilization around 5–6% when it exceeded 3D. |
| AI compute workload | Used the already-loaded local nomic-embed-text model through its existing HTTP service, with batches of 128 synthetic documents; no download or Python. First run: 10 batches / 40.9 seconds, Compute peak 61.89%. Second: 17 batches / 66.3 seconds; 120 native samples including recovery. |
| Task Manager comparison | During the second run Task Manager visibly showed Compute 0 spikes, 44% Compute / 44% overall and 6% 3D in one snapshot. The native MachineGlass framebuffer showed 60.4% Compute / 60.4% overall and 5.8% 3D in another. Both identify Compute 0 as dominant and show the same burst pattern. These are different one-second intervals, not proof of exact simultaneous numerical equality. |
| VRAM allocation with compute idle | Before compute, approximately 3.01 GiB dedicated and 0.36 GiB shared with Compute 0 at zero. After compute, about 2.90 GiB dedicated remained while Compute was zero. Task Manager displayed approximately 3.0/0.4 GB in the low-activity observation. |
| Stop / recovery | Second trace: first post-stop sample retained 17.75% from its overlapping interval; next sample was 0%, about 1.7 seconds after completion. All remaining 55 samples had Compute 0 at zero. |
| Overhead | Previous portable build, 20-second observation: 0.161% total-machine CPU, 90.7 MiB working set, 181.4 MiB private, 298 handles. Initial GPU build: 0.137% CPU, 99.6 MiB working set, 199.4 MiB private, 326 handles. Final installed-build observation is recorded below. |

Second-run 64 compute-phase samples: Compute maximum 61.22%, 3D 1.36–7.47%,
dedicated memory 2.83–2.95 GiB. Acquisition averaged 21.75 ms during compute and
20.22 ms across 56 recovery samples. Probe samples include a one-second sleep
plus acquisition time; production worker subtracts acquisition time from its
configured one-second interval. Configuration remained 1000 ms / 1800 seconds
for CPU/RAM; GPU retention is independently bounded to 90 seconds / 91 samples.

The actual native framebuffer was inspected at the default 1180x800 size. The
final layout keeps all three GPU graphs, adapter name, dominant label, dedicated
and shared memory and temperature availability visible together. It retains CPU,
RAM and process inspection. User tray Quit of the comparison instance was later
observed in the lifecycle log: collector shutdown 1.3 ms, tray removal 1.7 ms,
and event-loop return about 80 ms after Quit. No tray redesign was made.

DXGI dedicated capacity measured 16,963,133,440 bytes (15.80 GiB); Task Manager
showed 16.0 GB physical with 207 MB hardware-reserved. Shared limit measured
17,114,249,216 bytes (15.94 GiB). Shared available is limit minus adapter usage,
not free physical RAM. Temperature is explicitly unavailable, even though Task
Manager itself can display it; the app currently has no reliable temperature
provider. We did not add a vendor/driver dependency to obtain it.

Formatting, all nine tests, Clippy `--all-targets --locked -- -D warnings`,
release build and portable licence packaging passed. Tests cover same-engine
process aggregation, adapter isolation, multiple compute maxima, missing/zero
semantics and 90-second history bounds. Tests do not replace hardware acceptance.
No claim is made for physical multi-GPU, integrated-GPU accounting, non-English
Windows, driver reset/hotplug, sustained leak behaviour, fully idle desktop,
heavy 3D gaming, exact synchronized agreement or temperature support.

Local raw evidence: `verification/gpu-desktop.json`, `gpu-workload.jsonl`,
`gpu-comparison.jsonl`, `comparison-summary.csv`, `gpu-final.bmp`, baseline/GPU/
final overhead JSON files. Runtime captures and inventory remain excluded from
Git and portable packaging. Storage, network and recorder remain unimplemented.

### Final installed build and matched overhead comparison

Final installed build, separate 30-second observation: 0.475% CPU, 91.7 MiB
working set, 191.2 MiB private, 321 handles. Because this CPU reading exceeded
the first short comparison, rebuilt the previous committed source revision
under ignored `verification/baseline-source` and measured both executables
together over the same 30-second desktop interval, after the build completed:

| Build | Total-machine CPU | Working set | Private bytes | Handles |
|---|---:|---:|---:|---:|
| Previous committed source | 0.381% | 112.8 MiB | 197.7 MiB | 345 |
| Installed GPU build | 0.345% | 102.3 MiB | 191.9 MiB | 360 |

No material CPU/memory increase was observed in the matched interval. Different
short observations vary with desktop/render activity; the earlier 0.475% is
retained rather than discarded or assigned an unproven cause. This remains a
short footprint check, not a sustained leak/performance guarantee. The matched
baseline instance was stopped afterward; the installed GPU build remains running.
The final installed native window was inspected as well as the compute framebuffer.

Portable and release executable SHA256:
`D69B2A8C7145BF6231D14BB4BEBF3290C127CD946F2849293B152365A039FC4E`.
The portable executable is 5,818,880 bytes. Packaging preserved dependency/font
notices; raw verification files are not distributed. Source changes were still local and uncommitted at this verification stage;
the subsequent requested checkpoint is recorded below.

## Compact dashboard — 9 October 2026

Native default-size framebuffer `verification/compact-default.bmp` inspected:
CPU and RAM in row 1, dedicated/shared GPU memory in row 2; Overall GPU Activity,
busiest Compute and 3D in one row beneath. The new memory graphlets use the same
measured adapter identity and 90-second history as engine activity, normalized
to dedicated total or shared limit. The cards show used / total (or limit) and
remaining available bytes. Missing totals/usage or zero denominators produce
graph gaps rather than invented percentages. Acquisition remains unchanged.

Measured rendered cell heights: CPU/RAM 139 pixels versus prior 277 (~50%
reduction); GPU activity 119 versus prior 227 (~48% reduction). Compact memory
cells match the CPU/RAM height. Smaller padding and text leave room for actual
graphs, 0–100% scale and unavailable-gap captions.

Default client 944x720; minimum client 832x640 (previous minimum 1040x720).
Computer Use resized the real native window to the minimum; returned outer
frame was 834x672, consistent with client 832x640 plus native border/title bar.
Scrolled to the top and verified all seven cards and their graphs fit vertically.
CPU/memory value labels, GPU name, dominant label and temperature status remain
readable. Process inspection at minimum width retains all five columns and
search/sort controls. Vertical scrolling still exposes processes and expanded
engine details; unavailable reasons may naturally wrap to extra lines.

`cargo fmt`, all nine locked tests, locked all-target Clippy with warnings denied,
and locked release build passed. This is a presentation change; the earlier
GPU workload/provider verification remains applicable. Multi-adapter native
layout and additional DPI scales have not been physically tested. Private
framebuffers stay excluded from Git. Source was local and uncommitted at this verification stage.

## Tighter cell typography — 9 October 2026

Inspected `verification/tight-cells.bmp`, the actual 944x720 native framebuffer.
All seven graphlets render with numeric values at the same 11-point size as
cell titles. Removed the repeated history/scale captions from the layout and
retained that information as a graph hover tooltip. CPU/RAM and GPU-memory cells
are 113 pixels high (previous 139); GPU activity cells are 93 (previous 119).
Graphs retain their prior heights and measured histories. The original compact
2x2 memory/CPU grid and three-cell activity row remain unchanged. Sampling,
providers, aggregation, missing-value handling and minimum size are unchanged.

Formatting, nine locked tests, all-target locked Clippy with warnings denied,
and the locked release build passed. Portable packaging was refreshed with
licence notices. Raw capture remains ignored. Changes were uncommitted at this verification stage.

## Requested GitHub checkpoint — 9 October 2026

User requested the accumulated GPU and compact-display changes be checkpointed
and pushed. Reviewed all intended source changes and refreshed development-log
coverage. Re-ran formatting check, all nine locked tests, locked all-target
Clippy with warnings denied and locked release build; all passed. No code was
changed during this checkpoint review. Runtime captures, probe traces, baseline
source/builds and distribution binaries remain excluded from Git.

The final compact portable/release executable SHA256 is
`7E6611DEE97CD93D39086C151873FD5C7D57BDF0A019A3D25C2DAF573D5D3DCF`.
Earlier hashes above describe earlier verified builds, not the final artifact.
The requested checkpoint uses main at PopeKael/MachineGlass. After commit and
push, verify HEAD against both fetched origin/main and the live GitHub main ref,
and require an empty tracked/untracked worktree status. Exact resulting commit
identity is available in Git history and the checkpoint completion response.

## 17 October 2026 — Storage cold-picture verification

### Finding

The prior startup path treated the detailed inventory as the UI model. The
manager decoded `Entry` records and applied 256 at a time into the same tree
used by the renderer. Parent/child indexes therefore grew progressively, and
egui repeatedly laid out an incomplete hierarchy. The cache is emitted from a
hash map, so early visible items were not a meaningful render order. The
one-time “cached map displayed” diagnostic was triggered by the first nonempty
partial tree; it did not mean the full picture existed. In one instrumented
cold run, it logged 1,536 entries before the eventual 391,065-record load.

### Persisted representation and native results

Added a versioned JSON render sidecar per drive. It stores parent-indexed
hierarchy nodes, labels, kinds, cumulative sizes and aggregate counts, volume
identity, root capacity and capture time. It keeps the renderer's top 79 nodes
per directory plus an “Other discovered items” aggregate and nested descendants
through the renderer's existing eight-level limit. Full inventory remains in
the existing `.json` file. All valid sidecars load before detailed caches.
Cached inventory populates a separate inventory tree; refresh retains and
updates the render tree.

| Drive | Sidecar bytes | Render nodes | Snapshot parse | UI map available |
|---|---:|---:|---:|---:|
| C: | 7,703,968 | 70,770 | 201 ms | 430 ms |
| D: | 3,993,466 | 34,507 | 77–88 ms | 409 ms |
| E: | 234,980 | 1,936 | 3–5 ms | 408 ms |
| F: | 2,900,571 | 27,101 | 56–67 ms | 392 ms |
| G: | 1,459,107 | 13,122 | 27–32 ms | 402 ms |

Times are from real release launches, manager start to first-frame UI log for
availability. Internal framebuffer capture then showed the same complete maps
for C: through G:. The C: image includes the familiar root folders and the
large pagefile tile from the first picture. The delayed C: native capture, after
the refresh log reported `existing_inventory_entries=391026` and
`cached_render_snapshot=true`, showed the complete map still present while
refresh was active. These are actual rendered pictures, not acceptance by entry
count alone. Captures are local at
`C:\Users\Warren\.codex\visualizations\2026\10\16\01a14723-c9b1-75a1-adf4-c9f87f4ac24f\storage-C-cold-render-final.bmp`,
`storage-D-cold-render-final.bmp` through `storage-G-cold-render-final.bmp`,
and `storage-C-during-refresh-final.bmp`.

F: cache records had a 12 KiB root total and zero-valued directory totals above
nonzero descendants. The first faithful sidecar therefore looked blank, as did
an older native capture. The final builder derives each directory's total from
cached child sizes, preserving any larger cached value. The final F: picture
shows the expected SteamLibrary and AI trees. Inventory JSON was not rewritten
by this correction.

During the verification, a leftover running verification instance checkpointed
the actual C.json. Its final observed state is 144,705,145 bytes / 391,026
entries, identity-valid, SHA256
`C42E486377FC8377093134B967E988307EB1B120CF99B6C15425E4798DB1AC21`. That
checkpoint was retained. The supplied baseline was approximately 144,736,179
bytes / 391,065 entries. The final C sidecar was generated from the current
valid inventory. No verification app instances remain running.

### Validation and boundaries

`cargo fmt`, full `cargo test --locked` (18 library tests plus one native layout
test), `cargo clippy --all-targets --locked -- -D warnings`, and
`cargo build --release --locked` passed. One earlier full-test attempt hit
MSVC `LNK1201` writing the unrelated `collector-probe` PDB; the final retry
passed. The render
sidecar is ordinary JSON and does not alter the detailed cache format. The
duplicate scanner cache decode remains unchanged. No SQLite, compression, USN
Journal work, binary cache, or memory-ownership redesign was introduced.
