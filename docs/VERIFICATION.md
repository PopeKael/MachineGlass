# Local verification — 4 October 2026

This is a Milestone 1 proof of concept, verified on the current Windows desktop.

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
