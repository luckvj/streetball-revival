//! Finding matches on the same network without typing anything: a hosting
//! game broadcasts a small beacon once a second on [`LAN_PORT`], and the
//! Online page lists the beacons it hears.

use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    time::Duration,
};

/// The UDP port beacons go to (the match server's port plus one).
pub const LAN_PORT: u16 = crate::DEFAULT_PORT + 1;

const BEACON_MAGIC: &[u8; 6] = b"NBS2LB";

/// What a beacon says about a hosted match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Beacon {
    /// The protocol version the host speaks ([`crate::VERSION`]).
    pub version: u8,
    /// The match server's port on the beacon's sender.
    pub port: u16,
    /// People seated, and how many seats the host opened (2 or 6).
    pub people: u8,
    pub capacity: u8,
    /// The match has started (late joiners still get a free seat).
    pub started: bool,
    /// Who is hosting, and the match's name on the server.
    pub host: String,
    pub match_name: String,
    /// The host's join code, when it has one (empty if not): a joiner who
    /// typed it joins over the network instead.
    pub code: String,
}

impl Beacon {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = BEACON_MAGIC.to_vec();
        out.push(self.version);
        out.extend(self.port.to_le_bytes());
        out.extend([self.people, self.capacity, u8::from(self.started)]);
        for s in [&self.host, &self.match_name, &self.code] {
            let bytes = &s.as_bytes()[..s.len().min(32)];
            out.push(bytes.len() as u8);
            out.extend(bytes);
        }
        out
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        let rest = data.strip_prefix(BEACON_MAGIC)?;
        let (&[version, p0, p1, people, capacity, started], rest) = rest.split_first_chunk()?;
        let mut rest = rest;
        let mut text = || -> Option<String> {
            let (&len, tail) = rest.split_first()?;
            let len = usize::from(len);
            let s = String::from_utf8_lossy(tail.get(..len)?).into_owned();
            rest = &tail[len..];
            Some(s)
        };
        let host = text()?;
        let match_name = text()?;
        let code = text()?;
        Some(Self {
            version,
            port: u16::from_le_bytes([p0, p1]),
            people,
            capacity,
            started: started != 0,
            host,
            match_name,
            code,
        })
    }
}

/// Sends a hosting game's beacon.
pub struct Announcer {
    socket: UdpSocket,
    sent: Option<Duration>,
}

impl Announcer {
    pub fn new() -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, sent: None })
    }

    /// Broadcast `beacon` if a second has passed since the last.
    pub fn update(&mut self, now: Duration, beacon: &Beacon) {
        if self
            .sent
            .is_some_and(|t| now.saturating_sub(t) < Duration::from_secs(1))
        {
            return;
        }
        self.sent = Some(now);
        let to = SocketAddrV4::new(Ipv4Addr::BROADCAST, LAN_PORT);
        let _ = self.socket.send_to(&beacon.encode(), to);
    }
}

/// Listens for beacons and keeps the matches heard lately.
pub struct Finder {
    socket: UdpSocket,
    heard: HashMap<SocketAddr, (Beacon, Duration)>,
}

impl Finder {
    /// Listen on [`LAN_PORT`]. Fails when another game on this machine
    /// already listens there.
    pub fn new() -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, LAN_PORT))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            heard: HashMap::new(),
        })
    }

    /// Read the beacons waiting; forget matches silent for three seconds.
    pub fn update(&mut self, now: Duration) {
        let mut buf = [0u8; 256];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((n, from)) => {
                    if let Some(beacon) = Beacon::decode(&buf[..n]) {
                        let server = SocketAddr::new(from.ip(), beacon.port);
                        self.heard.insert(server, (beacon, now));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(_) => break,
            }
        }
        self.heard
            .retain(|_, (_, at)| now.saturating_sub(*at) < Duration::from_secs(3));
    }

    /// The matches heard, each with its server's address, by host name.
    pub fn matches(&self) -> Vec<(SocketAddr, Beacon)> {
        let mut out: Vec<(SocketAddr, Beacon)> = self
            .heard
            .iter()
            .map(|(addr, (beacon, _))| (*addr, beacon.clone()))
            .collect();
        out.sort_by(|a, b| a.1.host.cmp(&b.1.host).then(a.0.cmp(&b.0)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacons_round_trip_and_reject_noise() {
        let beacon = Beacon {
            version: crate::VERSION,
            port: crate::DEFAULT_PORT,
            people: 1,
            capacity: 6,
            started: false,
            host: "Player".into(),
            match_name: "street".into(),
            code: "7QK3-M2X".into(),
        };
        assert_eq!(Beacon::decode(&beacon.encode()), Some(beacon.clone()));
        assert_eq!(Beacon::decode(b"hello"), None);
        let bytes = beacon.encode();
        assert_eq!(Beacon::decode(&bytes[..bytes.len() - 1]), None);
        // Long names are cut, not refused.
        let long = Beacon {
            host: "x".repeat(100),
            ..beacon
        };
        assert_eq!(Beacon::decode(&long.encode()).unwrap().host.len(), 32);
    }
}
