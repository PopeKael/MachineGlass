# MachineGlass development log

Keep this log with each project checkpoint: what changed, why, what was
verified, and what remains unresolved. New entries go first. Record useful
findings as well as features; distinguish a successful observation from a
proven fix. Keep raw logs, inventory and captures local under `verification/`.
Detailed measurement evidence belongs in [verification](VERIFICATION.md).

## 22 October 2026 — Storage-map checkpoint validation

Revalidated the pending persisted-map follow-up before synchronizing the
checkpoint. Formatting, all 19 locked tests, all-target Clippy with warnings
denied, and the optimized release build passed. Refreshed the authoritative
`dist\\MachineGlass\\MachineGlass.exe` package; its SHA-256 matches the
release binary at
`BA85D588A3C383B1FEB1E3C357F36594EE2D8C8ED8A5FDBAECA0612DD11578DA`.

Launched that packaged executable against the real user data directory and
captured the native Storage page for F:. The displayed saved map contained
116,298 inventory records and the expected `AI\\WSL\\Ubuntu-24.04\\ext4.vhdx`
tile at 56.97 GiB. The screenshot is a local verification artifact under
`target/` and is not tracked. The saved map rendered with refresh queued; this
short acceptance run did not complete a new full-volume F: scan.

## 17 October 2026 — Refresh shortcut executable

The shortcut target at `dist\\MachineGlass\\MachineGlass.exe` was stale: its
SHA-256 did not match the current optimized release. Rebuilt and packaged the
current source into that exact portable folder. The shortcut executable now
matches `target\\release\\MachineGlass.exe` at SHA-256
`BA85D588A3C383B1FEB1E3C357F36594EE2D8C8ED8A5FDBAECA0612DD11578DA`.

Launched the exact packaged path with the Storage page selected and the real
user cache directory supplied explicitly (the verification sandbox redirects
`LOCALAPPDATA` by default). The 944×720 native capture showed the complete
saved C: treemap, “Cached map · verifying…”, and “Refreshing saved map” while
the scan was active. The run ended after capture, before the five-minute
checkpoint; the existing C: inventory and treemap sidecar retained their
recorded file timestamps. Capture is kept outside Git. `cargo test --locked`
(18 library and one layout test), all-target Clippy with warnings denied, and
the locked release build passed. `cargo fmt --all` could not enumerate the
workspace in this environment and returned Windows access denied.

## 17 October 2026 — Cold-start cached Storage map verification

Investigated the report that C: appeared to restart as an empty scan after a
cold application restart. The real `%LOCALAPPDATA%` C.json was found and
identity-validated (144,736,179 bytes, 391,065 entries). The manager published
all 391,065 entries into drive session 1 and the Storage UI rendered that same
session at 2.210 seconds after manager startup. Background refresh began only
after all drive caches had been published, at about 5.262 seconds, and logged
`existing_entries=391065 cached_snapshot=true` for the same session. Selecting
D: and returning to C: during refresh retained session 1, all entries, and the
visible C: treemap. D:, E:, F:, and G: were also selected in the native UI and
each displayed its own cached session.

The map was not replaced, cleared, or rebuilt from an empty scanner tree. The
observed delay is the existing cache pipeline: C: is decoded in 644 ms, then
its 391,065 entries are applied progressively to the UI model over about 2.2
seconds. The scanner starts later and independently deserializes the cache
again for scan reconciliation; that duplicate read remains unchanged for the
separately requested memory investigation. Added concise lifecycle logs for
cache validation/entry count, manager publication, refresh reuse, and UI
display/selection, each correlated by per-drive session ID. The actual C.json
SHA-256 and byte count matched before and after the cold run. No cache, map,
refresh, or presentation behavior was changed; the evidence does not show a
tree replacement bug requiring a functional fix.

Validation: `cargo fmt`, 16 locked tests, Clippy with warnings denied, release
build, actual cold launch against the user's caches, native C: display during
active refresh, and native D:/E:/F:/G: selection followed by C: reselection
during refresh. Native captures, raw logs, and cache samples remain outside
Git. Detailed timings and limitations are in
[verification](VERIFICATION.md).

