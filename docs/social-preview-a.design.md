# Rapidash emblem-led social preview design record (retained version A)

## Evidence and claim boundaries

- Evidence mode: **local-static**. Inspected the current source and supplied screenshots; did not build, run, or exercise application actions.
- `README.md`: Rust/GTK4/Libadwaita Linux desktop utility; alpha status; process and browser memory management are the current development surface.
- `Cargo.toml`: package `rapidash`, Rust 2021, GTK4 and Libadwaita dependencies; version 0.1.0.
- `src/main.rs`, `src/types.rs`, `src/collectors/proc_linux.rs`: RAM/swap collection, process memory and CPU sampling, grouping, search/filtering, and UI update paths are present.
- `src/actions/controller.rs`: process stop/continue/terminate actions are present; behavior and safety were not runtime-tested.
- `src/collectors/chrome_bridge.rs`: local WebSocket bridge and browser command paths are present; functionality depends on a connected browser integration.
- NVIDIA-specific GPU polling exists in the collector, but this is not evidence of broad hardware support. It receives no visual claim in this cover.
- `src/vfx.rs` and `assets/rapidash.css`: warm flame accents, dark and light desktop styling, original symbolic fire and speed icons.
- `docs/ROADMAP.md`: broader CPU/thermal, GPU/fan, storage, network, container, and distribution work remains planned or subject to verification. Windows and macOS are future targets according to the user, and are not depicted.
- Source files establish implementation paths, not a successful build or validated release. No stability, universal hardware support, automatic optimization, benchmark, or measured performance claim is made.

## Promise, proof, exclude

- Promise: **System Monitoring & Memory Management**.
- Proof: Rust-powered; Linux-first; open source, supported by package metadata, Linux collection code, README and MIT license.
- Exclude: invented CPU/GPU statistics, guaranteed RAM savings, roadmap features, platform availability beyond Linux, release/stability badges, browser session contents and personal information.

## Source-material diagnosis

Inspected all four screenshots in `assets/screenshots/`, both raster branding assets (`assets/firefly.png` and the packaged app icon), the fire/speed symbolic SVGs, and application CSS.

| Material | Asset role | Identity / beauty / readiness / noise / deficit (0–3) | Decision |
| --- | --- | --- | --- |
| Dark overview / process explorer | Complete application artifacts | 3 / 2 / 1 / 3 / 0 | Strong implementation evidence, but excessive small text for this cover; omit pixels. |
| Light Chrome tabs | Complete application artifact | 2 / 2 / 0 / 3 / 0 | Contains personal session information; omit pixels. |
| Dark search | Complete application artifact | 2 / 2 / 1 / 2 / 1 | Too wide and text-heavy to support the requested emblem-led identity. |
| Existing raster mascot/icon | Primary subject | 3 / 2 / 0 / 1 / 0 | Excluded under the user's original-art requirement. |
| Fire / gallop symbolic icons, CSS | Glyphs and visual-system reference | 2 / 2 / 2 / 0 / 1 | Retain warm fire/speed semantics; draw a new emblem with independent geometry. |

Interpretation: **level 2, selective extraction of visual language**, with no screenshot reconstruction or embedded source pixels. Preserve flame color and speed semantics; remove UI density and existing character art; supplement with the original abstract emblem explicitly requested by the user. Aspect fit and fragment seams are not applicable because no source pixels are embedded.

## Direction and production

- Chosen: **ignition dial** — one forward-leaning flame inside an open performance-gauge arc, with a precise diagonal negative-space needle. The dial is an abstract brand symbol, with no numeric scale, measured readings, history, or simulated UI.
- Alternative considered: typography above an isolated screenshot crop. Rejected because thumbnail text and the requested emblem would compete, while a partial interface would add little proof at useful scale.
- Route: code-native SVG. Original vector geometry and editable live text; no image generation or external assets.
- Material: opaque `#111318` background with a subtle localized warm lighting field, `#F8FAFC` type, orange `#FF6B35`, amber `#FFB703`, and restrained red `#E63946`. No circuit-board pattern, grid wallpaper, neon halo, or decorative chart.
- Typography: installed Noto Sans Display/Noto Sans with standard sans-serif fallbacks. Text remains editable; no downloaded fonts or external runtime resources.
- Composition: title and two-line primary tagline in the left region; large emblem in the right region; secondary text below the tagline. Essential content stays inside at least 72 px margins.
- Topology: one flame, one enclosing open dial, one integrated needle. The gauge does not represent CPU/GPU availability or an achieved optimization value.
- Line ledger: gauge arc = symbolic instrument boundary; short radial ticks = instrument scale marks without values; diagonal cut through flame = needle/speed direction. No data series, connectors, or unexplained circuitry.

## Version and validation

- First cover at the requested paths; no previous social preview existed.
- Editable source: `docs/social-preview-a.svg`.
- Raster: `docs/social-preview-a.png`.
- Review sheet: review sheet inspected during original creation; current review sheet now shows the interface-led alternative.
- Renderer: existing system librsvg/Cairo; Sharp is not installed and no dependency installation is needed.
- Mechanical validation: **pass** using the bundled `validate_preview.py`. PNG is exactly **1280×640**, RGB with an opaque background, **144,540 bytes**, below 1,000,000 bytes. SVG has matching dimensions and viewBox, nonempty title/description, editable text, and no external resources.
- Full-size visual review: **pass**. Opened the rendered 1280×640 image; checked all text, the flame silhouette, smooth needle junction, dial endpoints, lighting, separation, and safe margins. No collisions, clipped words, unsupported telemetry, or copied mascot imagery.
- Thumbnail review on light and dark surrounds: **pass**. Opened the review sheet with two native 320×160 images. The title, two-line purpose, flame/gauge silhouette, and increased 28 px secondary text remain distinct; the opaque background preserves contrast on both surrounds.
- Refinements after first review: increased secondary text from 24 to 28 px, brightened it slightly, balanced the composition vertically, and joined the needle geometry smoothly before rerendering and repeating validation.
- Version verdict: **promote** as the initial cover after full-size and thumbnail inspection; no earlier baseline to replace.
- Upload status: not requested. Scope: local cover assets and this design record only.
