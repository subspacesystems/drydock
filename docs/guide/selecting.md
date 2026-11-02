# Selecting

Before you can change an edge or a face, you have to select it. Selecting is a small skill, but it makes everything else in Drydock feel quick and predictable.

## Clicking

Press `Space` to choose the Select tool. Click an edge or a face to select it, and click empty space to clear the selection. A plain click replaces whatever was selected before, so you always end up with just the thing you clicked. When something is selected it is highlighted, which confirms that the next command will act on it and nothing else.

## Adding and removing

Sometimes you want more than one piece at a time. Hold `Shift` while clicking to add to the selection. Hold `Ctrl` to toggle: it adds a piece that is not selected yet, and removes one that is already selected. Hold `Alt` to remove a piece from the selection. These three keys let you build up a careful set of edges or faces without starting over each time.

## Selecting with a box

Drag in empty space to draw a selection box. Drag from left to right to make a window box: only the things completely inside the box are selected. Drag from right to left to make a crossing box: things inside the box and things that only touch it are selected. The same modifier keys work with boxes, so you can add, toggle or remove while you drag. This is the fastest way to gather a group of edges on a busy model.

## Selecting connected geometry

A double click on a face or an edge selects it together with everything that bounds it. A triple click selects everything connected to it. A box makes a small example. A double click on one face of the box selects that face along with its edges. A triple click on the same face selects the whole box, because all of its faces and edges are connected. Once the whole box is selected you can move it, copy it or delete it as one piece.
