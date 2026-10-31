# Contributing to Rapidash

Thanks for your interest in contributing. Rapidash is an evolving Rust/GTK4 application focused on **useful metrics, predictable actions, and a non-cluttered Linux desktop UX**.

## Before you start

1. Check existing issues and [the roadmap](docs/ROADMAP.md).
2. Open or comment on an issue for major features or architectural changes.
3. Keep contributions focused and avoid mixing UI redesigns with backend behavior changes.
4. Do not add ML/LLM dependencies or decorative animation features to roadmap work without prior discussion.

## Local development

Install the build prerequisites in [README.md](README.md), then:

```bash
cargo check
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets
```

If the current tree is not rustfmt-clean, run `cargo fmt --all` in a dedicated change and avoid combining large unrelated formatting diffs with functional changes. Use `cargo run --release` for manual UI validation.

## Pull requests

- Describe the problem, changes, and manual tests.
- Add screenshots for visual changes; redact window titles, account details, usernames, and other private data.
- State what distributions and hardware you tested on.
- Clearly identify experimental system calls, permissions, and failure modes.
- Avoid writing user-specific paths or assumptions into the code.
- Do not commit compiled artifacts, dependency caches, secrets, or copyrighted Pokémon artwork.

## System-control guidelines

Changes that freeze or terminate processes, affect browser sessions, or alter caches or thermal behavior must be reviewed for permission errors and safe failure behavior. Any fan control needs explicit hardware compatibility and failsafe analysis; fan control is *not* a generic Linux feature.

## Communication

Be respectful. Follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Issues should be reproducible, specific, and constructive.
