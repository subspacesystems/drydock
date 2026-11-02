# 3D printing

Drydock helps you check a part and get it ready for a printer. When the part is ready, see [files](files.md) for exporting it.

## Is it a solid?

Select one group or component. Entity Info (View > Entity Info) then shows Solid: Yes or No, its Volume, and, only when above zero, counts of Open edges, Loose edges, Shared edges (used by three or more faces), Reversed faces, Internal faces and Nested groups, and Shells when there is not exactly one. A part is a solid when it is watertight, which means every edge joins exactly two faces, so a slicer can tell inside from outside. The problem edges and faces are drawn in magenta in the viewport while the part is selected.

## Repairing

The Repair button in Entity Info is enabled when something is wrong, and it is one undo step. It removes internal and stray faces and loose edges, fills flat holes, and turns faces that point the wrong way. It then reports what it did, and what it could not fix: holes that are not flat, ring-shaped holes, and edges used by three or more faces. Components share their definition, so repairing one repairs every copy.

## Print Setup

Tools > Print Setup holds the printing settings. Printer bed presets are 180, 220, 256, 300 and 350 mm squares, or you can set your own Bed X, Bed Y and optional Bed Z in mm. Show bed outline draws the bed on the ground, centered on the origin. Check fit compares the size of the selected parts (or the whole model) with the bed; it checks size, not position, and does not turn parts. Drop to Bed moves the selected groups or components down (or up) together until the lowest point sits on the ground. The settings are remembered between sessions and are not part of the model. The Overhang angle (deg) box is also here.

## Overhangs

View > Overhangs tints faces that hang over empty space more than the angle (45 degrees to start), measured from straight down; the lowest faces resting on the bed are not tinted. Change the angle in Print Setup. Turn it off the same way.

## Fit check and clearance

Select two groups or components and choose Tools > Fit Check; faces of either part closer to the other than the clearance turn yellow and the status bar counts them (as triangles). The Clearance box (in mm, 0.2 to start) in the status bar changes the distance and updates the check; any edit to the model ends the check. Tools > Grow by Clearance and Tools > Shrink by Clearance make each selected flat-faced solid bigger or smaller on every face by that distance, in one undo step, so a printed part fits its mate.

## 3D text

Tools > 3D Text asks for Text, Font size (mm), Height (mm), and Emboss or Engrave. After OK, an outline follows the cursor over the faces of the part you are editing; click to place it, raised or cut in. If Drydock cannot make it, the status bar says why and the text stays pending so you can try another spot; `Esc` or choosing a tool drops it. One line, upright, one built-in font. It works inside an open group or component.
