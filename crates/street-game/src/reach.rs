//! Letting friends on the internet reach a match hosted here, without
//! anyone touching their router: the hosting game asks the router to open
//! its port (UPnP), reads the address the internet sees, and shows it as a
//! join code (`street_net::code`). The router's answer takes a moment, so
//! the asking runs on a thread.
//!
//! When the router says no (UPnP off or missing), a STUN query still finds
//! the public address, so a host who has forwarded UDP 7375 by hand gets a
//! code as well. A carrier-shared address (CGNAT) can't be reached at all;
//! the lobby says so.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        Mutex,
        mpsc::{Receiver, channel},
    },
    time::Duration,
};

use igd_next::{PortMappingProtocol, SearchOptions};
use street_net::code;

/// What the router and the internet said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The router opened the port: friends anywhere can join with the code.
    Open { public: SocketAddrV4 },
    /// The router would not open it; the public address, if found, works
    /// only if the port is forwarded by hand.
    Closed { public: Option<SocketAddrV4> },
    /// The internet provider shares the address with others (CGNAT):
    /// nobody outside can reach this machine.
    Shared,
}

impl Outcome {
    /// The join code, when there is an address to give out.
    pub fn code(&self) -> Option<String> {
        match self {
            Outcome::Open { public }
            | Outcome::Closed {
                public: Some(public),
            } => Some(code::encode(*public)),
            _ => None,
        }
    }
}

/// The router conversation for a hosted port.
pub struct Reach {
    rx: Mutex<Receiver<Outcome>>,
    pub outcome: Option<Outcome>,
    port: u16,
}

impl Reach {
    /// Ask the router to open UDP `port` to `local` (this machine's address
    /// on its network).
    pub fn start(port: u16, local: Option<IpAddr>) -> Self {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(ask(port, local));
        });
        Self {
            rx: Mutex::new(rx),
            outcome: None,
            port,
        }
    }

    /// Take the answer once it has come.
    pub fn poll(&mut self) -> Option<&Outcome> {
        if self.outcome.is_none()
            && let Ok(outcome) = self
                .rx
                .get_mut()
                .map_err(|_| ())
                .and_then(|rx| rx.try_recv().map_err(|_| ()))
        {
            self.outcome = Some(outcome);
        }
        self.outcome.as_ref()
    }

    /// Close the port again, on a thread (the router may be slow); the
    /// handle lets a closing game wait for it.
    pub fn close(&self) -> Option<std::thread::JoinHandle<()>> {
        if !matches!(self.outcome, Some(Outcome::Open { .. })) {
            return None;
        }
        let port = self.port;
        Some(std::thread::spawn(move || {
            if let Ok(gateway) = igd_next::search_gateway(options()) {
                let _ = gateway.remove_port(PortMappingProtocol::UDP, port);
            }
        }))
    }
}

fn options() -> SearchOptions {
    let mut options = SearchOptions::default();
    options.timeout = Some(Duration::from_secs(3));
    options
}

/// The whole conversation: UPnP first, STUN when the router won't help.
fn ask(port: u16, local: Option<IpAddr>) -> Outcome {
    if let Some(IpAddr::V4(local)) = local
        && let Ok(gateway) = igd_next::search_gateway(options())
    {
        let external = match gateway.get_external_ip() {
            Ok(IpAddr::V4(ip)) => Some(ip),
            _ => None,
        };
        if external.is_some_and(|ip| !code::is_public(ip)) {
            // The router's own address is a carrier's or a second
            // router's: opening its port doesn't reach the internet.
            return match stun() {
                Some(public) if external != Some(*public.ip()) => Outcome::Shared,
                public => Outcome::Closed { public },
            };
        }
        let to = SocketAddr::V4(SocketAddrV4::new(local, port));
        // Two hours, so a game that never closes it (a crash) doesn't leave
        // the router open; some routers only take 0 (until removed).
        let description = "Streetball Revival online";
        let opened = [7200, 0].into_iter().any(|lease| {
            gateway
                .add_port(PortMappingProtocol::UDP, port, to, lease, description)
                .is_ok()
        });
        if opened && let Some(ip) = external {
            return Outcome::Open {
                public: SocketAddrV4::new(ip, port),
            };
        }
    }
    Outcome::Closed {
        public: stun().map(|a| SocketAddrV4::new(*a.ip(), port)),
    }
}

/// This machine's public address as a STUN server sees it (RFC 5389
/// binding request; the XOR-MAPPED-ADDRESS of the answer).
fn stun() -> Option<SocketAddrV4> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket
        .set_read_timeout(Some(Duration::from_millis(1500)))
        .ok()?;
    let server = std::net::ToSocketAddrs::to_socket_addrs("stun.l.google.com:19302")
        .ok()?
        .find(SocketAddr::is_ipv4)?;
    let mut request = [0u8; 20];
    request[1] = 0x01; // binding request
    request[4..8].copy_from_slice(&0x2112_a442u32.to_be_bytes());
    let id: [u8; 12] = std::array::from_fn(|k| (k as u8).wrapping_mul(37) ^ 0x5a);
    request[8..20].copy_from_slice(&id);
    for _ in 0..2 {
        socket.send_to(&request, server).ok()?;
        let mut buf = [0u8; 512];
        let Ok((n, _)) = socket.recv_from(&mut buf) else {
            continue;
        };
        return parse_stun(&buf[..n], &id);
    }
    None
}

fn parse_stun(data: &[u8], id: &[u8; 12]) -> Option<SocketAddrV4> {
    if data.len() < 20 || data[0..2] != [0x01, 0x01] || &data[8..20] != id {
        return None;
    }
    let mut at = 20;
    while at + 4 <= data.len() {
        let kind = u16::from_be_bytes([data[at], data[at + 1]]);
        let len = usize::from(u16::from_be_bytes([data[at + 2], data[at + 3]]));
        let value = data.get(at + 4..at + 4 + len)?;
        // XOR-MAPPED-ADDRESS, IPv4.
        if kind == 0x0020 && len >= 8 && value[1] == 0x01 {
            let port = u16::from_be_bytes([value[2], value[3]]) ^ 0x2112;
            let ip = u32::from_be_bytes([value[4], value[5], value[6], value[7]]) ^ 0x2112_a442;
            return Some(SocketAddrV4::new(Ipv4Addr::from(ip), port));
        }
        at += 4 + len.div_ceil(4) * 4;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stun_answer_gives_the_public_address() {
        let id = [7u8; 12];
        let mut answer = vec![0x01, 0x01, 0x00, 0x0c, 0x21, 0x12, 0xa4, 0x42];
        answer.extend(id);
        // XOR-MAPPED-ADDRESS 203.0.113.5:7375.
        let port = 7375u16 ^ 0x2112;
        let ip = u32::from(Ipv4Addr::new(203, 0, 113, 5)) ^ 0x2112_a442;
        answer.extend([0x00, 0x20, 0x00, 0x08, 0x00, 0x01]);
        answer.extend(port.to_be_bytes());
        answer.extend(ip.to_be_bytes());
        assert_eq!(
            parse_stun(&answer, &id),
            Some(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 5), 7375))
        );
        assert_eq!(parse_stun(&answer, &[0; 12]), None);
    }
}
