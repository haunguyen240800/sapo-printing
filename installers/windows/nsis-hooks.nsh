; NSIS installer hooks — chạy sau khi install/trước khi uninstall.
; Include vào bundle qua tauri.conf.json: bundle.windows.nsis.installerHooks

; Installer boundary values. Canonical Rust source: src-tauri/src/infrastructure/platform/agent_config.rs.
!define SAPO_AGENT_SERVICE_NAME "SapoPrinterAgent"
!define SAPO_AGENT_EXE_NAME "sapo-printer-cert-manager.exe"

; Chạy TRƯỚC khi copy files. Dừng service để giải phóng lock trên
; sapo-printer-cert-manager.exe (nếu đang chạy từ bản cài trước) → cho phép ghi đè khi update.
!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Dừng Sapo Printer Pro Max Agent (nếu đang chạy)..."
  nsExec::ExecToStack 'sc.exe stop "${SAPO_AGENT_SERVICE_NAME}"'
  Pop $0
  Pop $1
!macroend

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

  DetailPrint "Đăng ký Sapo Printer Pro Max Agent service..."
  nsExec::ExecToStack 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\register-agent-service.ps1" -ServiceName "${SAPO_AGENT_SERVICE_NAME}" -AgentExeName "${SAPO_AGENT_EXE_NAME}"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    DetailPrint "Cảnh báo: Đăng ký service thất bại (exit $0). Update sẽ dùng cơ chế cũ (có UAC)."
    DetailPrint "Output: $1"
  ${Else}
    DetailPrint "Agent service đã được đăng ký + khởi động."
  ${EndIf}
!macroend

; Chạy trước khi gỡ files khi uninstall — stop + delete service.
!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Gỡ Sapo Printer Pro Max Agent service..."
  nsExec::ExecToStack 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\unregister-agent-service.ps1" -ServiceName "${SAPO_AGENT_SERVICE_NAME}"'
  Pop $0
  Pop $1
!macroend