## 17 October 2026 — Persistent Storage cache benchmark

Benchmarked the current JSON caches and refresh path before considering
storage changes. All five real cache files were copied to a private staging
directory and MachineGlass was launched with that directory as
`LOCALAPPDATA`; the original files remained unchanged. Timings are from an
optimized Windows release. Cache-load durations cover deserialize, validation
and entry collection. “Map usable” is measured when the UI thread has applied
all bounded-channel entry chunks for that drive. Startup memory was sampled
every 125 ms; the single-instance refresh sample used one-second process
memory, CPU and Windows process-I/O counters.

| Drive | Cache bytes | Entries | Deserialize | Load/validate | Map usable since manager start |
|---|---:|---:|---:|---:|---:|
| C: | 144,736,179 | 391,065 | 524 ms | 666 ms | 2.248 s |
| D: | 21,443,720 | 69,665 | 88 ms | 122 ms | 3.232 s |
| E: | 713,465 | 2,846 | 2 ms | 10 ms | 3.257 s |
| F: | 41,610,699 | 116,298 | 160 ms | 204 ms | 4.402 s |
| G: | 26,761,811 | 86,261 | 95 ms | 130 ms | 5.241 s |
| H: | no cache | 0 | — | 0 ms | 5.261 s |

The five snapshots total 235,265,874 bytes and 666,135 entries. Process
private/working memory before cache loading was 192.2/84.4 MiB. After each
map became usable it measured: C: 716.1/593.9 MiB; D: 815.6/690.7 MiB; E:
815.6/690.7 MiB; F: 977.8/854.1 MiB; G: 1,059.7/938.6 MiB. H: had no
snapshot and did not materially change memory. The all-map increase over the
pre-cache process was about 868 MiB private memory. The scanner then loaded C:
again; the short startup sample reached 1,206 MiB private memory during that
second load. During the separate five-minute refresh, steady private memory
peaked at 1,407 MiB before checkpoint and at 1,433 MiB around checkpoint.
One-second sampling observed about 26 MiB of additional private memory around
the save; it may miss a shorter peak.

The checkpoint cloned 391,065 records in 314 ms. The current streaming writer
completed serialization plus buffered temporary-file writes in 417 ms, flush
in under 1 ms, `sync_all` in 124 ms, and atomic replacement in 15 ms; total
time from clone start through replacement was 874 ms. It wrote a 144,736,215
byte staged C: cache. Because `serde_json::to_writer` streams into the
`BufWriter`, serialization CPU and temporary-file writes are interleaved and
cannot be separated in this path. A separate staged phase probe measured
serialization to memory at 232 ms, writing the resulting 144,736,209-byte
buffer at 121 ms, flush under 1 ms, sync at 10 ms, and replacement over an
existing staged file at 14 ms. That probe allocated the serialized buffer; it
is not the current streaming save implementation. Its process peaked at
580,005,888 private bytes, including the parsed cache and 144.7 MB output
buffer.

The current scanner does deserialize a drive cache again after
`StorageManager` has loaded it. `ScanHandle::start` receives only the `Drive`,
then independently calls `load_cache` to create its cached entry vector and
fingerprint for scan reconciliation. The manager’s already decoded entries
are retained in its `StorageSession` and are not passed into the scanner. For
C:, the second deserialize took 690 ms and the full scanner load/validation
took 822 ms. This repeats per drive as each sequential scan begins; only C:
was reached in this benchmark.

