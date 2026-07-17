//! HTTPS local server — Axum + rustls.
//!
//! Sprint 4: REST endpoints (ping, pair, jobs, printers) + auth middleware + CORS + rate limit.
//! Sprint 5: SSE endpoint /events.
//!
//! Bind với port fallback range 18901-18910. TLS load từ `server.pem` / `server.key`
//! do helper service ghi ra.

pub mod bootstrap;
pub mod cors;
pub mod handlers;
pub mod middleware;
pub mod router;
pub mod server;
pub mod sse;
pub mod state;

pub use bootstrap::{start as start_bootstrap, BootstrapResult, JOB_STATUS_EVENTS};
pub use server::{start_server, AgentMetadata};
pub use state::HttpServerState;
