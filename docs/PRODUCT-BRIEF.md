# MachineGlass
## Product & Build Brief v0.1

### 1. Product Vision

**MachineGlass** is a standalone Windows system diagnostics and observability application.

Its purpose is simple:

> **See what your machine is actually doing.**

MachineGlass combines the most useful concepts normally scattered across Windows Task Manager, Resource Monitor, hardware-monitoring tools, storage-visualisation tools, network utilities, process explorers and historical performance recorders into one coherent application.

It is not an Ariadne component.

It is an independent diagnostic instrument capable of observing Ariadne, Ollama, AI workloads and every other process on the machine just as it observes any normal Windows application.

The long-term objective is a polished commercial desktop product that can be installed through Steam or another standard Windows installer with effectively one click.

The core design principle is:

> **Minimal footprint. Maximum feedback.**

---

# 2. Product Principles

These are architectural constraints, not suggestions.

## 2.1 Standalone

MachineGlass must operate independently of:

- Ariadne
- Ollama
- Python
- Node.js
- Docker
- browser software
- cloud services
- AI models
- externally installed development runtimes

If another application on the machine crashes, MachineGlass should remain operational and ideally help diagnose why.

## 2.2 Native desktop application

MachineGlass must have its own controlled desktop interface.

Do not implement the primary UI as:

- a localhost web application
- an Electron application
- a browser tab
- an embedded administration website

The user launches MachineGlass and receives a conventional desktop application.

The application owns its windowing, layout and rendering behaviour.

## 2.3 Commercial deployment from the beginning

Every architectural decision must assume that MachineGlass may eventually be sold commercially.

Target deployment experience:

1. User purchases or downloads MachineGlass.
2. User clicks Install.
3. User clicks Run.
4. MachineGlass detects the machine automatically.
5. Useful telemetry appears.

The customer must not be expected to install compilers, SDKs, Python packages, hardware-monitoring suites or command-line dependencies.

## 2.4 Capability detection, not configuration hell

Different PCs expose different sensors and capabilities.

MachineGlass must detect what is available.

For example:

- GPU temperature available → display it.
- GPU power measurement unavailable → report unavailable.
- SMART data unavailable → do not fabricate or substitute zero.
- Unsupported sensor → clearly identify it as unsupported.

**Unknown, unsupported and zero are different states.**

This principle is inspired by TMOG's measurement philosophy, which explicitly distinguishes zero from unavailable measurements and leaves gaps where measurements cease.

## 2.5 Measurements before interpretation

The telemetry engine records facts.

Higher-level diagnostics may interpret those facts.

Never manufacture measurements or infer a sensor value that was not actually obtained.

MachineGlass should eventually be able to explain behaviour, but:

> **Measurement and interpretation must remain separate layers.**

## 2.6 Provider-based architecture

No subsystem should be permanently tied to one external project or vendor library.

Create normalized internal models for:

- CPU
- GPU
- memory
- storage
- network
- processes
- services
- hardware sensors
- system events
- historical telemetry

Individual providers populate those models.

Providers must be replaceable without redesigning the UI or recorder.

---

# 3. Preferred Technology

## Core language

**Rust**

Use Rust for the main executable, telemetry engine, process management, history engine, system modelling and application logic.

Primary reasons:

- native compiled binary
- low runtime overhead
- strong memory safety
- good concurrency
- strong Windows API access
- suitable for long-running monitoring software
- commercially deployable without an external runtime

## Windows APIs

Prefer direct Windows facilities wherever practical.

Candidates include:

- Win32 APIs
- Windows performance counters
- ETW
- WMI/CIM where justified
- Windows networking APIs
- filesystem APIs
- service-control APIs
- storage/device APIs

Do not introduce an external dependency when Windows already provides a robust source.

## GUI

Initial preferred investigation:

**egui / eframe**

Requirements:

- native desktop rendering
- no browser dependency
- fast startup
- low memory footprint
- commercially friendly licensing
- suitable for dense diagnostic dashboards
- charts, tables, process trees and interactive drill-down interfaces

Do not permanently commit to a GUI framework until a small proof of concept confirms it can deliver the required responsiveness and visual density.

## Local persistence

Use a lightweight embedded local datastore where appropriate.

SQLite is acceptable for configuration, metadata, events and historical indexing.