During a single-instance refresh, MachineGlass process CPU averaged 16.6% of
one logical core equivalent (95th percentile 24.6%, maximum 57.7%) over 294
one-second samples. These are whole-process figures, including telemetry and
UI work, not scanner-thread-only CPU. Process I/O counters recorded 362.4 MiB
read across startup cache loading and the duplicate C: load, no additional
read-transfer bytes after the first 12 seconds, and 138.0 MiB written by the
checkpoint. Directory enumeration metadata is not represented by those byte
counters. Windows `PhysicalDisk` performance counters were unavailable in
this environment (`c0000bc8`), so physical-device throughput was not measured.
An actual native frame captured during refresh showed the cached C: map,
“Background refresh · active”, and a telemetry sample updated 0.2 seconds
earlier; this establishes a responsive rendered frame, not input-latency
coverage. The five-minute scan encountered access denial at
`C:\Windows\WUModels` and did not complete a full-volume refresh.

**Assessment:** JSON loading and checkpoint latency are sub-second per large
C: cache and do not provide evidence for a compact persistent format. The
resident tree is the larger cost: five maps added about 868 MiB private memory
above the pre-cache process, and the process reached 1.43 GiB during refresh
and checkpoint. A compact file format alone would not reduce that decoded
tree. The smallest follow-up candidate is to investigate reusing the manager’s
decoded snapshot in `ScanHandle`; the duplicate C: parse costs 690 ms and
creates another cached entry vector. Measure its memory effect before
implementing it. No cache format or architecture changes were made in this
benchmark pass. Temporary source instrumentation and the helper probe were
removed; staged copies, sample CSVs and screenshots remain outside Git.

## 17 October 2026 — Persistent multi-drive Storage manager

Storage now starts with MachineGlass rather than with the Storage page. A
low-priority cache loader reads each local drive's existing JSON snapshot on a
worker and publishes its entries into a per-drive `StorageSession`; after the
cache pass, one low-priority filesystem scanner refreshes drives sequentially.
The manager and maps stay alive while the page is closed, inventory is checked
once per minute for removable-drive changes, and a completed drive cycle waits
one hour before the next pass. Selecting a drive changes only the displayed
model. The treemap remains visible while its saved entries are reconciled with
the live filesystem.

Removed the 64 MiB cache-size rejection. Cache loading now validates schema,
volume serial, root, capacity, and every entry's path/parent relationship;
corrupt and incompatible files retain a specific diagnostic. JSON remains the
format. Checkpoints use the existing flushed temporary-file/atomic-replace
path every five minutes during a scan and at scan exit, including interrupted
progress. The cache records the last completed refresh time; older snapshots
use their file modification time, matching the old save-after-completion
behavior.

**Verification:** On this machine the existing 144,736,179-byte C: cache
loaded as a valid 391,065-entry snapshot. The existing D:, E:, F:, and G:
snapshots also passed identity checks (69,665; 2,846; 116,298; and 86,261
entries respectively). A release native capture showed the C: treemap
immediately while its background refresh remained active. The refresh was
allowed to run for 34 seconds; the full C: scan was not completed in this
verification. A separate regression test loads valid JSON padded beyond 64
MiB. `cargo fmt`, all 15 library tests plus the UI layout test, all-target
Clippy with warnings denied, and the optimized Windows release build passed.
Captures and staged private runtime cache copies were kept outside the repo.

**Still open:** Full-volume completion timing and a complete changed-file
reconciliation were not measured. Legacy cache timestamps are necessarily
coarse file modification times. During the initial cycle the cache loader and
the scanner each deserialize a drive's JSON snapshot on their own worker; this
is correct but duplicates the initial parse. JSON parse and atomic checkpoint
cost remain unbenchmarked for the full 145 MB snapshot.

## 14 October 2026 — Storage refresh diagnostics and stable cached sizing

The user observed cached F: treemap boxes changing during a repeat scan despite
no known filesystem changes. The refresh had been rebuilding directory totals
from only the bytes found so far, making cached totals temporarily collapse
before the worker reached deeper descendants. Directory totals now start from
the cached estimate and adjust by discovered file/aggregate deltas; unchanged
refreshes preserve the displayed proportions. Small-file group sizes are
committed after their directory enumeration so the cached group does not shrink
to a partial count. Completed scans compare a stable fingerprint of the old and
new tree and report “Scan complete · no changes” when equal. If any locations
were unreadable, the scan reports that explicitly and does not claim the map
was unchanged because cached data may have been retained below those locations.

