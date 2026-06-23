//! Document Downloader Module
//!
//! Downloads PDF documents from S3 URLs for bulk printing.
//! Uses a circuit breaker pattern to prevent retry storms when S3 is unavailable.

pub mod circuit_breaker;
pub mod document_downloader;
pub mod reqwest_downloader;

pub use circuit_breaker::CircuitBreaker;
pub use document_downloader::DocumentDownloader;
pub use reqwest_downloader::ReqwestDownloader;
