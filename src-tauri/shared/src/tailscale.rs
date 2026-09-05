//! Tailscale interface discovery.
//!
//! Tailscale creates a virtual network interface on macOS (typically named
//! `utun3` or similar) and assigns the host an IPv4 address inside the
//! 100.64.0.0/10 CGNAT range — the same range every node on the user's
//! tailnet pulls addresses from. This is the only interface the user's
//! iPhone can reach the Mac over from outside the LAN.
//!
//! We bind the Trace remote-access HTTP server explicitly to that address
//! (rather than 0.0.0.0) so the socket is invisible to anyone on the local
//! Wi-Fi / Ethernet network. Tailscale's WireGuard tunnel handles encryption
//! end-to-end; we only need to know *where* to listen.
//!
//! If Tailscale isn't running, `tailscale_ipv4()` returns `None` and the
//! caller should refuse to start the server rather than silently falling
//! through to a less-private bind.

use std::net::Ipv4Addr;

/// Return the IPv4 address assigned to this host by Tailscale, if any.
///
/// We don't shell out to the `tailscale` CLI — we just walk the OS's interface
/// list (via `if-addrs`) and pick the first IPv4 address inside the CGNAT
/// range `100.64.0.0/10`. That's the same algorithm Tailscale itself uses to
/// reserve addresses, so any 100.64.x.x – 100.127.x.x address on a local
/// interface is, in practice, a tailnet address.
pub fn tailscale_ipv4() -> Option<Ipv4Addr> {
    let interfaces = if_addrs::get_if_addrs().ok()?;
    for iface in interfaces {
        if iface.is_loopback() {
            continue;
        }
        if let if_addrs::IfAddr::V4(v4) = iface.addr {
            if is_cgnat(v4.ip) {
                return Some(v4.ip);
            }
        }
    }
    None
}

/// True if `ip` is inside 100.64.0.0/10 — the carrier-grade NAT block Tailscale
/// uses for tailnet addresses. The /10 means the top 10 bits are fixed:
/// the first octet is 100, and the second octet's top 2 bits are `01`,
/// giving the range 100.64.0.0 – 100.127.255.255.
fn is_cgnat(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (octets[1] & 0b1100_0000) == 0b0100_0000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cgnat_lower_bound_is_inside() {
        assert!(is_cgnat(Ipv4Addr::new(100, 64, 0, 0)));
    }

    #[test]
    fn cgnat_upper_bound_is_inside() {
        assert!(is_cgnat(Ipv4Addr::new(100, 127, 255, 255)));
    }

    #[test]
    fn typical_tailscale_address_is_inside() {
        assert!(is_cgnat(Ipv4Addr::new(100, 96, 42, 17)));
    }

    #[test]
    fn private_lan_is_outside() {
        assert!(!is_cgnat(Ipv4Addr::new(192, 168, 1, 5)));
        assert!(!is_cgnat(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(!is_cgnat(Ipv4Addr::new(172, 16, 0, 1)));
    }

    #[test]
    fn ip_just_below_cgnat_is_outside() {
        // 100.63.x.x is in 100.0.0.0/10 but not 100.64.0.0/10.
        assert!(!is_cgnat(Ipv4Addr::new(100, 63, 255, 255)));
    }

    #[test]
    fn ip_just_above_cgnat_is_outside() {
        // 100.128.x.x falls outside the /10.
        assert!(!is_cgnat(Ipv4Addr::new(100, 128, 0, 0)));
    }

    #[test]
    fn public_ip_with_100_first_octet_is_outside() {
        // 100.1.2.3 has the right first octet but is a real public address,
        // not CGNAT — second-octet check must catch it.
        assert!(!is_cgnat(Ipv4Addr::new(100, 1, 2, 3)));
    }
}
