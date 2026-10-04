# Dependency review — initial increment

Direct dependencies:

| Dependency | Role | Licence | Runtime / packaging implications |
|---|---|---|---|
| eframe / egui 0.33.3 | Native window, OpenGL rendering and widgets | MIT OR Apache-2.0 | Compiled into executable; no browser runtime. Uses OS graphics driver. Framework remains provisional. |
| windows-sys 0.61.2 | Typed Win32 imports | MIT OR Apache-2.0 | Windows system DLLs; no installed monitoring library. |
| serde / serde_json | Configuration and probe serialization | MIT OR Apache-2.0 | Compiled into executable. No network client. |
| tray-icon 0.26.0 / muda 0.21.0 | Native Windows tray icon and context menu | MIT OR Apache-2.0 | Compiled into executable; existing Windows message loop. Default non-Windows features disabled. |
| embed-resource 3.0.11 | Compile the executable's icon resource | MIT | Developer build-time helper only; Windows SDK resource compiler. No customer install. |

eframe is pinned to the verified 0.33.3 API. A newer 0.36 series exists; upgrading
is not a milestone requirement. The GUI dependency tree is substantially larger
than the telemetry code, so a small direct dependency list is not a claim of a
small transitive graph. `Cargo.lock` pins the complete resolution. Only Windows
dependencies are used by the Windows build; registry fetching also obtains
other-platform source packages.

The package script traverses the resolved Windows dependency graph, records
package names / versions / SPDX licence expressions, and copies provided
licence / notice texts into `third-party`. Font licences are included too:
egui's bundled fonts include OFL-1.1 and Ubuntu-font-1.0 obligations. The resolved
graph also contains Apache-2.0, MIT, Unicode-3.0, BSL-1.0, ISC, BSD and Zlib
expressions. This is a source licence inventory, not a claim of a completed
commercial release audit or vulnerability assessment.

Some registry archives omit repository-level licences. For those packages,
`assets/third-party-notices` holds texts retrieved from the exact upstream
commits recorded in `.cargo_vcs_info.json`; `sources.json` records provenance.
The package script includes these fallbacks and fails on an unknown missing
notice. This is licence-preservation work, not a change to upstream terms.

Official sources:

- https://docs.rs/eframe/0.33.3/eframe/trait.App.html
- https://github.com/emilk/egui
- https://github.com/microsoft/windows-rs
- https://serde.rs/
- https://docs.rs/tray-icon/0.26.0/tray_icon/
- https://docs.rs/embed-resource/3.0.11/embed_resource/
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes
- https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo
- https://learn.microsoft.com/en-us/windows/win32/api/tlhelp32/nf-tlhelp32-process32firstw

Before public sale: validate all licence obligations and font notices, audit
locked dependencies, benchmark rendering on integrated GPUs / remote sessions,
verify Windows-version coverage, sign the executable and package an installer.