Storage diagnostics now log cache hit/miss and identity-rejection reasons,
scan start/completion, cache-save outcome, and each unreadable path, operation,
and OS error. Hovering the unreadable count displays the paths and reasons
(first eight, plus a remainder count). This distinguishes access denial,
enumeration failures, metadata failures, and transient filesystem errors
without interpreting hidden/system attributes.

**Verification:** An added repeat-scan regression fixture includes nested and
small files; it verifies stable directory totals, identical cache fingerprints,
and the no-changes result. All 15 locked tests, formatting, all-target Clippy
with warnings denied, and the optimized Windows release build passed. Captured
and inspected the actual release Storage page at its initial drive-selection
state. The unreadable hover popup was not visually captured. The existing
`dist\MachineGlass` package was refreshed after its running copy closed; its
executable SHA256 matches the release build.

## 14 October 2026 — Progressive Storage map

Added the first native Storage page. Drive enumeration and volume capacity/free
space use Windows APIs; directory enumeration uses a cancellable Rust worker,
a bounded message channel, and a normalized `StorageTree`. The UI polls the
session and paints the current tree on the egui thread. It shows the root
treemap against the drive's fixed total capacity, with a neutral “Free + not
yet mapped” remainder. Directories subdivide as their discovered sizes grow;
files below 512 KiB are grouped per directory and excess visible items are
grouped for drawing. The view supports directory drill-in/parent navigation,
hover path/size, and opening the selected/hovered location in Explorer.

The first native C: rendering showed why a discovered-only treemap was wrong:
`pagefile.sys` temporarily occupied almost the whole map. The corrected map
uses actual volume capacity/free-space readings, so each new discovery grows
inside a stable volume frame. C: was observed at 464.34 GiB total and 199.13
GiB free; while still scanning, 5,554 entries represented 31.65 GiB discovered
and 432.69 GiB remained in the combined free/not-yet-mapped tile. One unreadable
path was reported without stopping collection. A 640x480 client capture showed
the Storage controls and map area remain usable at the reduced minimum.

Reduced the minimum client size from 832x640 to 640x480 and changed the fixed
156 px navigation rail into a resizable 96–180 px rail with width-filling
buttons. Existing typography and colors are preserved. No telemetry/provider
code or unrelated architecture changed, and no dependency was added.

Formatting, all 14 locked tests, all-target Clippy with warnings denied, and an
optimized Windows release build passed. The storage test verifies that
  top-level items are reported before nested files, cumulative directory sizes
  arrive, cancellation is reported, and the scan emits completion. Native C: rendering remained
progressive and the window stayed responsive while more than 5,000 entries
were reported. Detailed captures, dimensions, Windows edge cases, and
limitations are in [VERIFICATION](VERIFICATION.md).

**Still open:** observed C: scanning was in progress, not a verified complete
whole-volume inventory. File `metadata.len()` is logical length; sparse or
compressed allocation and hard links can make the treemap differ from actual
physical usage. The remainder intentionally combines actual free space with
items not yet mapped. Reparse points (including junctions and mount points) are
shown without descending into them to avoid loops/duplicate traversal.
Extremely deep trees can be drilled into, while drawing is capped at eight
nested levels and 80 direct items plus an aggregate tile. Broader responsive
polish for the header/status strip, DPI testing, and sustained scans remain
presentation/runtime follow-up work.

The first user review showed long bands of similarly sized tiles in deep
folders. The renderer now uses squarified area placement, stable folder colors,
and keeps file tiles in their parent folder's color. Scan events are coalesced
into batches, cumulative directory sizes are emitted periodically, and the UI
maintains a parent index instead of rebuilding it from every path each frame.
On Windows, `DirEntry::metadata()` reuses enumeration metadata and avoids the
extra path lookup made by the first scanner.

