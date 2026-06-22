use std::path::PathBuf;

use tauri::AppHandle;
#[cfg(debug_assertions)]
use tauri::Manager;

#[cfg(debug_assertions)]
const APP_DIR_NAME: &str = env!("SAPO_APP_SLUG");

pub struct AppDirs {
    pub data_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub log_dir: PathBuf,
    pub print_config_path: PathBuf,
    pub db_path: PathBuf,
    pub db_path_str: String,
}

pub fn init_directories(app: &AppHandle) -> AppDirs {
    let data_dir = resolve_data_root(app);
    std::fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create data directory: {e}");
        std::process::exit(1);
    });

    let temp_dir = data_dir.join("temp");
    std::fs::create_dir_all(&temp_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create temp directory: {e}");
        std::process::exit(1);
    });

    let log_dir = data_dir.join("logs");
    let print_config_path = data_dir.join("print-config.json");

    let db_path = data_dir.join("config.db");
    let db_path_str = db_path
        .to_str()
        .unwrap_or_else(|| {
            eprintln!("Database path contains non-UTF-8 characters");
            std::process::exit(1);
        })
        .to_string();

    AppDirs {
        data_dir,
        temp_dir,
        log_dir,
        print_config_path,
        db_path,
        db_path_str,
    }
}

/// Data root dùng chung cho mọi user: lưu ngay cạnh file thực thi (install dir).
/// Nhờ đó DB/config/logs được chia sẻ giữa các Windows user cùng cài đặt.
///
/// Debug build dùng OS data dir để tránh làm bẩn thư mục `target/`.
fn resolve_data_root(app: &AppHandle) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let base = app.path().data_dir().unwrap_or_else(|e| {
            eprintln!("Cannot resolve OS data directory: {e}");
            std::process::exit(1);
        });
        return base.join(APP_DIR_NAME);
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = app;
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| {
                eprintln!("Cannot resolve executable directory for data root");
                std::process::exit(1);
            })
    }
}
