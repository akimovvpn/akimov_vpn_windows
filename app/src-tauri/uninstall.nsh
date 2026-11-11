; Uninstalling removes everything the app saved for this user. An update
; (the installer started with /UPDATE, as the in-app updater does) keeps it.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    ; The recovery file exists only while the app's proxy is set in Windows;
    ; turn it off before the file that records it is deleted.
    ${If} ${FileExists} "$APPDATA\akimov-vpn\proxy-recovery.json"
      WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Internet Settings" "ProxyEnable" 0
    ${EndIf}
    RMDir /r "$APPDATA\akimov-vpn"
    RMDir /r "$APPDATA\${BUNDLEID}"
    RMDir /r "$LOCALAPPDATA\${BUNDLEID}"
  ${EndIf}
!macroend
