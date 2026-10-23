# m4-app-texture-position: move, turn and scale a texture on a face

One `Editor` method (`app/crates/modeler/src/editor.rs`, on main as `unimplemented!()`) and
the `TextureOp` enum beside it. Only `editor.rs` changes. Tests:
`app/crates/modeler/tests/editor_texture_position.rs`. The handles and typed-value UI that
call it come later. Read `docs/specs/m4-texture-model.md` for `TextureMap`, `side_map` and
`set_texture_maps` in `modeler_kernel::texture`.

## `position_texture(model, face, side, op) -> bool`

Works on the editor's open body (`self.body(model)`). Texture-space numbers are meters, as
in `TextureMap`.

1. The face must exist in the body and the material on `side` (`face_materials` front or
   back) must be set and have a texture. Otherwise: message `That side has no texture`,
   nothing changes, false. (A face with no own material, or a missing face, counts as no
   texture.)
2. The current map is `texture::side_map(model, body, face, side)`. The new map is:
   - `Move(d)`: `map.then(DAffine2::from_translation(d))`.
   - `Rotate { angle, about }`: the image turns by `angle` about the texture-space point
     `about`: `map.then(...)` with the affine `translate(about) · rotate(angle) ·
     translate(-about)` applied to `(s, t)`.
   - `Scale { factor, about }`: likewise with a scale about `about`. A factor with a
     component that is not finite or not > 0 is refused (below).
   - `Reset`: no stored map (`None`), so the default placement returns.
3. Store it with `texture::set_texture_maps` for that one face and side. Label
   `Position Texture`, or `Reset Texture` for `Reset`. Success: message cleared, true when
   the stored value changed. If it did not change (a zero move, a reset of a side with no
   stored map) there is no undo step, the message is cleared and the result is false.
4. Non-finite numbers in `op`, or a bad scale factor: message `Can't position that
   texture`, nothing changes, false. Kernel errors: the same message.

Only the given side changes; the other side's map, both materials and every other face are
untouched. Selection, the current tool and the Paint Bucket's material are not touched.
Undo restores the previous stored value exactly (including `None`).
