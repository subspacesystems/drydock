# m8-guide-text-more: guide pages, the rest of the tools, files and 3D printing

`m8-guide-text-start` wrote the first six pages in `docs/guide/` (welcome, window, navigation, selecting, drawing, push-pull). This task writes seven more, adds them to `SUMMARY.md` and links them from the welcome page. Only Markdown changes; no Rust. Do not change the tests.

Same style rules as the first six: plain, friendly English for someone new to 3D modeling, in your own words, second person, short paragraphs, bullets for steps, key names in backticks (`Shift`, `F2`). The product is Drydock; never write "modeler" (as a word) or name any other product. No images, no links outside the guide. Each `##` section below needs at least 15 words and each page at least 250 words (the shortcut page: sections of 8 words, 120 in all). State only what is written here; do not invent features or defaults.

## Menu items are written `Menu > Item`

A test (`drydock/tests/guide_coverage.rs`) needs every menu item to appear somewhere in the guide in the form `File > Export STL`, case-insensitive, without the dots (`Save As`, not `Save As...`). Use that form each time you name a menu item. The items: File (New, Open, Import, Export 3MF, Export STL, Save, Save As, Exit); Edit (Undo, Redo, Cut, Copy, Paste, Erase, Make Component, Make Group, Explode, Flip Along Red / Green / Blue); View (Switch to Parallel Projection, Switch to Perspective, Zoom Extents, Iso, Top, Front, Right, Back, Left, Bottom, Hide Rest of Model, Axes, Overhangs, Entity Info, Materials, Components, Shadows, Texture Position, Toolbars, Reset Panels, Model Info); Tools (Fit Check, Grow by Clearance, Shrink by Clearance, 3D Text, Print Setup); Help (User Guide, About Drydock). The Views and projection items belong on `shortcuts.md`; the rest wherever they fit below. Every tool's toolbar label must appear in the guide as written there: Select, Line, Rectangle, Circle, Arc, 3-Point Arc, Extrude, Move, Rotate, Scale, Tape Measure, Erase, Paint Bucket, Orbit, Pan, Zoom. The start pages may lack two: add one sentence to `push-pull.md` saying the toolbar button is called Extrude, and one to `drawing.md` saying the button for `Shift+A` is called 3-Point Arc. Check with the test, not by eye.

## `SUMMARY.md` and `welcome.md`

Append, after the six existing entries and in this order: `modify.md`, `measuring.md`, `groups.md`, `materials.md`, `files.md`, `printing.md`, `shortcuts.md`. The six keep their order. In `welcome.md`'s "Where to start" list add one linked line per new page (link target written `modify.md`, etc.), in the same order.

## Pages and required headings

Each page starts with the `#` title shown; the `##` headings must appear exactly as written (more are fine).

### `modify.md`: **Moving, rotating and scaling**
Headings: `Move`, `Rotate`, `Scale`, `Flip`, `Erase`. Select what you want to change first (see the selecting page).
- *Move*: tool `M`. Click a start point, then a second point; the selection moves by that offset. Typing a distance and `Enter` while moving moves along the direction the cursor shows (a negative number goes the other way). Right after a move, typing a new distance and `Enter` changes that move. Tap `Ctrl` once while the Move tool is active to switch copy mode on or off: with it on, the second click makes a copy instead of moving, and the copy becomes the selection so you can copy again. Choosing another tool turns copy mode off. `Esc` cancels.
- *Rotate*: tool `Q`. Click the center of the protractor, click the start of the angle, then click the end; or type an angle in degrees and press `Enter` (a negative angle turns the other way). Typing a new angle right after a rotation changes that rotation. Points snap as when drawing.
- *Scale*: tool `S`. The selection shows handles on its bounding box (corners, edge middles, face middles); press on a handle and drag, or type factors and press `Enter`. Holding `Ctrl` scales about the center; holding `Shift` flips the uniform/non-uniform choice (say it this way and no more).
- *Flip*: Edit > Flip Along Red, Edit > Flip Along Green and Edit > Flip Along Blue mirror the selection along that axis; they are greyed out with nothing selected.
- *Erase*: tool `E`, or press `Delete` with things selected (Edit > Erase). Erasing a face leaves its edges; erasing an edge also removes the faces that used it, unless two flat faces on either side of it simply merge into one. Undo (`Ctrl+Z`, Edit > Undo) takes back every one of these.

### `measuring.md`: **Measuring and units**
Headings: `Tape Measure`, `Guides`, `Choosing units`, `Typing lengths`.
- *Tape Measure*: tool `T`. Click a start point and an end point; the distance shows in the measurements box, and nothing in the model changes.
- *Guides*: if the first click is on an edge, the second click places a guide line parallel to that edge through the second point; clicking a point that is on the edge's own line says so instead. Holding `Ctrl` while clicking with Tape Measure puts a guide point. Guides can be undone like other edits.
- *Choosing units*: View > Model Info opens Model Info; its Units section has Unit (Millimeters, Centimeters, Meters, Inches, Feet, Architectural) and Precision (decimal places, or a fraction of an inch for Architectural). The choice is saved with the model and every length and area in Drydock then uses it, including Entity Info and the measurements box. Changing it is one undo step ("Set Units").
- *Typing lengths*: a number typed while drawing or moving uses the model's unit unless you add one yourself (`10cm`, `1/2"`).

