# Private GitHub documentation repository

Repository: https://github.com/PopeKael/MachineGlass

The authorized initial upload contains **README.md and LICENSE only**. These
files are prepared in the local `github/` directory. MachineGlass's original
software and documentation use MIT. The application source remains local.

The local `main` branch contains the complete development history. The remote
`main` branch has separate documentation-only history. **Do not push local
`main` to remote `main`**, including when the repository is private, without an
explicit change in upload scope.

The remote is named `docs-only`. Its configured push mapping is
`github-docs:main`, and `github-docs` tracks `docs-only/main`. Local `main` has
no remote upstream. To update documentation, publish only the two prepared
files to that documentation branch or use the GitHub contents API, then fetch
the remote. Do not upload binaries, verification captures or process inventory.

Keep repository visibility private until explicitly instructed otherwise.
MIT allows reuse with the copyright and licence notice retained. It does not
require public source publication. Third-party components keep their own licences.

Official licence: https://opensource.org/license/mit
