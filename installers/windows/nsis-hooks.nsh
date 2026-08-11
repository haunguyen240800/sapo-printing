; NSIS installer hooks — chạy sau khi install/trước khi uninstall.
; Include vào bundle qua tauri.conf.json: bundle.windows.nsis.installerHooks

; Chạy sau khi tất cả files được copy vào INSTALLDIR
!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Đang cài CA cert..."
  nsExec::ExecToStack 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\install-ca-cert.ps1"'
  Pop $0 ; exit code
  Pop $1 ; output
  ${If} $0 != 0
    DetailPrint "Cảnh báo: Cài CA cert thất bại (exit $0). App vẫn hoạt động nhưng web có thể không kết nối được."
    DetailPrint "Output: $1"
  ${Else}
    DetailPrint "CA cert đã được cài thành công."
  ${EndIf}
!macroend

