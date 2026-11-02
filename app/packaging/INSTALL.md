# Subspace Drydock beta

## Install

Run `drydock-<version>-setup.exe` and follow the installer. Installing over an older version
upgrades it in place; your models and settings are kept. To remove the program, open Windows
Settings > Apps, find Subspace Drydock and choose Uninstall.

## Check the download

The release includes `SHA256SUMS`. Compare the downloaded installer with the recorded hash in
PowerShell:

```powershell
Get-FileHash .\drydock-<version>-setup.exe -Algorithm SHA256
```

The hash must match the line for the installer in `SHA256SUMS`.

## Unsigned build and SmartScreen

This build is unsigned for now. Windows SmartScreen may say "Windows protected your PC".
Click More info, then Run anyway.

## Smart App Control

On Windows 11, Smart App Control blocks unsigned programs and offers no way to click through.
To run this beta you must turn Smart App Control off in Windows Security. It cannot be turned
back on without resetting Windows, so please weigh that before installing.

## Your files

Settings live in `%APPDATA%\Drydock`. Uninstalling Subspace Drydock keeps your models and
settings.
