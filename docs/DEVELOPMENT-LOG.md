# MachineGlass development log

Keep this log with each project checkpoint: what changed, why, what was
verified, and what remains unresolved. New entries go first. Record useful
findings as well as features; distinguish a successful observation from a
proven fix. Keep raw logs, inventory and captures local under `verification/`.
Detailed measurement evidence belongs in [verification](VERIFICATION.md).

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
