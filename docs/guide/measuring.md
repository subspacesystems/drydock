# Measuring and units

Measuring is a read-only operation: you can find a distance without changing the model. If you need help with the drawing tools, see [drawing](drawing.md).

## Tape Measure

The Tape Measure tool has key `T` and the toolbar label Tape Measure. Click a start point and an end point. The distance appears in the measurements box, and nothing in the model changes. You can use it to check a gap, a length or the size of a face before you make an edit.

## Guides

If the first click is on an edge, the second click places a guide line parallel to that edge through the second point. Clicking a point that is on the edge's own line says so instead of making a guide. Holding `Ctrl` while clicking with Tape Measure puts a guide point. Guides can be undone like other edits, so an unwanted guide is easy to remove.

## Choosing units

View > Model Info opens the Model Info panel. Its Units section has Unit (Millimeters, Centimeters, Meters, Inches, Feet, Architectural) and Precision (decimal places, or a fraction of an inch for Architectural). The choice is saved with the model, and every length and area in Drydock then uses it, including Entity Info and the measurements box. Changing it is one undo step ("Set Units").

The Background section of Model Info sets the plain background color and, with their checkboxes, the sky and ground colors. They are saved with the model, and changing them is one undo step ("Set Background").

## Typing lengths

A number typed while drawing or moving uses the model's unit unless you add one yourself, such as `10cm` or `1/2"`. Your own unit only applies to that value. This lets you work in a familiar unit even when the model is set to a different one.