Added per-volume snapshots under `%LOCALAPPDATA%\MachineGlass\storage`. A
snapshot is loaded in the worker and shown while the live scan refreshes it;
drive root, volume serial and capacity must match. Completed directories prune
stale cached children, while inaccessible/incomplete directories retain their
cached subtrees. Only a non-cancelled scan atomically replaces the snapshot.
This improves repeat-launch first paint. The live pass still walks the full
tree; no USN Journal or timestamp-based scan skipping is used. Software-side
event and metadata overhead is reduced, but a comparative full-volume timing
against SpaceMonger has not yet been measured.

## 10 October 2026 — First public preview

User accepted the utility for everyday AI-workload monitoring and authorized
public GitHub visibility. Reframed the README around this modest, working use
case: compact CPU/RAM/GPU graphs, GPU memory and optional always-on-top display.
The original broader brief remains historical design context, not a promise of
future features. Feedback is invited through GitHub Issues.

Prepared v0.1.0 as an unsigned portable Windows x64 preview with the already
verified release executable and all dependency/font notices. Reviewed tracked
file paths across all 14 existing commits and scanned history for common secret
patterns (no matches). Raw captures, private runtime evidence and generated
binaries remain excluded from Git. No application code changed; the preceding
nine-test, Clippy, release and native-window verification remains applicable.
Publication checks cover ZIP contents/hash, public anonymous repository/download
access, exact local/GitHub commit parity and clean working-tree status.

Publication completed: repository visibility is public, Issues are enabled and
v0.1.0 is a downloadable prerelease. Anonymous repository and release-page
requests returned HTTP 200. Downloaded the ZIP without authentication and
confirmed its SHA256 matches the local archive; every one of its 322 file
entries also matched the prepared folder. Release tag targets publication
commit `57eda7fd772fbe6207b6f30c016e5c12e96732fd`. This final documentation
entry records the completed checks; the application binary is unchanged.

## 10 October 2026 — Optional always-on-top window

Added a sidebar checkbox using egui's native window-level command so the
monitor can remain visible while another application has focus. Clearing the
checkbox restores normal stacking. The option starts off and lasts for the
current application session, including tray hiding/restoring. Telemetry and
the compact dashboard layout are unchanged.

Validation: formatting, nine locked tests, Clippy with warnings denied and
release build passed. Inspected the actual portable native UI; read-only Win32
observations confirmed topmost enabled with Opera focused, and topmost cleared
after disabling. Updated the portable build. Detailed evidence is recorded in
VERIFICATION.md. User accepted the result and requested this GitHub checkpoint.
Reviewed the four changed source/documentation files; private runtime evidence
and packaged binaries remain ignored. The preceding validation applies to this
unchanged implementation. Commit and push target the existing main branch;
exact GitHub-tip parity and clean working-tree status are checked afterward.

## 9 October 2026 — GPU telemetry and compact dashboard checkpoint

User accepted the compact display and requested a GitHub checkpoint. Reviewed
all accumulated GPU-provider, normalized model/history, dashboard, probe and
documentation changes. The final UI has a 2x2 CPU/RAM/dedicated/shared-memory
grid above one Overall/Compute/3D row, title-sized values and graph tooltips.
The RX 7800 XT measurement comparison, workload recovery, allocated-memory
behaviour and matched overhead observations are preserved in VERIFICATION.md.
Temperature remains unavailable; no storage, networking or recorder work began.

Checkpoint validation: `cargo fmt --check`, all nine `cargo test --locked`
tests, `cargo clippy --all-targets --locked -- -D warnings`, and
`cargo build --release --locked` passed again. The final portable executable
matches the release SHA256 recorded in verification. Earlier native checks
cover actual default/minimum rendering and the narrower process page.
Reviewed source/documentation only; private runtime evidence, screenshots,
local baseline builds and portable binaries remain ignored. This checkpoint
uses the existing private repository's main branch without changing visibility.
Commit/push completion and exact remote-tip parity are checked after committing.

