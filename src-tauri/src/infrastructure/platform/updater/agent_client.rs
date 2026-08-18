//! Client phía app: gửi yêu cầu update tới `sapo-printer-agent` (SYSTEM) qua named pipe.
//!
//! App KHÔNG tự chạy installer khi có service — service (SYSTEM) làm để tránh UAC.
//! Nếu service không phản hồi, caller (update_command) tự fallback về đường UAC cũ.

use crate::infrastructure::platform::tls::{IpcRequest, IpcResponse, ipc_client};

/// Yêu cầu service tải + verify + cài bản `expected_version`, và relaunch app.
/// `app_pid` để service chờ app hiện tại thoát trước khi ghi đè file.
///
/// Trả `Ok(())` nếu service đã nhận và staged bản cập nhật thành công.
pub async fn request_update(expected_version: String, app_pid: u32) -> Result<(), String> {
    let resp = ipc_client::send(IpcRequest::RequestUpdate {
        expected_version,
        app_pid,
    })
    .await
    .map_err(|e| e.to_string())?;

    match resp {
        IpcResponse::Ok { .. } => Ok(()),
        IpcResponse::Err { error, .. } => Err(error),
    }
}
