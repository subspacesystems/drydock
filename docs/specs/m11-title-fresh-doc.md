# m11-title-fresh-doc: the window title after startup

## Why
The window is created with the title "Drydock" (`main.rs`). After startup it must change to the
fresh document's title, `untitled.drydock - Drydock`. `m11-dialog-window` made that title change
conditional (`title_pending = !about_open`) so a real-window test could find the main window by
its bare name. This task makes sure the normal start is right and keeps it right.

## Behaviour
- Normal start (no file argument, no test hook): the first frame sends the document's title, so
  the window is titled `untitled.drydock - Drydock` and no window is titled exactly `Drydock`.
- Start with `DRYDOCK_OPEN_DIALOG=about` (test hook only): the main window keeps the bare
  title `Drydock`. Keep this; `tests/dialog_window.rs` relies on it.
- Opening a file or File > New sets the title as before (unchanged).

## Work
1. Check the start path in `App::new` (the `else` branch for no file). It must leave
   `title_pending` true unless the About hook is on. If it already does, change no behaviour.
2. Remove any other way for the startup title to be lost (e.g. `title_pending` cleared before
   the first `ui` call).
3. Move the "should the first frame retitle?" decision into a small pure function in
   `app.rs`, `startup_title_pending(about_hook: bool) -> bool` (true unless the hook is on),
   with a unit test, so the rule is stated once.

## Tests (already written, do not edit)
`tests/title_fresh_doc.rs`:
- `a_fresh_document_has_the_untitled_title` (headless, always runs).
- `the_real_window_is_titled_with_the_fresh_document_after_startup` (Xvfb + xdotool, needs
  `DRYDOCK_GUI_TESTS=1`).

## Where to look
- `App::new`, `App::ui` (the `title_pending` block), `App::new_document` in `drydock/src/app.rs`
- `Fresh::new` in `drydock/src/start.rs`; `Document::title` in `drydock/src/document.rs`
- Real-window helpers in `drydock/tests/dialog_window.rs` (`windows`, `wait_for`, `Kill`)
