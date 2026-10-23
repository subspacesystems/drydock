# m4-fix-refresh-speed: don't reload textures and rebuild twice on every refresh

Milestone M4, follow-up from Tyrel's M4 review ("it takes a couple seconds on
complex models to get in to edit a component").

## Measurements (release build, corpus models)

| Model | build the kernel scene | read, decode and mip all its textures |
| --- | --- | --- |
| house (64 textures) | 0.32 s | 0.96 s |
| manor (37 textures) | 0.32 s | 1.65 s |
| desk and chair (12 textures) | 0.03 s | 1.34 s |

Every scene upload (`Renderer::upload`) reads and decodes every texture file again and
rebuilds its mip chain, and `Document::refresh_editing` builds the scene twice (once to
draw, once more for the status-bar stats). Opening or closing an instance redraws through
the same path, so it costs more than a second on textured models, and so does every edit.

## What to build

### 1. Texture cache in `Renderer` (`crates/modeler-render/src/lib.rs`)

The renderer keeps the textures it has loaded and reuses them in later uploads:

- Key: the texture file path. An entry is reused only while the file's modification time
  and length (from `std::fs::metadata`) are what they were when it was loaded; otherwise
  the file is read again and the entry replaced.
- A file that can't be read or decoded is not remembered as failed: every upload tries it
  again and reports its warning again (as today), so a texture that appears later loads.
  A missing file is still a read attempt.
- Materials sharing a path in one scene read it once (as today).
- `GpuScene` holds shared references to cached textures (for example
  `Arc<LoadedTexture>`), so dropping an old `GpuScene` never frees a texture a newer
  one uses.
- `Renderer::texture_loads(&self) -> usize` (skeleton on main): how many times this
  renderer has tried to read a texture image file, successful or not. `upload` keeps
  taking `&self`; use interior mutability (`Mutex`, atomics) so `Renderer` stays `Send`
  and `Sync`.
- Entries are never evicted in this task (a session's textures are a few hundred MB at
  most; eviction can come later).

### 2. `Document::show_path` (`crates/modeler/src/document.rs`)

```rust
pub fn show_path(&mut self, path: &EditPath, rest: RestView)
```

Sets `scene` to `kernel_scene_editing(kernel, path, rest)`. `stats` and `modified` stay
as they are (opening or closing an instance is not an edit). Without a kernel model it
does nothing.

### 3. `Document::refresh_editing` builds the scene once at the root

At the root path the editing scene is the normal scene, so compute `stats` from it instead
of building `kernel_scene` a second time. Inside an instance it still needs the full scene
for the stats (the drawn scene is partial); keep that.

### 4. App wiring (`crates/modeler/src/app.rs`)

`App::sync_editor_path` (the open path changed with no model edit) calls
`Document::show_path` instead of `refresh_document`, then uploads the scene and updates
`view.bounds` and `shown_path` as `refresh_document` does. It doesn't touch the title or
the modified flag. Toggling View > Hide Rest of Model goes through the same path-only
redraw.

## Tests

- `crates/modeler-render/tests/texture_cache.rs` (8): repeated uploads read nothing,
  shared paths read once, only new files are read, textures dropped from a scene are
  reused later, a changed file (new mtime and length) is read again, a missing file is
  retried and warned every upload until it exists, and a scene from cached textures
  renders after an older scene is dropped. Needs a headless adapter (lavapipe), like
  `tests/render.rs`.
- `crates/modeler/tests/document_show_path.rs` (6): `show_path` draws the editing scene,
  root draws the normal scene, stats and the modified flag are untouched, and
  `refresh_editing` still counts the whole model.

## Out of scope

Incremental scene building, background texture loading, cache eviction.
