; Passive mode creates the MUI window before PREINSTALL runs. Hide an updater-owned passive run
; during GUI initialization, before its first page can be painted. Manual installs are unchanged.
!define MUI_CUSTOMFUNCTION_GUIINIT WinerHidePassiveUpdate

Function WinerHidePassiveUpdate
  ClearErrors
  ${GetOptions} $CMDLINE "/P" $R0
  ${IfNot} ${Errors}
    ClearErrors
    ${GetOptions} $CMDLINE "/UPDATE" $R0
    ${IfNot} ${Errors}
      HideWindow
    ${EndIf}
  ${EndIf}
FunctionEnd

; Keep it hidden if a page or plug-in brings the installer forward before files are copied.
!macro NSIS_HOOK_PREINSTALL
  ${If} $UpdateMode = 1
  ${AndIf} $PassiveMode = 1
    HideWindow
  ${EndIf}
!macroend
