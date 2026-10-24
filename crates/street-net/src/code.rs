//! Join codes: a host's public address as a few letters to read out or
//! paste, instead of an IP address and port.
//!
//! The code is the IPv4 address (and the port, when it is not
//! [`DEFAULT_PORT`]) in Crockford's base 32: seven characters for the
//! default port (`7QK3-M2X`), ten for another. Reading one back ignores
//! case, spaces and dashes, and takes `O` for `0` and `I`/`L` for `1`.

use std::net::{Ipv4Addr, SocketAddrV4};

use crate::DEFAULT_PORT;

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The code for `addr`.
pub fn encode(addr: SocketAddrV4) -> String {
    let ip = u64::from(u32::from(*addr.ip()));
    let (value, chars) = if addr.port() == DEFAULT_PORT {
        (ip, 7)
    } else {
        ((ip << 16) | u64::from(addr.port()), 10)
    };
    let mut out: Vec<u8> = (0..chars)
        .rev()
        .map(|k| ALPHABET[((value >> (5 * k)) & 31) as usize])
        .collect();
    out.insert(4, b'-');
    String::from_utf8(out).expect("ascii")
}

/// The address a code stands for, or `None` if `text` is not a code.
pub fn decode(text: &str) -> Option<SocketAddrV4> {
    let mut value: u64 = 0;
    let mut chars = 0;
    for c in text.chars() {
        let c = match c.to_ascii_uppercase() {
            '-' | ' ' => continue,
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        let digit = ALPHABET.iter().position(|&a| char::from(a) == c)?;
        value = (value << 5) | digit as u64;
        chars += 1;
        if chars > 10 {
            return None;
        }
    }
    let (ip, port) = match chars {
        7 if value >> 32 == 0 => (value as u32, DEFAULT_PORT),
        10 if value >> 48 == 0 => ((value >> 16) as u32, (value & 0xffff) as u16),
        _ => return None,
    };
    let ip = Ipv4Addr::from(ip);
    (!ip.is_unspecified() && port != 0).then_some(SocketAddrV4::new(ip, port))
}

/// Whether `ip` is reachable from the internet (not a home network's,
/// a carrier's shared range, loopback or link-local).
pub fn is_public(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || (a == 100 && (64..128).contains(&b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_forgive_typing() {
        let home = SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 5), DEFAULT_PORT);
        let code = encode(home);
        assert_eq!(code.len(), 8);
        assert_eq!(&code[4..5], "-");
        assert_eq!(decode(&code), Some(home));
        assert_eq!(decode(&code.to_lowercase().replace('-', " ")), Some(home));
        let other = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 77), 40000);
        let code = encode(other);
        assert_eq!(code.len(), 11);
        assert_eq!(decode(&code), Some(other));
        // O for 0, I and L for 1.
        let zeros = encode(SocketAddrV4::new(Ipv4Addr::new(1, 0, 0, 1), DEFAULT_PORT));
        assert_eq!(
            decode(&zeros.replace('0', "O").replace('1', "l")),
            decode(&zeros)
        );
        // Not codes: addresses, wrong lengths, other letters.
        assert_eq!(decode("192.168.1.20"), None);
        assert_eq!(decode("ABCD-EF"), None);
        assert_eq!(decode("UUUU-UUU"), None);
    }

    #[test]
    fn home_and_carrier_addresses_are_not_public() {
        assert!(is_public(Ipv4Addr::new(203, 0, 113, 5)));
        assert!(!is_public(Ipv4Addr::new(192, 168, 1, 20)));
        assert!(!is_public(Ipv4Addr::new(10, 0, 0, 2)));
        assert!(!is_public(Ipv4Addr::new(100, 72, 1, 1)));
        assert!(!is_public(Ipv4Addr::new(127, 0, 0, 1)));
    }
}
