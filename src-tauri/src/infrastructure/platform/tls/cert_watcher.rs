//! File watcher — detect helper service ghi `server.pem` mới → hot reload rustls.
//!
//! Watch cả `server.pem` và `server.key`. Debounce 500ms để tránh reload
//! khi file được ghi partial (helper viết 2 file consecutive).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum_server::tls_rustls::RustlsConfig;
use notify::{RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::shared::errors::InfrastructureError;

const DEBOUNCE: Duration = Duration::from_millis(500);

pub fn spawn_watcher(
    server_pem: PathBuf,
    server_key: PathBuf,
    tls_config: Arc<RustlsConfig>,
) -> Result<(), InfrastructureError> {
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();

    let watch_tx = tx.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if matches!(
                event.kind,
                notify::EventKind::Modify(_) | notify::EventKind::Create(_)
            ) {
                let _ = watch_tx.send(());
            }
        }
    })
    .map_err(|e| InfrastructureError::TlsError(format!("notify watcher init: {}", e)))?;

    // Watch cert dir (both files thay đổi trong cùng dir).
    let dir = server_pem
        .parent()
        .ok_or_else(|| InfrastructureError::TlsError("server_pem has no parent".into()))?
        .to_path_buf();
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| InfrastructureError::TlsError(format!("notify watch: {}", e)))?;

    tokio::spawn(async move {
        // Keep watcher alive by moving into task.
        let _watcher = watcher;
        loop {
            match rx.recv().await {
                Some(()) => {
                    // Debounce: drain remaining events trong window.
                    tokio::time::sleep(DEBOUNCE).await;
                    while let Ok(()) = rx.try_recv() {}
                    match tls_config
                        .reload_from_pem_file(&server_pem, &server_key)
                        .await
                    {
                        Ok(_) => tracing::info!("TLS cert hot-reloaded"),
                        Err(e) => tracing::error!(error = %e, "TLS reload failed"),
                    }
                }
                None => break,
            }
        }
    });

    Ok(())
}
