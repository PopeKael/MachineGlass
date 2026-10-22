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
- The user's shortcut launches `dist/MachineGlass/MachineGlass.exe`; treat
  that exact package path as the authoritative executable for delivery and
  native verification. Build from this checkout, refresh that package in place,
  and verify its hash against the release build. Do not invent alternate
  package/output directories or ask the user to test a different executable.
  If the file is in use, prepare the current release and wait for it to close
  before replacing it. Run native acceptance against the real user data path
  when the sandbox redirects `LOCALAPPDATA`.
- Preserve third-party licence / font notices with portable packages. Keep
  private runtime inventory and screenshot captures out of Git.
- State what was measured, what was tested and what is still unimplemented.
- At each checkpoint, update `docs/DEVELOPMENT-LOG.md` with changes, reasons,
  validation and unresolved findings; keep detailed evidence in
  `docs/VERIFICATION.md`. Do not call an unreproduced issue fixed.
