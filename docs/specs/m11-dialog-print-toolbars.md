# m11-dialog-print-toolbars: Print Setup and Customize Toolbars in their own windows

Part of M11. Depends on `m11-dialog-window` (the helper `dialog::dialog_window`).

## What changes
Tools > Print Setup... and the Customize Toolbars window stop being `egui::Window`s inside the
main window and become dialog windows: their own OS windows with their own titles, opened
centered over the main window, closed from the title bar or with Escape. Both are modeless:
the main window stays usable while they are open.

The window bodies are already in their own functions (skeleton, on main):
- `print_setup::print_setup_window(ctx, open, texts, view) -> PrintSetupEvent`
- `toolbars::customize_window(ctx, open, layout, state) -> bool` (true when the layout changed)

`app.rs` already calls both; **do not edit app.rs**. Change only the two function bodies.

## Steps
1. `print_setup_window`: replace the `egui::Window` by `dialog::dialog_window` with
   `DialogWindow { title: "Print Setup", size }`. Pick a size that fits the contents (about
   360 x 420 points; check nothing is cut off). Call `print_setup_ui` inside it and return
   its event. Nothing is drawn when `!*open`. Closing from the title bar or Escape clears `open`.
2. `customize_window`: the same with title "Customize Toolbars" (about 520 x 480 points; two
   columns of tool rows must fit). Return whether `customize_panel` changed the layout.
3. `print_setup::window_pos` and `WINDOW_MARGIN` stay as they are (another test uses them);
   they are no longer used by the app.
4. Do not touch the contents (`print_setup_ui`, `customize_panel`), the labels, or the events.

## Dragging in Customize Toolbars
Dragging a tool between "On the toolbar" and "Available" happens entirely inside the dialog's
own window (both lists, the insertion line and the drop are in `customize_panel`); it never
drops onto the main window's toolbars. So no cross-window drag is needed. Pointer positions
inside a dialog window's viewport are in that window's own coordinates, which is what
`customize_panel` already uses (`interact_pointer_pos`, the lists' rects), so it must keep
working unchanged. Do not add any code for this; the test
`dragging_a_tool_into_the_list_works_inside_the_window` guards the embedded case. The
real-window drag is checked at sign-off.

## Escape and the rename dialog
Customize Toolbars opens a modal prompt (new/rename/delete toolbar) over its contents. While
such a prompt is up, Escape belongs to the prompt. If `dialog_window` closes the window on
Escape regardless, make `customize_window` pass the prompt state through so Escape with a
prompt open does not also close the window (`CustomizeState` has the open prompt, see
`show CustomizeState`). If `dialog_window` already handles this, nothing to do. The tests in
`tests/confirm.rs` and `tests/toolbar_customize_rename.rs` must keep passing.

## Tests
`crates/drydock/tests/dialog_print_toolbars.rs` (already in place): contents shown while open and
absent while closed, buttons still produce events inside the window, drag still inserts a tool,
Escape closes each window (these two fail until the helper is used). UI tests embed viewports,
so these run the inline fallback. All existing tests (`print_setup*.rs`, `toolbar_*.rs`,
`dialogs_enter.rs`, `confirm.rs`) must keep passing.

## Where to look
- `show dialog_window`, `show DialogWindow`, `show centered_position` (src/dialog.rs)
- `show print_setup_window`, `show print_setup_ui`, `show PrintSetupView` (src/print_setup.rs)
- `show customize_window`, `show customize_panel`, `show CustomizeState` (src/toolbars.rs)
- Caller (read only): `show App::print_setup_window` in src/app.rs
- Test helpers: `drag`, `centre` in tests/dialog_print_toolbars.rs
