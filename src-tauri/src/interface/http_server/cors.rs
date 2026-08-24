use axum::http::{HeaderName, HeaderValue, Method};
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};
use url::Url;

pub fn build_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(is_allowed_origin))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
        ])
        .allow_credentials(false)
        // Chromium Private Network Access preflights HTTPS origins before they
        // may call a plain HTTP service on the loopback address.
        .allow_private_network(true)
        .max_age(Duration::from_secs(3600))
}

pub fn is_allowed_origin(origin: &HeaderValue, _req: &axum::http::request::Parts) -> bool {
    let Ok(s) = origin.to_str() else {
        return false;
    };
    check(s)
}

pub fn check(s: &str) -> bool {
    let Ok(url) = Url::parse(s) else {
        return false;
    };
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    host == "mysapo.net" || host.ends_with(".mysapo.net")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_root_domain() {
        assert!(check("https://mysapo.net"));
    }

    #[test]
    fn accepts_subdomain() {
        assert!(check("https://admin.mysapo.net"));
        assert!(check("https://a.b.mysapo.net"));
    }

    #[test]
    fn rejects_http() {
        assert!(!check("http://admin.mysapo.net"));
    }

    #[test]
    fn rejects_typosquat() {
        // Host chuẩn hóa qua URL parse — không có `mysapo.net.evil.com`.
        assert!(!check("https://mysapo.net.evil.com"));
    }

    #[test]
    fn rejects_lookalike() {
        assert!(!check("https://notmysapo.net"));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(!check("not a url"));
        assert!(!check(""));
    }

    #[test]
    fn accepts_with_port() {
        assert!(check("https://admin.mysapo.net:8443"));
    }
}
