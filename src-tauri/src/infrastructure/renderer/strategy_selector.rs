use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::direct_pdf_renderer::DirectPdfRenderer;
use super::document_renderer::{ColorMode, DocumentRenderer, RenderConfig};
use super::pdfium_renderer::PdfiumRenderer;
use crate::infrastructure::printer::PrinterManager;

/// Selects the optimal rendering strategy based on printer capabilities and render config.
///
/// Caches per-printer capability detection results for `cache_ttl` (default 5s)
/// to avoid repeated OS API calls. Thread-safe via `Mutex<HashMap>`.
///
/// # Selection logic
///
/// ```text
/// printer supports native PDF?
/// ├── NO  → PdfiumRenderer (always)
/// └── YES → config requires margins or color conversion?
///           ├── YES (margins > 0 OR color ≠ RGB) → PdfiumRenderer
///           └── NO (margins == 0 AND color == RGB) → DirectPdfRenderer
/// ```
pub struct StrategySelector {
    printer_manager: Arc<dyn PrinterManager>,
    cache: Mutex<HashMap<String, (bool, Instant)>>,
    cache_ttl: Duration,
}

impl StrategySelector {
    pub fn new(printer_manager: Arc<dyn PrinterManager>) -> Self {
        Self {
            printer_manager,
            cache: Mutex::new(HashMap::new()),
            cache_ttl: Duration::from_secs(5),
        }
    }

    /// Creates a selector with a custom cache TTL (useful for testing).
    #[cfg(test)]
    fn with_ttl(printer_manager: Arc<dyn PrinterManager>, cache_ttl: Duration) -> Self {
        Self {
            printer_manager,
            cache: Mutex::new(HashMap::new()),
            cache_ttl,
        }
    }

    /// Selects the best renderer for the given printer and config.
    ///
    /// Returns `Arc<dyn DocumentRenderer>` — either `DirectPdfRenderer` (fast path)
    /// or `PdfiumRenderer` (control path).
    pub fn select_renderer(
        &self,
        printer_name: &str,
        config: &RenderConfig,
    ) -> Arc<dyn DocumentRenderer> {
        let supports_pdf = self.check_capability(printer_name);

        if supports_pdf && can_use_direct_pdf(config) {
            Arc::new(DirectPdfRenderer::new())
        } else {
            Arc::new(PdfiumRenderer::new(config.dpi))
        }
    }

    /// Clears all cached capability entries.
    pub fn invalidate_cache(&self) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.clear();
    }

    /// Clears the cached entry for a specific printer.
    pub fn invalidate_printer(&self, printer_name: &str) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.remove(printer_name);
    }

    fn check_capability(&self, printer_name: &str) -> bool {
        {
            let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(&(value, instant)) = cache.get(printer_name) {
                if instant.elapsed() < self.cache_ttl {
                    return value;
                }
            }
        }

        let value = self.printer_manager.supports_direct_pdf(printer_name);

        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(printer_name.to_string(), (value, Instant::now()));

        value
    }
}

