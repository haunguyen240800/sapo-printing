use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::application::ports::{TempFileHandle, TempFileManager};
use crate::domain::print_job::PrintJobId;
use crate::infrastructure::configs::db::DbPool;
use crate::shared::errors::InfrastructureError;

pub const DEFAULT_RETENTION_HOURS: u32 = 24;

pub struct TempPdfFile {
    path: PathBuf,
    keep_on_drop: bool,
}

impl TempPdfFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            keep_on_drop: false,
        }
    }

    pub fn try_new(path: PathBuf, temp_dir: &Path) -> Result<Self, InfrastructureError> {
        let canonical_temp = temp_dir
            .canonicalize()
            .unwrap_or_else(|_| temp_dir.to_path_buf());
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());

        if !canonical_path.starts_with(&canonical_temp) {
            return Err(InfrastructureError::ValidationError(format!(
                "TempPdfFile path {:?} is not inside temp directory {:?}",
                path, temp_dir
            )));
        }

        Ok(Self {
            path,
            keep_on_drop: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn keep(&mut self) {
        self.keep_on_drop = true;
    }

    pub fn release(mut self) -> PathBuf {
        self.keep_on_drop = true;
        self.path.clone()
    }
}

impl Drop for TempPdfFile {
    fn drop(&mut self) {
        if !self.keep_on_drop {
            if let Err(e) = std::fs::remove_file(&self.path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("Failed to cleanup temp file {:?}: {}", self.path, e);
                }
            }
        }
    }
}

pub struct FilesystemTempFileManager {
    temp_dir: PathBuf,
}

impl FilesystemTempFileManager {
    pub fn new(temp_dir: PathBuf) -> Self {
        Self { temp_dir }
    }

    fn job_temp_path(&self, job_id: &PrintJobId) -> PathBuf {
        self.temp_dir.join(format!("{}.pdf", job_id))
    }
}

impl TempFileHandle for TempPdfFile {
    fn path(&self) -> &Path {
        TempPdfFile::path(self)
    }

    fn keep(&mut self) {
        TempPdfFile::keep(self);
    }
}

impl TempFileManager for FilesystemTempFileManager {
    fn wrap(&self, path: PathBuf) -> Result<Box<dyn TempFileHandle>, InfrastructureError> {
        let temp_file = TempPdfFile::try_new(path, &self.temp_dir)?;
        Ok(Box::new(temp_file))
    }

    fn cleanup_for_job(&self, job_id: &PrintJobId) {
        let temp_path = self.job_temp_path(job_id);
        if temp_path.exists() {
            match std::fs::remove_file(&temp_path) {
                Ok(()) => {
                    tracing::info!(
                        target = "sapo_printer::temp_file",
                        path = ?temp_path,
                        "Cleaned up temp file"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        target = "sapo_printer::temp_file",
                        path = ?temp_path,
                        error = %e,
                        "Failed to cleanup temp file"
                    );
                }
            }
        }
    }
}

pub fn load_retention(pool: &DbPool) -> Duration {
    let hours = (|| -> Option<u32> {
        let conn = pool.get().ok()?;
        let value: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'temp_file_retention_hours'",
                [],
                |row| row.get(0),
            )
            .ok()?;
        value.parse::<u32>().ok()
    })()
        .unwrap_or(DEFAULT_RETENTION_HOURS);

    Duration::from_secs(hours as u64 * 3600)
}

pub fn startup_cleanup(temp_dir: &Path, retention: Duration) {
    if !temp_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(temp_dir) {
            tracing::warn!("Failed to create temp directory {:?}: {}", temp_dir, e);
        }
        return;
    }

    let entries = match std::fs::read_dir(temp_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("Failed to read temp directory {:?}: {}", temp_dir, e);
            return;
        }
    };

    let mut removed = 0usize;
    let mut kept = 0usize;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to read directory entry: {}", e);
                continue;
            }
        };

        let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }

        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            Some("tmp") => {
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("Failed to remove .tmp file {:?}: {}", path, e);
                } else {
                    removed += 1;
                }
            }
            Some("pdf") if is_older_than(&path, retention) => {
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("Failed to remove old .pdf file {:?}: {}", path, e);
                } else {
                    removed += 1;
                }
            }
            Some("pdf") => {
                kept += 1;
            }
            _ => {
                kept += 1;
            }
        }
    }

    tracing::info!(
        target = "sapo_printer::temp_file",
        retention_secs = retention.as_secs(),
        removed,
        kept,
        "Startup temp-dir cleanup complete"
    );
}

