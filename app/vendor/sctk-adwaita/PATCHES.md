# Local changes to sctk-adwaita 0.10.1

This directory is the crates.io source of sctk-adwaita 0.10.1, wired in with
`[patch.crates-io]` in `app/.cargo/config.toml` and the repository's `.cargo/config.toml`.
The published `Cargo.lock` is left out (cargo ignores it in a dependency). Only the
changes below differ from the published crate; each is marked
`Local patch (Drydock, PATCHES.md)` in the source. Keep this list complete when you change
anything here, and carry it forward when sctk-adwaita is upgraded.

## Enabled title-bar buttons

Goal: a window can ask for no Minimize/Maximize button, while the compositor and the layout
still decide what is available. A button shows when the layout has it, the compositor allows
it and the window allows it; Close always shows.

- `src/buttons.rs`: `Buttons` keeps the compositor limits, the window's enabled buttons and
  resizability as separate fields, plus the last `arrange` arguments. `set_enabled` and
  `set_resizable` re-filter and re-place the buttons; `allows_maximize` combines all three
  conditions; `layout` returns the shown buttons per side in on-screen order.
- `src/lib.rs`: `AdwaitaFrame::set_enabled_buttons` passes the window's choice to `Buttons` and
  marks the frame dirty when the effective buttons changed; `AdwaitaFrame::set_resizable` also
  updates `Buttons`; the title double-click drops `MAXIMIZE` from the capabilities passed to
  `MouseState::click` when the window cannot maximize.
- `src/pointer.rs`: the module is public (hidden from docs) so the crate's integration tests can
  drive `MouseState::click` directly.
