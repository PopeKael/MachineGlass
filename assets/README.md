# MachineGlass stethoscope

Original project artwork, covered by the project's MIT licence.

`stethoscope.svg` is the editable vector reference. `scripts/Generate-Icon.ps1`
mirrors that geometry with Windows drawing APIs to generate `stethoscope.png`,
`stethoscope.rgba` and `machineglass.ico`. Keep the SVG and script geometry in
step when changing the design. The ICO contains six Windows sizes; the RGBA
asset supplies the tray and window at runtime without image decoder dependencies.

`machineglass.rc` embeds the ICO in the executable during the Rust build.
Customers need no image-generation tooling or external icon files.
