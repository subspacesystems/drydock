# m11-window-text-focus: text fields that take focus as a window opens get keys on X11

## Problem
On Linux/X11 without an input method server (the worker image's Xvfb, and possibly real
desktops), a `TextEdit` that calls `request_focus()` on the first frame of a window turns text
input (IME) on before the window has keyboard focus. After that the window never receives its
focus event, egui keeps `viewport().focused == Some(false)`, and the window gets no key events:
typing, Enter and Escape do nothing. It hits the Make Group / Make Component name prompt
(`chrome::name_prompt`), `dialog::name_dialog`, and both windows of `examples/modal_probe.rs`.
Clicking a field after the window has focus works (Materials search), so only the "focus as it
opens" case is broken.

A workaround was tried: request focus only while `!has_focus() && viewport().focused != Some(false)`
(`None` in kittest and inline dialogs, so the UI tests keep working). Focus and keys come back,
but typed text then arrived reversed ("box" -> "xob"), probably IME commit events landing at a
stale cursor. A fix that leaves text in the wrong order is not a fix.

## What to build
1. A shared helper in `dialog.rs`: "focus this field once its window has keys" (shape is yours;
   suggested `focus_when_window_active(ui, &response)`). It asks for focus only when the field
   lacks it and the window is not known to be unfocused, so the XIC is not created before the
   window has focus. In UI tests (focus `None`) it behaves like `request_focus()`.
2. Use it everywhere a field takes focus as its window opens: the Make Group/Component prompt,
   `dialog::name_dialog`, and `examples/modal_probe.rs` (both its main-window box and the
   prompt's Name box; the probe is orchestrator-owned but you may edit it for this).
3. Typed text arrives in order. Find out why it reverses (reproduce with the probe under Xvfb;
   debug with `DEBUG`-style eprintln of events) and fix the cause: in our code if possible,
   otherwise a small local patch in the vendored winit/egui-winit recorded in `PATCHES.md`
   (look at how the other M11 patches are recorded). Do not turn IME off for the whole app
   unless the cause turns out to be unfixable otherwise; if you do, say so in `PATCHES.md`/the
   helper's doc.
4. Rename `MODELER_GUI_TESTS` to `DRYDOCK_GUI_TESTS` in `tests/modal_window_x11.rs`,
   `tests/dialog_sun.rs` and `tests/dialog_window.rs` (messages and docs too), so they run in the
   test gate, which sets only `DRYDOCK_GUI_TESTS`. In `modal_window_x11.rs` change the main-window
   regex `^probe ` to `^probe clicks` (xdotool `--name` is a case-insensitive regex and
   `^probe ` also matches "Probe Name"). Grep for any other use of the old name (docs, harness
   notes that are tracked) and update it.
5. Make sure the modal-window real-window tests run in the gate: they need the examples built.
   If `harness/` is not yours to change, make the tests build what they need themselves (for
   instance `cargo build -p drydock --examples` from the test when the probe binary is missing,
   using the same target dir) so they pass from a clean `cargo test`. Never skip a failing test.

## Acceptance
`tests/window_text_focus_x11.rs` (spec commit; do not edit) with `DRYDOCK_GUI_TESTS=1` under
Xvfb: probe main-window box gets `abcdef` in order; the prompt's Name box gets `hello` in order
and Enter answers; the app's Make Group prompt takes typing and Enter. Also the three existing
real-window files pass with the renamed variable. The kittest tests for the prompts still pass.

To run by hand: `cargo build -p drydock --examples --bins` first (test filters do not build
examples), `DRYDOCK_GUI_TESTS=1`, under Xvfb with xdotool, xprop; mount `design/` and
`CONTRIBUTORS` into the container.

## Where to look
- `show name_dialog`, `show modal_dialog_window`, `show dialog_window` (dialog.rs): where the
  helper goes and who calls `request_focus`.
- `show name_prompt` in chrome.rs (Make Group/Component; `request_focus` near line 165).
- `examples/modal_probe.rs`: the probe, with its title format `probe clicks=N typed=T answer=A`.
- `tests/modal_window_x11.rs`, `tests/window_text_focus_x11.rs`: xdotool helpers (`the_window`, `type_into`, `wait_for`).
- `PATCHES.md` and `show mark_modal_title`: how local winit/egui-winit patches are done.
- egui `ViewportInfo::focused`, `Response::has_focus`, egui-winit's IME handling (`set_ime_allowed`, `Ime::Commit`).
