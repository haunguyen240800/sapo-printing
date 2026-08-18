//! Update executor chạy trong `sapo-printer-cert-manager` (SYSTEM).
//!
//! Nhiệm vụ: tải installer từ endpoint CỐ ĐỊNH, verify chữ ký minisign, chạy installer
//! im lặng (không UAC vì đã là SYSTEM), rồi relaunch app trong session của user.
//!
//! Nguyên tắc bảo mật (xem docs/silent-auto-update-privileged-agent.md §7):
//! - Endpoint hard-code, KHÔNG nhận url/signature từ IPC client.
//! - Verify minisign trước khi execute — fail thì xoá file, abort.
//! - Chỉ chạy khi version của manifest khớp `expected_version` app báo.

use std::path::{Path, PathBuf};

use crate::infrastructure::platform::agent_config::UPDATER_PLATFORM_KEY;
use base64::Engine;

/// Values owned by Tauri and injected by `build.rs` from `tauri.conf.json`.
const LATEST_JSON_URL: &str = env!("SAPO_UPDATER_ENDPOINT");
const UPDATER_PUBKEY_B64: &str = env!("SAPO_UPDATER_PUBKEY");

#[derive(Debug)]
pub struct StagedUpdate {
    pub version: String,
    pub installer_path: PathBuf,
}

/// Tải + verify bản cập nhật. KHÔNG chạy installer (để caller quyết định thời điểm).
pub async fn stage_update(expected_version: &str, data_dir: &Path) -> Result<StagedUpdate, String> {
    let manifest = fetch_manifest().await?;

    if manifest.version != expected_version {
        return Err(format!(
            "version mismatch: manifest={} expected={}",
            manifest.version, expected_version
        ));
    }

    let update_dir = data_dir.join("update");
    std::fs::create_dir_all(&update_dir).map_err(|e| format!("create update dir: {}", e))?;

    let installer_path = update_dir.join(format!("sapo-printer-setup-{}.exe", manifest.version));
    download_to(&manifest.url, &installer_path).await?;

    if let Err(e) = verify_signature(&installer_path, &manifest.signature) {
        let _ = std::fs::remove_file(&installer_path);
        return Err(format!("signature verify failed: {}", e));
    }

    tracing::info!(version = %manifest.version, "Update staged + verified");
    Ok(StagedUpdate {
        version: manifest.version,
        installer_path,
    })
}

struct Manifest {
    version: String,
    url: String,
    signature: String,
}

async fn fetch_manifest() -> Result<Manifest, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {}", e))?;
    let resp = client
        .get(LATEST_JSON_URL)
        .send()
        .await
        .map_err(|e| format!("fetch latest.json: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("latest.json HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse latest.json: {}", e))?;

    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or("latest.json missing version")?
        .to_string();
    let platform = json
        .get("platforms")
        .and_then(|p| p.get(UPDATER_PLATFORM_KEY))
        .ok_or_else(|| format!("latest.json missing platform {}", UPDATER_PLATFORM_KEY))?;
    let url = platform
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("latest.json missing url")?
        .to_string();
    let signature = platform
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or("latest.json missing signature")?
        .to_string();

    Ok(Manifest {
        version,
        url,
        signature,
    })
}

async fn download_to(url: &str, dest: &Path) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {}", e))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("download body: {}", e))?;
    std::fs::write(dest, &bytes).map_err(|e| format!("write installer: {}", e))?;
    Ok(())
}

/// Verify chữ ký minisign của installer.
///
/// `signature_b64` là nội dung field `signature` trong latest.json — base64 của cả file
/// `.sig` (đúng định dạng Tauri sinh ra). Pubkey lấy từ const đã nhúng.
fn verify_signature(installer: &Path, signature_b64: &str) -> Result<(), String> {
    use minisign_verify::{PublicKey, Signature};

    // Pubkey: base64 -> nội dung file pubkey (2 dòng: comment + key) -> dòng key.
    let pk_file_bytes = base64::engine::general_purpose::STANDARD
        .decode(UPDATER_PUBKEY_B64.trim())
        .map_err(|e| format!("decode pubkey b64: {}", e))?;
    let pk_file = String::from_utf8(pk_file_bytes).map_err(|e| format!("pubkey utf8: {}", e))?;
    let pk_line = pk_file
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("untrusted comment"))
        .ok_or("pubkey line not found")?;
    let public_key = PublicKey::from_base64(pk_line).map_err(|e| format!("parse pubkey: {}", e))?;

    // Signature: base64 -> nội dung file .sig -> Signature::decode.
    let sig_file_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim())
        .map_err(|e| format!("decode signature b64: {}", e))?;
    let sig_file =
        String::from_utf8(sig_file_bytes).map_err(|e| format!("signature utf8: {}", e))?;
    let signature = Signature::decode(&sig_file).map_err(|e| format!("parse signature: {}", e))?;

    let data = std::fs::read(installer).map_err(|e| format!("read installer: {}", e))?;
    public_key
        .verify(&data, &signature, false)
        .map_err(|e| format!("verify: {}", e))?;
    Ok(())
}

