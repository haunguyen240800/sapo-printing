//! Bind the local HTTP API to the fixed IPv4 loopback endpoint.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};

pub const DEFAULT_PORT: u16 = 18901;

pub fn bind() -> std::io::Result<TcpListener> {
    TcpListener::bind(default_socket_addr())
}

#[cfg(test)]
fn bind_port(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
}

fn default_socket_addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, DEFAULT_PORT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_to_ipv4_loopback() {
        let listener = bind_port(0).unwrap();
        assert_eq!(listener.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    }

    #[test]
    fn production_endpoint_is_fixed_to_127_0_0_1_18901() {
        assert_eq!(default_socket_addr(), "127.0.0.1:18901".parse().unwrap());
    }

    #[test]
    fn returns_addr_in_use_instead_of_falling_back() {
        let held = bind_port(0).unwrap();
        let occupied_port = held.local_addr().unwrap().port();
        let error = bind_port(occupied_port).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
    }
}
