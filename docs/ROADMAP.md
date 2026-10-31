# Rapidash Roadmap

> **Roadmap, not a promise of implemented functionality.** Scope and order may change as hardware support and usability are validated.

## Principles

1. Show the few metrics that help people make decisions; keep deeper diagnostics in detail views.
2. Separate **observation** (read-only) from **actions** (potentially disruptive).
3. Clearly state unavailable data rather than displaying estimated values as measured values.
4. Prefer safe, reversible behavior; never claim a cache purge or RAM saving succeeded without confirming the operation.
5. Build Linux-first without claiming universal fan/GPU support.
6. No ML/LLM features, and no animation work until core functionality is stable.

## Phase 0 — Stabilize what exists (highest priority)

- [ ] Make `cargo check`, `cargo run --release`, and basic GitHub CI reproducible.
- [ ] Verify memory cards, App Groups, Live Process Explorer, and Chrome tabs/windows on a clean install.
- [ ] Verify which process and browser actions work; add clear success/failure feedback.
- [ ] Review permissions, edge cases, and safeguards for potentially destructive actions.
- [ ] Capture accurate redacted screenshots and publish supported-environment notes.

## Phase 1 — CPU and memory visibility

- [ ] CPU total usage, per-core history, and per-process CPU use.
- [ ] CPU temperature and thermal sensors where exposed by the OS/hardware.
- [ ] Memory use, available vs cached memory, pressure, swap, and clearer reclaim semantics.
- [ ] Low-noise detail drilldowns and meaningful status indicators.

## Phase 2 — GPU, cooling, and processes

- [ ] GPU utilization, VRAM use, and temperature using available driver interfaces.
- [ ] Fan RPM readouts and thermal/throttling indicators where supported.
- [ ] Process CPU/memory/I/O details and grouping, including terminal workloads.
- [ ] Docker/container visibility where Docker is installed and permissions allow it.
- [ ] **Research only:** opt-in fan controls for supported hardware with explicit limits and fail-safe logic. Do not assume universal support.

## Phase 3 — Storage and network

- [ ] Storage usage and disk I/O activity.
- [ ] TRIM capability/status visibility (not blind execution).
- [ ] Network throughput and per-process connection details if safely available.

## Phase 4 — Reliability and distribution

- [ ] Configuration and preference persistence.
- [ ] Accessibility, localization, and performance testing.
- [ ] Repeatable Linux packaging and distribution-specific docs.
- [ ] Signed/tagged releases with changelog, assets, and integrity checks as appropriate.

## UI organization (proposed)

- **Overview:** RAM, CPU, and the most actionable high-level signals.
- **Processes:** grouped applications and detailed Live Process Explorer.
- **Browser:** Chrome tabs/windows and browser-specific actions.
- **System details:** GPU, thermals/fans, storage, and network in progressive detail.
- **Settings:** appearance, refresh rate, permissions, safeguards, and diagnostics.

## Not in current scope

- LLM/ML systems.
- Decorative animation overhauls.
- Automatic invasive fan control or one-click claims of universal hardware optimization.
