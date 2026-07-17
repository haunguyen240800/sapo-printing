//! Sapo Printer Agent — elevated helper service.
//!
//! Nhiệm vụ:
//! 1. Ensure CA + server cert tồn tại (first-run sinh, sau đó load).
//! 2. Install CA vào system trust store (idempotent, cần elevated).
//! 3. IPC server (named pipe / unix socket) cho app trigger renew on-demand.
//! 4. Renewal loop 24h — auto renew server cert khi < 30 ngày,
//!    log cảnh báo khi CA sắp hết hạn (< 60 ngày).
//!
//! Chạy với quyền SYSTEM (Windows Service) / root (launchd/systemd).
//! Sprint 7 sẽ thêm SCM integration cho Windows và service unit files.
//!
//! Dev/CLI usage: `sapo-printer-agent --data-dir <path>` để test local.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sapo_printer::infrastructure::platform::tls::{
    cert_checker, cert_generator::CertGenerator, cert_installer::CertInstaller, ipc::IPC_ENDPOINT,
    IpcRequest, IpcResponse, PlatformInstaller, RenewalStatus,
};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const RENEWAL_TICK: Duration = Duration::from_secs(24 * 3600);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let data_dir = resolve_data_dir(&args)?;

    // Subcommands for installer use (all require elevated).
    match subcommand(&args) {
        Some("--install-ca") => {
            let bundle = CertGenerator::load_or_generate(&data_dir)?;
            let paths = sapo_printer::infrastructure::platform::tls::CertPaths::under(&data_dir);
            PlatformInstaller::default().install_ca(&paths.ca_pem)?;
            tracing::info!(
                newly_generated = bundle.is_newly_generated,
                "CA installed"
            );
            return Ok(());
        }
        Some("--uninstall-ca") => {
            PlatformInstaller::default().uninstall_ca()?;
            tracing::info!("CA uninstalled");
            return Ok(());
        }
        Some("--renew") => {
            CertGenerator::renew_server_cert(&data_dir)?;
            tracing::info!("Server cert renewed");
            return Ok(());
        }
        Some("--check") => {
            let bundle = CertGenerator::load_or_generate(&data_dir)?;
            let status = sapo_printer::infrastructure::platform::tls::cert_checker::needs_renewal(
                &bundle,
            );
            println!("{:?}", status);
            return Ok(());
        }
        _ => {}
    }

    tracing::info!(data_dir = %data_dir.display(), "Sapo Printer Agent starting");

    let installer: Arc<dyn CertInstaller> = Arc::new(PlatformInstaller::default());

    // 1. Ensure CA + server cert.
    let bundle = CertGenerator::load_or_generate(&data_dir)?;
    tracing::info!(
        newly_generated = bundle.is_newly_generated,
        ca_expires_at = ?bundle.ca_expires_at,
        server_expires_at = ?bundle.server_expires_at,
        "Cert bundle ready"
    );

    // 2. Install CA if newly generated or not trusted yet.
    if bundle.is_newly_generated || !installer.is_ca_trusted() {
        let paths = sapo_printer::infrastructure::platform::tls::CertPaths::under(&data_dir);
        match installer.install_ca(&paths.ca_pem) {
            Ok(_) => tracing::info!("CA installed into system trust store"),
            Err(e) => tracing::error!(error = %e, "CA install failed — app cannot serve HTTPS"),
        }
    }

    // 3. Spawn renewal loop.
    let renewal_data_dir = data_dir.clone();
    tokio::spawn(async move {
        renewal_loop(renewal_data_dir).await;
    });

    // 4. IPC server.
    run_ipc_server(data_dir).await?;
    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("SAPO_AGENT_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn subcommand(args: &[String]) -> Option<&str> {
    args.iter()
        .find(|a| !a.starts_with("--data-dir=") && a.starts_with("--"))
        .map(String::as_str)
}

fn resolve_data_dir(args: &[String]) -> Result<PathBuf, Box<dyn std::error::Error>> {
    for a in args {
        if let Some(path) = a.strip_prefix("--data-dir=") {
            return Ok(PathBuf::from(path));
        }
    }
    if let Some(v) = std::env::var_os("SAPO_AGENT_DATA_DIR") {
        return Ok(PathBuf::from(v));
    }
    #[cfg(target_os = "windows")]
    {
        let program_data =
            std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
        Ok(PathBuf::from(program_data).join("SapoPrinter"))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(PathBuf::from("/Library/Application Support/SapoPrinter"))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(PathBuf::from("/var/lib/sapo-printer"))
    }
}

// ===================== Renewal loop =====================

