# m3-app-erase: Erase in the editor

The editor (`app/crates/modeler/src/editor.rs`) erases the selection through the kernel's
`modeler_kernel::erase::erase` (read `docs/specs/m3-erase.md`; it is being built in parallel and
will be on main before you start). Skeletons already on main: `Tool::Erase`, `Action::Erase`
(Delete key and Edit menu item, wired in `app.rs`), the toolbar button, and two
`unimplemented!` bodies in `editor.rs`. Only `editor.rs` changes. Tests:
`app/crates/modeler/tests/editor_erase.rs`. The other tests must keep passing.

## `Editor::erase_selection(&mut self, model) -> bool`

- Empty selection: message `Select something to erase`, model unchanged, false.
- Otherwise call `erase(model, BodyId::Root, selection, undo, "Erase")`. On success: clear the
  message, drop the erased entities from the selection (`prune`), true. On `Err`: message
  `Can't erase that`, model unchanged, false.
- Any operation in progress (line, shape, push/pull, rotate, move, scale, tape, typed text) is
  dropped first, the way undo does it, so no stale preview is left. The hint is cleared.

## The Erase tool

`click` with `Tool::Erase` (count ignored): pick under the cursor as the Select tool would.
Nothing there: false, no message. An edge, face or instance of the root body: erase just that
entity (not the selection) with the label `Erase`, same success and failure handling as above;
the selection keeps whatever is still alive. `pointer_move` does nothing for this tool and
never changes the model. The tool takes no typed text.

## Keys

- `Backspace` (`EditKey::Backspace`): while typed text is non-empty it removes the last
  character, as now. With no typed text and the tool `Select` or `Erase` it calls
  `erase_selection` and returns its result; with any other tool and no text it does nothing.
- `input::key("Delete", plain)` is `Action::Erase` and `input::key("E", plain)` selects the
  Erase tool (already on main).
