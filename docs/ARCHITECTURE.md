# Architecture Notes (early development)

Rapidash is a Rust-based Linux desktop app using GTK4 and Libadwaita. The current development tree has used:

- `src/main.rs`: main application UI and behavior integration.
- `src/vfx.rs`: UI interaction feedback/effects.
- `assets/rapidash.css`: application styling, including light/dark visual treatment.

The file structure above reflects the known development layout; verify the checked-in paths against the current tree. This document intentionally **does not claim a stable internal module structure or API**.

## Proposed separation as the codebase grows

1. **Collectors:** read-only CPU, memory, process, GPU, thermals, storage, and network data.
2. **Actions:** explicit user-requested operations with permission checks, dry-run support where applicable, and clear error results.
3. **Presentation:** GTK widgets, progressive detail, status, and feedback.
4. **Integrations:** browser-specific control and optional vendor/hardware adapters.

Actions should be isolated from UI events so a toast or animation never masquerades as proof that an underlying action succeeded.

## Future architecture decisions

- Sampling interval, data-source selection, and platform feature detection.
- Process privilege model and safeguards.
- Browser integration protocol, failure recovery, and permission model.
- Distribution/packaging requirements.
