# Public GitHub source repository

Repository: https://github.com/PopeKael/MachineGlass

MachineGlass's application source, build scripts, original icon assets, product
documentation, licence notices and MIT licence are stored here. The repository
is public following the owner's publication decision on 10 October 2026.
The first public preview is v0.1.0, with a portable Windows x64 ZIP in Releases.
Feedback and bug reports are welcome in GitHub Issues.

Local `main` tracks `origin/main` on GitHub. A checkpoint means reviewing and
committing the intended source changes, pushing them, and verifying identical
local / remote commit IDs and a clean working tree. Feature branches can hold
future changes separately before review and merge into `main`; a separate
"code" branch is not required.

At each checkpoint, update [the development log](DEVELOPMENT-LOG.md) with the
change, reason, validation and unresolved findings. Keep detailed measured
evidence in [verification](VERIFICATION.md). Review those documentation changes
alongside the source before staging and pushing.

The first upload was mistakenly limited to README and LICENSE. That small
GitHub history is retained through a merge with the local development history;
no force push is needed. The old local `github-docs` branch is only a historical
reference. Normal future checkpoints use `main` and `origin`.

Do not commit `target/`, `dist/`, `verification/`, logs, machine inventories or
credentials. The generated icon assets are intentional source assets; the
compiled application executable is not.

The owner authorized public visibility and distribution of the verified portable
build. Keep raw local evidence and runtime data out of source and release assets.
Release ZIPs include the executable, README, documentation, MIT licence and
third-party/font notices. No separately installed runtime is required.
MIT allows reuse with the copyright and licence notice retained. It does not
require public source publication. Third-party components keep their own licences.

Official licence: https://opensource.org/license/mit