High-frequency telemetry may use an efficient rolling in-memory representation with periodic persistence rather than blindly inserting every measurement into SQLite.

This should be benchmarked rather than guessed.

---

# 4. Application Architecture

Conceptual structure:

```text
Hardware / Windows
        │
        ▼
┌───────────────────────┐
│ Telemetry Providers   │
│                       │
│ CPU                   │
│ GPU                   │
│ Memory                │
│ Storage               │
│ Network               │
│ Processes             │
│ Services              │
│ Sensors               │
│ Events                │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ Normalized System     │
│ State Model           │
└───────────┬───────────┘
            │
     ┌──────┼─────────┐
     ▼      ▼         ▼
 Live UI  Recorder  Diagnostics
                    Engine
     │      │         │
     └──────┼─────────┘
            ▼
      Future API/MCP
```

The UI must consume normalized MachineGlass models rather than directly calling individual telemetry libraries.

---

# 5. MVP

The first useful version should replace most routine trips into Task Manager and Resource Monitor.

Do not attempt to build the complete long-term vision in one pass.

## 5.1 Overview Dashboard

Display at minimum:

- total CPU utilization
- physical RAM used / available
- GPU utilization where available
- GPU VRAM used / available where available
- per-drive capacity
- disk activity
- network send/receive throughput
- system uptime
- major temperatures where available

Provide recent short-term graphs for important metrics.

Sampling rate should be configurable internally but initially target approximately one-second updates without creating noticeable monitoring overhead.

## 5.2 Processes

Provide:

- process name
- PID
- parent PID
- process hierarchy/tree
- CPU utilization
- working-set memory
- disk activity if available
- network activity if practical
- executable path
- process start time

Support:

- sorting
- filtering/search
- expandable process trees
- process detail view

Actions such as terminate/suspend should be treated carefully and should not be necessary for the first read-only milestone.

Process identity should not rely solely on PID because Windows can recycle PIDs. TMOG uses PID plus creation identity for this reason.

## 5.3 Storage

Display all local volumes with:

- total capacity
- used space
- free space
- filesystem
- drive type where identifiable
- current read/write throughput
- activity percentage where available

Add a storage-analysis view capable of scanning a selected volume and showing where space is consumed.

The long-term target is a SpaceMonger-style visual representation.

For the first version, a hierarchical directory-size view is acceptable if it provides a clean foundation for later treemap visualisation.

Never automatically delete, clean, optimise or modify files.

MachineGlass diagnoses. It does not become another questionable “PC Cleaner.”

## 5.4 Hardware

Display detected hardware information including:

- CPU model
- core/thread topology
- installed physical memory
- GPU model(s)
- VRAM where available
- storage devices
- network adapters

Where reliable sensor information is available, display:

- temperature
- utilization
- clock frequency
- fan speed
- power consumption

Do not require all sensors to exist.

## 5.5 Network

Display:

- network adapters
- adapter status
- current send/receive rate
- cumulative traffic
- active connections where accessible
- local/remote endpoint
- protocol
- owning process where resolvable

## 5.6 Services

Display:

- service name
- display name
- current state
- startup type where available
- associated process/PID where applicable

Initial implementation may remain read-only.

---

# 6. Flight Recorder

Historical telemetry is strategically important and should influence architecture from the beginning even if the complete feature arrives after the first MVP.

MachineGlass should maintain a rolling history buffer.

Initial target:

**30 minutes minimum rolling history**

Eventually configurable to longer periods.

Record core metrics such as:

- CPU
- memory
- GPU
- VRAM
- disk activity
- network activity
- important process metrics

TMOG demonstrates the value of recording and replaying telemetry through its Flight Recorder feature.

MachineGlass should eventually allow a user to select a previous point in time and inspect what the system looked like then.

Example:

```text
09:41 CPU 18%
09:42 CPU 31%
09:43 CPU 96%  ← select
09:44 CPU 87%
09:45 CPU 22%
```

Selecting 09:43 should reveal which processes and resources were responsible as far as recorded telemetry permits.

This is potentially one of MachineGlass's major differentiating features.

---

# 7. Event Markers

Design the recorder so that events can be placed on the historical timeline.

Automatically generated examples:

- process started
- process exited
- service started
- service stopped
- network adapter changed state
- disk mounted/unmounted
- major CPU spike
- major GPU spike
- low disk-space threshold crossed

