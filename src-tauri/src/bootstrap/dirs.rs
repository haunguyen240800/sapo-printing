use std::path::PathBuf;

/// Các đường dẫn thư mục cần thiết cho app, được khởi tạo một lần lúc startup.
pub struct AppDirs {
    pub data_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub db_path: PathBuf,
    pub db_path_str: String,
}

/// Tạo và kiểm tra tất cả các thư mục cần thiết.
/// Gọi `std::process::exit(1)` nếu có lỗi không thể phục hồi.
pub fn init_directories() -> AppDirs {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());

    let data_dir = PathBuf::from(&home).join(".sapo-printer");
    std::fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create data directory: {e}");
        std::process::exit(1);
    });

    let temp_dir = data_dir.join("temp");
    std::fs::create_dir_all(&temp_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create temp directory: {e}");
        std::process::exit(1);
    });

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
        db_path,
        db_path_str,
    }
}
