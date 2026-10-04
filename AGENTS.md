# MachineGlass project guidance

Read `docs/PRODUCT-BRIEF.md` before implementation. MachineGlass is independent
of Ariadne and must remain a native Windows Rust desktop instrument.

- Implement one verified milestone increment at a time. Preserve provider,
  normalized model, recording, analysis and presentation boundaries.
- Prefer authoritative user-mode Windows APIs. Keep missing, unsupported,
  warming-up and actual zero readings distinct. Never introduce fake telemetry.
- Process identities require PID plus creation identity. Expect access denial
  and process disappearance. Close all acquired handles.
- Normal collection stays off the UI thread; history stays bounded. Expensive
  future scans need cancellation and separate workers.
- Keep monitoring read-only. No process control, cleanup, driver, cloud or AI
  dependency in the initial milestones.
- Run `cargo fmt`, `cargo test --locked` and
  `cargo clippy --all-targets --locked -- -D warnings` for relevant code changes.
  Build release and verify actual native rendering for UI changes.
- Preserve third-party licence / font notices with portable packages. Keep
  private runtime inventory and screenshot captures out of Git.
- State what was measured, what was tested and what is still unimplemented.
