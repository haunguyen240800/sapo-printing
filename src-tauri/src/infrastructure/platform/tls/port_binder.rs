//! Port binder với fallback range.
//!
//! Preferred `18901`. Nếu bị chiếm (EADDRINUSE) → thử `18902..=18910`.
//! Trả về `TcpListener` + port thực tế. Caller ghi ra `agent.json` cho webapp discover.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::ops::RangeInclusive;

pub const DEFAULT_PORT: u16 = 18901;
pub const FALLBACK_RANGE: RangeInclusive<u16> = 18901..=18910;

pub fn bind_with_fallback(
    preferred: u16,
    range: RangeInclusive<u16>,
) -> std::io::Result<(TcpListener, u16)> {
    match try_bind(preferred) {
        Ok(listener) => return Ok((listener, preferred)),
        Err(e) if is_addr_in_use(&e) => {
            tracing::warn!(port = preferred, "Preferred port in use, trying fallback");
        }
        Err(e) => return Err(e),
    }

    for port in range {
        if port == preferred {
            continue;
        }
        match try_bind(port) {
            Ok(listener) => {
                tracing::info!(port, "Bound to fallback port");
                return Ok((listener, port));
            }
            Err(e) if is_addr_in_use(&e) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AddrInUse,
        "All ports in fallback range are in use",
    ))
}

fn try_bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
}

fn is_addr_in_use(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::AddrInUse
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_preferred_when_free() {
        // Dùng port cao random để tránh xung đột trong CI.
        let (listener, port) = bind_with_fallback(29901, 29901..=29910).unwrap();
        assert_eq!(port, 29901);
        drop(listener);
    }

    #[test]
    fn falls_back_when_preferred_taken() {
        let _hold = TcpListener::bind((Ipv4Addr::LOCALHOST, 29911)).unwrap();
        let (listener, port) = bind_with_fallback(29911, 29911..=29915).unwrap();
        assert!(port > 29911 && port <= 29915);
        drop(listener);
    }

    #[test]
    fn returns_err_when_all_taken() {
        let _h1 = TcpListener::bind((Ipv4Addr::LOCALHOST, 29921)).unwrap();
        let _h2 = TcpListener::bind((Ipv4Addr::LOCALHOST, 29922)).unwrap();
        let result = bind_with_fallback(29921, 29921..=29922);
        assert!(result.is_err());
    }
}
