# MachineGlass

**See what your machine is actually doing.**

MachineGlass is a native Windows system observability and diagnostics application,
built in Rust. It brings live performance measurements, process inspection and,
over time, hardware, storage, networking and historical activity into one desktop
instrument.

## Project status

Early development: the first technical milestone is running locally.

Implemented so far:

- Live CPU utilization and physical memory readings.
- Searchable, sortable process inspection with executable paths and creation identities.
- Recent CPU and memory graphs and a bounded 30-minute core history buffer.
- A self-monitoring view and explicit unavailable / unsupported measurement states.
- Native system tray controls with original stethoscope branding for the tray,
  window and executable. Closing the window hides it while monitoring continues;
  the tray provides Show, Hide and Quit.

The release build, collector/history tests and static checks pass. The native
window icon and live dashboard have been visually checked. Interactive tray
hide/restore/quit acceptance remains to be completed.

Planned increments include process hierarchy, hardware identification, storage,
networking, GPU telemetry, services and a flight recorder with historical
process correlation.

## Design principles

- Standalone native Windows application, with no browser or external development runtime required by users.
- Offline operation, ordinary user-mode monitoring and no cloud or AI dependency.
- Measurement before interpretation: missing data is never substituted with zero.
- Replaceable telemetry providers and a normalized internal system model.
- Low overhead, bounded history and read-only diagnostics.
- No automatic cleanup, tuning or process control in the initial milestones.

## Repository scope

This private repository currently contains the project README and its licence.
Application source, build files, binaries, runtime inventory and development
history remain local. No public release or download is available yet.

## Licence

Copyright © 2026 Warren Gerdes.

MachineGlass's original software and documentation use the [MIT License](LICENSE).
Third-party components retain their own licences. The application source has
not yet been uploaded to this repository.
