# ADR-001: Stack

Status: Accepted, 2026-10-18

## Context
The plan fixes Rust and a desktop app (Linux first, Windows second). We need a GPU renderer that also runs headless on the build VM (Mesa lavapipe) for render-comparison tests, and a UI toolkit for tool palettes, dialogs, an entity info panel and typed-value entry (VCB-style measurement box). The ArtCraft Crafting Apps (github.com/storytold) are MIT/Apache Rust apps; CADCraft is a close cousin (2D/3D CAD, same toolkit questions).

## Decision
- **Language:** Rust, edition 2024, toolchain pinned in `app/rust-toolchain.toml` (1.99.1). `unsafe_code = "forbid"` workspace-wide; exceptions need an ADR.
- **Rendering:** `wgpu` (MIT OR Apache-2.0), WGSL shaders. Vulkan on Linux, DX12/Vulkan on Windows. Tests render offscreen through lavapipe (`WGPU_BACKEND=vulkan`, Mesa `lvp` ICD).
- **UI and windowing:** `eframe` + `egui` + `egui-wgpu` (MIT OR Apache-2.0), on `winit`. The 3D viewport is an egui paint callback into our own wgpu pipeline, so egui owns chrome and we own the scene. Same pattern CADCraft uses, which keeps reuse cheap. `egui_kittest` for UI tests.
- **Math:** `glam` (f64 types for the kernel, f32 for GPU buffers).
- **Version policy:** take the egui/eframe release that CADCraft and craft-libs use (0.36 today) rather than the newest (0.37, released 2026-10-15), so craft-libs crates link without a second egui. Move together with craft-libs.
- **ArtCraft reuse** (licenses checked 2026-10-18, all `MIT OR Apache-2.0` with Apache LICENSE/NOTICE files):
  - `storytold/craft-libs` as git dependencies pinned by `rev`: candidates `craft-ui` (egui controls), `craft-settings` (config storage), `craft-atomic` (crash-safe saves for the native format). Each one is adopted only when a task needs it; `deny.toml` allows this one git source.
  - `storytold/cadcraft`: reference for the app shell (eframe setup, wgpu canvas callback, packaging). Copy individual files only when it saves real work, keep their license header, and list them in `NOTICE`. No CADCraft assets (icons, fonts) unless their row in CADCraft's `ATTRIBUTION.md` shows a license we accept.
  - Snippet-scan matches against storytold repos are expected for copied files and go on the scanner's reviewed allowlist (`harness/oracle/snippet-allowlist.toml`); matches against copyleft projects never do.
- **DAE import:** our own COLLADA 1.4 reader on a streaming XML parser (`quick-xml`, MIT), written from the Khronos spec. No existing COLLADA crate is assumed; evaluate in the milestone 0 spec.

## Consequences
- One toolkit for app, tests and headless rendering; offscreen render tests work on the VM without a GPU.
- egui is immediate-mode: fine for tool UI, but the viewport's input handling (inference, snapping, typed dimensions) is ours and lives outside egui.
- Tracking craft-libs revs couples our egui version to theirs; revisit if that blocks an upgrade we need.
