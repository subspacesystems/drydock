# Updates

On Windows, Drydock can look for a newer version and install it for you. The Linux build does not update itself, so none of this appears there.

## Works offline, always

Drydock never needs the network. The check runs quietly in the background, never delays the start of the program or any tool, and a missing connection just means no update this time: no error window, no message. Nothing is sent about you or your computer. The check is a plain request for a small signed file that names the newest version, and nothing else.

## The three choices

Help > Check for Updates opens the Updates window. It holds the setting for what Drydock does by itself:

- **Check automatically** (the default): shortly after start, and then about once a day, Drydock looks for a newer version and downloads it in the background.
- **Notify only**: Drydock looks, tells you when a newer version exists, and downloads it only when you press Download.
- **Off**: Drydock never connects by itself.

Your choice is kept with your other settings and stays when Drydock is updated. Check now in the same window looks right away, in every mode, and tells you "Drydock is up to date." or that the update server could not be reached. If a build of Drydock has no update address set, the window says so and nothing is checked.

## Installing an update

When a new version has been downloaded, Drydock shows "Drydock X.Y.Z is ready to install" with a link to the release notes. Later hides the message until the next start; Restart and update closes Drydock, runs the installer without questions and starts the new version. If your model has unsaved changes you are asked first, as when you close the program: Save, Don't save, or Cancel. Cancel stops the update and nothing changes. If the download or the install fails, the version you have keeps working.

Before anything is installed, Drydock checks that the update information is signed with Drydock's own key and that the downloaded installer is exactly the one it describes. Anything that does not match is thrown away.

## Windows warnings

Until the installer is code-signed, Windows SmartScreen may warn about it when it runs. Choose More info and then Run anyway if you trust where the update came from. Builds made for development use a test key and a test address, so they never accept a real release.
