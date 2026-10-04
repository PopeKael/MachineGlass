# Private GitHub source repository

Repository: https://github.com/PopeKael/MachineGlass

MachineGlass's application source, build scripts, original icon assets, product
documentation, licence notices and MIT licence are stored here. The repository
is private. Only the owner and explicitly authorized collaborators can access it.

Local `main` tracks `origin/main` on GitHub. A checkpoint means reviewing and
committing the intended source changes, pushing them, and verifying identical
local / remote commit IDs and a clean working tree. Feature branches can hold
future changes separately before review and merge into `main`; a separate
"code" branch is not required.

The first upload was mistakenly limited to README and LICENSE. That small
GitHub history is retained through a merge with the local development history;
no force push is needed. The old local `github-docs` branch is only a historical
reference. Normal future checkpoints use `main` and `origin`.

Do not commit `target/`, `dist/`, `verification/`, logs, machine inventories or
credentials. The generated icon assets are intentional source assets; the
compiled application executable is not.

Keep repository visibility private until explicitly instructed otherwise.
MIT allows reuse with the copyright and licence notice retained. It does not
require public source publication. Third-party components keep their own licences.

Official licence: https://opensource.org/license/mit
