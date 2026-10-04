# MachineGlass

**See what your machine is actually doing.**

A standalone, offline Windows observability instrument. This first increment is
the **Milestone 1 technical skeleton**, not the full product described in the
[product brief](docs/PRODUCT-BRIEF.md).

## Run

Open `dist\MachineGlass\MachineGlass.exe`. The release uses a native desktop
window and a statically linked C runtime. No browser, Node, Python, Docker,
monitoring suite, administrator prompt or development server is required.
The teal stethoscope appears in the Windows notification area, title bar,
taskbar and executable icon. Windows may place a new tray icon under the
notification-area overflow arrow.

**Closing the window hides it to the tray; monitoring and history continue.**
Click the tray icon to restore the window. Right-click for **Show MachineGlass**,
**Hide to tray** and **Quit MachineGlass**. Quit shuts down collection and removes
the icon. If tray creation fails, a status warning appears and closing the window
exits normally, rather than leaving an inaccessible background process.

The current renderer requires a working OpenGL 3.3 graphics driver; renderer
failure is logged. Compatibility with older / remote-session GPUs is still a
proof-of-concept question.

## Available now

- Live total CPU, physical RAM used / available, uptime and process count.
- Searchable and sortable process list: PID, observed parent PID, CPU, working
  set and executable path. Click an accessible process to inspect details.
- Process identity uses PID **and creation time**, preventing PID reuse from
  matching a different process. Protected or disappearing processes show
  unavailable values rather than invented zeroes.
- CPU / RAM graphs covering 90 seconds; a bounded 30-minute core history buffer.
- Self-observation, source information and capability status in **The instrument**.

Process CPU uses total machine capacity: one saturated logical processor on a
16-thread machine is approximately 6.25%. Working set is resident memory and is
not Task Manager's private working set column. Physical RAM use is total minus
available. CPU uses idle-time deltas, rather than Task Manager's frequency-scaled
processor utility; the numbers can differ. Multiple processor groups are
explicitly unsupported by this first total-CPU collector.

GPU / VRAM, temperatures, process trees, disks, network, services, storage
scanning, process history, trace saving and historical replay are **not yet
implemented**. There are no simulated values or actions that modify processes
or clean files. A graph gap means no reading, not zero usage.

## Configuration and logs

`%LOCALAPPDATA%\MachineGlass\config.json` is created on first launch:

```json
{
  "sample_interval_ms": 1000,
  "history_seconds": 1800
}
```

Restart after editing. Intervals are bounded to 500–10000 ms, retention to
30–120 minutes. Invalid JSON is preserved; defaults apply. Internal logs live
beside configuration, rotate at approximately 1 MiB, and contain lifecycle and
internal errors rather than process inventory. No usage telemetry is sent.

## Develop

Requires a current Rust MSVC toolchain and Windows build tools **on the developer
machine only**. Dependencies are locked in `Cargo.lock`.

```powershell
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
.\scripts\Package.ps1
.\target\release\collector-probe.exe
```

The console probe exports only its own process plus core metrics for verification;
it is not included in customer packaging. `MachineGlass.exe --capture <path.bmp>`
saves one actual rendered framebuffer after four samples for local visual QA.
Optional `--view processes` / `--view instrument` opens that view directly;
`--search <text>` initializes the process filter.

See [architecture](docs/ARCHITECTURE.md), [dependency review](docs/DEPENDENCIES.md)
and [verification](docs/VERIFICATION.md). The original brief is preserved verbatim.

This increment is a local, unsigned portable build. An installer, signing,
Steam integration and a public repository have not been created.

## Licence and GitHub

MachineGlass's original source and documentation are licensed under the
[MIT License](LICENSE), copyright 2026 Warren Gerdes. Third-party components
retain their own licences and notices.

The [private GitHub repository](https://github.com/PopeKael/MachineGlass) contains
only the prepared project README and licence. Source and binaries remain local.
See [upload scope and branch layout](docs/GITHUB.md) before pushing changes.
