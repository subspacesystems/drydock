# Extrude

Extrude gives a flat face some thickness. It is the step that turns your drawings into solid shapes, and it is one of the most satisfying tools to use.

## Pushing a face

Press `P` for the Extrude tool. Click a face, then move the cursor along the direction you want to pull it out or push it in. A preview shows the new shape as you move, so you can see the result before you commit. Click again to finish. If you move the cursor over a corner elsewhere in the model, the height snaps to match that corner, which is a simple way to keep two parts level. A distance of zero does nothing, `Esc` abandons the push, and undo takes back a finished one. When you are learning, use the preview freely: nothing changes until the second click.

## Typing a distance

While you are pushing, type a length and press `Enter` to set the distance exactly. A negative number reverses the direction, so you can push inward instead of pulling outward. Right after a push, you can type a new length and press `Enter` to change that push instead of adding another one. This is useful when you finish a pull and immediately decide it should be a little thicker or thinner; no extra step is needed.

## Repeating a push

Double click another face to push it by the same distance as the last push. This saves time when several faces need the same thickness, such as the walls of a room or the steps of a staircase. Check the preview, then let the double click do the work. Once you are comfortable with this, you can build up detailed models from a few flat shapes and a series of consistent pushes.

## Offset

Press `I` for the Offset tool. Click a face, then move the cursor toward the side you want the face to grow or shrink, or type a length and press `Enter`. Moving outward grows the face and moving inward shrinks it. Holes move the other way, so they shrink when the face grows and grow when it shrinks. If a distance is too big for the face, or if the offset edges would run into another face, the change is refused and a message explains why. `Esc` abandons an offset in progress, and one undo takes back a finished one.

## Follow Me

Follow Me carries a flat profile along a path and builds a solid from the trail. Press `F`, click the Follow Me button on the toolbar next to Offset, or choose **Tools > Follow Me**.

- **Pick the profile.** Click the face you want to carry.
- **Pick the path.** Click the path's edges one by one; a second click on an edge takes it out again. Or click a face to use its outline as the path.
- **Check the preview.** It shows the shape the sweep will make.
- **Apply.** Press `Enter`, or click the path face a second time. `Esc` cancels. One undo takes back a finished sweep.

The profile has to be a free face: not drawn on another face, and with its edges used by no other face. The path edges must be connected and must not branch. Corners of the path are mitred, and a circle as the profile gives a round pipe. To stand a profile up away from the path's plane, lock the drawing plane with an arrow key before you draw it (see [Drawing](drawing.md)).

Follow Me refuses a profile that stands on the path face, and a sweep whose end would lie along other edges. A message in the status bar says why.
