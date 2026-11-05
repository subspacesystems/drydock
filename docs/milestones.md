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
| M6 | UI | Tyrel's deferred UI items: icon toolbars, docking panels, Entity Info, Components | M5 |
| M7 | 3D printing | Solid check and repair, 3MF/STL export and import, print setup, print helpers | M6 |
| M8 | Windows | Windows build and installer | M7 |
| M9 | Medium priority | Follow Me, offset | M3 |
| M10 | Low priority | Tags, model library hook | M4 |

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
| `m3-pushpull-merge` | Push/Pull follow-up: new sides merge with coplanar neighbours, hole punching through a solid (new-start-face modifier and pocket-through-wall later) | `m3-pushpull` |
| `m3-move-merge` | Moved geometry heals and splits against the rest (sticky) | `m3-transform` |
| `m3-tape` | Tape measure math and guide data (`tape.rs`: measure, parallel guide, guide point, `Guides` collection) | |
| `m3-guides-model` | Guides stored in the model: undoable add/remove/clear, native file round trip | `m3-tape` |
| `m3-tool-rotate` | Rotate tool state machine (`tools/rotate.rs`): center, start arm, 15 degree snaps, typed angle (also right after) | `m3-transform` |
| `m3-tool-scale` | Scale tool state machine (`tools/scale.rs`): handles, typed factors, uniform and per-axis | `m3-transform` |
| `m3-tool-tape` | Tape tool state machine (`tools/tape.rs`): live distance, parallel guide lines, guide points, typed offset | `m3-guides-model` |
| `m3-tool-move` | Move tool state machine (`tools/translate.rs`): from point to point, inference, typed distance, copy modifier | `m3-move-merge` |
| `m3-app-pushpull` | Push/Pull tool in the editor: hover face, drag distance with snaps, typed distance (also right after), double click repeats | `m3-pushpull-merge` |
| `m3-app-move` | Move tool in the editor: selection or hovered entity, wires `tools/translate.rs` | `m3-tool-move`, `m3-app-pushpull` |
| `m3-app-rotate` | Rotate tool in the editor: protractor plane from inference, wires `tools/rotate.rs` | `m3-tool-rotate`, `m3-app-pushpull` |
| `m3-app-scale-flip` | Scale tool in the editor (wires `tools/scale.rs`) and Flip about red/green/blue | `m3-tool-scale`, `m3-app-pushpull` |
| `m3-app-tape` | Tape tool in the editor: wires `tools/tape.rs` | `m3-tool-tape`, `m3-app-pushpull` |
| `m3-ui-keys` | Keys P/M/Q/S/T for the new tools; projection toggle on Shift+P; `Action::Flip` | `m3-app-tape` |
| `m3-ui-overlay` | Pure overlay primitives for the M3 tools (face highlight, scale handles, protractor, guides) | `m3-ui-keys` |
| `m3-app-ui` | Window wiring for the M3 tools: toolbar, View/Edit menus (projection, flip), pointer events, modifiers, overlays painted; egui_kittest tests | `m3-ui-keys`, `m3-ui-overlay` |
| `m3-erase` | Erase in the kernel (`erase.rs`): selected edges, faces and instances deleted, edges that leave faces open take them along, one undo step (kernel, orchestrator) | |
| `m3-app-erase` | Del and Backspace erase the selection, Erase tool in the sidebar, Edit menu item | `m3-erase`, `m3-app-ui` |

- Push/Pull with typed distance; extrudes faces, creating side faces.
- Move (points, edges, faces, selection; with inference and typed distance; copy with modifier).
- Rotate with protractor, snapping to 15 degrees, typed angle.
- Scale with handles, typed factors, uniform and per-axis.
- Flip about red/green/blue planes of the selection.
- Tape measure with live distance; guide lines.
- Done on main from Tyrel's notes (no task rows): Help > About with version and git hash, selected faces as a tint only, Rotate protractor shown before the center click, exact Rotate angle when the cursor snaps to geometry.

## M4: Components and materials

