# 🔥 Rapidash

**A modern, open-source Linux desktop utility for system monitoring, process management, and browser memory optimization.**

![Platform](https://img.shields.io/badge/platform-Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust-orange)
![GTK4](https://img.shields.io/badge/UI-GTK4%20%2B%20Libadwaita-blue)
![License: MIT](https://img.shields.io/badge/license-MIT-green)
![Status](https://img.shields.io/badge/status-active%20development-yellow)

> [!IMPORTANT]
> **Early development / alpha.** Rapidash is not yet a stable system administration tool. Some controls and integrations are experimental, and the project does not currently provide guaranteed cross-distribution support. See [Roadmap](docs/ROADMAP.md) for planned work.

## Why Rapidash?

Desktop resource monitors often show *what* is consuming memory, but make it harder to act on individual applications or browser sessions. Rapidash aims to bring useful visibility and carefully designed actions together in a modern GNOME-style interface.

The project is built with **Rust**, **GTK4**, and **Libadwaita**. It is focused on being understandable and lightweight rather than adding AI features or overwhelming the interface with metrics.

## Preview

<!-- Add real, redacted app screenshots before publishing this section. -->
<!-- ![Rapidash overview](assets/screenshots/overview.png) -->
<!-- ![Live Process Explorer](assets/screenshots/process-explorer.png) -->
<!-- ![Chrome Tabs and Windows](assets/screenshots/chrome-tabs.png) -->

Screenshots will be added as the interface stabilizes. See [Screenshot and branding guidelines](docs/ASSETS.md).

## Current development surface

The items below describe what is represented in the current application/UI code. **A visible button or control does not necessarily mean that its underlying operation has been validated on every system.**

| Area | Current development UI | Notes |
| --- | --- | --- |
| Memory overview | RAM summary and usage-oriented cards | Displayed information depends on platform support and integration status. |
| App Groups | Application/group-oriented resource view | Designed to make memory consumers easier to understand. |
| Live Process Explorer | Process listing, search, filtering, and process action controls | Freeze/wake/quit/force-quit actions are potentially disruptive; validate behavior before relying on them. |
| Chrome Tabs & Windows | Tab/window views, search, and browser optimization controls | Hibernation, extension, and GPU/cache operations depend on supported browser integration and permissions. |
| Interface | GNOME-style light/dark themes and action feedback | Uses GTK4/Libadwaita, styling, and visual feedback. |

Current code paths include `src/main.rs`, `src/vfx.rs`, and `assets/rapidash.css` in the development tree. Availability of individual files may vary by branch or revision.

## Planned capabilities

The following are **roadmap goals, not released features**:

- **CPU:** per-core usage, process/core breakdown, temperature, and thermal information where available.
- **Cooling & thermal telemetry:** fan RPM readings, throttling indicators, and vendor-dependent fan controls only where safe and supported.
- **GPU:** utilization, VRAM, and temperature monitoring where drivers expose the necessary data.
- **Memory:** clearer memory-pressure indicators, reclamation visibility, and safe cache-management operations.
- **Processes & containers:** richer CPU/memory/I/O details and Docker visibility.
- **Storage & network:** disk capacity and I/O metrics, network bandwidth, and connection summaries.

See the [prioritized roadmap](docs/ROADMAP.md) for sequencing, design principles, and limitations.

## Requirements

**Primary development environment:** Linux with a GNOME-compatible desktop; originally developed on Ubuntu 22.04. Distribution and library version compatibility are still being assessed.

To build from source, you need:

- Rust toolchain (`rustc` and `cargo`, preferably installed through `rustup`)
- GTK 4 and Libadwaita development libraries
- C build tools and `pkg-config`
- A graphical Linux session to launch the app
- Google Chrome or a compatible browser **only for browser-specific functionality**

### Ubuntu / Debian prerequisites

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-4-dev libadwaita-1-dev
```

If the repository's feature versions require a newer GTK/Libadwaita than your distribution supplies, the build may fail; check the `Cargo.toml` feature requirements and your installed library versions before changing system packages.

Install Rust if necessary using the official instructions at [rustup.rs](https://rustup.rs/).

## Build and run

```bash
git clone https://github.com/devanshkhandekar/rapidash.git
cd rapidash

cargo check
cargo run --release
```

For a full build:

```bash
cargo build --release
```

If the project includes a dedicated setup script, review the script before executing it. Current development builds are **source-first**; prebuilt packages are not yet promised.

## Safe usage

Rapidash may expose operations that pause, terminate, hibernate, or otherwise affect applications and browser activity. Such actions can interrupt work or cause unsaved data to be lost. Review each action before using it, especially on production or shared machines. Availability of GPU, thermal, cache, browser, and process controls depends on hardware, drivers, kernel permissions, and desktop environment.

## Contributing

Contributions and well-scoped issues are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md), check the [roadmap](docs/ROADMAP.md), and open an issue before proposing a large architectural change.

Useful first contributions include testing on different Linux distributions, documenting system metric sources, accessibility fixes, reproducible bug reports, and small improvements to process/browser UX.

## Project maturity

- **Status:** alpha / ongoing development
- **Primary target:** Linux desktop
- **Packaging:** planned after reproducible builds are validated
- **Stable releases:** not yet available
- **LLM/ML components:** not part of the current scope

## Security

For potentially sensitive vulnerabilities, follow [SECURITY.md](SECURITY.md) instead of posting exploit details in a public issue.

## License

Rapidash source code is available under the [MIT License](LICENSE). Third-party libraries, icons, fonts, and other assets retain their respective licenses; see the [third-party licensing checklist](docs/THIRD_PARTY.md). All project branding must be original or used with documented permission.

---

**Built in Rust. Designed for a clear, fast Linux desktop experience.**
