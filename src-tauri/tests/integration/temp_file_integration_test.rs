use sapo_printer::infrastructure::temp_file::{startup_cleanup, TempPdfFile};
use std::fs;
use std::path::PathBuf;

fn make_test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sapo_inttest_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_lifecycle_immediate_cleanup() {
    let dir = make_test_dir();
    let pdf_path = dir.join("test_job.pdf");
    fs::write(&pdf_path, b"%PDF-1.4 test").unwrap();

    assert!(pdf_path.exists());
    {
        let _temp = TempPdfFile::new(pdf_path.clone());
    }
    assert!(!pdf_path.exists(), "File should be deleted on drop");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_lifecycle_deferred_cleanup() {
    let dir = make_test_dir();
    let pdf_path = dir.join("failed_job.pdf");
    fs::write(&pdf_path, b"%PDF-1.4 test").unwrap();

    {
        let mut temp = TempPdfFile::new(pdf_path.clone());
        temp.keep();
    }
    assert!(pdf_path.exists(), "File should survive drop when keep=true");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_startup_cleanup_full_scenario() {
    let dir = make_test_dir();

    let tmp_file = dir.join("orphan.tmp");
    fs::write(&tmp_file, b"incomplete download").unwrap();

    let recent_pdf = dir.join("recent.pdf");
    fs::write(&recent_pdf, b"%PDF-1.4").unwrap();

    startup_cleanup(&dir);

    assert!(!tmp_file.exists(), ".tmp files must be deleted on startup");
    assert!(recent_pdf.exists(), "Recent .pdf must be kept");

    let _ = fs::remove_dir_all(&dir);
}
