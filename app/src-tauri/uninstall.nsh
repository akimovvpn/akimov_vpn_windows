; Saved data never outlives the app: uninstalling removes it, and installing
; when the app is not installed starts clean. An update (the installer started
; with /UPDATE, as the in-app updater does, or installing over an existing
; copy) keeps it.
!ifndef AKIMOV_DATA
  !define AKIMOV_DATA "akimov-vpn"
!endif

!macro AKIMOV_WIPE
  SetShellVarContext current
  ; The recovery file exists only while the app's proxy is set in Windows;
  ; turn it off before the file that records it is deleted.
  ${If} ${FileExists} "$APPDATA\${AKIMOV_DATA}\proxy-recovery.json"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Internet Settings" "ProxyEnable" 0
  ${EndIf}
  RMDir /r "$APPDATA\${AKIMOV_DATA}"
  RMDir /r "$APPDATA\${BUNDLEID}"
  RMDir /r "$LOCALAPPDATA\${BUNDLEID}"
!macroend

; "Uninstall before installing" runs the uninstaller of the version being
; replaced. Up to 1.0.2 that one left the data folder behind, so the installer
; checks too: no uninstall entry means nothing is installed any more.
!macro NSIS_HOOK_PREINSTALL
  ${If} $UpdateMode <> 1
    Push $0
    ReadRegStr $0 HKCU "${UNINSTKEY}" "UninstallString"
    ${If} $0 == ""
      !insertmacro AKIMOV_WIPE
    ${EndIf}
    Pop $0
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    !insertmacro AKIMOV_WIPE
  ${EndIf}
!macroend
