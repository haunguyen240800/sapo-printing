// Entry point for Tauri application
// This file initializes the Tauri runtime and registers commands

// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sapo_printer::infrastructure::database::{DbPool, run_migrations};

fn main() {
    // 1. Ensure ~/.sapo-printer/ data directory exists
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let data_dir = std::path::PathBuf::from(&home).join(".sapo-printer");
    std::fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create data directory: {e}");
        std::process::exit(1);
    });
    let db_path = data_dir.join("config.db");
    let db_path_str = db_path.to_str().unwrap_or_else(|| {
        eprintln!("Database path contains non-UTF-8 characters");
        std::process::exit(1);
    });

    // 2. Initialize database connection
    let pool = DbPool::new(db_path_str).unwrap_or_else(|e| {
        eprintln!("Database init failed: {e}");
        std::process::exit(1);
    });

    // 3. Run migrations
    {
        let mut conn = pool.get();
        run_migrations(&mut conn).unwrap_or_else(|e| {
            eprintln!("Migration failed: {e}");
            std::process::exit(1);
        });
    }

    // 4. Start Tauri — pool registered as managed state for use by command handlers
    tauri::Builder::default()
        .manage(pool)
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Failed to start Tauri application:");
            eprintln!("  {e}");
            std::process::exit(1);
        });
}
