# m8-guide-wording: general-purpose intro and Extrude naming

Tyrel's feedback on the guide: Drydock is a general 3D modeler. Buildings and rooms come first; 3D printing is one use it supports well. The tool formerly called Push/Pull is now Extrude in the app (key still `P`). Only guide text and file names change; no Rust source.

## Steps

1. `git mv docs/guide/push-pull.md docs/guide/extrude.md`. Title `# Extrude`. Keep the three level-2 headings ("Pushing a face", "Typing a distance", "Repeating a push") and the body, but replace every "Push/Pull" with "Extrude". Line 3 becomes: "Extrude gives a flat face some thickness. It is the step that turns your drawings into solid shapes, and it is one of the most satisfying tools to use." Line 7 starts "Press `P` for the Extrude tool." and drops the sentence "On the toolbar its button is called Extrude." Also change "build up surprisingly detailed parts" to "detailed models" and "walls of a tray or the ribs of a bracket" to "walls of a room or the steps of a staircase".
2. `docs/guide/SUMMARY.md`: entry `- [Extrude](extrude.md)` in the same position.
3. `docs/guide/welcome.md`:
   - Replace the opening paragraph with: "Welcome! Drydock is a friendly 3D modeling program for designing buildings, rooms, furniture and anything else you can picture. You do not need any special equipment to begin: a mouse, a keyboard and a little patience are enough to make your first model."
   - Replace the first paragraph under "What Drydock is" with: "Drydock is a general-purpose 3D modeler. Many people use it for architecture: houses, rooms and the things inside them. Others design furniture, products or tools. A design is built from simple pieces: edges and faces. An edge is a straight line between two points, and a face is the flat surface that those edges enclose. You start by drawing flat shapes on the ground or on an existing face, and then you pull those shapes into solids. A flat rectangle becomes a box, and a flat circle becomes a cylinder. Because every solid is made from these basic pieces, you can keep changing a design as your idea develops."
   - Second paragraph under that heading: replace "add a little material where you need strength, remove a little where you want to save weight, and check the result from any angle before you print it" with "add a wall where you need one, cut an opening where you want a window, and check the result from any angle as you go". Then add a new third paragraph: "3D printing is one use Drydock supports especially well. When you want a physical copy, the [3D printing](printing.md) page shows how to check a model and send it to a printer."
   - The bullet "Then read [Push/Pull](push-pull.md) to turn those flat shapes into solid parts." becomes "- Then read [Extrude](extrude.md) to turn those flat shapes into solids."
   - The last bullet about printing keeps its link but reads "- [3D printing](printing.md) checks a model and prepares it for a printer." The text of the files link, if any, may stay.
4. Other pages (not `printing.md` or `files.md`): remove print-centric framing. Known spots: `materials.md` line 3 ("so a printed part can be planned" becomes "so the design is easier to plan"), `navigation.md` lines 3 and 19 ("a whole part" becomes "a whole model"; "two parts line up" becomes "two pieces line up"), `groups.md` and `window.md` lines mentioning parts (use "pieces" or "models"). Use neutral words: models, designs, pieces. Scan every page once for the rest.
5. Do not touch `render.rs` or Rust sources. `drydock/tests/guide_coverage` must keep passing (menu and tool names stay in `shortcuts.md`, which already lists Extrude).

## Tests (already written; do not edit)

- `drydock-guide/tests/wording.rs`: no page says Push/Pull, push pull or links push-pull; `extrude.md` exists with the `# Extrude` title and `push-pull.md` is gone; the welcome page's first two paragraphs contain no "print" and name "building"; outside `printing.md`, `files.md` and `welcome.md` no page mentions "print" or "solid part".
- `text_start.rs` and `text_more.rs` now name `extrude.md` / title "Extrude".

## Where to look

- `drydock-guide/tests/wording.rs`, `text_start.rs` (PAGES table), `text_more.rs`.
- `docs/guide/*.md` and `SUMMARY.md`; `grep -rniE "print|solid part|push" docs/guide` finds the spots.
- `drydock/tests/` guide_coverage test (`harness/bin/run-tests -p drydock guide_coverage`).
