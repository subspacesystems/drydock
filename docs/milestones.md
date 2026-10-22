# Milestones

In build order. Each milestone ends when every listed task is merged and Tyrel's validation for its features can be done in the app. Specs and failing tests for a milestone are written just before it starts, in batches; this file lists the intended tasks so the order and dependencies are visible.

| | Milestone | Section 2 features | Depends on |
| --- | --- | --- | --- |
| M0 | Viewer | Orbit, pan, zoom; DAE import (display only) | |
| M1 | Geometry kernel and native file | (foundation for every editing feature) | M0 |
| M2 | Drawing | Basic drawing tools, typed dimensions, select | M1 |
| M3 | Modify | Push/Pull, move, rotate, scale, flip, tape measure | M2 |
| M4 | Components and materials | Components, materials | M3 |
| M5 | Sun | Geolocation and shadows | M4 |
| M6 | Medium priority | Follow Me, offset | M3 |
| M7 | Low priority | Tags, model library hook | M4 |
| M8 | Windows | Windows build and installer | M4 |

MVP = M0 to M5 (all High features).

## M0: Viewer

A window that opens a corpus DAE file and orbits, pans and zooms it.

| Task | What | Needs |
| --- | --- | --- |
| `m0-dae-import` | COLLADA 1.4 reader to a structured model, and flattening to world-space batches | |
| `m0-camera` | Orbit/pan/zoom camera, standard views, zoom extents; property tests | |
| `m0-render` | wgpu renderer: materials, textures, transparency, lines; headless rendering on lavapipe | |
| `m0-app` | eframe window: open file (argument, dialog, drag and drop), mouse/keyboard navigation, status bar | the three above; (licenses) |
| `m0-corpus-render` | Render every corpus model from iso/top/front; compare silhouettes to the reference PNGs | `m0-render` (orchestrator writes the comparison and calibrates thresholds) |

Done when: `modeler` on the reference house model opens in under 5 s on modeler-build (release build), looks right in all standard views, and navigation is smooth on Tyrel's desktop.

## M1: Geometry kernel and native file (orchestrator-designed, kernel tasks)

- Data model: vertices, edges, faces with outer and inner loops, groups and component definitions/instances. Edges and faces are shared ("sticky") within a context; groups/components isolate.
- Invariants (property-tested): every face is planar within tolerance and bounded by closed edge loops; no two edges overlap; an edge crossing another splits both; coplanar edges that close a loop create a face; deleting the edge between two coplanar faces merges them, and deleting any other edge that bounds a face removes the face.
- Undo/redo as reversible operations on the model.
- Native file format: versioned, documented in `docs/format.md`, crash-safe save (write-then-rename).
- DAE import into the kernel: merge triangulated faces back into polygons (coplanar neighbours with shared edges), keep component hierarchy and materials.

## M2: Drawing

- Inference engine: snap to endpoints, midpoints, edge/face points, centers; axis-locked directions; visual hints.
- Tools: line (point to point), rectangle, circle, arc.
- Typed dimensions: parser for `5'4`, `5' 4"`, `100mm`, `4 1/2"`, `2.5m`, model units default; typed entry box during tools.
- Select: click, Shift+click to add, Ctrl+click to modify (exact semantics confirmed with Tyrel before M2); window (left to right, fully inside) and crossing (right to left, touching) boxes; double/triple click selects connected.