## 9 October 2026 — Tighter cell typography

Matched numeric reading text to the 11-point cell-title size and removed the
repeated graph-history captions. History/scale/gap information is available on
graph hover instead. Kept graph heights, measured data and the 2x2 plus three-cell
layout unchanged. Native rendering confirms CPU/memory cells reduced from 139
to 113 pixels and activity cells from 119 to 93 pixels (another 26 pixels each).
Formatting, all nine tests, Clippy with warnings denied and release build pass.
Verified the actual default-size framebuffer; refreshed the portable build.

## 9 October 2026 — Compact dashboard and GPU memory graphlets

Rearranged the overview into CPU / physical RAM above dedicated / shared GPU
memory (2x2), followed by the existing Overall / Compute / 3D row. Added both
GPU memory graphlets from the existing normalized 90-second history. They show
used memory as a percentage of the measured capacity/limit; missing readings
remain gaps. No provider, sampling or aggregation changes were required.

Reduced CPU/memory cells from approximately 277 to 139 pixels tall and GPU
activity cells from approximately 227 to 119 pixels. Reduced minimum client
width 20%, from 1040 to 832 pixels. New default client size is 944x720, minimum
832x640. Narrowed navigation and made the process-name column responsive so
process inspection still fits at the new width.

Validation: formatting, all nine tests, Clippy with warnings denied and release
build passed. Inspected native framebuffer at default size; used native window
resizing to verify all seven graphlets at 832x640. Checked the narrower process
page: name, PID, parent PID, CPU, working set, search and sort remain visible.
The expanded engine list and long error reasons can increase content height;
vertical scrolling remains available. Physical multi-GPU layouts remain untested.

## 9 October 2026 — Basic WDDM GPU telemetry

Moved basic GPU monitoring into the current milestone without beginning storage,
networking or Flight Recorder. The existing telemetry worker now uses DXGI for
adapter identity/capacity and Windows PDH GPU counters for utilization and
adapter memory. No vendor utility, Python, external runtime or elevation is
required. Monitoring remains read-only.

Every exposed engine is retained by adapter LUID, physical index and engine
index, with its actual Windows label. Per-process slices are combined only for
the same engine. Overall GPU Activity is the busiest engine, with separate
busiest-Compute and 3D series. Missing values remain unavailable; the first
sample warms up. GPU history is bounded to 90 seconds; CPU/RAM retention and
unrelated functionality remain unchanged. The dashboard includes dedicated and
shared memory and an explicit unavailable temperature status. Shortened GPU
graphs after native rendering showed memory readings below the default fold.

Validation: formatting, nine tests, Clippy with warnings denied and release
build pass. Tested engine grouping, adapter isolation, multiple Compute maxima,
measured zero versus missing, and history retention. The RX 7800 XT exposed eight
engines. Compared native rendering and Task Manager during normal desktop use
and bounded AI embedding workloads. Both show Compute 0 driving overall
activity during compute, with much lower 3D activity. Compute returned to zero
within two samples of workload completion while VRAM remained allocated.
Matched 30-second overhead: previous source 0.381% CPU / 112.8 MiB working set;
GPU build 0.345% / 102.3 MiB. No material increase was observed in that interval.
Portable packaging includes new dependency licence texts. Detailed measurements
and limitations are recorded in VERIFICATION.md; private traces remain ignored.

Limits: no native temperature provider; no physical multi-GPU, hotplug/driver
reset or non-English Windows acceptance yet. Desktop background video activity
remained present during low-activity checks, so a completely quiescent all-engine
zero test is not claimed. DXGI dedicated capacity is usable memory (15.80 GiB
here), whereas Task Manager displays 16.0 GB physical memory and separately
reports 207 MB hardware-reserved. No unmeasured physical total is substituted.

## 4 October 2026 — Hidden-window tray recovery

**Trigger:** After launching through the Windows shortcut and closing with X,
the user could neither restore the window nor quit through the tray. The
identified process remained alive, but its launch produced no primary-log
entries. The reason for those missing writes is not yet established.

