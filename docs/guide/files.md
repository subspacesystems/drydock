# Files, import and export

For checking a part before you print it, see [printing](printing.md).

## Saving and opening

File > New starts a fresh model. File > Open (`Ctrl+O`) opens a file, File > Save (`Ctrl+S`) saves it, File > Save As (`Ctrl+Shift+S`) saves it under another name, and File > Exit closes the program. Drydock's own files end in `.drydock`. Under File the most recent files are listed for one-click opening. Unsaved changes are asked about before New, Open or Exit. An opened DAE, STL or 3MF file is not a Drydock file, so Save asks for a `.drydock` name. Files saved by earlier versions with the old extension still open.

## Importing models

File > Open also takes DAE, STL and 3MF files. An STL file does not say what its numbers mean, so Drydock asks for the unit (millimeters first; mm, cm, m or in). A 3MF file carries its own unit and is not asked. File > Import adds a file's solids to the model you have open, each as a group at the place it has in the file; one undo step removes them. Choosing Cancel on the unit question leaves the model untouched.

## Exporting for printing

File > Export 3MF and File > Export STL are greyed out until the model has a group or component. A window asks what to export: Whole model or Selection (the selected groups and components). For STL it also asks the unit (mm, cm, m, in) and One file per object or Single file; 3MF is always millimeters in one file. Curve tolerance (mm) sets how finely curved edges are cut into flat pieces; smaller is smoother and bigger files. Only solid groups and components are written (see the printing page); anything else is skipped, and the status bar names each skipped part and why. Exporting never changes your model.