| Task | What | Needs |
| --- | --- | --- |
| `m4-paint` | Paint one side of faces and instances, sample, create/edit/purge materials with unique names (kernel, `paint.rs`) | |
| `m4-make-component` | Make component / make group: the selection moves into a new definition, one instance in its place (kernel, orchestrator) | |
| `m4-explode` | Explode an instance into its parent body: geometry sticks and merges, nested instances move up (kernel, orchestrator) | `m4-make-component` |
| `m4-edit-path` | Editing inside an instance: open path, world/local transforms, pick, infer and box select in the open body with the rest still snappable (kernel, orchestrator) | |
| `m4-texture-model` | Texture size on materials and per-face texture placement; texture coordinates of a face point; native format v3 (kernel, orchestrator) | |
| `m4-texture-follow` | Texture maps move with their faces: transform (move, rotate, scale, flip), make component and explode compose each side's map with the inverse placement | `m4-texture-model` |
| `m4-texture-scene` | Kernel scene draws textures with coordinates; DAE import keeps texture placement, so imports stay textured after edits | `m4-texture-model` |
| `m4-texture-save` | Texture files stay found after Save As to another folder (paths rewritten relative to the new file) | `m4-texture-model` |
| `m4-app-edit-context` | Editor works in the open instance: double click opens, Esc or a click outside closes, every tool uses the open body | `m4-edit-path` |
| `m4-app-components` | Editor: make component / make group / explode on the selection in the open body (undo steps, messages) | `m4-make-component`, `m4-explode`, `m4-app-edit-context` |
| `m4-app-components-ui` | Edit menu Make Component / Make Group / Explode with a name prompt | `m4-app-components` |
| `m4-app-edit-view` | App wiring: while editing inside an instance the rest is drawn faded; View option to hide it instead | `m4-edit-view-scene`, `m4-app-edit-context` |
| `m4-edit-view-scene` | Scene drawing while editing inside an instance: the rest faded or hidden; status counts skip unused definitions (`kernel_scene_editing`, `document.rs`) | `m4-edit-path` |
| `m4-app-paint` | Paint Bucket tool (B): paints the clicked side, or the selection when clicking into it; Alt samples; current material | `m4-paint` |
| `m4-app-texture-position` | Editor: move, turn, scale and reset a texture on one side of a face (`position_texture`, one undo step each) | `m4-texture-model`, `m4-texture-scene` |
| `m4-app-texture-position-ui` | Texture position UI: typed move/turn/scale values and reset for the selected face's textured side | `m4-app-texture-position` |
| `m4-app-materials` | Editor: create (becomes current), edit and purge materials, one undo step each, messages | `m4-paint`, `m4-texture-model` |
| `m4-app-materials-ui` | Materials panel: list with swatches, new and edit (name, color, opacity, texture image and size), purge unused | `m4-app-materials` |
| `m4-fix-edit-overlays` | selection, Push/Pull face and Scale box drawn in world coordinates inside an open instance (`overlay::selection`, `transformed`) | |
| `m4-fix-refresh-speed` | renderer keeps loaded textures across uploads; opening/closing an instance only redraws (`Document::show_path`) | |
| `m4-fix-materials-width` | long material names and paths don't widen the Materials column | |
| `m4-orbit-pivot` | orbit about the model point under the cursor (`Camera::orbit_about`, `FlatScene::raycast`, `ViewState::pivot`) | |
| `m4-orbit-ground` | orbit over empty space pivots on the ground (z = 0) under the cursor (`Camera::ground_pivot`); orchestrator wires the fallback after `m4-orbit-pivot` merges | |
| `m4-dae-pinch-faces` | DAE import splits merged faces whose outline touches itself into a few clean faces instead of raw triangles (kernel import, orchestrator) | |
| `m4-copy` | duplicate a selection with a transform into a body (sticky), and clipboard extract/insert (kernel, orchestrator) | |
| `m4-app-move-copy` | Move tool: Ctrl toggles copy, the originals stay | `m4-copy` |
| `m4-app-copy-paste` | Edit > Cut / Copy / Paste (Ctrl+X/C/V); the pasted copy follows the cursor until a click places it | `m4-copy` |
| `m4-app-texture-handles` | texture position with handles on the face in the viewport (move, turn and scale) instead of typed values | |
| `m4-ui-menus` | File > Exit and recent files, Tools menu (every tool, grouped, with keys), About with license and contributors | |
| `m4-ui-status` | status bar without model counts; View > Model Info window with them; the measurement box moves into the status bar | |
| `m4-ui-axes` | red/green/blue axes drawn at the origin (View > Axes toggle); inside an open instance at its origin | |
| `m4-ui-materials-layout` | Materials panel buttons and a search box at the top, the list scrolls under them; list or tiles view | |
| `m4-zoom-cursor` | wheel and Zoom-tool zoom scale about the model point under the cursor, so a step close to a surface stays small and never passes it | |
| `m4-fix-copy-speed` | Move-copy froze on a large house: `validate` without all-pairs loops; copy/explode/make component check only the bodies they change; instance copies don't explode; explode leaves faces away from the copy alone (kernel, orchestrator) | |
| `m4-fix-dae-edges` | an opened DAE draws the kernel scene, so edges show at once (not only after the first edit) | |
| `m4-fix-clipboard-keys` | Ctrl+X/C/V through egui's Cut/Copy/Paste events (no key events arrive for them) | |
| `m4-axes-depth` | axes hidden behind nearer geometry (drawn in the GPU pass with depth) | |
| `m4-material-swatch-texture` | textured materials show their image in the Materials list and tiles | |
| `m4-texture-handles-2` | texture handles: short help in the status bar, grips that look different and change the cursor, drag from any corner, snapping to geometry, turn grip off the corners | |
| `m4-zoom-through` | Ctrl+wheel steps through the surface under the cursor | `m4-zoom-cursor` |
| `m4-ui-gpu-info` | Model Info names the graphics adapter and backend in use | |
| `m4-axes-screen` | axes thin again: extra lines keep a fixed pixel width and screen-space dashes, drawn past the scene's far distance (renderer) | |
| `m4-texture-turn` | texture turns are rigid on the face (no skew after a stretch); press-drag-release on the texture grips | |
| `m4-purge-confirm` | Purge Unused lists the materials it will remove and asks first | |