fn is_older_than(path: &Path, retention: Duration) -> bool {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|modified| {
            SystemTime::now()
                .duration_since(modified)
                .map(|age| age > retention)
                .unwrap_or(false) // Clock skew backward → mtime in future → file is recent, keep it
        })
        .unwrap_or(true) // Metadata read failure → treat as old (safe default: delete)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sapo_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const TEST_RETENTION: Duration = Duration::from_secs(24 * 60 * 60);

    #[test]
    fn test_temp_file_deleted_on_drop_when_keep_false() {
        let dir = make_test_dir();
        let file_path = dir.join("test.pdf");
        fs::write(&file_path, b"%PDF-1.4 test").unwrap();
        assert!(file_path.exists());

        {
            let _temp = TempPdfFile::new(file_path.clone());
        }

        assert!(!file_path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_temp_file_kept_on_drop_when_keep_true() {
        let dir = make_test_dir();
        let file_path = dir.join("test.pdf");
        fs::write(&file_path, b"%PDF-1.4 test").unwrap();

        {
            let mut temp = TempPdfFile::new(file_path.clone());
            temp.keep();
        }

        assert!(file_path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_temp_file_path_accessible() {
        let dir = make_test_dir();
        let file_path = dir.join("test.pdf");
        fs::write(&file_path, b"%PDF-1.4 test").unwrap();

        let temp = TempPdfFile::new(file_path.clone());
        assert_eq!(temp.path(), file_path.as_path());
        let _ = temp.release();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_startup_cleanup_removes_tmp_files() {
        let dir = make_test_dir();
        let tmp_file = dir.join("orphan.tmp");
        fs::write(&tmp_file, b"incomplete download").unwrap();

        startup_cleanup(&dir, TEST_RETENTION);

        assert!(!tmp_file.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_startup_cleanup_removes_old_pdf() {
        let dir = make_test_dir();
        let old_pdf = dir.join("old.pdf");
        fs::write(&old_pdf, b"%PDF-1.4").unwrap();

        let old_time = SystemTime::UNIX_EPOCH
            + Duration::from_secs(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - TEST_RETENTION.as_secs()
                - 3600,
        );
        filetime::set_file_mtime(&old_pdf, filetime::FileTime::from_system_time(old_time)).unwrap();

        startup_cleanup(&dir, TEST_RETENTION);

        assert!(!old_pdf.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_startup_cleanup_keeps_recent_pdf() {
        let dir = make_test_dir();
        let recent_pdf = dir.join("recent.pdf");
        fs::write(&recent_pdf, b"%PDF-1.4").unwrap();

        startup_cleanup(&dir, TEST_RETENTION);

        assert!(recent_pdf.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_startup_cleanup_nonexistent_dir_creates_it() {
        let base = make_test_dir();
        let nonexistent = base.join("nonexistent");
        assert!(!nonexistent.exists());

        startup_cleanup(&nonexistent, TEST_RETENTION);

        assert!(nonexistent.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_drop_no_panic_when_file_already_deleted() {
        let dir = make_test_dir();
        let file_path = dir.join("missing.pdf");

        {
            let _temp = TempPdfFile::new(file_path);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_temp_pdf_file_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TempPdfFile>();
    }

    #[test]
    fn test_release_does_not_delete() {
        let dir = make_test_dir();
        let file_path = dir.join("release_test.pdf");
        fs::write(&file_path, b"%PDF-1.4 test").unwrap();

        let temp = TempPdfFile::new(file_path.clone());
        let returned_path = temp.release();

        assert!(file_path.exists());
        assert_eq!(returned_path, file_path);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_try_new_accepts_path_inside_temp_dir() {
        let dir = make_test_dir();
        let file_path = dir.join("job.pdf");
        fs::write(&file_path, b"%PDF-1.4 test").unwrap();

        let result = TempPdfFile::try_new(file_path.clone(), &dir);
        assert!(result.is_ok(), "try_new should accept path inside temp_dir");

        let temp = result.unwrap();
        assert_eq!(temp.path(), file_path.as_path());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_try_new_rejects_path_outside_temp_dir() {
        let dir = make_test_dir();
        let outside_path = std::env::temp_dir().join("outside_file.pdf");
        fs::write(&outside_path, b"%PDF-1.4 test").unwrap();

        let result = TempPdfFile::try_new(outside_path.clone(), &dir);
        assert!(
            result.is_err(),
            "try_new should reject path outside temp_dir"
        );

        let _ = fs::remove_file(&outside_path);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_retention_uses_app_settings_value() {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("sapo_retention_{nanos}.db"));
        let pool = DbPool::new(path.to_str().unwrap()).unwrap();
        {
            let mut conn = pool.get().unwrap();
            crate::infrastructure::configs::db::run_migrations(&mut *conn).unwrap();
            conn.execute(
                "UPDATE app_settings SET value = '48' WHERE key = 'temp_file_retention_hours'",
                [],
            )
                .unwrap();
        }

        let retention = load_retention(&pool);
        assert_eq!(retention, Duration::from_secs(48 * 3600));

        drop(pool);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_load_retention_falls_back_to_default_when_missing() {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("sapo_retention_missing_{nanos}.db"));
        let pool = DbPool::new(path.to_str().unwrap()).unwrap();
        {
            let mut conn = pool.get().unwrap();
            crate::infrastructure::configs::db::run_migrations(&mut *conn).unwrap();
            conn.execute(
                "DELETE FROM app_settings WHERE key = 'temp_file_retention_hours'",
                [],
            )
                .unwrap();
        }

        let retention = load_retention(&pool);
        assert_eq!(
            retention,
            Duration::from_secs(DEFAULT_RETENTION_HOURS as u64 * 3600)
        );

        drop(pool);
        let _ = std::fs::remove_file(&path);
    }
}
