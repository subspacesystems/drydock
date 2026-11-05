# Drydock on Linux

There is no installer for Drydock on Linux: you run the program from the folder where you built or unpacked it. So that the dock, task switcher and application launcher can show Drydock's icon, the program creates its own desktop entry and icon files each time it starts.

## Where the files go

They live in the user data folder. Drydock uses `$XDG_DATA_HOME` when that variable is set to an absolute path, and otherwise falls back to `~/.local/share`. Under that folder it writes `applications/com.subspacesystems.Drydock.desktop` and PNG icons under `icons/hicolor/` at 48, 128 and 256 pixels square. The `.desktop` file is what the desktop matches to Drydock's window, and `hicolor` is the standard icon theme that launchers and docks read.

Drydock only rewrites these files when one is missing or has the wrong contents, so starting the program again normally does nothing. If you move Drydock to a different folder, the next start updates the desktop entry so it points at the new place; the icons stay as they are. It touches no other files. If the data folder is read-only or a particular file cannot be written, Drydock reports the problem on the error output and carries on.

## Removing them

To remove the entries, delete the `.desktop` file and the Drydock icons under `icons/hicolor/`. This does not remove Drydock itself; the program will create the files again the next time you start it from its folder. You can safely leave them alone while you keep using Drydock. If you want them back after deleting them, simply start Drydock once and it will recreate both the entry and the icons.
