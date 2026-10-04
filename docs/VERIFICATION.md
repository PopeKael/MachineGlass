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
provided licence / font notice files. No standalone licence text was missing
from that inventory. The registry lockfile includes additional-platform packages
that are not part of the resolved Windows graph.

GPU / VRAM, temperatures, storage, network, services, process trees, historical
process correlation and trace persistence remain unimplemented. No installer,
signing, public release or Steam integration is claimed. Protected-process
access, disappearing processes, unsupported CPU groups and source errors are
handled in code; a comprehensive permission / hardware / fault matrix is not
yet tested. Sorting controls and process-detail interactions still need broader
hands-on acceptance beyond the rendered filtered-list check.

Local raw probe output, comparisons and native framebuffer captures are under
`verification/` and excluded from Git. The source is a separate local Git
checkpoint; it has not been pushed to a remote.