### `groups.md`: **Groups and components**
Headings: `Making a group or component`, `Editing inside`, `The Components panel`, `Exploding`, `Copies of a component`.
- *Making*: select things, then Edit > Make Group or Edit > Make Component; both ask for a name first. A group is one-off; a component is a named part you can place again.
- *Editing inside*: double click a group or component with the Select tool to open it; only its contents can then be edited. Double click empty space outside it, or press `Esc`, to close it. View > Hide Rest of Model hides everything else while you are inside (the same item shows it again).
- *The Components panel*: View > Components. Lists the model's components (groups never appear) with small pictures, a search box (matches part of the name, ignoring case), and a switch between list and tiles. Click an entry to place it; the copy follows the cursor until you click. The panel can also import a component from a model file (DAE, STL or 3MF; an STL is read in millimeters there without asking).
- *Exploding*: select a group or component and choose Edit > Explode to turn it back into plain geometry.
- *Copies of a component*: all copies of a component share one definition, so editing inside one changes every copy. Groups are separate each time.

### `materials.md`: **Materials and painting**
Headings: `Paint Bucket`, `The Materials panel`, `Textures`, `Picking a material from the model`.
- *Paint Bucket*: tool `B`. Click a face or a group/component to paint it with the current material; if what you clicked is part of the selection, the whole selection is painted. The side of the face you see is the side that gets painted. Painting with the default material takes paint off. Painting with what is already there does nothing.
- *The Materials panel*: View > Materials. A list with a color swatch per material; click one to make it the Paint Bucket's current material. New... and Edit... open a form: name, color, opacity, an image file for a texture, and the image's real width and height (in the model's unit). Purge removes the materials nothing uses.
- *Textures*: View > Texture Position opens a panel to move, turn and scale the texture on the selected face (fields such as Move X, Move Y, Turn in degrees, Scale X, Scale Y).
- *Picking a material from the model*: hold `Alt` and click with the Paint Bucket to take the material from a face or group; or use the eyedropper button in the Materials panel and click anything in the model, which also switches to the Paint Bucket.

### `files.md`: **Files, import and export**
Headings: `Saving and opening`, `Importing models`, `Exporting for printing`. Do not write the old file extension as a word; say "files saved by earlier versions with the old extension still open".
- *Saving and opening*: File > New, File > Open (`Ctrl+O`), File > Save (`Ctrl+S`), File > Save As (`Ctrl+Shift+S`), File > Exit. Drydock's own files end in `.drydock`. Under File the most recent files are listed for one-click opening. Unsaved changes are asked about before New, Open or Exit. An opened DAE, STL or 3MF file is not a Drydock file, so Save asks for a `.drydock` name.
- *Importing models*: File > Open also takes DAE, STL and 3MF files. An STL file does not say what its numbers mean, so Drydock asks for the unit (millimeters first; mm, cm, m or in); a 3MF file carries its own unit and is not asked. File > Import adds a file's solids to the model you have open, each as a group at the place it has in the file; one undo step removes them. Choosing Cancel on the unit question leaves the model untouched.
- *Exporting for printing*: File > Export 3MF and File > Export STL (greyed out until the model has a group or component). A window asks what to export: Whole model or Selection (the selected groups and components). For STL it also asks the unit (mm, cm, m, in) and One file per object or Single file; 3MF is always millimeters in one file. Curve tolerance (mm) sets how finely curved edges are cut into flat pieces; smaller is smoother and bigger files. Only solid groups and components are written (see the printing page); anything else is skipped, and the status bar names each skipped part and why. Exporting never changes your model.

### `printing.md`: **3D printing**
Headings: `Is it a solid?`, `Repairing`, `Print Setup`, `Overhangs`, `Fit check and clearance`, `3D text`. Link `files.md` for exporting.
- *Is it a solid?*: select one group or component; Entity Info (View > Entity Info) then shows Solid: Yes or No, its Volume, and, only when above zero, counts of Open edges, Loose edges, Shared edges (used by three or more faces), Reversed faces, Internal faces and Nested groups, and Shells when there is not exactly one. A part is a solid when it is watertight (every edge joins exactly two faces) so a slicer can tell inside from outside. The problem edges and faces are drawn in magenta in the viewport while the part is selected.
- *Repairing*: the Repair button in Entity Info (enabled when something is wrong; one undo step) removes internal and stray faces and loose edges, fills flat holes, and turns faces that point the wrong way. It then reports what it did, and what it could not fix (holes that are not flat, ring-shaped holes, edges used by three or more faces). Components share their definition, so repairing one repairs every copy.
- *Print Setup*: Tools > Print Setup. Printer bed presets (180, 220, 256, 300, 350 mm squares) or your own Bed X, Bed Y and optional Bed Z in mm; Show bed outline draws the bed on the ground, centered on the origin. Check fit compares the size of the selected parts (or the whole model) with the bed; it checks size, not position, and does not turn parts. Drop to Bed moves the selected groups or components down (or up) together until the lowest point sits on the ground. The settings are remembered between sessions and are not part of the model. The Overhang angle (deg) box is also here.
- *Overhangs*: View > Overhangs tints faces that hang over empty space more than the angle (45 degrees to start), measured from straight down; the lowest faces resting on the bed are not tinted. Change the angle in Print Setup. Turn it off the same way.
- *Fit check and clearance*: select two groups or components and choose Tools > Fit Check; faces of either part closer to the other than the clearance turn yellow and the status bar counts them (as triangles). The Clearance box (in mm, 0.2 to start) in the status bar changes the distance and updates the check; any edit to the model ends the check. Tools > Grow by Clearance and Tools > Shrink by Clearance make each selected flat-faced solid bigger or smaller on every face by that distance, in one undo step, so a printed part fits its mate.
- *3D text*: Tools > 3D Text asks for Text, Font size (mm), Height (mm), and Emboss or Engrave. After OK, an outline follows the cursor over the faces of the part you are editing; click to place it, raised or cut in. If Drydock cannot make it, the status bar says why and the text stays pending so you can try another spot; `Esc` or choosing a tool drops it. One line, upright, one built-in font. It works inside an open group or component.

### `shortcuts.md`: **Keyboard shortcuts**
Headings: `Tools`, `Views and navigation`, `Files and editing`, `Modifier keys`. Mostly tables, with a one-sentence lead for each.
- *Tools*: a table with one row per tool in this order, label then key in backticks: Select `Space`, Line `L`, Rectangle `R`, Circle `C`, Arc `A`, 3-Point Arc `Shift+A`, Extrude `P`, Move `M`, Rotate `Q`, Scale `S`, Tape Measure `T`, Erase `E`, Paint Bucket `B`, Orbit `O`, Pan `H`, Zoom `Z`. Each row must be one line holding the label and the backticked key (a test checks).
- *Views and navigation*: View > Iso `F2`, View > Top `F3`, View > Front `F4`, View > Right `F5`, View > Back `F6`, View > Left `F7`, View > Bottom `F8`, View > Zoom Extents `Shift+Z`, View > Switch to Parallel Projection and View > Switch to Perspective `Shift+P`.
- *Files and editing*: File > New `Ctrl+N`, File > Open `Ctrl+O`, File > Save `Ctrl+S`, File > Save As `Ctrl+Shift+S`, Edit > Undo `Ctrl+Z`, Edit > Redo `Ctrl+Y`, Edit > Cut `Ctrl+X`, Edit > Copy `Ctrl+C`, Edit > Paste `Ctrl+V`, Edit > Erase `Delete`, Help > User Guide `F1`, Help > About Drydock.
- *Modifier keys*: one bullet per tool or situation, pointing to the page for detail: Select (`Shift` adds, `Ctrl` toggles, `Alt` removes), Move (`Ctrl` toggles copy), Scale (`Ctrl` about center, `Shift` uniform), Tape Measure (`Ctrl` guide point), Paint Bucket (`Alt` picks a material), drawing (arrow keys lock to an axis, `Shift` locks the current direction), orbit (`Shift` pans).

## Tests

- `app/crates/drydock-guide/tests/text_more.rs`: the contents lists the six old pages first and the seven new ones after them in order; titles and `##` headings as above; key words present; sections have real text; no banned names; the welcome page links every new page; the guide builds with no broken link or anchor and no page outside the contents.
- `app/crates/drydock/tests/guide_coverage.rs`: every menu item appears as `Menu > Item`; every toolbar label appears; `shortcuts.md` has one line per tool with its label and backticked key; the menu shortcuts are listed in backticks.

Run `harness/bin/run-tests -p drydock-guide` and `harness/bin/run-tests -p drydock --test guide_coverage`. Link anchors are heading ids (`show heading_id`); prefer plain page links like `files.md`.

## Where to look

- `docs/guide/*.md` (the first six pages): the style to match.
- `show build`, `show parse_summary`, `show render_markdown`, `show heading_id` in `drydock-guide/src/lib.rs`.
- `show TOOL_BUTTONS` and `show tool_shortcut` in `drydock/src/chrome.rs` (labels and keys); `show menu_bar_with_toolbars` for the menu items and shortcuts; `show key` in `drydock/src/input.rs`.
- `drydock/tests/guide_coverage.rs` (`MENU_ITEMS`) and `drydock-guide/tests/text_more.rs` (`PAGES`): exactly what is checked.
- To confirm a behavior before writing it: `show entity_info_panel`, `show status_rows` (solid_ui.rs), `show repair_message`, `show print_setup_ui` (print_setup.rs), `show export_dialog` (mesh_export.rs), `show text_dialog` (text_tool.rs), `show units_section` (units_ui.rs), `show materials_panel`, `show components_panel`.