async fn renewal_loop(data_dir: PathBuf) {
    let mut ticker = tokio::time::interval(RENEWAL_TICK);
    // Skip immediate tick — first check khi service vừa lên, cert vừa sinh.
    ticker.tick().await;
    loop {
        ticker.tick().await;
        if let Err(e) = check_and_renew(&data_dir).await {
            tracing::error!(error = %e, "Renewal check failed");
        }
    }
}

async fn check_and_renew(data_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let bundle = CertGenerator::load_or_generate(data_dir)?;
    match cert_checker::needs_renewal(&bundle) {
        RenewalStatus::Ok => {
            tracing::debug!("Cert healthy, no action");
        }
        RenewalStatus::RenewSoon => {
            tracing::info!("Server cert expiring soon, renewing");
            CertGenerator::renew_server_cert(data_dir)?;
            tracing::info!("Server cert renewed");
        }
        RenewalStatus::CaRotationNeeded => {
            tracing::warn!(
                "CA expiring in < 60 days — manual rotation flow required (not yet implemented)"
            );
        }
        RenewalStatus::Expired => {
            tracing::error!("Cert expired, forcing renew");
            CertGenerator::renew_server_cert(data_dir)?;
        }
    }
    Ok(())
}

// ===================== IPC =====================

async fn run_ipc_server(data_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(endpoint = IPC_ENDPOINT, "IPC server listening");
    #[cfg(unix)]
    {
        unix_ipc(data_dir).await
    }
    #[cfg(windows)]
    {
        windows_ipc(data_dir).await
    }
}

#[cfg(unix)]
async fn unix_ipc(data_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::net::UnixListener;

    let _ = std::fs::remove_file(IPC_ENDPOINT);
    let listener = UnixListener::bind(IPC_ENDPOINT)?;

    // Restrict socket to owner+group (installer add app user to group `sapo-printer`).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(IPC_ENDPOINT)?.permissions();
        perm.set_mode(0o660);
        std::fs::set_permissions(IPC_ENDPOINT, perm)?;
    }

    loop {
        let (stream, _) = listener.accept().await?;
        let dir = data_dir.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, dir).await {
                tracing::warn!(error = %e, "IPC connection error");
            }
        });
    }
}

#[cfg(windows)]
async fn windows_ipc(data_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::net::windows::named_pipe::ServerOptions;

    loop {
        let server = ServerOptions::new()
            .first_pipe_instance(false)
            .create(IPC_ENDPOINT)?;
        server.connect().await?;
        let dir = data_dir.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(server, dir).await {
                tracing::warn!(error = %e, "IPC connection error");
            }
        });
    }
}

async fn handle_connection<S>(stream: S, data_dir: PathBuf) -> std::io::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (rd, mut wr) = tokio::io::split(stream);
    let mut lines = BufReader::new(rd).lines();
    if let Some(line) = lines.next_line().await? {
        let response = handle_request(&line, &data_dir);
        let mut json = serde_json::to_string(&response)
            .unwrap_or_else(|_| r#"{"ok":false,"error":"encode failure"}"#.to_string());
        json.push('\n');
        wr.write_all(json.as_bytes()).await?;
        wr.flush().await?;
    }
    Ok(())
}

fn handle_request(line: &str, data_dir: &std::path::Path) -> IpcResponse {
    let req: IpcRequest = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return IpcResponse::err(format!("bad request: {}", e)),
    };
    match req {
        IpcRequest::Ping => IpcResponse::ok_empty(),
        IpcRequest::RenewNow => match CertGenerator::renew_server_cert(data_dir) {
            Ok(b) => IpcResponse::Ok {
                ok: true,
                server_expires_at: Some(system_time_to_unix(b.server_expires_at)),
                ca_expires_at: Some(system_time_to_unix(b.ca_expires_at)),
                ca_trusted: None,
                renewal_status: Some("renewed".into()),
            },
            Err(e) => IpcResponse::err(e.to_string()),
        },
        IpcRequest::GetStatus => match CertGenerator::load_or_generate(data_dir) {
            Ok(b) => {
                let status = cert_checker::needs_renewal(&b);
                IpcResponse::Ok {
                    ok: true,
                    server_expires_at: Some(system_time_to_unix(b.server_expires_at)),
                    ca_expires_at: Some(system_time_to_unix(b.ca_expires_at)),
                    ca_trusted: Some(PlatformInstaller::default().is_ca_trusted()),
                    renewal_status: Some(format!("{:?}", status)),
                }
            }
            Err(e) => IpcResponse::err(e.to_string()),
        },
        IpcRequest::RotateCa => IpcResponse::err("rotate_ca not implemented"),
    }
}

fn system_time_to_unix(t: std::time::SystemTime) -> i64 {
    t.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// silence unused import warnings for platform-inactive branches
#[allow(dead_code)]
fn _unused(_v: Value) {}
