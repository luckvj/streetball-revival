//! Refresh the named matches on a server without occupying a seat.
use crate::{
    Message, Transport,
    protocol::{MatchInfo, Reassembly},
};
use std::{net::SocketAddr, time::Duration};

pub struct Browser {
    transport: Box<dyn Transport>,
    server: SocketAddr,
    sent: Option<Duration>,
    id: u32,
    parts: Reassembly,
    matches: Vec<MatchInfo>,
    heard: bool,
}
impl Browser {
    pub fn new(transport: Box<dyn Transport>, server: SocketAddr) -> Self {
        Self {
            transport,
            server,
            sent: None,
            id: 0,
            parts: Reassembly::default(),
            matches: Vec::new(),
            heard: false,
        }
    }
    pub fn update(&mut self, now: Duration) {
        if self
            .sent
            .is_none_or(|t| now.saturating_sub(t) >= Duration::from_secs(1))
        {
            self.id = self.id.wrapping_add(1);
            self.transport
                .send(self.server, &Message::List { id: self.id }.encode());
            self.sent = Some(now);
        }
        while let Some((from, data)) = self.transport.recv() {
            if from != self.server {
                continue;
            }
            let Ok(mut message) = Message::decode(&data) else {
                continue;
            };
            if let Message::Part {
                id,
                index,
                count,
                bytes,
            } = message
            {
                let Some(whole) = self.parts.add(from, now, id, index, count, bytes) else {
                    continue;
                };
                let Ok(m) = Message::decode(&whole) else {
                    continue;
                };
                message = m;
            }
            if let Message::Matches { id, matches } = message
                && id == self.id
            {
                self.matches = matches;
                self.heard = true;
            }
        }
    }
    pub fn matches(&self) -> &[MatchInfo] {
        &self.matches
    }
    pub fn answered(&self) -> bool {
        self.heard
    }
}
