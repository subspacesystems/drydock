# m4-fix-clipboard-keys: Ctrl+X, Ctrl+C and Ctrl+V work

Milestone M4, from Tyrel's report: Ctrl+C and Ctrl+V do nothing; Copy and
Paste from the Edit menu work.

## Why it happens today

`App::global_input` (`crates/modeler/src/app.rs`) looks for `Event::Key` with C, X or V and
Ctrl. egui's window backend never sends those: it turns Ctrl+X/C/V (Cmd on macOS) into
`Event::Cut`, `Event::Copy` and `Event::Paste(text)` and returns without a key event.
Worse, it only sends `Event::Paste` when the system clipboard holds non-empty text, and the
model clipboard puts nothing there.

## Behaviour

- Ctrl+C copies the selection, Ctrl+X cuts it, Ctrl+V starts a paste, exactly like the
  Edit menu items (same `Action::Copy`/`Cut`/`Paste` through `App::apply`).
- After a successful Copy or Cut from the keys or the menu, the app also puts a short text
  on the system clipboard with `ctx.copy_text(...)`, for example
  `"modeler: 3 items copied"`. That keeps the system clipboard non-empty, so the next
  Ctrl+V reaches the app as `Event::Paste`. (If the user copies text in another program,
  Ctrl+V in the viewport still pastes the model clipboard; that is fine.)
- When a text field has keyboard focus (Materials search, a measurement being typed, the
  name prompt), the keys belong to that field: the app does nothing with them, as today
  (`ctx.egui_wants_keyboard_input()`), and while a measurement is being typed in the
  status bar they are ignored like other non-editing keys (today's `typing` check).

## What to build

### `chrome::clipboard_actions` (`crates/modeler/src/chrome.rs`, skeleton on main)

```rust
pub fn clipboard_actions(events: &[eframe::egui::Event]) -> Vec<Action>
```

For each event, in order: `Event::Copy` → `Action::Copy`; `Event::Cut` → `Action::Cut`;
`Event::Paste(_)` (any text, even empty) → `Action::Paste`; an `Event::Key` that is pressed,
not a repeat, with `ctrl` or `command` and neither `shift` nor `alt`, for `Key::C`, `Key::X`
or `Key::V` → `Copy`, `Cut` or `Paste` (so tests and backends that do send key events keep
working). Everything else gives nothing.

### App wiring (`crates/modeler/src/app.rs`)

In `global_input`, after the `egui_wants_keyboard_input` early return: apply the actions of
`clipboard_actions(&events)` (skipped while typing, as today), and remove the C/X/V arms
from the key loop so a key event can't apply twice. In the `Effect::Copy` and `Effect::Cut`
arms, when the editor reports success, call `ctx.copy_text` with the short text.

## Acceptance tests

`crates/modeler/tests/clipboard_keys.rs`: clipboard events become actions in order (Paste
with any text); Ctrl and Command key presses for C/X/V count; releases, repeats, Shift or
Alt combinations, other keys and text events give nothing; no events give nothing.

The orchestrator checks the wiring in the real window (Xvfb, xdotool `ctrl+c`, `ctrl+v`):
select an instance, Ctrl+C, Ctrl+V, click to place.
