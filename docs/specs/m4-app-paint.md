# m4-app-paint: Paint Bucket in the editor

The editor (`app/crates/modeler/src/editor.rs`) gets a Paint Bucket tool that puts the
current material on faces and instances through the kernel's
`modeler_kernel::paint::{paint, sample, Side}` (read `docs/specs/m4-paint.md`; it is on
main). Already on main: `Tool::Paint`, the `B` key, the toolbar button `Paint Bucket`,
`Editor::material` / `Editor::set_material` and an `unimplemented!` body for
`Editor::click_paint`. Only `editor.rs` changes. Tests:
`app/crates/modeler/tests/editor_paint.rs`. The other tests must keep passing.

## Current material

`Editor::material()` is the material the Paint Bucket puts down (`None` = the default
material, which also "unpaints"). It starts as `None`, is set by `set_material` and by
sampling, and survives tool changes.

## A click with `Tool::Paint` (`count` ignored)

1. Pick under the cursor the way the Select tool does (`pick_at`). Nothing there: return
   false, no message, nothing changes.
2. **Seen side** of the picked entity: for a face, take the ray direction `dir` from
   `Viewport::ray(cursor)` and the normal `n` of `Context::face_plane(face)` in the root
   body; the seen side is `Side::Front` when `dir · n < 0`, else `Side::Back` (a face seen
   exactly edge-on, or with no plane, counts as `Front`). For an edge or an instance the seen
   side is `Front`.
3. **Alt held** (whatever else is held): sample instead of painting. For a face or an
   instance, the current material becomes `sample(model, BodyId::Root, entity, seen side)`
   (inner value; an unpainted side gives `None`). The message is cleared. For an edge,
   nothing happens. Either way the model and the undo stack don't change and the click
   returns false.
4. **Otherwise paint.** Targets: the whole selection when the picked entity is in it,
   else just the picked entity. If the picked entity is an edge that is not selected, return
   false with no message and no change. Call
   `paint(model, BodyId::Root, targets, seen side, self.material(), undo, "Paint")`:
   - `Ok(n)`: clear the message; return `n > 0` (painting what is already that material
     changes nothing and pushes no undo step);
   - `Err(_)` (for example the current material was deleted): message `Can't paint that`,
     return false; nothing changes.
5. Painting never changes the selection. Shift and Ctrl don't change what a click does.

`pointer_move` with this tool sets no hint and never changes the model.

## Not in scope

- Painting faces inside an instance (needs `m4-edit-path`), painting all connected or all
  matching faces, texture placement.
- The materials panel and a display of the current material (`m4-app-materials`).
