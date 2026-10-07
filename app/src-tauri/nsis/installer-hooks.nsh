!macro NSIS_HOOK_PREINSTALL
  ${If} $UpdateMode = 1
  ${AndIf} $PassiveMode = 1
    HideWindow
  ${EndIf}
!macroend
