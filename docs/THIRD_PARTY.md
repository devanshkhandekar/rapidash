# Third-party Components and License Review

Rapidash's own code is intended to be licensed under the root `LICENSE` (MIT). **A repository-wide MIT license does not change the licensing of dependencies, source code, icons, fonts, photos, or other assets obtained from third parties.**

Before public release:

1. Review `Cargo.toml` and `Cargo.lock`, as well as any vendored libraries or manually copied source.
2. Check that dependencies permit your intended source and binary distribution, including their required notices.
3. Audit `assets/`, CSS resources, embedded SVGs, fonts, and logos for original authorship or explicit permission.
4. Remove or replace copyrighted Pokémon character art, sprites, and derivative icons if any are present; see `docs/ASSETS.md`.
5. If a third-party asset requires attribution, keep its copyright notice, license, source URL, and any changes in a dedicated `assets/THIRD_PARTY_NOTICES.md` (create it when needed).
6. Check whether packaged binaries must include additional license texts or notices.

This document is a checklist rather than a verified license inventory. **No dependency or asset audit has yet been performed on the user's local codebase.**