**Changes:** Tray restore now calls Windows `ShowWindow(SW_RESTORE)` on the
borrowed eframe HWND before queuing viewport commands. Quit uses the same
native wake-up before requesting graceful close. Previously both depended on
a hidden window drawing another frame. Added a per-process temporary log
fallback that includes the primary write error. Promoted the existing
`raw-window-handle` dependency to direct use; no new package was introduced.

**Verification:** All six release tests and Clippy pass. A regression test
creates a real hidden Windows window and verifies that restore shows it
without running an egui frame. Release packaging and native dashboard capture
succeeded. Computer Use clicked X and confirmed the window disappeared while
the process stayed alive, with close-to-tray entries in the log. The identified
old stuck instance was stopped and the replacement installed at the shortcut's
existing target. The user subsequently exercised the manual lifecycle, including
exit/restart, and reported that it worked correctly. The replacement's log
confirms graceful tray Quit in 77 ms, including 1.2 ms for collector shutdown
and 20.2 ms for tray-icon removal. No MachineGlass process remained when checked.

**Limits:** The successful manual check covers the reported workflow, not every
Windows session or shell-restart scenario. A fallback log from testing captured
an Access denied primary-write error, verifying fallback operation; it does not
establish why the earlier shortcut-launched instance produced no entries.

## 4 October 2026 — Shutdown diagnostics

**Trigger:** Tray Quit was reported to hang, requiring Task Manager to end
MachineGlass. An older log showed an eleven-second gap but could not identify
the responsible shutdown stage or distinguish multiple app instances.

**Changes:** Internal log entries now include millisecond timestamps and
process IDs. Added markers for the tray Quit callback, UI observation and
close event, framework exit hook, app destruction, collector stop/join,
tray-icon removal, and native event-loop return. Quit and close-to-tray
behaviour are preserved. Added a regression check that collector shutdown
interrupts a ten-second sampling wait.

**Verification:** Formatting, all five tests, Clippy with warnings denied,
and the release build passed. The portable package was refreshed. The user
performed a successful tray Quit; the log measured 996 ms from callback to
event-loop return, including 1.1 ms for collector shutdown and 11.2 ms for
tray-icon removal. No MachineGlass process remained.

**Open:** The earlier hang was not reproduced. Its cause remains unresolved;
this is a diagnostic improvement and one verified graceful exit, not a claim
that the intermittent failure is fixed. Most time in the successful attempt
preceded the UI's observation of Quit.

## 4 October 2026 — Source repository and tray milestone

Retrospective entry from the existing Git history and verification notes.

- Corrected the initial documentation-only GitHub upload: private `main` now
  contains the Rust source, scripts, assets and documentation. Preserved both
  histories through a merge; generated binaries and private captures stay out
  of Git.
- Added original stethoscope branding and native tray Show / Hide / Quit.
  Closing the window hides it while telemetry continues; failed tray creation
  allows ordinary window-close exit.
- Verified native icon rendering and tray registration, four existing tests,
  Clippy and the release build. Broader Show / Hide / restore interaction
  coverage remains incomplete; the later Quit check is recorded above.
- Completed missing third-party licence notice fallbacks with upstream
  provenance and packaging checks.

## 4 October 2026 — Milestone 1 foundation

Retrospective entry from the existing source and verification notes.

Built the standalone Rust Windows desktop skeleton with direct Windows CPU,
memory and process readings, process identity using PID plus creation time,
search/sort/details, bounded history and graphs, configuration, internal logs,
and a portable release package. Verified actual native rendering and live
measurements. This remains Milestone 1, not the full v1.0 product: GPU,
storage, networking and flight-recorder features are still future work.

## 17 October 2026 — Persisted render-ready Storage maps

