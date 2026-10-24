//! Moving datagrams: the [`Transport`] trait, real UDP ([`UdpTransport`]),
//! and an in-memory network with loss, delay and jitter ([`MemoryNetwork`])
//! for tests and local experiments.

use std::collections::VecDeque;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A datagram socket. Sends are best effort; `recv` never blocks.
pub trait Transport: Send + Sync {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]);
    /// The next datagram waiting, if any.
    fn recv(&mut self) -> Option<(SocketAddr, Vec<u8>)>;
}

/// What each UDP socket asks for as its send and receive buffers.
const SOCKET_BUFFER: usize = 4 << 20;

/// A non-blocking UDP socket.
pub struct UdpTransport {
    socket: UdpSocket,
    buf: Vec<u8>,
}

impl UdpTransport {
    /// Bind to `addr` (`"0.0.0.0:7375"`, or port 0 for any).
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        // A match setup goes out as a burst of a hundred or more parts;
        // the system's default buffers (64 KiB on Windows) drop most of
        // them, so a join with a setup never arrived. Best effort.
        let s = socket2::SockRef::from(&socket);
        let _ = s.set_recv_buffer_size(SOCKET_BUFFER);
        let _ = s.set_send_buffer_size(SOCKET_BUFFER);
        Ok(Self {
            socket,
            buf: vec![0; 65536],
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl Transport for UdpTransport {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]) {
        // Lost like any datagram if the socket is full or the host unreachable.
        let _ = self.socket.send_to(bytes, to);
    }

    fn recv(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        loop {
            match self.socket.recv_from(&mut self.buf) {
                Ok((n, from)) => return Some((from, self.buf[..n].to_vec())),
                // Windows reports an earlier send's ICMP "port unreachable"
                // as an error on a later receive; skip it.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(_) => return None,
            }
        }
    }
}

/// How the in-memory network misbehaves.
#[derive(Clone, Copy, Debug)]
pub struct Conditions {
    /// Fraction of datagrams dropped, 0..1.
    pub loss: f32,
    /// Fraction delivered twice.
    pub duplicate: f32,
    /// One-way delay.
    pub delay: Duration,
    /// Extra delay, uniform in 0..jitter (so datagrams overtake each other).
    pub jitter: Duration,
}

impl Default for Conditions {
    fn default() -> Self {
        Self {
            loss: 0.0,
            duplicate: 0.0,
            delay: Duration::ZERO,
            jitter: Duration::ZERO,
        }
    }
}

struct InFlight {
    at: Duration,
    from: SocketAddr,
    to: SocketAddr,
    bytes: Vec<u8>,
}

struct Net {
    now: Duration,
    rng: u32,
    conditions: Conditions,
    flying: VecDeque<InFlight>,
    sent: u64,
    dropped: u64,
}

impl Net {
    fn unit(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// A shared in-memory network on a clock the caller moves
/// ([`MemoryNetwork::set_time`]), so tests are exact and fast.
#[derive(Clone)]
pub struct MemoryNetwork(Arc<Mutex<Net>>);

impl MemoryNetwork {
    pub fn new(conditions: Conditions, seed: u32) -> Self {
        Self(Arc::new(Mutex::new(Net {
            now: Duration::ZERO,
            rng: seed.max(1),
            conditions,
            flying: VecDeque::new(),
            sent: 0,
            dropped: 0,
        })))
    }

    /// A socket at `addr`.
    pub fn endpoint(&self, addr: SocketAddr) -> MemoryTransport {
        MemoryTransport {
            net: self.clone(),
            addr,
        }
    }

    /// Move the network's clock: datagrams due by `now` become receivable.
    pub fn set_time(&self, now: Duration) {
        self.0.lock().expect("network").now = now;
    }

    pub fn set_conditions(&self, conditions: Conditions) {
        self.0.lock().expect("network").conditions = conditions;
    }

    /// Datagrams sent and dropped so far.
    pub fn stats(&self) -> (u64, u64) {
        let n = self.0.lock().expect("network");
        (n.sent, n.dropped)
    }
}

/// One address on a [`MemoryNetwork`].
pub struct MemoryTransport {
    net: MemoryNetwork,
    addr: SocketAddr,
}

impl Transport for MemoryTransport {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]) {
        let mut n = self.net.0.lock().expect("network");
        n.sent += 1;
        let c = n.conditions;
        if n.unit() < c.loss {
            n.dropped += 1;
            return;
        }
        let copies = if n.unit() < c.duplicate { 2 } else { 1 };
        for _ in 0..copies {
            let jitter = c.jitter.mul_f32(n.unit());
            let at = n.now + c.delay + jitter;
            n.flying.push_back(InFlight {
                at,
                from: self.addr,
                to,
                bytes: bytes.to_vec(),
            });
        }
    }

    fn recv(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        let mut n = self.net.0.lock().expect("network");
        let now = n.now;
        // The earliest due datagram for this address.
        let k = n
            .flying
            .iter()
            .enumerate()
            .filter(|(_, d)| d.to == self.addr && d.at <= now)
            .min_by_key(|(_, d)| d.at)
            .map(|(k, _)| k)?;
        let d = n.flying.remove(k).expect("index");
        Some((d.from, d.bytes))
    }
}
