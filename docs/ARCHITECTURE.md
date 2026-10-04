# Milestone 1 architecture

```text
Windows APIs → WindowsProvider → normalized Snapshot
                                      ├─ latest Arc<Snapshot> → egui UI
                                      └─ bounded HistoryPoint ring
```

- `model.rs`: readings, process creation identity, snapshot, core history points.
- `provider.rs`: replaceable `TelemetryProvider` trait and Win32 implementation.
- `engine.rs`: dedicated acquisition thread, latest snapshot publication,
  bounded retention, interruptible sampling wait and joined graceful shutdown.
- `config.rs`: bounded configuration and bounded lifecycle/error logging.
- `main.rs`: native eframe / egui presentation, virtualized process rows,
  graphs and process detail window. The UI never calls the provider directly.
- `collector_probe.rs`: developer measurement check independent of the GUI.
- `tray.rs`: Windows notification-area icon and menu lifetime on the native
  message-loop thread. Tray events issue viewport commands directly so a hidden
  window can be restored or quit without depending on visible-frame rendering.

The provider enumerates processes every sample and opens only user-mode query
handles; RAII closes every handle even on early returns. No process-control
rights are requested. Per-process failures remain in the model. A failed
enumeration invalidates the process collection, rather than publishing partial
rows as complete. Provider panics stop acquisition but leave the UI alive with
a stale status and an internal log entry. These are fault-containment measures,
not proof that every future provider failure is harmless.

An `Arc` shares the latest process snapshot without copying its strings every
frame. Only small core metric records are retained; process history and event
markers belong to later recorder work. The UI sees acquisition freshness and
renders gaps for missing values or sample intervals exceeding three configured
periods. Sampling is independent of the window's repaint loop.

No datastore is introduced until persistence is required. No AI, MCP, remote
control, driver, external command or service is part of runtime collection.

The close button hides to tray only when a tray object exists and Quit has not
been requested. A shared quit flag lets the menu exit bypass that cancellation.
Collection remains independent of window visibility. The tray library handles
Explorer's `TaskbarCreated` notification; Explorer restart recovery is supported
by the dependency but has not been exercised by restarting the user's shell.

Next increment: process hierarchy, CPU model / topology, drive capacity and
network throughput. Keep each additional Windows collector independently
verifiable. Record the source and error state before adding interpretation.
