; Runs the hooks from src-tauri/uninstall.nsh against throwaway folders and a
; throwaway registry key, never a real installation. Driven by uninstall-hook.ps1.
Unicode true
!include LogicLib.nsh
!include FileFunc.nsh
!define AKIMOV_DATA "akimov-vpn-hooktest"
!define BUNDLEID "vpn.akimov.hooktest"
!define UNINSTKEY "Software\akimov-hooktest"
Var UpdateMode
!include "..\src-tauri\uninstall.nsh"
Name "akimov hook test"
OutFile "${OUT}"
RequestExecutionLevel user
SilentInstall silent
Section
  ${GetParameters} $1
  ClearErrors
  ${GetOptions} $1 "/UPDATE" $2
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}
  ClearErrors
  ${GetOptions} $1 "/UNINSTALL" $2
  ${IfNot} ${Errors}
    !insertmacro NSIS_HOOK_POSTUNINSTALL
  ${Else}
    !insertmacro NSIS_HOOK_PREINSTALL
  ${EndIf}
SectionEnd
