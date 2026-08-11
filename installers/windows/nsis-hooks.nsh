; NSIS installer hooks — chạy sau khi install/trước khi uninstall.
; Include vào bundle qua tauri.conf.json: bundle.windows.nsis.installerHooks

; Chạy trước khi installer bắt đầu copy files
!macro NSIS_HOOK_PREINSTALL
  ; Dừng service cũ nếu đang chạy (để upgrade không bị lỗi file lock)
  DetailPrint "Kiểm tra service cũ..."
  nsExec::ExecToStack 'powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "Stop-Service -Name SapoPrinterAgent -Force -ErrorAction SilentlyContinue"'
  Pop $0
  Pop $1
!macroend

; Chạy sau khi tất cả files được copy vào INSTALLDIR
!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Đang cài CA cert và đăng ký Windows Service..."
  nsExec::ExecToStack 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\register-service.ps1"'
  Pop $0 ; exit code
  Pop $1 ; output
  ${If} $0 != 0
    DetailPrint "Cảnh báo: Đăng ký service thất bại (exit $0). App vẫn hoạt động nhưng web có thể không kết nối được."
    DetailPrint "Output: $1"
  ${Else}
    DetailPrint "CA cert và Windows Service đã được cài thành công."
  ${EndIf}
!macroend

