# m4-texture-save: texture files stay found after Save As

Native files store a material's image path relative to the model file. In memory the
document keeps the path resolved, so it stays right wherever the file is saved.

## Functions (`modeler::document`)
- `resolve_texture_path(texture, dir)`: `texture` unchanged when absolute, else
  `dir.join(texture)` as a string (`dir` may be empty: the result is `texture`).
- `texture_path_for_file(texture, dir)`: the text to write for an in-memory path in a file
  saved into `dir`. Make both absolute (relative ones against the current directory, no disk
  access), drop `.` components, let `..` cancel a normal component before it, find the common
  leading components, then write one `..` per remaining component of `dir` followed by the
  rest of the texture path, joined with `/`. With no common root (different drives) the
  absolute texture path is returned. Same folder gives just the file name.

## Document
- `Document::open` of a native file: every material texture path is replaced by
  `resolve_texture_path(path, folder of the file)`. Apply it with
  `Model::apply(&ModelRecord::SetMaterial { id, from, to })`; no undo entry exists yet.
  DAE imports are untouched (their paths are already resolved).
- `save` and `save_as`: write a clone of the kernel model whose texture paths are
  `texture_path_for_file(path, folder of the target)`. The kernel model in the document is
  not changed, and `scene`/`stats` are not rebuilt. After `save_as` the document's paths are
  still the resolved ones, so a later save to a third folder is right too.
- Materials without a texture, and sizes, are written as before.

Run `cargo fmt --all` and clippy before you finish.
