# Dependency review — initial increment

Direct dependencies:

| Dependency | Role | Licence | Runtime / packaging implications |
|---|---|---|---|
| eframe / egui 0.33.3 | Native window, OpenGL rendering and widgets | MIT OR Apache-2.0 | Compiled into executable; no browser runtime. Uses OS graphics driver. Framework remains provisional. |
| windows-sys 0.61.2 | Typed Win32 imports | MIT OR Apache-2.0 | Windows system DLLs; no installed monitoring library. |
| serde / serde_json | Configuration and probe serialization | MIT OR Apache-2.0 | Compiled into executable. No network client. |

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

Official sources:

- https://docs.rs/eframe/0.33.3/eframe/trait.App.html
- https://github.com/emilk/egui
- https://github.com/microsoft/windows-rs
- https://serde.rs/
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes
- https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo
- https://learn.microsoft.com/en-us/windows/win32/api/tlhelp32/nf-tlhelp32-process32firstw

Before public sale: validate all licence obligations and font notices, audit
locked dependencies, benchmark rendering on integrated GPUs / remote sessions,
verify Windows-version coverage, sign the executable and package an installer.
