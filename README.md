# MachineGlass

**See what your machine is actually doing.**

A small, native Windows monitor for keeping an eye on CPU, RAM and GPU activity
while you work. I built it to sit beside my AI projects so I could see how hard
I was pushing the machine without continually switching away from the program
I was using.

Turn on **Always on top**, place it where it suits you, and leave the graphs
running. It shows overall GPU activity, the busiest compute engine, 3D activity,
and dedicated/shared GPU memory together in one compact dashboard.

This first public preview is already useful on my Windows desktop with an
RX 7800 XT. You're welcome to use it, report problems or suggest improvements.
There may be more tidying up as we go. The aim for now is simple: see what's
happening in the background while getting on with the work.

## Run

1. Download **MachineGlass-0.1.0-windows-x64.zip** from the
   [v0.1.0 release](https://github.com/PopeKael/MachineGlass/releases/tag/v0.1.0).
2. Extract the whole ZIP into a folder of your choice.
3. Open `MachineGlass\MachineGlass.exe` and select **Always on top** if wanted.

Windows x64 portable application; no installer. The build is unsigned.
The release uses a native desktop
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

Select **Always on top** in the left sidebar to keep MachineGlass above other
normal windows while another application has focus. Clear it to restore normal
window stacking. This option applies for the current session and starts off
when the application is relaunched.

## Available now

- Live total CPU, physical RAM used / available, uptime and process count.
- Searchable and sortable process list: PID, observed parent PID, CPU, working
  set and executable path. Click an accessible process to inspect details.
- Process identity uses PID **and creation time**, preventing PID reuse from
  matching a different process. Protected or disappearing processes show
  unavailable values rather than invented zeroes.
- CPU / RAM graphs covering 90 seconds; a bounded 30-minute core history buffer.
- Native WDDM GPU dashboard: adapter name, busiest-engine overall activity,
  dominant Windows engine label, busiest Compute and 3D graphs, dedicated and
  shared memory. All Windows engine measurements remain available internally
  and in the expandable engine list. GPU history is bounded to 90 seconds.
  Compact 2x2 CPU/RAM/dedicated/shared memory graphlets sit above the three GPU
  activity graphlets. Minimum client width: 832 pixels.
- Self-observation, source information and capability status in **The instrument**.

Process CPU uses total machine capacity: one saturated logical processor on a
16-thread machine is approximately 6.25%. Working set is resident memory and is
not Task Manager's private working set column. Physical RAM use is total minus
available. CPU uses idle-time deltas, rather than Task Manager's frequency-scaled
processor utility; the numbers can differ. Multiple processor groups are
explicitly unsupported by this first total-CPU collector.

Temperatures, process trees, disks, network, services, storage
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
Entries include millisecond Unix timestamps and process IDs. Shutdown markers
identify the UI, collector, tray and event-loop stages if Quit stalls.
If the primary log cannot be written, entries are attempted in
`%TEMP%\MachineGlass-<PID>.log`, including the primary write error.

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

The console probe exports only its own process plus CPU/RAM/GPU metrics for verification;
it is not included in customer packaging. Use `--samples 120` for bounded JSONL sampling. `MachineGlass.exe --capture <path.bmp>`
saves one actual rendered framebuffer after four samples for local visual QA.
Optional `--view processes` / `--view instrument` opens that view directly;
`--search <text>` initializes the process filter.

See [architecture](docs/ARCHITECTURE.md), [dependency review](docs/DEPENDENCIES.md)
and [verification](docs/VERIFICATION.md). Follow changes and unresolved findings
in the [development log](docs/DEVELOPMENT-LOG.md). The original brief is preserved verbatim.

This is an early portable release, tested on one Windows desktop with an
RX 7800 XT. Other GPU models, multiple adapters, additional DPI scales and remote
sessions still need wider testing. There is no installer or code signing yet.

## Feedback and project notes

Comments, bug reports and practical suggestions are welcome in
[GitHub Issues](https://github.com/PopeKael/MachineGlass/issues). For a problem,
include your Windows version, GPU model, what you expected and what you saw.
Please avoid posting private process inventories or other sensitive information.

The [development log](docs/DEVELOPMENT-LOG.md) records what changed and why;
[verification notes](docs/VERIFICATION.md) distinguish measured results from
things still untested. The [original product brief](docs/PRODUCT-BRIEF.md) is a
broader design notebook, preserved for context. Its future ideas are not features
of this release or promises about what comes next.

## Licence and GitHub

MachineGlass's original source and documentation are licensed under the
[MIT License](LICENSE), copyright 2026 Warren Gerdes. Third-party components
retain their own licences and notices.

The [GitHub repository](https://github.com/PopeKael/MachineGlass) contains
the application source, build scripts, original assets, documentation and MIT
licence. Local `main` tracks GitHub `main`; checkpoints include the source code.
Generated application binaries are distributed through GitHub Releases; logs
and machine captures are excluded. See [GitHub workflow](docs/GITHUB.md).
