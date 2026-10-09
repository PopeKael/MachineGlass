# MachineGlass development log

Keep this log with each project checkpoint: what changed, why, what was
verified, and what remains unresolved. New entries go first. Record useful
findings as well as features; distinguish a successful observation from a
proven fix. Keep raw logs, inventory and captures local under `verification/`.
Detailed measurement evidence belongs in [verification](VERIFICATION.md).

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
