# Moving, rotating and scaling

Select what you want to change first; see [selecting](selecting.md) if you need a reminder.

## Move

The Move tool has key `M` and the toolbar label Move. Click a start point, then click a second point; the selection moves by the offset between them. While moving, type a distance and press `Enter` to move along the direction the cursor shows; a negative number goes the other way. Right after a move, typing a new distance and pressing `Enter` changes that move. Tap `Ctrl` once while the Move tool is active to switch copy mode on or off. With copy mode on, the second click makes a copy instead of moving the original, and the copy becomes the selection so you can copy again. Choosing another tool turns copy mode off. `Esc` cancels.

## Rotate

The Rotate tool has key `Q` and the toolbar label Rotate. Click the center of the protractor, click the start of the angle, then click the end. You can also type an angle in degrees and press `Enter`; a negative angle turns the other way. Typing a new angle right after a rotation changes that rotation. Points snap while you rotate, as they do when drawing.

## Scale

The Scale tool has key `S` and the toolbar label Scale. The selection shows handles on its bounding box: corners, edge middles and face middles. Press on a handle and drag, or type factors and press `Enter`. Holding `Ctrl` scales about the center. Holding `Shift` flips the uniform or non-uniform choice.

## Flip

Edit > Flip Along Red, Edit > Flip Along Green and Edit > Flip Along Blue mirror the selection along that axis. These items are greyed out when nothing is selected, so select something first.

## Erase

The Erase tool has key `E` and the toolbar label Erase. You can also press `Delete` with things selected, or use Edit > Erase. Erasing a face leaves its edges. Erasing an edge also removes the faces that used it, unless two flat faces on either side of it simply merge into one. Undo (`Ctrl+Z`, Edit > Undo) takes back every one of these.
