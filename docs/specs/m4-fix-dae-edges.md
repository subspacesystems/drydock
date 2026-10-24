# m4-fix-dae-edges: an opened DAE file shows its edges at once

Milestone M4, from Tyrel's report: after opening a large DAE model no edges are
drawn; any edit (he used Purge Unused in the Materials panel) makes them all appear.

## Why it happens today

`Document::open` (`crates/modeler/src/document.rs`) draws a DAE file with
`modeler_dae::flatten` of the imported file: the file's own triangles and the few lines it
lists. Exported files rarely list face edges. Every edit then calls `Document::refresh`,
which draws `kernel_scene` of the converted kernel model, with every edge.

## Behaviour

When the DAE file converts to a kernel model (`kernel` is `Some`), the document's `scene`
on open is `kernel_scene(&kernel)`, built *after* the texture paths are resolved (the same
order as today), and `stats` come from that scene (`stats_from_scene(&scene, Some(&kernel))`).
So the scene right after opening is exactly what `refresh` would draw, and the stats are
the same. When conversion fails (`kernel` is `None`, `kernel_error` set) the document keeps
drawing the flattened file as today. Native files are unchanged. `modified` stays false on
open.

Nothing else changes; the import itself, `refresh`, `refresh_editing` and `show_path` stay.

## Acceptance tests

- `crates/modeler/tests/document_dae_edges.rs`: the slab fixture opens with 5 lines (its
  quad's four edges and the post; the file lists only the post) and 2 triangles; for the
  slab and the tables fixture the opened scene equals `kernel_scene` of the document's
  kernel and equals the scene and stats after `refresh`; an opened DAE is not modified and
  draws lines.
- `crates/modeler/tests/document.rs` `open_reports_stats` and `crates/modeler/tests/native.rs`
  `dae_document_has_a_kernel_model` now expect 5 lines for the slab (they expected the
  flattened file's 1).
