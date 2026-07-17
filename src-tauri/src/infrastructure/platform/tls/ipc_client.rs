//! IPC client — connect tới `sapo-printer-agent` helper service.
//!
//! Windows: named pipe. Unix: unix domain socket. Timeout 5s.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::ipc::{IpcRequest, IpcResponse, IPC_ENDPOINT};
use crate::shared::errors::InfrastructureError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn send(request: IpcRequest) -> Result<IpcResponse, InfrastructureError> {
    tokio::time::timeout(REQUEST_TIMEOUT, do_send(request))
        .await
        .map_err(|_| InfrastructureError::IpcError("request timeout".into()))?
}

async fn do_send(request: IpcRequest) -> Result<IpcResponse, InfrastructureError> {
    let mut json = serde_json::to_string(&request)
        .map_err(|e| InfrastructureError::IpcError(format!("encode: {}", e)))?;
    json.push('\n');

    #[cfg(unix)]
    {
        use tokio::net::UnixStream;
        let stream = tokio::time::timeout(CONNECT_TIMEOUT, UnixStream::connect(IPC_ENDPOINT))
            .await
            .map_err(|_| InfrastructureError::IpcError("connect timeout".into()))?
            .map_err(|e| InfrastructureError::IpcError(format!("connect: {}", e)))?;
        exchange(stream, json.as_bytes()).await
    }

    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;
        let stream = tokio::time::timeout(CONNECT_TIMEOUT, async {
            loop {
                match ClientOptions::new().open(IPC_ENDPOINT) {
                    Ok(s) => break Ok::<_, std::io::Error>(s),
                    Err(e)
                        if e.raw_os_error() == Some(231) /* ERROR_PIPE_BUSY */ =>
                    {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(e) => break Err(e),
                }
            }
        })
        .await
        .map_err(|_| InfrastructureError::IpcError("connect timeout".into()))?
        .map_err(|e| InfrastructureError::IpcError(format!("connect: {}", e)))?;
        exchange(stream, json.as_bytes()).await
    }
}

async fn exchange<S>(stream: S, req: &[u8]) -> Result<IpcResponse, InfrastructureError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (rd, mut wr) = tokio::io::split(stream);
    wr.write_all(req)
        .await
        .map_err(|e| InfrastructureError::IpcError(format!("write: {}", e)))?;
    wr.flush()
        .await
        .map_err(|e| InfrastructureError::IpcError(format!("flush: {}", e)))?;

    let mut line = String::new();
    BufReader::new(rd)
        .read_line(&mut line)
        .await
        .map_err(|e| InfrastructureError::IpcError(format!("read: {}", e)))?;
    serde_json::from_str::<IpcResponse>(line.trim())
        .map_err(|e| InfrastructureError::IpcError(format!("parse: {}", e)))
}