Future external integrations should also be able to inject markers.

Example:

```text
10:14:03  Ollama started
10:14:08  GPU VRAM +8.7 GB
10:14:09  Disk F: peaked at 740 MB/s
10:14:12  CPU reached 87%
```

MachineGlass does not need to understand Ollama specifically to record these facts.

Application-specific intelligence can be layered on later.

---

# 8. Future Diagnostic Intelligence

Do not make AI a dependency.

MachineGlass must remain completely functional offline without any language model.

However, design normalized telemetry so a future diagnostics engine can answer questions such as:

- What caused the CPU spike?
- Which process consumed the most disk bandwidth?
- What changed immediately before the machine became sluggish?
- Which process is using the most VRAM?
- Why did free memory suddenly fall?
- Which applications opened network connections during this interval?

Initial diagnostic explanations should preferably be deterministic and rule-based.

Example:

> Between 14:31 and 14:33, CPU utilization increased from 22% to 94%. `example.exe` accounted for approximately 61 percentage points of the increase.

That conclusion comes from recorded telemetry, not language-model speculation.

---

# 9. Future MCP / API Layer

MachineGlass may eventually expose system information to external tools through:

- local API
- Model Context Protocol server
- CLI
- automation hooks

This is an interface into MachineGlass, not MachineGlass's fundamental architecture.

The GUI and monitoring engine must not depend upon MCP.

A future AI system should be able to ask MachineGlass questions without requiring MachineGlass itself to contain an AI model.

---

# 10. Performance Requirements

MachineGlass must not become the reason the machine is slow.

Design targets for normal monitoring:

- idle CPU overhead should be very low
- memory consumption should remain modest
- telemetry collection must not create significant disk I/O
- history buffers must have bounded memory usage
- expensive scans must run separately from normal telemetry
- storage scans must be cancellable
- UI rendering must not block telemetry acquisition

MachineGlass should eventually contain a **self-monitoring view** showing its own CPU, memory and resource consumption.

TMOG explicitly supports monitoring itself, which is a useful engineering principle.

---

# 11. Reliability Requirements

MachineGlass is a diagnostic instrument.

Reliability matters more than cleverness.

The application must:

- handle unavailable sensors gracefully
- tolerate disappearing processes
- tolerate removable drives
- tolerate changing network adapters
- survive provider errors
- isolate provider failures where practical
- log internal errors
- never convert missing data into zero
- avoid crashing because one telemetry source fails

A process disappearing between enumeration and inspection is normal Windows behaviour and must not be treated as an exceptional application failure.

---

# 12. Security Model

Prefer ordinary user-mode operation.

Avoid kernel drivers unless a future requirement provides compelling value that cannot reasonably be obtained otherwise.

Elevation should occur only when required for a specific operation.

Normal monitoring should work without permanent administrator privileges wherever possible.

MachineGlass should not transmit:

- hardware inventory
- process lists
- usage telemetry
- user identifiers

unless a future user-enabled feature explicitly requires communication.

Offline operation is a requirement.

---

# 13. Third-Party Dependency Policy

Open-source code may be used where it saves substantial engineering effort, but every dependency must be reviewed for:

- licence
- commercial redistribution rights
- maintenance status
- runtime footprint
- packaging requirements
- security implications
- vendor/project longevity

Potential projects may be studied for architecture and capabilities, including:

- System Informer
- LibreHardwareMonitor
- Open Task Manager
- ProcMaster
- similar Windows diagnostic projects

Do not copy proprietary TMOG source or proprietary implementation.

Ideas and behaviours may inform MachineGlass, but implementation must be ours or come from appropriately licensed dependencies.

Prefer a small number of well-understood dependencies over dozens of fragile packages.

---

# 14. Commercial Packaging

Architect MachineGlass so it can eventually be distributed as:

- Steam software
- standalone Windows installer

The production build must not expose development infrastructure.

The customer should not see:

- terminals
- development servers
- localhost browser dashboards
- compiler messages
- package managers
- dependency installers

Normal launch:

```text
MachineGlass.exe
        │
        ▼
MachineGlass UI
```

Configuration and application data should use conventional Windows application-data locations.

Portable traces may be exportable separately.

---

# 15. User Experience

