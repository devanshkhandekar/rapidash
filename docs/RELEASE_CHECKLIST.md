# Release Checklist (for a future alpha)

There is **no stable Rapidash release** yet. Do not publish a prebuilt release just because the repository is public.

- [ ] `cargo check --locked` passes from a clean checkout.
- [ ] `cargo test --locked` passes (or test limitations are documented).
- [ ] `cargo run --release` launches on a supported desktop.
- [ ] Target Linux distro, GTK4/Libadwaita requirements, and hardware limitations are documented.
- [ ] Core UI controls were exercised manually; known unsafe/unimplemented actions are labeled or disabled.
- [ ] Screenshots show actual functionality and contain no private data.
- [ ] Third-party dependency/asset licensing reviewed: [THIRD_PARTY.md](THIRD_PARTY.md).
- [ ] Version in `Cargo.toml` updated appropriately.
- [ ] `CHANGELOG.md` updated with user-visible changes and known issues.
- [ ] No credentials, unpublished data, or licensed character imagery are in the release artifacts or history.
- [ ] CI passes; only then create an annotated tag, e.g. `git tag -a v0.1.0 -m "Rapidash v0.1.0 alpha"` and `git push origin v0.1.0`.
- [ ] If shipping binaries, document how artifacts were built and verify their contents and license obligations.
