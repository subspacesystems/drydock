# m4-app-components-ui: Edit menu items and the name prompt

Make Component, Make Group and Explode become reachable from the window. The editor side
(`Editor::make_component`, `Editor::explode_selection`) is on main
(`docs/specs/m4-app-components.md`), and so is the wiring: `Action::{MakeComponent,
MakeGroup, Explode}`, `Effect::{MakeComponent { group }, Explode}`, `MenuState::can_explode`,
and `App` showing the prompt and calling the editor. Only `app/crates/modeler/src/chrome.rs`
changes:

- three new items in the Edit menu (`menu_bar`);
- the body of `name_prompt` (an `unimplemented!` skeleton now).

Tests: `app/crates/modeler/tests/chrome_components.rs` (egui_kittest, in-process). The other
tests must keep passing.

## Edit menu

After the Erase item and its separator, before the Flip items, add (then a separator, so the
Flip items stay a group of their own):

| Label | Enabled when | Returns |
| --- | --- | --- |
| `Make Component...` | `state.has_selection` | `Action::MakeComponent` |
| `Make Group...` | `state.has_selection` | `Action::MakeGroup` |
| `Explode` | `state.can_explode` | `Action::Explode` |

A disabled item does nothing when clicked. Clicking an enabled one closes the menu, like the
other items. No keyboard shortcut.

## `name_prompt(ctx, prompt) -> Option<PromptOutcome>`

Called every frame while the prompt is open; returns `Some` on the frame it is answered
(`App` then drops the prompt) and `None` otherwise.

- A window titled `Make Component`, or `Make Group` when `prompt.group`. Not collapsible,
  not resizable; it has a close button.
- Inside: a label `Name`, a one-line text field editing `prompt.name`, then buttons `OK` and
  `Cancel`.
- The text field has keyboard focus as soon as the prompt opens (typing without clicking
  goes into it).
- `OK`, or Enter: `PromptOutcome::Ok(name)` with `prompt.name` trimmed of surrounding
  whitespace. An empty name is fine (the kernel picks `Component` / `Group`).
- `Cancel`, Escape, or the window's close button: `PromptOutcome::Cancel`.
- Answer at most once per click or key press.

## Out of scope

Drawing the rest of the model faded or hidden while editing inside an instance (row
`m4-app-edit-view`), keyboard shortcuts, the editor methods themselves.
