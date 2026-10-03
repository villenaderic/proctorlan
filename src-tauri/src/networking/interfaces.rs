//! Lists usable IPv4 interfaces and picks a LAN address to bind. We never bind to 0.0.0.0 by
//! default so the server is not exposed on public/VPN interfaces unnecessarily.

use std::net::{IpAddr, Ipv4Addr};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetInterface {
    pub name: String,
    pub ip: String,
    /// RFC1918 or link-local: a normal classroom LAN address.
    pub is_private: bool,
}

pub fn is_private_lan(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

/// Pure so it can be tested without touching real network cards.
pub fn filter_candidates(raw: Vec<(String, IpAddr)>) -> Vec<NetInterface> {
    let mut out: Vec<NetInterface> = raw
        .into_iter()
        .filter_map(|(name, ip)| match ip {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() && !v4.is_multicast() => {
                Some(NetInterface { name, ip: v4.to_string(), is_private: is_private_lan(v4) })
            }
            _ => None,
        })
        .collect();
    // private LAN addresses first, then stable by name
    out.sort_by(|a, b| b.is_private.cmp(&a.is_private).then(a.name.cmp(&b.name)));
    out.dedup_by(|a, b| a.ip == b.ip);
    out
}

pub fn list_interfaces() -> Vec<NetInterface> {
    let raw = if_addrs::get_if_addrs()
        .map(|v| v.into_iter().map(|i| (i.name.clone(), i.ip())).collect())
        .unwrap_or_default();
    filter_candidates(raw)
}

/// Honour the teacher's choice if that address still exists; otherwise the first private LAN
/// address; otherwise loopback (server still works for same-machine testing).
pub fn choose_bind_ip(interfaces: &[NetInterface], preferred: Option<&str>) -> Ipv4Addr {
    preferred
        .and_then(|p| interfaces.iter().find(|i| i.ip == p))
        .or_else(|| interfaces.iter().find(|i| i.is_private))
        .and_then(|i| i.ip.parse().ok())
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(items: &[(&str, &str)]) -> Vec<(String, IpAddr)> {
        items.iter().map(|(n, ip)| (n.to_string(), ip.parse().unwrap())).collect()
    }

    #[test]
    fn filters_loopback_ipv6_and_unspecified() {
        let r = filter_candidates(raw(&[("lo", "127.0.0.1"), ("eth0", "192.168.1.100"), ("eth0", "fe80::1"), ("x", "0.0.0.0")]));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].ip, "192.168.1.100");
    }

    #[test]
    fn private_ranges_are_recognised() {
        for ip in ["10.1.2.3", "172.16.0.9", "172.31.255.1", "192.168.0.5", "169.254.3.4"] {
            assert!(is_private_lan(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "172.32.0.1", "100.64.0.1"] {
            assert!(!is_private_lan(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn private_addresses_sort_before_public() {
        let r = filter_candidates(raw(&[("wan", "203.0.113.9"), ("wifi", "192.168.1.5")]));
        assert_eq!(r[0].ip, "192.168.1.5");
        assert!(!r[1].is_private);
    }

    #[test]
    fn bind_choice_prefers_selection_then_private_then_loopback() {
        let ifs = filter_candidates(raw(&[("wan", "203.0.113.9"), ("wifi", "192.168.1.5"), ("eth", "10.0.0.7")]));
        assert_eq!(choose_bind_ip(&ifs, Some("10.0.0.7")).to_string(), "10.0.0.7");
        assert_eq!(choose_bind_ip(&ifs, Some("1.2.3.4")).to_string(), "10.0.0.7", "stale choice falls back (eth sorts first)");
        assert_eq!(choose_bind_ip(&ifs, None).to_string(), "10.0.0.7");
        assert_eq!(choose_bind_ip(&[], None), Ipv4Addr::LOCALHOST);
        let only_public = filter_candidates(raw(&[("wan", "203.0.113.9")]));
        assert_eq!(choose_bind_ip(&only_public, None), Ipv4Addr::LOCALHOST, "never auto-bind to a public address");
    }

    #[test]
    fn duplicates_are_removed() {
        assert_eq!(filter_candidates(raw(&[("a", "10.0.0.1"), ("a", "10.0.0.1")])).len(), 1);
    }
}
