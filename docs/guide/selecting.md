# Selecting

## Clicking

The Select tool is the first button on the toolbar, and its key is `Space`. Click an edge or a face to select it, and click empty space to clear the selection. A plain click replaces whatever was selected before. A double click selects a face or edge together with everything that bounds it. A triple click selects everything connected to it, so you can grab a whole part in one go.

## Adding and removing

Modifier keys change what a click does to the selection. Hold `Shift` to add to the selection. Hold `Ctrl` to toggle: an unselected thing is added, and a selected thing is removed. Hold `Alt` to remove something from the selection. These keys also work while you drag a selection box, so you can build up a selection piece by piece.

## Selecting with a box

Drag with the left mouse button on empty space to draw a box. Drag from left to right for a window box: only things completely inside the box are selected. Drag from right to left for a crossing box: things inside the box or touching it are selected. The direction of the drag decides which kind of box you get. The same `Shift`, `Ctrl` and `Alt` keys work with boxes.

## Selecting connected geometry

A box is a good example. Double click one of its faces and the face is selected along with its bounding edges. Triple click the same face and the whole box is selected, because all of its geometry is connected. Use a double click when you want one face and its border, and a triple click when you want the entire connected piece.
