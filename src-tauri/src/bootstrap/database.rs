use std::path::Path;

use crate::infrastructure::{
    configs::db::{DbPool, run_migrations},
    temp_file,
};

/// Khởi tạo database pool, chạy migrations, dọn temp files cũ.
/// Trả về `DbPool` đã sẵn sàng dùng.
/// Gọi `std::process::exit(1)` nếu database không thể khởi tạo.
pub fn init_database(db_path_str: &str, temp_dir: &Path) -> DbPool {
    let pool = DbPool::new(db_path_str).unwrap_or_else(|e| {
        eprintln!("Database init failed: {e}");
        std::process::exit(1);
    });

    {
        let mut conn = pool.get().unwrap_or_else(|e| {
            eprintln!("Database connection failed: {e}");
            std::process::exit(1);
        });
        run_migrations(&mut *conn).unwrap_or_else(|e| {
            eprintln!("Migration failed: {e}");
            std::process::exit(1);
        });
    }

    let retention = temp_file::load_retention(&pool);
    temp_file::startup_cleanup(temp_dir, retention);

    pool
}
