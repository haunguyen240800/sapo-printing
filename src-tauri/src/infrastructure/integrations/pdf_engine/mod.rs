pub mod renderer;
pub mod bitmap_strategy;
pub mod pdfium_loader;

/// Native PDF render strategy. Disabled by default — see `Cargo.toml`
/// feature `native_pdf_render`. The FFI bridge to `FPDF_RenderPage` is not
/// implemented yet; gating prevents the strategy from being accidentally
/// wired in and failing every job at render time.
#[cfg(feature = "native_pdf_render")]
pub mod native_strategy;