/// Chạy installer im lặng (SYSTEM → không UAC) và relaunch app trong session user.
///
/// Trình tự: chờ app (`app_pid`) thoát → chạy installer `/S` → relaunch `app_exe`.
/// Chạy trong std thread (blocking) vì đợi process + WaitForSingleObject.
#[cfg(windows)]
pub fn finalize_update(installer: PathBuf, app_pid: u32, app_exe: PathBuf) {
    std::thread::spawn(move || {
        wait_for_process_exit(app_pid, std::time::Duration::from_secs(60));

        tracing::info!(installer = %installer.display(), "Running silent installer");
        let status = std::process::Command::new(&installer).arg("/S").status();
        match status {
            Ok(s) if s.success() => tracing::info!("Installer finished OK"),
            Ok(s) => tracing::error!(code = ?s.code(), "Installer non-zero exit"),
            Err(e) => {
                tracing::error!(error = %e, "Installer spawn failed");
                return;
            }
        }

        if let Err(e) = relaunch_in_user_session(&app_exe) {
            tracing::error!(error = %e, "Relaunch in user session failed");
        }
    });
}

#[cfg(not(windows))]
pub fn finalize_update(_installer: PathBuf, _app_pid: u32, _app_exe: PathBuf) {
    tracing::warn!("finalize_update is Windows-only");
}

#[cfg(windows)]
fn wait_for_process_exit(pid: u32, timeout: std::time::Duration) {
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    unsafe {
        let handle = match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(h) if !h.is_invalid() => h,
            _ => {
                // Process đã thoát hoặc không mở được → tiếp tục ngay.
                return;
            }
        };
        let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        let r = WaitForSingleObject(handle, ms);
        if r != WAIT_OBJECT_0 {
            tracing::warn!("App did not exit within timeout; proceeding anyway");
        }
        let _ = CloseHandle(handle);
    }
}

/// Relaunch `exe` trong session console đang active bằng token của user (CreateProcessAsUser).
/// Giải quyết session-0 isolation: service chạy ở session 0, app cần chạy ở session user.
#[cfg(windows)]
fn relaunch_in_user_session(exe: &Path) -> Result<(), String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::RemoteDesktop::{WTSGetActiveConsoleSessionId, WTSQueryUserToken};
    use windows::Win32::System::Threading::{
        CREATE_NEW_CONSOLE, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW, PROCESS_INFORMATION,
        STARTUPINFOW,
    };
    use windows::core::PWSTR;

    unsafe {
        let session_id = WTSGetActiveConsoleSessionId();
        if session_id == 0xFFFF_FFFF {
            return Err("no active console session".into());
        }

        let mut token = HANDLE::default();
        WTSQueryUserToken(session_id, &mut token)
            .map_err(|e| format!("WTSQueryUserToken: {}", e))?;

        // Command line buffer (mutable, NUL-terminated UTF-16). Bọc trong ngoặc kép.
        let mut cmdline: Vec<u16> = format!("\"{}\"", exe.display())
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let startup = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut proc_info = PROCESS_INFORMATION::default();

        let result = CreateProcessAsUserW(
            token,
            None,
            PWSTR(cmdline.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_CONSOLE,
            None,
            None,
            &startup,
            &mut proc_info,
        );

        let _ = CloseHandle(token);

        result.map_err(|e| format!("CreateProcessAsUser: {}", e))?;

        if !proc_info.hProcess.is_invalid() {
            let _ = CloseHandle(proc_info.hProcess);
        }
        if !proc_info.hThread.is_invalid() {
            let _ = CloseHandle(proc_info.hThread);
        }
        Ok(())
    }
}
