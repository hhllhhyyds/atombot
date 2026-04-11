//! Network security utilities — SSRF protection and internal URL detection.

use std::net::{IpAddr, ToSocketAddrs};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Only http/https allowed, got '{0}'")]
    InvalidScheme(String),
    #[error("Missing domain")]
    MissingDomain,
    #[error("Missing hostname")]
    MissingHostname,
    #[error("Cannot resolve hostname: {0}")]
    UnresolvableHostname(String),
    #[error("Blocked: {0} resolves to private/internal address {1}")]
    PrivateAddress(String, String),
}

/// Checks if the given IP address falls within a blocked private/internal network.
fn is_blocked_private(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 0.0.0.0/8
            if octets[0] == 0 {
                return true;
            }
            // 10.0.0.0/8
            if octets[0] == 10 {
                return true;
            }
            // 100.64.0.0/10 (carrier-grade NAT)
            if octets[0] == 100 && (octets[1] & 0b11000000) == 0b01000000 {
                return true;
            }
            // 127.0.0.0/8
            if octets[0] == 127 {
                return true;
            }
            // 169.254.0.0/16 (link-local)
            if octets[0] == 169 && octets[1] == 254 {
                return true;
            }
            // 172.16.0.0/12
            if octets[0] == 172 && (octets[1] & 0b11110000) == 0b00010000 {
                return true;
            }
            // 192.168.0.0/16
            if octets[0] == 192 && octets[1] == 168 {
                return true;
            }
            false
        }
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            // ::1/128 (loopback)
            if ipv6.is_loopback() {
                return true;
            }
            // fc00::/7 (unique local)
            let first = segments[0];
            if (first & 0xfe00) == 0xfc00 {
                return true;
            }
            // fe80::/10 (link-local)
            if (first & 0xffc0) == 0xfe80 {
                return true;
            }
            false
        }
    }
}

/// Validate a URL is safe to fetch: scheme, hostname, and resolved IPs.
///
/// Returns `Ok(())` if safe, or `Err(NetworkError)` with details.
pub fn validate_url_target(url: &str) -> Result<(), NetworkError> {
    let parsed = url::Url::parse(url).map_err(|_e| NetworkError::MissingDomain)?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(NetworkError::InvalidScheme(scheme.to_string()));
    }

    let hostname = parsed.host_str().ok_or(NetworkError::MissingHostname)?;

    // Resolve hostname to IP addresses
    let addrs: std::vec::Vec<std::net::SocketAddr> = (hostname, 80)
        .to_socket_addrs()
        .map_err(|_| NetworkError::UnresolvableHostname(hostname.to_string()))?
        .collect();

    for addr in addrs {
        if is_blocked_private(&addr.ip()) {
            return Err(NetworkError::PrivateAddress(
                hostname.to_string(),
                addr.ip().to_string(),
            ));
        }
    }

    Ok(())
}

/// Validate an already-fetched URL (after redirect). Only checks the resolved IP.
pub fn validate_resolved_url(url: &str) -> Result<(), NetworkError> {
    let parsed = match url::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return Ok(()), // Can't parse, skip validation
    };

    let hostname = match parsed.host_str() {
        Some(h) => h,
        None => return Ok(()), // No hostname, skip
    };

    // Check if the host itself is an IP address
    if let Ok(ip) = hostname.parse::<IpAddr>() {
        if is_blocked_private(&ip) {
            return Err(NetworkError::PrivateAddress(
                hostname.to_string(),
                ip.to_string(),
            ));
        }
        return Ok(());
    }

    // Otherwise resolve the domain
    let addrs: std::vec::Vec<std::net::SocketAddr> = match (hostname, 80).to_socket_addrs() {
        Ok(addrs) => addrs.collect(),
        Err(_) => return Ok(()), // Can't resolve, skip
    };

    for addr in addrs {
        if is_blocked_private(&addr.ip()) {
            return Err(NetworkError::PrivateAddress(
                hostname.to_string(),
                addr.ip().to_string(),
            ));
        }
    }

    Ok(())
}

/// Check if a string contains any URLs that point to internal/private addresses.
pub fn contains_internal_url(text: &str) -> bool {
    let url_regex = regex::Regex::new(r#"https?://[^\s"'`;|<>]+"#).unwrap();
    for cap in url_regex.find_iter(text) {
        let url = cap.as_str();
        if validate_url_target(url).is_err() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocks_private_ips() {
        assert!(validate_url_target("http://127.0.0.1/").is_err());
        assert!(validate_url_target("http://localhost/").is_err());
        assert!(validate_url_target("http://10.0.0.1/").is_err());
        assert!(validate_url_target("http://192.168.1.1/").is_err());
        assert!(validate_url_target("http://172.16.0.1/").is_err());
    }

    #[test]
    fn test_allows_public_urls() {
        assert!(validate_url_target("https://example.com/").is_ok());
        assert!(validate_url_target("https://www.google.com/").is_ok());
    }

    #[test]
    fn test_rejects_invalid_scheme() {
        assert!(validate_url_target("ftp://example.com/").is_err());
        assert!(validate_url_target("file:///etc/passwd").is_err());
    }
}
