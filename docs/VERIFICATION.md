# Local verification — 4 October 2026

This is a Milestone 1 proof of concept, verified on the current Windows desktop.

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