/// Returns true if the config allows Direct PDF (no margins, RGB color).
fn can_use_direct_pdf(config: &RenderConfig) -> bool {
    config.margin_left_mm == 0.0
        && config.margin_right_mm == 0.0
        && config.margin_top_mm == 0.0
        && config.margin_bottom_mm == 0.0
        && config.color_mode == ColorMode::Rgb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::printer::{Printer, PrinterStatus};
    use std::sync::OnceLock;

    static PDFIUM_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    fn pdfium_lock() -> std::sync::MutexGuard<'static, ()> {
        PDFIUM_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap()
    }

    struct MockPrinterManager {
        supports_pdf: bool,
        call_count: Arc<Mutex<u32>>,
    }

    impl MockPrinterManager {
        fn new(supports_pdf: bool) -> Self {
            Self {
                supports_pdf,
                call_count: Arc::new(Mutex::new(0)),
            }
        }

        fn call_count(&self) -> u32 {
            *self.call_count.lock().unwrap()
        }
    }

    impl PrinterManager for MockPrinterManager {
        fn discover_printers(&self) -> Vec<Printer> {
            vec![]
        }

        fn get_status(&self, _name: &str) -> PrinterStatus {
            PrinterStatus::Online
        }

        fn supports_direct_pdf(&self, _printer_name: &str) -> bool {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            self.supports_pdf
        }
    }

    fn zero_margin_rgb_config() -> RenderConfig {
        RenderConfig {
            margin_left_mm: 0.0,
            margin_right_mm: 0.0,
            margin_top_mm: 0.0,
            margin_bottom_mm: 0.0,
            color_mode: ColorMode::Rgb,
            ..Default::default()
        }
    }

    #[test]
    fn test_direct_pdf_selected_when_capable_and_no_margins() {
        let mock = Arc::new(MockPrinterManager::new(true));
        let selector = StrategySelector::new(mock.clone());
        let config = zero_margin_rgb_config();

        let renderer = selector.select_renderer("test_printer", &config);
        let output = render_test_pdf(&*renderer, &config);

        assert!(
            output.starts_with(b"%PDF-"),
            "Direct PDF output must start with %PDF-"
        );
    }

    #[test]
    fn test_pdfium_selected_when_capable_but_has_margins() {
        let _lock = pdfium_lock();
        let mock = Arc::new(MockPrinterManager::new(true));
        let selector = StrategySelector::new(mock);
        let config = RenderConfig {
            margin_left_mm: 10.0,
            ..zero_margin_rgb_config()
        };

        let renderer = selector.select_renderer("test_printer", &config);
        let output = render_test_pdf(&*renderer, &config);

        assert!(
            !output.starts_with(b"%PDF-"),
            "PDFium output must NOT start with %PDF-"
        );
        assert!(
            output.len() >= 4,
            "PDFium output must have page count header"
        );
    }

    #[test]
    fn test_pdfium_selected_when_capable_but_non_rgb_color() {
        let _lock = pdfium_lock();
        let mock = Arc::new(MockPrinterManager::new(true));
        let selector = StrategySelector::new(mock);
        let config = RenderConfig {
            color_mode: ColorMode::Gray,
            ..zero_margin_rgb_config()
        };

        let renderer = selector.select_renderer("test_printer", &config);
        let output = render_test_pdf(&*renderer, &config);

        assert!(
            !output.starts_with(b"%PDF-"),
            "Non-RGB must use PDFium, not Direct PDF"
        );
    }

    #[test]
    fn test_pdfium_selected_when_not_capable() {
        let _lock = pdfium_lock();
        let mock = Arc::new(MockPrinterManager::new(false));
        let selector = StrategySelector::new(mock);
        let config = zero_margin_rgb_config();

        let renderer = selector.select_renderer("test_printer", &config);
        let output = render_test_pdf(&*renderer, &config);

        assert!(
            !output.starts_with(b"%PDF-"),
            "Non-capable printer must use PDFium"
        );
    }

    #[test]
    fn test_cache_hit_avoids_repeated_detection() {
        let mock = Arc::new(MockPrinterManager::new(true));
        let selector = StrategySelector::new(mock.clone());
        let config = zero_margin_rgb_config();

        selector.select_renderer("test_printer", &config);
        selector.select_renderer("test_printer", &config);

        assert_eq!(
            mock.call_count(),
            1,
            "Second call should use cache, not re-detect"
        );
    }

    #[test]
    fn test_cache_expiry_triggers_redetection() {
        let mock = Arc::new(MockPrinterManager::new(true));
        let selector = StrategySelector::with_ttl(mock.clone(), Duration::from_millis(10));
        let config = zero_margin_rgb_config();

        selector.select_renderer("test_printer", &config);
        std::thread::sleep(Duration::from_millis(20));
        selector.select_renderer("test_printer", &config);

        assert_eq!(mock.call_count(), 2, "After TTL expiry, should re-detect");
    }

    #[test]
    fn test_invalidate_cache_clears_all_entries() {
        let mock = Arc::new(MockPrinterManager::new(true));
        let selector = StrategySelector::new(mock.clone());
        let config = zero_margin_rgb_config();

        selector.select_renderer("printer_a", &config);
        selector.select_renderer("printer_b", &config);
        assert_eq!(mock.call_count(), 2);

        selector.invalidate_cache();

        selector.select_renderer("printer_a", &config);
        selector.select_renderer("printer_b", &config);
        assert_eq!(
            mock.call_count(),
            4,
            "After invalidate_cache, all should re-detect"
        );
    }

    #[test]
    fn test_invalidate_printer_clears_specific_entry() {
        let mock = Arc::new(MockPrinterManager::new(true));
        let selector = StrategySelector::new(mock.clone());
        let config = zero_margin_rgb_config();

        selector.select_renderer("printer_a", &config);
        selector.select_renderer("printer_b", &config);
        assert_eq!(mock.call_count(), 2);

        selector.invalidate_printer("printer_a");

        selector.select_renderer("printer_a", &config);
        assert_eq!(mock.call_count(), 3, "printer_a should re-detect");

        selector.select_renderer("printer_b", &config);
        assert_eq!(mock.call_count(), 3, "printer_b should still be cached");
    }

    #[test]
    fn test_strategy_selector_is_send_sync() {
        let mock = Arc::new(MockPrinterManager::new(true));
        let selector = Arc::new(StrategySelector::new(mock));
        let config = zero_margin_rgb_config();

        let selector_clone = selector.clone();
        let config_clone = config.clone();
        let handle = std::thread::spawn(move || {
            let _renderer = selector_clone.select_renderer("thread_test", &config_clone);
        });

        handle.join().unwrap();
    }

    fn render_test_pdf(renderer: &dyn DocumentRenderer, config: &RenderConfig) -> Vec<u8> {
        let dir = std::env::temp_dir().join("sapo_strategy_selector_tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.pdf");

        let pdf_content = b"%PDF-1.4\n\
            1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
            2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
            3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
            xref\n0 4\n\
            0000000000 65535 f \n\
            0000000009 00000 n \n\
            0000000058 00000 n \n\
            0000000115 00000 n \n\
            trailer\n<< /Size 4 /Root 1 0 R >>\n\
            startxref\n190\n%%EOF";

        std::fs::write(&path, pdf_content).unwrap();
        renderer.render(&path, config).unwrap()
    }
}
