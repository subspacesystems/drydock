# m11-wayland-dialog-buttons: dialog windows show only Close on GNOME Wayland

Tyrel (Ubuntu): dialog windows have minimize and maximize buttons; on Windows they show
only Close. On GNOME Wayland the title bar is drawn by the app (winit's `AdwaitaFrame` from the
vendored `sctk-adwaita`). It shows every button in GNOME's button-layout setting that the
compositor allows, nothing per window. winit's Wayland `set_enabled_buttons`/`enabled_buttons`
are TODO no-ops and `WindowAttributes::enabled_buttons` is never applied. `dialog.rs` already asks
for `with_minimize_button(false).with_maximize_button(false).with_resizable(false)`; egui-winit
passes that on. No GNOME or Wayland compositor runs here, so Tyrel checks on Ubuntu.

Mark every change `Local patch (Drydock, PATCHES.md)` and list it in both
`app/vendor/winit/PATCHES.md` and a new `app/vendor/sctk-adwaita/PATCHES.md` (same style as
winit's: what differs from the published crate, file by file).

## 1. sctk-adwaita (`src/buttons.rs`, `src/lib.rs`, `src/pointer.rs`)

Skeleton on main: `buttons` and `pointer` are `#[doc(hidden)] pub` modules so the crate's own
integration tests (`vendor/sctk-adwaita/tests/buttons.rs`) can reach them; `Buttons` has the new
methods below as `todo!()`.

Rule: a button is shown when **the layout has it AND the compositor allows it AND the window allows
it**. Close always shows.

- `Buttons::set_enabled(minimize, maximize)`: what the window allows (default: both true).
- `Buttons::set_resizable(resizable)`: a window that is not resizable has no Maximize (GNOME's rule).
  Keep this a separate condition from `set_enabled`: a resizable floating panel can still drop
  Minimize and keep Maximize, and setting resizable back to true must not undo `set_enabled(_, false)`.
- `Buttons::allows_maximize()`: compositor allows Maximize AND window allows it AND resizable.
- `Buttons::layout()`: shown buttons per side in on-screen order, left to right (the right side is
  stored reversed; `layout()` flips it back). The `appmenu` entry is ignored as today.
- Every change (these setters and `update_wm_capabilities`/`update_buttons`) re-filters the layout
  and re-places the buttons: remember the last `arrange` arguments (width, margin) and call
  `arrange` again, otherwise the remaining buttons keep stale offsets. Compositor limits and window
  limits are kept separately, so a later capability update does not lose the window's choice and
  vice versa.
- `AdwaitaFrame::set_enabled_buttons(&mut self, minimize: bool, maximize: bool)` (inherent method):
  calls `buttons.set_enabled`, and sets `dirty` when the effective buttons changed (compare the
  `layout()` before and after). `AdwaitaFrame::set_resizable` also calls `buttons.set_resizable`.
- Double-click on the title must not maximize a window without Maximize: in the frame's click
  handler, when `!self.buttons.allows_maximize()` pass `MouseState::click` the capabilities with
  `WindowManagerCapabilities::MAXIMIZE` removed (then it only moves). `pointer.rs` needs no other
  change beyond being public.

## 2. winit Wayland (`wayland/window/mod.rs`, `wayland/window/state.rs`)

- `WindowState` stores `enabled_buttons: WindowButtons` (default all) with a getter and a setter
  `set_enabled_buttons(&mut self, WindowButtons) -> bool` that updates the frame (concrete
  `WinitFrame` is the `AdwaitaFrame`, so call its inherent method with
  `buttons.contains(MINIMIZE)`, `contains(MAXIMIZE)`) and returns whether it changed.
  Compare how `resizable`/`set_resizable` reach the frame (~state.rs 541) and do the same.
- `Window::new`: apply `attributes.enabled_buttons` before the first frame draw, next to the
  `set_resizable(attributes.resizable)` call (~mod.rs 147). If the frame does not exist yet when
  it is set (the frame is created later, on configure), apply the stored value when the frame is
  created, wherever `set_resizable` is applied to a new frame.
- `Window::set_enabled_buttons` stores and applies, then `request_redraw()` when it changed (as
  `set_resizable`); `Window::enabled_buttons` returns the stored value. Remove the TODO text.
- With the other frame types (no CSD, server-side decorations) nothing is drawn: storing the value
  is enough.
- X11: out of scope; leave its no-op (Mutter draws those frames and drops the buttons for the
  DIALOG-typed windows the existing patch makes).

## 3. Who gets what

No change in `drydock` source: dialogs and prompts (`dialog.rs`) already ask for close only;
floating panels (`floating.rs`, resizable) keep Minimize and Maximize. Do not edit them.

## Tests

- `vendor/sctk-adwaita/tests/buttons.rs` (13 tests, behaviour: three buttons, close only, not
  resizable, compositor without minimize, order and side, independent conditions, re-placing after a
  change, double-click rule). Run: `cargo test -p sctk-adwaita`; the oracle's workspace run covers it.
- `crates/drydock/tests/wayland_dialog_buttons.rs`: source checks that the winit patch exists
  (no compositor to run). Make them pass by doing the work, not by adding comment text.
- Neither file may be edited. Keep `cargo clippy -D warnings` clean on the vendored crates' changed
  code (`cargo clippy -p sctk-adwaita -p winit --all-targets`).

## Where to look

- `show Buttons::update_buttons`, `show Buttons::arrange`, `show Buttons::parse_button_layout`,
  `show Buttons::right_buttons_start_x` (buttons.rs); `show MouseState::click` (pointer.rs).
- `show AdwaitaFrame::update_wm_capabilities`, `AdwaitaFrame::set_resizable`,
  `AdwaitaFrame::resize`, `AdwaitaFrame::click` (lib.rs; the click handler near line 480).
- winit: `WindowState::set_resizable` (wayland/window/state.rs ~541), `Window::new`,
  `Window::set_resizable`, `Window::set_enabled_buttons` (wayland/window/mod.rs ~147, ~408, ~421);
  `WindowAttributes::enabled_buttons`, `WindowButtons` (src/window.rs).
- `vendor/winit/PATCHES.md` for the entry style; `crates/drydock/src/dialog.rs` (~440) for the
  attributes dialogs already set.
