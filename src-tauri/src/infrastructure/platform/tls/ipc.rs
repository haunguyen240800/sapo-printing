//! IPC protocol giữa helper service (`sapo-printer-agent`) và app runtime.
//!
//! Transport:
//! - Windows: named pipe `\\.\pipe\sapo-printer-agent`.
//! - Unix: unix domain socket `/var/run/sapo-printer-agent.sock`.
//!
//! Wire format: JSON, một message = một dòng (newline-delimited).
//! - Request: `{"cmd":"renew_now"}\n`
//! - Response: `{"ok":true, ...}\n`
//!
//! Server đọc 1 dòng, xử lý, ghi 1 dòng, đóng connection.

use serde::{Deserialize, Serialize};

pub use crate::infrastructure::platform::agent_config::IPC_ENDPOINT;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcRequest {
    RenewNow,
    GetStatus,
    RotateCa,
    Ping,
    /// App yêu cầu service (SYSTEM) tự tải + verify + cài bản cập nhật im lặng.
    /// Service KHÔNG nhận url/signature từ client — tự đọc `latest.json` từ endpoint
    /// cố định. `expected_version` chỉ để đối chiếu; `app_pid` để service chờ app thoát
    /// trước khi ghi đè file rồi relaunch.
    RequestUpdate {
        expected_version: String,
        app_pid: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpcResponse {
    Ok {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        server_expires_at: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ca_expires_at: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ca_trusted: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        renewal_status: Option<String>,
        /// Trạng thái luồng update ("staged" = đã tải+verify xong, sẽ cài sau khi app thoát).
        #[serde(skip_serializing_if = "Option::is_none")]
        update_status: Option<String>,
    },
    Err {
        ok: bool,
        error: String,
    },
}

impl IpcResponse {
    pub fn err(msg: impl Into<String>) -> Self {
        Self::Err {
            ok: false,
            error: msg.into(),
        }
    }

    pub fn ok_empty() -> Self {
        Self::Ok {
            ok: true,
            server_expires_at: None,
            ca_expires_at: None,
            ca_trusted: None,
            renewal_status: None,
            update_status: None,
        }
    }

    /// Trả về khi service đã tải + verify bản cập nhật xong (sẽ cài sau khi app thoát).
    pub fn update_staged() -> Self {
        Self::Ok {
            ok: true,
            server_expires_at: None,
            ca_expires_at: None,
            ca_trusted: None,
            renewal_status: None,
            update_status: Some("staged".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_snake_case() {
        let json = serde_json::to_string(&IpcRequest::RenewNow).unwrap();
        assert_eq!(json, r#"{"cmd":"renew_now"}"#);
        let json = serde_json::to_string(&IpcRequest::GetStatus).unwrap();
        assert_eq!(json, r#"{"cmd":"get_status"}"#);
    }

    #[test]
    fn request_parses_snake_case() {
        let req: IpcRequest = serde_json::from_str(r#"{"cmd":"renew_now"}"#).unwrap();
        assert!(matches!(req, IpcRequest::RenewNow));
    }

    #[test]
    fn response_ok_serializes() {
        let r = IpcResponse::Ok {
            ok: true,
            server_expires_at: Some(1700000000),
            ca_expires_at: None,
            ca_trusted: Some(true),
            renewal_status: None,
            update_status: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains(r#""ok":true"#));
        assert!(json.contains("server_expires_at"));
        assert!(!json.contains("ca_expires_at"));
    }

    #[test]
    fn response_err_serializes() {
        let r = IpcResponse::err("boom");
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, r#"{"ok":false,"error":"boom"}"#);
    }
}