- Make component from selection; edit in place (all instances update); option to hide the rest of the model while editing; groups.
- Materials: create/edit colors and image textures; paint bucket; texture positioning basics; materials survive save/load and DAE import.

## M5: Sun

| Task | What | Needs |
| --- | --- | --- |
| `m5-solar` | Solar position from place, date and time (low-precision solar theory, own code), the sun's direction in model axes with the north angle (`sun.rs`; calendar helpers already on main) | |
| `m5-geo-model` | Geolocation and shadow settings stored in the model: undoable records that don't bump the revision, model equality, native format v4 | |
| `m5-render-sun` | Renderer: lit faces shaded by the sun (light/dark, side seen by the viewer) when `Style::sun` is set | |
| `m5-render-shadows` | Renderer: shadow map from the sun; faces receive shadows, optional shadows on the ground plane; lavapipe tests | `m5-render-sun` |
| `m5-app-sun` | Editor/app: the model's settings give `Style::sun` each frame; setting geolocation or shadow settings is one undo step; redraw on change | `m5-solar`, `m5-geo-model`, `m5-render-sun` |
| `m5-app-shadows-ui` | Shadows panel widget: shadows on/off, sun shading, time and date sliders, light/dark, on ground; View > Shadows item; egui_kittest tests | `m5-app-sun` |
| `m5-app-geo-ui` | Geolocation form widget: typed latitude, longitude, north angle and UTC offset with checks; egui_kittest tests | `m5-app-sun` |
| `m5-app-sun-windows` | App wiring: Shadows window (slider drags preview, release commits one undo step) and Geolocation window opened from Model Info; real-window check on 05-house | `m5-app-shadows-ui`, `m5-app-geo-ui` |
| `m5-shadow-cascades` | Fix from feedback: shadow edges jagged up close. Cascaded shadow maps fitted to slices of the view, texel-snapped, bias per cascade texel | |
| `m5-geo-globe` | From: a globe in the Geolocation window (Natural Earth outlines, ADR-006); drag turns, click picks latitude/longitude and a guessed UTC offset | |

