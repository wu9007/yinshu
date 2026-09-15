!macro NSIS_HOOK_PREINSTALL
  CreateDirectory "$LOCALAPPDATA\cn.yinshu.app\logs"
  FileOpen $0 "$LOCALAPPDATA\cn.yinshu.app\logs\install.log" w
  FileWrite $0 "PREINSTALL$\r$\n"
  FileClose $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$LOCALAPPDATA\cn.yinshu.app\logs"
  FileOpen $0 "$LOCALAPPDATA\cn.yinshu.app\logs\install.log" a
  FileWrite $0 "POSTINSTALL$\r$\n"
  FileClose $0
!macroend
