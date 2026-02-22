use get_if_addrs::{get_if_addrs, IfAddr};
use std::collections::BTreeSet;
use std::net::Ipv4Addr;

fn is_valid_lan_ipv4(addr: Ipv4Addr) -> bool {
    if addr.is_loopback() {
        return false;
    }
    if addr.is_link_local() {
        return false;
    }
    true
}

fn interface_priority(name: &str) -> usize {
    match name {
        "en0" => 0,
        "en1" => 1,
        "en2" => 2,
        "en3" => 3,
        _ if name.starts_with("en") => 10,
        _ if name.starts_with("bridge") => 20,
        _ => 30,
    }
}

pub fn lan_ipv4_candidates() -> Vec<String> {
    let Ok(addrs) = get_if_addrs() else {
        return Vec::new();
    };

    let mut buckets: Vec<(usize, String)> = Vec::new();
    let mut seen = BTreeSet::new();
    for iface in addrs {
        let IfAddr::V4(v4) = iface.addr else {
            continue;
        };
        if !is_valid_lan_ipv4(v4.ip) {
            continue;
        }
        let ip = v4.ip.to_string();
        if !seen.insert(ip.clone()) {
            continue;
        }
        buckets.push((interface_priority(&iface.name), ip));
    }
    buckets.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    buckets.into_iter().map(|(_, ip)| ip).collect()
}

pub fn recommend_lan_ipv4() -> Option<String> {
    lan_ipv4_candidates().into_iter().next()
}