**Observed cause:** The `.json` file is a detailed inventory, not a render
snapshot. Startup decoded it into 256-entry chunks and inserted them into the
same `StorageTree` used by egui. Each frame laid out whatever partial hierarchy
had arrived, rebuilding parent/child indexes as records arrived. The cache
sequence comes from `HashMap::into_values()`, so it has no render order. The
old “cached map displayed” log fired on the first nonempty partial tree; one
cold run logged only 1,536 applied records at that point, despite the eventual
391,065-record inventory. That was not evidence that the picture was ready.
Refresh was not clearing the map; partial cache hydration was the visible
rebuild.

**Change:** Added a separate versioned `C.treemap.json` sidecar containing the
renderer's retained hierarchy as parent-indexed nodes: names, directory/file /
aggregate kinds, cumulative sizes and aggregate counts, plus drive root,
volume identity, capacity and capture timestamp. It retains the renderer's
largest 79 children per directory, one “Other discovered items” aggregate,
and descendants through its eight-level render depth. No bitmap is stored and
the detailed `.json` format is unchanged. The startup worker publishes all
valid treemap sidecars before decoding any detailed inventory. `StorageSession`
keeps the render tree and detailed inventory separate; cached inventory chunks
only populate the inventory, while live scan events update the retained map.
Checkpoint and final-save sidecars use the same cache path as their inventory.

The F: inventory exposed directory entries whose stored cumulative sizes were
zero even though cached children had nonzero sizes. Snapshot construction now
recomputes directory totals bottom-up from the available cached children and
keeps the larger of that sum and the cached total. This changes only the
render-side model; it does not rewrite inventory records. UI count text now
identifies inventory loading separately from a ready saved picture.

**Real cold-start evidence:** Native 944×720 framebuffer captures for C:–G:
showed the complete stored maps in the first rendered view. The manager logged
C: snapshot load in 201 ms and “persisted treemap” display at 430 ms; D: 190 ms
and 409 ms; E: 3 ms and 408 ms; F: 59 ms and 392 ms; G: 30 ms and 402 ms.
Those display times are from StorageManager start, before detailed inventory
loading. C: showed its familiar Users/Windows/Program Files/ProgramData and
pagefile layout. F: showed SteamLibrary, AI and nested content after its saved
child totals were applied. A separate C: capture after refresh began still
showed the same complete map, with “Cached map · verifying…” and background
refresh active. Private framebuffers are under the task's local verification
folder and are not tracked.

Final observed sidecars: C: 70,770 nodes / 7,703,968 bytes; D: 34,507 /
3,993,466; E: 1,936 / 234,980; F: 27,101 / 2,900,571; G: 13,122 /
1,459,107. C's sidecar is about 5.3% of its then-current 144,705,145-byte
inventory file. On disk C: volume identity matched. The user-supplied baseline
was 144,736,179 bytes / 391,065 entries; during this verification a leftover
running verification instance checkpointed C.json, and the final valid cache
became 144,705,145 bytes / 391,026 entries (SHA256
`C42E486377FC8377093134B967E988307EB1B120CF99B6C15425E4798DB1AC21`). This
checkpoint was not reverted. All verification-launched MachineGlass instances
were stopped afterward.

**Validation:** `cargo fmt`, full `cargo test --locked` (18 library tests and
one native layout test), all-target Clippy with warnings denied, and the locked
release build passed. One earlier full-test attempt hit MSVC `LNK1201` while
writing the unrelated `collector-probe` PDB; the retry passed. Added regression
coverage for compact snapshot round-trip/overflow, recovering
cached directory totals, and keeping temporary scan sidecars beside their
provided cache instead of writing under the real drive letter. The sidecar
path test caught and fixed a test-isolation error before final captures.

**Limits:** The cold-start check used the native `--storage-drive` selector to
choose each real drive, not a mouse click. It verified each rendered map and a
live C: refresh framebuffer, but is not a sustained responsiveness test. The
renderer remains capped at its existing eight visible nesting levels; deeper
drilldown can fill as live scanning proceeds. Duplicate scanner deserialization
and memory optimization were not changed.