- Geolocation (latitude, longitude, true north angle), date and time.
- Solar position (own implementation from a public-domain algorithm description), shadow mapping in the renderer, light/dark settings.

Done when: on 05-house, setting a location, date and time shows shadows on the house and the ground that move as the time slider moves; the settings survive save and reopen; navigation stays smooth with shadows on (release build). After the first try: shadow edges stay sharp when zoomed in close, and the Geolocation window has a globe to pick the place.

## M6: UI

Tyrel's UI items deferred from M4, inserted after M5 at his request.

| Task | What | Needs |
| --- | --- | --- |
| `m6-icons` | Own vector icons for every tool and the Materials buttons (drawn in code, theme color), icon button with tooltip | |
| `m6-toolbar-layout` | Toolbar layout data: one toolbar per tool group, show/hide, dock on any edge or float, add/remove/reorder tools, new toolbars; layout file | |
| `m6-dock-layout` | Panel layout data: Entity Info, Materials, Components, Shadows open/closed, left/right column or floating, collapsed, column widths, drop rules; layout file | |
| `m6-toolbar-ui` | Toolbar strip widget (grip, icon buttons, tooltips), drop-target rule; app draws the toolbars from the layout, drag a grip to another edge or to float; layout saved | `m6-icons`, `m6-toolbar-layout` |
| `m6-toolbar-customize` | View > Toolbars (show/hide, Customize..., Reset); Customize window: add, remove, reorder tools, new toolbars | `m6-toolbar-ui` |
| `m6-materials-icons` | Materials panel: List/Tiles and New/Edit/Purge as icon buttons with tooltips | `m6-icons` |
| `m6-entity-info` | Entity Info panel widget: selection type, size/length/area, materials, definition and instance names; rename a component or group (one undo step) |  |
| `m6-components-panel` | Components panel widget: the model's definitions with drawn thumbnails, search, list/tiles; click reports which to place | |
| `m6-components-place` | Editor: place an instance of a definition (follows the cursor, click places, Esc cancels, one undo step) | `m6-components-panel` |
| `m6-components-import` | Kernel: `import_component` turns a loaded model into a new component definition (one undo step) | `m6-components-place` |
| `m6-components-import-ui` | Components panel Import button, file dialog, reads the file, then starts placing the new component | `m6-components-import`, `m6-dock-ui` |
| `m6-dock-ui` | App: host the four panels in the docking columns or floating windows; drag a title to dock or float, collapse, resize columns; View menu items; Shadows window becomes a panel; layout saved | `m6-dock-layout`, `m6-entity-info`, `m6-components-panel`, `m6-materials-icons` |
| `m6-panel-speed` | Feedback: Entity Info and Components keep their results between frames; thumbnails capped (the lock-up on a big group) | |
| `m6-menus` | Feedback: tool icons in the Tools menu, shortcuts right-aligned in every menu, a drawn submenu arrow (Roboto lacks egui's arrow glyph, shown as "?") | |
| `m6-panel-stack` | Feedback: each docked panel scrolls on its own (Components and Materials scrolled together) and can be resized; no doubled titles; View > panel opens or reveals it, only the close button hides it; drawn header buttons | `m6-panel-speed` |
| `m6-texture-panel` | Feedback: Texture Position becomes its own panel | `m6-panel-stack` |
| `m6-toolbar-drag` | Feedback: an outline shows where a dragged toolbar will land; dropping between toolbars inserts it there | |
| `m6-icons-2` | Feedback: Extrude, Rotate, Scale, Tape Measure, Eraser, Paint Bucket and Pan redrawn; arcs centred; every icon centred in its square | |
| `m6-rename-extrude` | Feedback: Push/Pull renamed Extrude in toolbar, menus, tooltips (after menus and toolbar-drag merge: shared test files) | `m6-menus`, `m6-toolbar-drag` |
| `m6-toolbar-customize-2` | Feedback: Customize window as two lists (on the toolbar / available) with icons; drag between them and to reorder | `m6-icons-2`, `m6-toolbar-drag`, `m6-rename-extrude` |
| `m6-eyedropper` | Feedback: Materials panel eyedropper; click a face and its material becomes current | `m6-panel-stack` |
| `m6-native-look` | Feedback: style pass toward a desktop look (system UI font, theme from the OS, spacing, menu highlight) | `m6-menus`, `m6-panel-stack` |
| `m6-eyedropper-2` | Round 3: sampling keeps the tool (a double-click painted), button stays pushed in and cancels, pipette icon and cursor, current tile boxed and scrolled into view | `m6-eyedropper` |
| `m6-menus-2` | Round 3: menus as wide as their content, icon column on every row, check marks for show/hide items, View regrouped, "..." on About and Model Info | `m6-menus` |
| `m6-panel-collapse` | Round 3: collapsing a docked panel keeps the other title bars in place (free space opens at the bottom of the column) | `m6-panel-stack` |
| `m6-toolbar-side-width` | Round 3: a toolbar docked left or right is as narrow as the toolbar, no resize handle | `m6-toolbar-drag` |
| `m6-toolbar-customize-3` | Round 3: Customize window drops the old per-tool buttons; rename a toolbar | `m6-menus-2` (toolbars.rs) |
| `m6-dialogs` | Round 3: dialogs after the Windows guidelines: text box labels end with ":", buttons at the bottom right on one line with a gap above, a highlighted default button that Enter presses, larger, bordered buttons | `m6-menus-2`, `m6-eyedropper-2` |
| `m6-eyedropper-deep` | Round 4: eyedropper and Alt+click sample the material drawn under the cursor, also on faces inside components (kernel `infer::material_in`) | `m6-eyedropper-2` |
| `m6-confirm` | Round 4: confirmation before deleting or resetting toolbars and resetting panels; New/Rename Toolbar ask for the name in a dialog | `m6-toolbar-customize-3`, `m6-dialogs` |
| `m6-units` | Round 4: kernel length format per model (mm, cm, m, in, decimal ft, architectural with 1/1 to 1/64 in), lengths and areas written in it, undoable setting, native file version 5 | |
| `m6-units-ui` | Round 4: Model Info "Units" section; every length and area in the app (measurement box, Entity Info, status bar, Model Info, texture and material sizes) in the model's format (write after `m6-units` merges) | `m6-units` |

- Icon toolbars by tool group (own icons), toolbars that can be shown, hidden and rearranged.
- Dockable side panels (Materials, Shadows, Entity Info, Components) instead of floating windows.
- Entity Info panel: the selection's type, tag/material, size and editable fields.
- Components pane: the model's definitions with thumbnails, place by drag or click.
- List/Tiles icons in the Materials panel.

Done when: tools are icon buttons grouped by kind along the top by default; each toolbar can be hidden, moved to another edge or floated, and customized (tools added, removed, reordered), and the arrangement survives a restart; Entity Info, Materials, Components and Shadows dock in the left or right column or float, collapse, and survive a restart; Entity Info describes the selection and renames components and groups; every length and area shows in the model's units, chosen in Model Info; Components lists the model's definitions with thumbnails, places one by click, and imports a file; the Materials panel buttons are icons. Checked in the real window before Tyrel tries it.

## M7: 3D printing

Tyrel's request: print-ready solids without plugins. Placed right after M6; order confirmed by Tyrel (2026-10-28): 3D printing, then Windows, then Follow Me/offset, then tags. Started 2026-10-29 (Tyrel lifted the limit). Mesh files live in the `modeler-mesh` crate.

| Task | What | Needs |
| --- | --- | --- |
| `m7-solid-check` | Kernel: per group or component, a solid report: open, loose and shared edges, reversed and internal faces, shells, cavities; volume when solid (kernel, orchestrator) | |
| `m7-solid-repair` | Kernel: reverse flipped faces, remove internal and stray faces, fill a hole whose border is one flat loop; one undo step; report what it couldn't fix (kernel, orchestrator) | `m7-solid-check` |
| `m7-curves` | Kernel: arcs and circles keep their center, radius and angles through edits and save (native format v6), so export can re-segment them by chord tolerance (kernel, orchestrator) | |
| `m7-curves-carry` | Kernel: curves carried into and out of groups and components (make group/component, explode, copy/paste, import) and onto push/pull's new cap, so a pulled circle is a cylinder with a curve at each end (kernel, orchestrator) | `m7-curves` |
| `m7-print-mesh` | Export mesh: each solid group or component (whole model or selection) as world-space triangles in mm, with name and material color; curves at the chosen tolerance | `m7-solid-check`, `m7-curves`, `m7-curves-carry` |
| `m7-export-stl` | Binary STL writer: one file per object, or all in one | `m7-print-mesh` |
| `m7-export-3mf` | 3MF writer: units, one object per solid, names, colors from materials | `m7-print-mesh` |
| `m7-import-stl` | STL (binary and ASCII) reader; each solid a group, triangles welded and merged into faces by the DAE import's merge (`modeler-mesh`) | |
| `m7-import-3mf` | 3MF reader: objects, units, names; same path to the kernel as STL | `m7-import-stl` |
| `m7-import-ui` | File > Open and File > Import... take STL and 3MF (import adds the groups to the open model), units choice (mm default) | `m7-import-stl`, `m7-import-3mf` |
| `m7-roundtrip` | Acceptance: closed corpus solids exported to 3MF and STL and re-imported keep volume and bounding box; deliberately broken models found by the check and fixed by repair; own mesh validator in the oracle | `m7-export-stl`, `m7-export-3mf`, `m7-import-stl`, `m7-import-3mf`, `m7-solid-repair` |
| `m7-solid-ui` | Solid status and volume in Entity Info, problem edges and faces highlighted in the viewport, Repair button | `m7-solid-repair`, `m7-clearance-ui` |
| `m7-export-ui` | File > Export 3MF... and Export STL...: selection or whole model, units (mm default), curve tolerance | `m7-export-stl`, `m7-export-3mf` |
| `m7-print-setup` | Print Setup panel: bed presets and custom size, bed outline in the viewport, fits-the-bed check, Drop to Bed, overhang angle field | `m7-print-mesh`, `m7-overhang` |
| `m7-overhang` | View > Overhangs: faces leaning past the overhang angle (45° until Print Setup offers it) tinted in the viewport; faces on the bed excluded | |
| `m7-clearance` | Fit check: pick two parts, highlight where the gap between them is under the clearance (default 0.2 mm) (Tyrel chose fit check first) | `m7-solid-check` |
| `m7-clearance-ui` | Tools > Fit Check: pick two groups or components, clearance field in mm (default 0.2), flagged faces tinted in the viewport, report in the status bar | `m7-clearance`, `m7-overhang` |
| `m7-clearance-offset` | Kernel: grow/shrink a flat-faced solid: every face moves in or out by a distance, vertices where the moved planes meet; refuses curved parts, collapses and self-overlap; one undo step for several parts (kernel, orchestrator) | `m7-clearance`, `m7-solid-repair` |
| `m7-clearance-offset-ui` | Tools > Grow by Clearance and Shrink by Clearance on the selected groups and components, using the fit check's clearance field; status bar says what changed and why a part was refused | `m7-clearance-offset`, `m7-clearance-ui` |
| `m7-text-outline` | Font outlines to flat loops (outer CCW, holes CW) at a chosen size (`modeler-dae/text.rs`, skrifa) | |
| `m7-text-3d` | Kernel: text loops on a face drawn and pushed out (emboss) or in (engrave) with push/pull, the part kept a solid; refuses text too big for the face, over its border, or deeper than the part; one undo step (kernel, orchestrator) | `m7-solid-repair` |
| `m7-text-3d-ui` | Tools > 3D Text: text, font size, height, emboss/engrave, click a face to place | `m7-text-outline`, `m7-text-3d` |
| `m7-fix-export-fit` | Sign-off fixes: Export 3MF/STL enabled when the model has a group or component; export file name from the model's name without its extension; viewport redraws after the Fit Check tint changes | |
| `m7-fix-marks` | Sign-off fix: open, loose and shared edges drawn in the problem color over the selection outline, so a selected part shows what keeps it from being a solid | |
| `m7-fix-dialogs` | Sign-off polish: Enter presses OK in the Export, Mesh units and 3D Text dialogs; Print Setup opens below the menu bar | |

Done when: Entity Info says whether a group or component is a solid, with its volume, and the viewport marks what keeps it from being one; Repair fixes reversed and internal faces and simple flat holes in one undo step; File > Export 3MF... and Export STL... write files at the right size (mm) that re-import with the same volume and bounding box; STL and 3MF files open or import as editable groups; the print bed outline and fits-the-bed check, Drop to Bed, overhang tint, fit check between two parts, grow/shrink by clearance and 3D text work. Checked in the real window before Tyrel tries it.

## M8: Windows

Moved up by Tyrel (2026-10-28) so beta testers can try the app after 3D printing.

- Windows build in CI (cross-compile or a Windows runner), release artifacts.
- A simple installer that removes older installed versions (upgrade in place) and an uninstaller. Auto-update comes later (Future).
- The product's public name is Drydock (Subspace Drydock, by Subspace Systems), Tyrel (2026-11-01); `modeler` stays the codename. The installer and packaging use the new name.

| Task | What | Needs |
| --- | --- | --- |
| `m8-rename-text` | User-visible name becomes Drydock: window title, About box (Subspace Drydock, by Subspace Systems), version line, menus, status and error messages, STL header, README; a test that no shown text says "modeler" | |
| m8-rename-crates (done directly, no task) | Every crate becomes `drydock-*` (the app crate and its program `drydock` / `drydock.exe`), crate folders, Rust paths and the developer environment variables with them (Tyrel, 2026-11-01: "Rename everything") | `m8-rename-text` |
| `m8-rename-extension` | Native files saved as `.drydock` with first line `drydock-model`; `.modeler` files and the old first line keep opening; the installer registers only `.drydock` (Tyrel, 2026-11-01) | `m8-rename-text` |
| `m8-rename-config` | Settings folder becomes `drydock` (Linux) / `Drydock` (Windows) and the override `DRYDOCK_CONFIG_DIR`; the old folder and variable are ignored, settings start fresh (Tyrel, 2026-11-01) | `m8-rename-crates` |
| `m8-guide-build` | User guide generator `drydock-guide`: Markdown in `docs/guide/` to static HTML with contents sidebar, anchors, link/image checks and an offline search index (Tyrel) | |
| `m8-guide-help` | Help > User Guide (F1) opens the bundled guide in the browser | `m8-guide-build` |
| `m8-guide-text-start` | Guide pages: welcome, window, navigation, selecting, drawing, push/pull | `m8-guide-build` |
| `m8-guide-text-more` | Remaining guide pages | `m8-guide-text-start` |
| `m8-win-exe` | The program builds for Windows (`drydock.exe`): no console window, icon and version info, Windows settings folder | `m8-rename-config` |
| `m8-installer` | NSIS installer script with in-place upgrade, `.drydock` file type and uninstaller | |
| `m8-release` | Release script `packaging/release.sh`: Windows build, installer, optional signing, SHA256SUMS, INSTALL notes (SmartScreen, Smart App Control) | `m8-installer`, `m8-win-exe` |
| m8-win-ci (done directly, no task) | Build checks compile the Windows target and the installer script (cross toolchain and NSIS in the build image) | `m8-installer-fix` |
| `m8-installer-fix` | Installer script compiles (64-bit registry view and all-users folders set at start-up, for the uninstaller too) and an upgrade removes the old version, wherever it was installed | |
| `m8-release-guide` | The release script builds the user guide into the installer when no guide folder is given | |
| `m8-version` | One version everywhere: Help > About, the .exe's file properties, the installer and Add/Remove Programs all show the release version, and About shows the commit instead of "unknown" (Tyrel, 2026-11-03) | |
| `m8-installer-notice` | When an older Drydock is installed, the installer says so and that it will replace it before going ahead (Tyrel) | `m8-version` |
| `m8-guide-search` | The guide's search box finds pages (its script ran before the box existed) | |
| `m8-guide-wording` | Guide describes Drydock as a general 3D modeler (architecture first, 3D printing one use) and calls the tool Extrude, not Push/Pull | |

## M9: Follow Me and offset

- Follow Me: extrude a face along a path of connected edges.
- Offset: offset a face's edges inward/outward by a distance, with typed value.

| Task | What | Needs |
| --- | --- | --- |
| `m9-sweep-path` | Kernel: a set of edges or a face's outer loop as one ordered sweep path (`sweep.rs`) | |
| `m9-sweep-face` | Kernel: sweep a profile face along a path (`sweepface.rs`: side faces, mitred joints, end caps, closed rings), one undo step | |
| `m9-followme-tool` | Follow Me tool: pick the profile face and the path, preview, apply | `m9-sweep-path`, `m9-sweep-face` |
| `m9-sweep-curves` | Curves carried through a sweep (profile arcs stay arcs on the swept surface edges) | `m9-sweep-face` |
| `m9-offset-face` | Kernel: offset a face's loops in its plane, mitred corners, inward split or outward ring (`faceoffset.rs`) | |
| `m9-offset-tool` | Offset tool: pick a face, drag or type the distance, preview, apply | `m9-offset-face` |
| `m9-offset-message` | Fix: Offset's refusal message stays in the status bar (the window calls the pointer move every frame, which cleared it) | `m9-offset-tool` |
| `m9-followme-undo` | Fix: Undo and Redo drop Follow Me's picks and preview, as for Offset and Extrude | `m9-followme-tool` |
| m9-guide-followme (done directly, no task) | User Guide: a Follow Me section, and the arrow keys that lock a shape's drawing plane | `m9-followme-tool` |
| `m9-followme-button` | Fix: Follow Me gets key `F`, a toolbar button after Offset, and its icon in the Tools menu | `m9-followme-tool` |
| m9-version (done directly, no task) | Fix: workspace version 0.9.0, so a plain cargo build's Help > About says 0.9.0 | |
| `m9-guide-dev` | Fix: Help > User Guide works from a plain `cargo build`/`cargo run` (the build makes the guide) | |
| `m9-linux-titlebar` | Fix: Linux window gets the GNOME-style title bar with title text (winit sctk-adwaita), app id and Drydock icon | |
| `m9-linux-desktop-entry` | Fix: Linux desktop entry and hicolor icons so the dock shows Drydock | `m9-linux-titlebar` |

Done when: Follow Me sweeps a face along an open and a closed path, and Offset insets and outsets a face by a dragged or typed distance, both one undo step each, checked in the real window.

## M10: Tags and model library hook

- Tags: assign entities to tags; show/hide tags.
- Library hook: import pipeline abstracted behind a "model source" interface so an online library could be added later.

## Future (unscheduled)

Ideas parked (woodworking, drawings, CNC export, plugin API, design library and more). Nothing there is planned until Tyrel opens its own issue or asks for it to be scheduled.

- Auto-update for the Windows build (Tyrel: wanted eventually; M8 ships a plain installer first).
- Tool cursors (Tyrel 2026-10-29: "for some time later"): every tool gets its own drawn cursor, arrow only for Select; drawing tools a pencil with its tip on the drawn point; the rest their toolbar icon with a hotspot on the working point (Paint Bucket: the bottom of the pouring paint); the bucket mirrored left-to-right everywhere it appears. Schedule with Tyrel after M7; stays open as its source.
