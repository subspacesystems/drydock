# Groups and components

Groups and components keep pieces of geometry together so you can move and change them as one. Select the things you want first; see [selecting](selecting.md) for the different ways to select.

## Making a group or component

Select things, then choose Edit > Make Group or Edit > Make Component. Both ask for a name first. A group is a one-off bundle that belongs only to this model. A component is a named piece that you can place again and again.

## Editing inside

Use a double click on a group or component with the Select tool to open it. Only its contents can then be edited, so the rest of the model stays out of the way. To close it, double click empty space outside it, or press `Esc`. View > Hide Rest of Model hides everything else while you are inside, and the same item shows it again.

## The Components panel

View > Components opens the Components panel. It lists the model's components; groups never appear there. Each entry has a small picture. A search box matches part of the name and ignores case. A switch changes between a list and tiles. Click an entry to place it; the copy follows the cursor until you click. The panel can also import a component from a model file (DAE, STL or 3MF; an STL is read in millimeters there without asking).

## Exploding

Select a group or component and choose Edit > Explode to turn it back into plain geometry. The name and the bundle go away, and the edges and faces are loose again.

## Copies of a component

All copies of a component share one definition, so editing inside one changes every copy. Groups are separate each time, so changing one group does not touch another.