MachineGlass is aimed at technically curious users and power users, but should not require system-administrator knowledge.

The top-level dashboard should answer:

> Is something wrong?

The next level should answer:

> Which subsystem?

The next level:

> Which process or device?

The next level:

> What exactly happened?

Navigation philosophy:

```text
SYSTEM
  ↓
SUBSYSTEM
  ↓
RESOURCE / PROCESS
  ↓
EVENT
  ↓
HISTORY
```

Dense information is acceptable.

Clutter is not.

Do not hide useful information merely to make the interface look fashionable.

---

# 16. Explicit Non-Goals for Initial Development

Do not initially build:

- AI chatbot
- cloud accounts
- remote monitoring
- mobile application
- Linux version
- macOS version
- automatic PC optimisation
- registry cleaner
- file cleaner
- antivirus
- kernel driver
- automated overclocking
- hardware tuning
- commercial licensing system
- Steam integration
- elaborate plugin marketplace

Build the diagnostic instrument first.

---

# 17. Initial Development Milestones

## Milestone 1: Technical Skeleton

Create the standalone Rust desktop application.

Establish:

- project structure
- GUI
- telemetry provider interfaces
- normalized data models
- application logging
- configuration
- graceful shutdown
- release build

Demonstrate live:

- CPU
- memory
- process list

**Scope amendment, 9 October 2026:** Basic read-only GPU/WDDM engine telemetry,
90-second GPU graphs and dedicated/shared memory move into the current milestone.
Overall GPU Activity is the busiest engine per adapter, never an engine average
or sum. Enumerate adapter/engine identities dynamically; retain every engine and
initially display the busiest Compute-type engine and 3D where exposed. Missing
values remain unavailable. Temperature is optional and does not block delivery.
This amendment does not begin storage, networking or Flight Recorder work.

## Milestone 2: Useful Task Manager Replacement

Add:

- process hierarchy
- process search
- CPU history
- memory history
- disks
- network throughput
- basic hardware identification

At this point MachineGlass should already be useful in normal troubleshooting.

## Milestone 3: Hardware & Storage

Add:

- advanced GPU / sensor capabilities beyond the basic current-milestone provider
- sensors where available
- drive details
- storage scanner
- directory-space visualization

## Milestone 4: System Diagnostics

Add:

- network connections
- services
- startup information
- richer process details

## Milestone 5: Flight Recorder

Add:

- rolling telemetry history
- timeline
- event markers
- historical process/resource correlation
- save/load trace capability

Only after these foundations are stable should more ambitious diagnostic intelligence be considered.

---

# 18. Development Method

Implement incrementally.

Do not attempt a giant one-pass application generation.

For each subsystem:

1. Identify the authoritative Windows data source.
2. Implement the smallest collector.
3. Verify measurements against Windows or another trusted diagnostic utility.
4. Normalize the data.
5. Add tests where practical.
6. Connect it to the UI.
7. Measure MachineGlass's own overhead.
8. Commit the working increment.
9. Continue to the next subsystem.

Do not replace working architecture simply because another framework appears fashionable.

Do not redesign unrelated components while implementing one subsystem.

Preserve a clean separation between:

- acquisition
- normalization
- storage
- analysis
- presentation

---

# 19. Definition of Success for v1.0

MachineGlass 1.0 succeeds when a Windows user can install one application and use it instead of repeatedly switching among several diagnostic utilities during ordinary troubleshooting.

A user experiencing a slow computer should be able to open MachineGlass and rapidly determine:

- whether CPU is overloaded
- whether memory is exhausted
- whether GPU/VRAM is heavily used
- whether a disk is saturated or full
- whether networking is unusually active
- which processes are responsible
- what significant hardware state exists
- what happened immediately before the problem

The application must remain fast, predictable and trustworthy while doing so.

---

# 20. Product Identity

**Product name:** MachineGlass

**Working description:**

> A native Windows system observability and diagnostics console combining live performance monitoring, process inspection, hardware telemetry, storage analysis, networking and historical system activity in one application.

**Core promise:**

> **See what your machine is actually doing.**

The internal philosophy remains the original Master Control Program idea:

Take the best instrumentation, diagnostic techniques and interface concepts available, normalize their information into one coherent system model, and build one control room above them.

Do not clone another utility.

Build the instrument we wish existed.
