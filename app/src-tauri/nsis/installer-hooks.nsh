; Passive mode creates and later re-shows the MUI window before PREINSTALL runs. For an
; updater-owned passive run, make that window fully transparent, non-activating and absent from the
; taskbar. Manual installs are unchanged.
!define MUI_CUSTOMFUNCTION_GUIINIT WinerHidePassiveUpdate

Function WinerHidePassiveUpdate
  ClearErrors
  ${GetOptions} $CMDLINE "/P" $R0
  ${IfNot} ${Errors}
    ClearErrors
    ${GetOptions} $CMDLINE "/UPDATE" $R0
    ${IfNot} ${Errors}
      ; GWL_EXSTYLE = -20. Add WS_EX_LAYERED, WS_EX_NOACTIVATE and WS_EX_TOOLWINDOW.
      System::Call 'user32::GetWindowLongW(p $HWNDPARENT, i -20) i .r0'
      IntOp $R0 $R0 | 0x08080080
      System::Call 'user32::SetWindowLongW(p $HWNDPARENT, i -20, i $R0) i .r1'
      ; LWA_ALPHA = 2, alpha 0: a later ShowWindow still paints no visible pixels.
      System::Call 'user32::SetLayeredWindowAttributes(p $HWNDPARENT, i 0, i 0, i 2) i .r1'
      HideWindow
    ${EndIf}
  ${EndIf}
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  ${If} $UpdateMode = 1
  ${AndIf} $PassiveMode = 1
    HideWindow
  ${EndIf}
!macroend
