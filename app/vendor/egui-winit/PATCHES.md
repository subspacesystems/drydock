# Local changes to egui-winit 0.36.2

This directory is the crates.io source of egui-winit 0.36.2, wired in with `[patch.crates-io]`
in `app/.cargo/config.toml` and the repository's `.cargo/config.toml`. The published
`Cargo.lock` is left out (cargo ignores it in a dependency). Only the changes below differ
from the published crate; each is marked `Local patch (Drydock, PATCHES.md)` in the
source. Keep this list complete when you change anything here, and carry it forward when egui
is upgraded.

- `LICENSE-MIT`, `LICENSE-APACHE`: added. The published crate carries no license files; these
  are the egui project's terms (MIT or Apache-2.0, as in `Cargo.toml`).
- `src/lib.rs` (`create_window`): the first window made for an `egui::Context` is the root
  viewport's (eframe creates it first); its id is kept in the context's temporary data. Every
  later window gets it through winit's `WindowAttributes::with_transient_parent` (patched winit,
  `vendor/winit/PATCHES.md`), so dialog and tool viewports belong to the main window.
  New public function `root_window_id`. eframe's glow backend builds its windows without
  `create_window` and is not covered; Drydock uses the wgpu backend.
- `src/lib.rs` (`create_window`): new public functions `mark_modal_title` and `is_modal_title`.
  A window made for a viewport whose title was marked (in the context's temporary data) gets
  winit's `WindowAttributes::with_modal(true)`; the root window never does. The title is the key
  because egui's `ViewportBuilder` has no modal flag and eframe does not pass the `ViewportId`
  to `create_window`.