| Task | What | Needs |
| --- | --- | --- |
| `m2-units` | Typed lengths: parse and format feet/inches, fractions and metric; model units default | |
| `m2-shapes` | Rectangle, circle and arc point generation (pure geometry) | |
| `m2-select-box` | Window and crossing boxes; double click selects bounded, triple click connected | |
| `m2-select-click` | Click modifiers: click replaces, Shift adds, Ctrl toggles, Alt removes | |
| `m2-inference` | Inference: snaps to endpoints, midpoints, edges, faces and centers; axis inference and direction lock; hint data; pick under cursor (kernel, orchestrator) | |
| `m2-tool-line` | Tool plumbing (`draw_segments`, one undo step, face material copy on split) and the line tool: clicks, typed lengths, chain ends on a face | `m2-units` |
| `m2-tool-shapes` | Rectangle, circle and arc tools on `draw_segments`; typed sizes and segment counts | `m2-shapes`, `m2-tool-line` |
| `m2-app-editor` | Editor logic for the app: select tool (click, modifiers, double/triple click, boxes) and line tool driven by inference; typed lengths, Shift and arrow-key locks, undo/redo, key map | `m2-tool-line`, `m2-inference`, `m2-select-box`, `m2-select-click` |
| `m2-app-shapes` | Rectangle, circle and arc tools in the editor: draw plane from the face under the cursor, segment counts | `m2-app-editor`, `m2-tool-shapes` |
| `m2-app-ui` | Window wiring: toolbar, measurement box, inference hints and previews, selection highlight, new empty model, scene refresh after edits | `m2-app-editor`, `m2-app-shapes` |
| `m2-undo-state` | `*` follows the saved undo state (undo/redo back to saved is clean); Edit menu with Undo/Redo | `m2-app-ui` |
| `m2-pick-imported` | Select and snap on geometry imported from DAE (box and click found only new geometry on 05-house); kernel/editor fix with tests | `m2-app-ui` |
| `m2-hints` | Inference feedback in the window: axis lock color and snap name next to the cursor | `m2-app-ui` |
| `m2-arc-3pt` | Three-point arc tool in the kernel (`Arc3Tool`, `three_point_arc`): start, end, bulge point; typed chord and bulge height | `m2-tool-shapes` |
| `m2-arc-3pt-app` | Three-point arc in the editor and window: tool, key, toolbar button | `m2-arc-3pt`, `m2-app-shapes` |
| `m2-infer-cache` | Pick and snap flatten the model once per revision, not on every pointer move; `Model::revision` | `m2-pick-imported` |

## M3: Modify

| Task | What | Needs |
| --- | --- | --- |
| `m3-transform` | `transform`: apply an affine matrix to a selection (move, rotate, scale, flip), one undo step, refuses results that break invariants | |
| `m3-pushpull` | Push/Pull a face by a distance: side faces, hole punching, merge with coplanar neighbours (kernel, orchestrator) | `m3-transform` |
| `m3-pushpull-merge` | Push/Pull follow-up: new sides merge with coplanar neighbours, hole punching through a solid, new-start-face modifier | `m3-pushpull` |
| `m3-move-merge` | Moved geometry heals and splits against the rest (sticky) | `m3-transform` |
| `m3-tape` | Tape measure math and guide data (`tape.rs`: measure, parallel guide, guide point, `Guides` collection) | |
| `m3-guides-model` | Guides stored in the model: undoable add/remove/clear, native file round trip | `m3-tape` |
| `m3-app-tools` | Editor and window: Move, Rotate, Scale, Flip, Push/Pull, Tape tools with inference, typed values, previews | all above |

- Push/Pull with typed distance; extrudes faces, creating side faces.
- Move (points, edges, faces, selection; with inference and typed distance; copy with modifier).
- Rotate with protractor, snapping to 15 degrees, typed angle.
- Scale with handles, typed factors, uniform and per-axis.
- Flip about red/green/blue planes of the selection.
- Tape measure with live distance; guide lines.

## M4: Components and materials

- Make component from selection; edit in place (all instances update); option to hide the rest of the model while editing; groups.
- Materials: create/edit colors and image textures; paint bucket; texture positioning basics; materials survive save/load and DAE import.

## M5: Sun

- Geolocation (latitude, longitude, true north angle), date and time.
- Solar position (own implementation from a public-domain algorithm description), shadow mapping in the renderer, light/dark settings.

## M6: Follow Me and offset

- Follow Me: extrude a face along a path of connected edges.
- Offset: offset a face's edges inward/outward by a distance, with typed value.

## M7: Tags and model library hook

- Tags: assign entities to tags; show/hide tags.
- Library hook: import pipeline abstracted behind a "model source" interface so an online library could be added later.

## M8: Windows

- Windows build in CI (cross-compile or a Windows runner), installer, release artifacts.
