; SPDX-License-Identifier: GPL-3.0-or-later
; Copyright (C) 2026 Subspace Systems

!ifndef VERSION
  !error "pass -DVERSION=x.y.z"
!endif
!ifndef STAGE
  !error "pass -DSTAGE=<folder with drydock.exe>"
!endif
!ifndef OUTFILE
  !define OUTFILE "drydock-setup-${VERSION}.exe"
!endif

Unicode true
Name "Subspace Drydock"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\Subspace Drydock"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Function .onInit
  SetRegView 64
  SetShellVarContext all

  ReadRegStr $R1 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "InstallLocation"
  StrCmp $R1 "" notice_done
  ReadRegStr $R2 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "DisplayVersion"
  IfSilent notice_done

  StrCmp $R2 "" notice_unknown
  StrCmp $R2 "${VERSION}" notice_same
  MessageBox MB_OKCANCEL|MB_ICONINFORMATION "Subspace Drydock $R2 is installed in $R1.$\nSetup will remove it and install ${VERSION}." IDOK notice_done
  Abort

notice_same:
  MessageBox MB_OKCANCEL|MB_ICONINFORMATION "Subspace Drydock ${VERSION} is installed in $R1.$\nSetup will remove it and reinstall it." IDOK notice_done
  Abort

notice_unknown:
  MessageBox MB_OKCANCEL|MB_ICONINFORMATION "Subspace Drydock is installed in $R1.$\nSetup will remove it and install ${VERSION}." IDOK notice_done
  Abort

notice_done:
FunctionEnd

Function un.onInit
  SetRegView 64
  SetShellVarContext all
FunctionEnd

Function RemoveOldVersion
  ReadRegStr $R1 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "InstallLocation"
  StrCmp $R1 "" done
  IfFileExists "$R1\uninstall.exe" 0 done
  ExecWait '"$R1\uninstall.exe" /S _?=$R1'
  Delete "$R1\uninstall.exe"
  RMDir "$R1"
done:
FunctionEnd

Section "Install"
  Call RemoveOldVersion

  SetOutPath "$INSTDIR"
  File "${STAGE}\drydock.exe"
  File /nonfatal /r "${STAGE}\guide"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\Subspace Drydock.lnk" "$INSTDIR\drydock.exe"

  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "DisplayName" "Subspace Drydock"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "Publisher" "Subspace Systems"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "DisplayIcon" "$INSTDIR\drydock.exe"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock" "NoRepair" 1

  WriteRegStr HKLM "Software\Classes\.drydock" "" "SubspaceDrydock.Model"
  WriteRegStr HKLM "Software\Classes\SubspaceDrydock.Model" "" "Drydock model"
  WriteRegStr HKLM "Software\Classes\SubspaceDrydock.Model\DefaultIcon" "" '"$INSTDIR\drydock.exe",0'
  WriteRegStr HKLM "Software\Classes\SubspaceDrydock.Model\shell\open\command" "" '"$INSTDIR\drydock.exe" "%1"'
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\drydock.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\guide"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\Subspace Drydock.lnk"

  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock"
  DeleteRegKey HKLM "Software\Classes\.drydock"
  DeleteRegKey HKLM "Software\Classes\SubspaceDrydock.Model"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd
