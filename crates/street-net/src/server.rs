//! The match server: a lobby of named matches, each run as the referee.
//!
//! The referee holds the disc: a [`Server`] is made with this machine's
//! [`Disc`] and plays every match on its tables, installed under each
//! match's own values ([`Disc::install`]), as each client plays it on its
//! own disc's. A joiner whose disc data differs (its digest) is refused.
//!
//! A match is created by the first join that names it, with the joiner's
//! setup; a join with none for a match that isn't there is refused (the
//! server makes no match of its own: a match needs the line-up's records
//! and looks, which a game builds from its disc). Joiners get the lowest
//! free seat on the side they ask for. The match starts once everyone seated is ready; people
//! may join later and take a free seat, and leaving hands a seat back to the
//! computer.
//!
//! Before the start the match is a lobby: people move to the other side's
//! free seats ([`Message::Side`]), show what they have picked
//! ([`Message::Pick`], passed on in the lobby's list), and the creator
//! replaces the setup with the match as everyone picked it
//! ([`Message::Setup`]), checked as a creation is. Each change welcomes
//! the people it concerns again; a new setup un-readies everyone.
//!
//! Each tick the server fixes every occupied seat's pad: the pad the client
//! sent for that tick, or, if it has not arrived, the seat's pad from the
//! tick before. It steps its own [`Match`] with the frame, keeps the frame
//! and the checksum, and sends every client the frames it lacks. Clients
//! report checksums of their confirmed states; a client whose checksum
//! differs, or who has fallen behind the frames kept, gets the whole state.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use street_sim::{Input, Phase};

use crate::disc::Disc;
use crate::game::{Frame, MAX_SEATS, Match};
use crate::protocol::{
    CloseReason, Join, MAX_DATAGRAM, MAX_RUN, MatchSetup, Message, Reassembly, Refusal, SeatEntry,
};
use crate::transport::Transport;
use crate::wire::DecodeError;

/// Seats for `join`'s people, `taken` being seated already: each the
/// lowest free seat on the side asked for, or on the side with fewer
/// people, with at most `per_side` people a side.
fn assign_seats(
    seat_teams: &[u8; MAX_SEATS],
    taken: &[u8],
    join: &Join,
    per_side: usize,
) -> Option<Vec<u8>> {
    let mut taken = taken.to_vec();
    let mut seats = Vec::new();
    let count = |team: u8, taken: &[u8]| {
        taken
            .iter()
            .filter(|&&s| seat_teams[usize::from(s)] == team)
            .count()
    };
    for p in &join.players {
        let team = p.team.unwrap_or_else(|| {
            if count(1, &taken) < count(0, &taken) {
                1
            } else {
                0
            }
        });
        if count(team, &taken) >= per_side {
            return None;
        }
        let free = (0..MAX_SEATS as u8)
            .find(|&s| seat_teams[usize::from(s)] == team && !taken.contains(&s))?;
        taken.push(free);
        seats.push(free);
    }
    Some(seats)
}

/// Tuning for a [`Server`].
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub max_matches: usize,
    /// A client silent this long loses its seats.
    pub timeout: Duration,
    /// Frames (and checksums) kept for resending, in ticks.
    pub history: usize,
    /// A finished match stays up this long after its won game's replay
    /// (long enough for the post-game pages and a rematch vote; a match
    /// nobody is on any more still closes on the timeout).
    pub linger: Duration,
    /// People a side may seat (1 for one on one, up to 3); the computer
    /// plays the other seats.
    pub seats_per_side: usize,
    /// What the server checks rosters with ([`crate::roster::check_setup`]):
    /// the disc's own tables, when it has the disc (a match's roster is then
    /// rebuilt and checked whole; without, what the roster data and the
    /// line-up show is checked), the roster mods it accepts, and whether it
    /// trusts a mod it lacks.
    pub rosters: std::sync::Arc<crate::roster::Known>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_matches: 64,
            seats_per_side: 3,
            timeout: Duration::from_secs(5),
            history: 600,
            linger: Duration::from_secs(120),
            rosters: Default::default(),
        }
    }
}

/// Something that happened, for logs and tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerEvent {
    Created {
        name: String,
    },
    Joined {
        name: String,
        addr: SocketAddr,
        seats: Vec<u8>,
    },
    Left {
        name: String,
        addr: SocketAddr,
        timed_out: bool,
    },
    Started {
        name: String,
    },
    /// The creator replaced the match's setup in the lobby.
    Reconfigured {
        name: String,
    },
    /// A client was sent the whole state at `tick`; `drifted` when its
    /// checksum differed (else it had fallen behind or just joined).
    Resynced {
        name: String,
        addr: SocketAddr,
        tick: u64,
        drifted: bool,
    },
    Closed {
        name: String,
    },
    /// One referee faulted; other matches on this server continue.
    Faulted {
        name: String,
        reason: String,
    },
    /// A join was turned down: the joiner's disc data differs from the
    /// server's, or the match it would create breaks the online rules, its
    /// world does not hold its roster (the anti-cheat), or the disc's data
    /// doesn't fit it.
    Refused {
        name: String,
        addr: SocketAddr,
        reasons: Vec<String>,
    },
}

/// Ticks per lead measuring window.
const LEAD_WINDOW: u64 = 20;
/// Most ticks run in one update when the server falls behind real time.
const MAX_CATCH_UP: u64 = 10;
/// Gap between full states sent to one client.
const STATE_GAP: Duration = Duration::from_millis(300);

/// One client in a match.
struct Remote {
    addr: SocketAddr,
    seats: Vec<u8>,
    names: Vec<String>,
    ready: bool,
    rematch: bool,
    restart_ack: bool,
    restart_sent_at: Option<Duration>,
    heard_at: Duration,
    /// The first confirmed frame the client lacks, by its last message.
    need: u64,
    /// Pads received for ticks not fixed yet, one per seat.
    pads: BTreeMap<u64, Vec<Input>>,
    /// The first tick whose pads are still wanted.
    ack: u64,
    /// The newest tick a pad arrived for.
    heard: u64,
    /// The worst lead this window and the last.
    lead_now: i16,
    lead_before: i16,
    /// Checksum reports before this tick predate the last full state.
    resync_floor: u64,
    state_sent_at: Option<Duration>,
    /// Pads that missed their tick.
    late: u64,
    /// What the machine's people have picked in the lobby.
    pick: Vec<u8>,
}

/// One match: its seats, its clients, and the referee's copy of the game.
pub struct Host {
    name: String,
    round: u32,
    setup: MatchSetup,
    /// The setup's changes in the lobby ([`Message::Setup`]).
    setup_version: u32,
    /// The machine that created the match, the one that may replace its
    /// setup.
    creator: Option<SocketAddr>,
    state: Match,
    remotes: Vec<Remote>,
    started_at: Option<Duration>,
    /// Ticks run since the start, against the clock.
    ticks_run: u64,
    history: VecDeque<Frame>,
    history_start: u64,
    /// `sums[k]` is the checksum of the state at the start of tick
    /// `sums_start + k`.
    sums: VecDeque<u64>,
    sums_start: u64,
    /// Each seat's last fixed pad, repeated when a pad is late.
    last: [Option<Input>; MAX_SEATS],
    lobby_dirty: bool,
    lobby_sent_at: Option<Duration>,
    over_at: Option<Duration>,
    empty_since: Option<Duration>,
}

impl Host {
    /// A match on `setup`, its world with this server's disc's tables
    /// installed; refused if they don't fit it.
    fn new(name: String, mut setup: MatchSetup, disc: &Disc) -> Result<Self, String> {
        disc.install(&mut setup.world)?;
        let state = Match::new(setup.world.clone(), setup.seat_teams);
        let first_sum = state.checksum();
        let tick = state.tick();
        Ok(Self {
            name,
            round: 0,
            setup,
            setup_version: 0,
            creator: None,
            state,
            remotes: Vec::new(),
            started_at: None,
            ticks_run: 0,
            history: VecDeque::new(),
            history_start: tick,
            sums: VecDeque::from([first_sum]),
            sums_start: tick,
            last: [None; MAX_SEATS],
            lobby_dirty: true,
            lobby_sent_at: None,
            over_at: None,
            empty_since: None,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The referee's match.
    pub fn state(&self) -> &Match {
        &self.state
    }

    /// The next tick to fix.
    pub fn tick(&self) -> u64 {
        self.state.tick()
    }

    pub fn started(&self) -> bool {
        self.started_at.is_some()
    }

    /// The checksum of the state at the start of `tick`, if still kept.
    pub fn checksum_at(&self, tick: u64) -> Option<u64> {
        let k = tick.checked_sub(self.sums_start)?;
        self.sums.get(k as usize).copied()
    }

    /// Pads that missed their tick, over all clients.
    pub fn late_pads(&self) -> u64 {
        self.remotes.iter().map(|r| r.late).sum()
    }

    /// The occupied seats.
    pub fn seats(&self) -> Vec<SeatEntry> {
        let mut seats: Vec<SeatEntry> = self
            .remotes
            .iter()
            .flat_map(|r| {
                r.seats.iter().zip(&r.names).map(|(&seat, name)| SeatEntry {
                    seat,
                    team: self.setup.seat_teams[usize::from(seat)],
                    name: name.clone(),
                    ready: r.ready,
                    pick: r.pick.clone(),
                })
            })
            .collect();
        seats.sort_by_key(|s| s.seat);
        seats
    }

    /// Move the machine at `addr`'s people to `team`'s free seats, if it
    /// has room for them all (`per_side` a side); their pads start over.
    fn move_side(&mut self, addr: SocketAddr, team: u8, per_side: usize) {
        let Some(k) = self.remotes.iter().position(|r| r.addr == addr) else {
            return;
        };
        let teams = self.setup.seat_teams;
        if self.remotes[k]
            .seats
            .iter()
            .all(|&s| teams[usize::from(s)] == team)
        {
            return;
        }
        let taken: Vec<u8> = self
            .remotes
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != k)
            .flat_map(|(_, r)| r.seats.clone())
            .collect();
        let join = Join {
            match_name: String::new(),
            players: self.remotes[k]
                .names
                .iter()
                .map(|n| crate::protocol::Joiner {
                    name: n.clone(),
                    team: Some(team),
                })
                .collect(),
            create: None,
            disc: [0; 32],
        };
        if let Some(seats) = assign_seats(&teams, &taken, &join, per_side) {
            let r = &mut self.remotes[k];
            r.seats = seats;
            r.ready = false;
            r.pads.clear();
            self.lobby_dirty = true;
        }
    }

    /// Seats for `join`'s people ([`assign_seats`]).
    fn assign(&self, join: &Join, per_side: usize) -> Option<Vec<u8>> {
        let taken: Vec<u8> = self.remotes.iter().flat_map(|r| r.seats.clone()).collect();
        assign_seats(&self.setup.seat_teams, &taken, join, per_side)
    }

    /// Fix the pads for the next tick and step the referee's match.
    fn fix_tick(&mut self, history: usize) {
        let t = self.state.tick();
        let mut frame = Frame::default();
        for r in &mut self.remotes {
            let pads = r.pads.remove(&t);
            r.pads.retain(|&k, _| k > t);
            if pads.is_none() {
                r.late += 1;
            }
            for (j, &seat) in r.seats.iter().enumerate() {
                let k = usize::from(seat);
                frame.seats[k] = Some(match &pads {
                    Some(p) => p[j],
                    None => self.last[k].unwrap_or_default(),
                });
            }
            r.ack = r.ack.max(t + 1);
            while r.pads.contains_key(&r.ack) {
                r.ack += 1;
            }
        }
        self.last = frame.seats;
        self.state.step(&frame);
        self.history.push_back(frame);
        self.sums.push_back(self.state.checksum());
        while self.history.len() > history {
            self.history.pop_front();
            self.history_start += 1;
        }
        while self.sums.len() > history {
            self.sums.pop_front();
            self.sums_start += 1;
        }
        if self.state.tick().is_multiple_of(LEAD_WINDOW) {
            for r in &mut self.remotes {
                r.lead_before = r.lead_now;
                r.lead_now = i16::MAX;
            }
        }
    }
}

/// A server hosting any number of matches on one socket.
pub struct Server {
    transport: Box<dyn Transport>,
    cfg: ServerConfig,
    /// This machine's disc: every match's tables, and the digest joiners
    /// must match.
    disc: std::sync::Arc<Disc>,
    now: Duration,
    matches: BTreeMap<String, Host>,
    /// Which match each client address is in.
    owners: HashMap<SocketAddr, String>,
    reassembly: Reassembly,
    part_id: u16,
    events: Vec<ServerEvent>,
}

impl Server {
    /// A server on `transport` refereeing on `disc`.
    pub fn new(
        transport: Box<dyn Transport>,
        cfg: ServerConfig,
        disc: std::sync::Arc<Disc>,
    ) -> Self {
        Self {
            transport,
            cfg,
            disc,
            now: Duration::ZERO,
            matches: BTreeMap::new(),
            owners: HashMap::new(),
            reassembly: Reassembly::default(),
            part_id: 0,
            events: Vec::new(),
        }
    }

    /// Read every waiting datagram, run the matches' ticks due by `now`
    /// (a monotonic clock), and send what is due. Call often: at least once
    /// a tick, ideally every millisecond or two.
    pub fn update(&mut self, now: Duration) -> Vec<ServerEvent> {
        self.now = now;
        while let Some((from, bytes)) = self.transport.recv() {
            match Message::decode(&bytes) {
                Ok(m) => self.handle(from, m),
                Err(DecodeError::Version(_)) => {
                    self.send(
                        from,
                        &Message::Refused {
                            reason: Refusal::Version,
                        },
                    );
                }
                Err(_) => {}
            }
        }
        let names: Vec<String> = self.matches.keys().cloned().collect();
        for name in names {
            self.run(&name);
        }
        std::mem::take(&mut self.events)
    }

    /// The matches, by name.
    pub fn matches(&self) -> impl Iterator<Item = &Host> {
        self.matches.values()
    }

    pub fn host(&self, name: &str) -> Option<&Host> {
        self.matches.get(name)
    }

    /// Tell every client the server is closing.
    pub fn shutdown(&mut self) {
        let addrs: Vec<SocketAddr> = self.owners.keys().copied().collect();
        for a in addrs {
            self.send(
                a,
                &Message::Closed {
                    reason: CloseReason::Shutdown,
                },
            );
        }
        self.matches.clear();
        self.owners.clear();
    }

    fn handle(&mut self, from: SocketAddr, m: Message) {
        if let Message::Round { round, bytes } = m {
            let Ok(inner) = Message::decode(&bytes) else {
                return;
            };
            if !inner.round_scoped() {
                return;
            }
            if self
                .owners
                .get(&from)
                .and_then(|name| self.matches.get(name))
                .is_some_and(|h| h.round == round)
            {
                self.handle_current(from, inner);
            }
            return;
        }
        if m.round_scoped()
            && self
                .owners
                .get(&from)
                .and_then(|name| self.matches.get(name))
                .is_some_and(|h| h.round > 0)
        {
            return;
        }
        self.handle_current(from, m);
    }

    fn handle_current(&mut self, from: SocketAddr, m: Message) {
        if let Message::List { id } = m {
            let matches = self
                .matches
                .values()
                .take(64)
                .map(|h| crate::protocol::MatchInfo {
                    name: h.name.clone(),
                    court: h.setup.court.clone(),
                    players: h.seats().len() as u8,
                    started: h.started(),
                    finished: matches!(h.state.world.phase, Phase::Over { .. }),
                })
                .collect();
            self.send(from, &Message::Matches { id, matches });
            return;
        }
        if let Message::Part {
            id,
            index,
            count,
            bytes,
        } = m
        {
            let whole = self.reassembly.add(from, self.now, id, index, count, bytes);
            if let Some(m) = whole.and_then(|b| Message::decode(&b).ok())
                && !matches!(m, Message::Part { .. })
            {
                self.handle(from, m);
            }
            return;
        }
        if let Message::Join(join) = m {
            self.join(from, join);
            return;
        }
        let Some(name) = self.owners.get(&from).cloned() else {
            return;
        };
        let now = self.now;
        let Some(host) = self.matches.get_mut(&name) else {
            return;
        };
        let Some(r) = host.remotes.iter_mut().find(|r| r.addr == from) else {
            return;
        };
        r.heard_at = now;
        match m {
            Message::Rematch { round } => {
                if round == host.round
                    && !r.seats.is_empty()
                    && matches!(host.state.world.phase, Phase::Over { .. })
                {
                    r.rematch = true;
                }
            }
            Message::RestartAck { round } => {
                if round == host.round && !r.restart_ack {
                    r.restart_ack = true;
                    r.ready = true;
                    host.lobby_dirty = true;
                }
            }
            Message::Ready { ready } => {
                if r.restart_ack && r.ready != ready {
                    r.ready = ready;
                    host.lobby_dirty = true;
                }
            }
            Message::Pick { pick } => {
                if r.pick != pick {
                    r.pick = pick;
                    host.lobby_dirty = true;
                }
            }
            Message::Side { team } => {
                if team < 2 && host.started_at.is_none() && !r.seats.is_empty() {
                    let per_side = self.cfg.seats_per_side.clamp(1, 3);
                    host.move_side(from, team, per_side);
                    self.welcome(&name, from);
                }
            }
            Message::Setup { setup } => {
                if host.creator == Some(from) && host.started_at.is_none() && host.round == 0 {
                    self.reconfigure(&name, from, setup);
                }
            }
            Message::Inputs {
                need,
                start,
                seats,
                ticks,
            } => {
                if seats != r.seats {
                    return;
                }
                r.need = need;
                let next = host.state.tick();
                for (n, pads) in ticks.iter().enumerate() {
                    let t = start + n as u64;
                    if t >= next && t < next + 1000 && pads.len() == seats.len() {
                        r.pads.insert(t, pads.clone());
                    }
                }
                if !ticks.is_empty() && host.started_at.is_some() {
                    let newest = start + ticks.len() as u64 - 1;
                    r.heard = r.heard.max(newest);
                    let lead = (newest as i64 - next as i64).clamp(-30_000, 30_000) as i16;
                    r.lead_now = r.lead_now.min(lead);
                }
                r.ack = r.ack.max(next);
                while r.pads.contains_key(&r.ack) {
                    r.ack += 1;
                }
            }
            Message::Checksums { reports } => {
                let floor = r.resync_floor;
                let drifted = reports.iter().any(|&(t, sum)| {
                    t >= floor && host.checksum_at(t).is_some_and(|ours| ours != sum)
                });
                if drifted {
                    self.send_state(&name, from, true);
                }
            }
            Message::Ping { id } => {
                let tick = host.state.tick();
                self.send(from, &Message::Pong { id, tick });
            }
            Message::Leave => self.drop_remote(&name, from, false),
            _ => {}
        }
    }

    fn join(&mut self, from: SocketAddr, join: Join) {
        // A repeated join: answer it again.
        if let Some(name) = self.owners.get(&from).cloned() {
            if name == join.match_name
                && let Some(host) = self.matches.get(&name)
                && let Some(r) = host.remotes.iter().find(|r| r.addr == from)
            {
                let welcome = Message::Welcome {
                    seats: r.seats.clone(),
                    setup: host.setup.clone(),
                    setup_version: host.setup_version,
                };
                self.send(from, &welcome);
                return;
            }
            self.drop_remote(&name, from, false);
        }
        let valid = join.players.len() <= MAX_SEATS
            && join.players.iter().all(|p| p.team.is_none_or(|t| t < 2))
            && join.create.as_ref().is_none_or(valid_setup);
        if !valid {
            self.refuse(from, Refusal::Invalid);
            return;
        }
        // Every machine plays its own disc's tables: they must be ours.
        if join.disc != self.disc.digest() {
            let hex = street_assets::roster_share::hex;
            self.events.push(ServerEvent::Refused {
                name: join.match_name.clone(),
                addr: from,
                reasons: vec![format!(
                    "its disc data {} is not this server's {} (another region's disc, or a \
                     modded one)",
                    hex(&join.disc),
                    hex(&self.disc.digest())
                )],
            });
            self.refuse(from, Refusal::Disc);
            return;
        }
        if !self.matches.contains_key(&join.match_name) {
            if join.players.is_empty() {
                self.refuse(from, Refusal::Invalid);
                return;
            }
            let Some(setup) = join.create.clone() else {
                self.refuse(from, Refusal::NoMatch);
                return;
            };
            if self.matches.len() >= self.cfg.max_matches {
                self.refuse(from, Refusal::Busy);
                return;
            }
            // The anti-cheat: the roster, and the world against it.
            if let Err(why) = crate::roster::check_setup(&setup, &self.cfg.rosters) {
                self.events.push(ServerEvent::Refused {
                    name: join.match_name.clone(),
                    addr: from,
                    reasons: why.reasons(),
                });
                let reason = match why {
                    crate::roster::RosterError::UnknownMod(_) => Refusal::RosterMod,
                    crate::roster::RosterError::Illegal(_) => Refusal::Roster,
                };
                self.refuse(from, reason);
                return;
            }
            let name = join.match_name.clone();
            let host = match Host::new(name.clone(), setup, &self.disc) {
                Ok(host) => host,
                Err(why) => {
                    self.events.push(ServerEvent::Refused {
                        name,
                        addr: from,
                        reasons: vec![why],
                    });
                    self.refuse(from, Refusal::Invalid);
                    return;
                }
            };
            let mut host = host;
            host.creator = Some(from);
            self.matches.insert(name.clone(), host);
            self.events.push(ServerEvent::Created { name });
        }
        let host = self.matches.get_mut(&join.match_name).expect("created");
        if host.remotes.len() >= 32 {
            self.refuse(from, Refusal::Full);
            return;
        }
        let Some(seats) = host.assign(&join, self.cfg.seats_per_side.clamp(1, 3)) else {
            self.refuse(from, Refusal::Full);
            return;
        };
        let tick = host.state.tick();
        host.remotes.push(Remote {
            addr: from,
            seats: seats.clone(),
            names: join.players.iter().map(|p| p.name.clone()).collect(),
            ready: false,
            rematch: false,
            restart_ack: true,
            restart_sent_at: None,
            heard_at: self.now,
            need: 0,
            pads: BTreeMap::new(),
            ack: tick,
            heard: tick,
            lead_now: i16::MAX,
            lead_before: i16::MAX,
            resync_floor: 0,
            state_sent_at: None,
            late: 0,
            pick: Vec::new(),
        });
        host.lobby_dirty = true;
        host.empty_since = None;
        let started = host.started();
        let welcome = Message::Welcome {
            seats: seats.clone(),
            setup: host.setup.clone(),
            setup_version: host.setup_version,
        };
        let name = join.match_name.clone();
        self.owners.insert(from, name.clone());
        self.send(from, &welcome);
        if started {
            self.send(from, &Message::Start { tick });
        }
        self.events.push(ServerEvent::Joined {
            name,
            addr: from,
            seats,
        });
    }

    fn refuse(&mut self, to: SocketAddr, reason: Refusal) {
        self.send(to, &Message::Refused { reason });
    }

    /// Welcome `addr` to match `name` again: its seats and the setup as
    /// they stand.
    fn welcome(&mut self, name: &str, addr: SocketAddr) {
        let Some(host) = self.matches.get(name) else {
            return;
        };
        let Some(r) = host.remotes.iter().find(|r| r.addr == addr) else {
            return;
        };
        let m = Message::Welcome {
            seats: r.seats.clone(),
            setup: host.setup.clone(),
            setup_version: host.setup_version,
        };
        self.send(addr, &m);
    }

    /// The creator's setup for match `name`, as everyone picked it in the
    /// lobby: checked as a creation is (its shape, the roster and the world
    /// against it, the disc's data), then the match's, everyone un-readied
    /// and welcomed to it again.
    fn reconfigure(&mut self, name: &str, from: SocketAddr, setup: MatchSetup) {
        let refused = |reasons: Vec<String>| ServerEvent::Refused {
            name: name.to_string(),
            addr: from,
            reasons,
        };
        if !valid_setup(&setup) {
            self.events
                .push(refused(vec!["the lobby's setup is not a match".into()]));
            return;
        }
        if let Err(why) = crate::roster::check_setup(&setup, &self.cfg.rosters) {
            self.events.push(refused(why.reasons()));
            return;
        }
        let fresh = match Host::new(name.to_string(), setup, &self.disc) {
            Ok(fresh) => fresh,
            Err(why) => {
                self.events.push(refused(vec![why]));
                return;
            }
        };
        let Some(host) = self.matches.get_mut(name) else {
            return;
        };
        host.setup_version = host.setup_version.wrapping_add(1);
        host.setup = fresh.setup;
        host.state = fresh.state;
        host.history.clear();
        host.history_start = fresh.history_start;
        host.sums = fresh.sums;
        host.sums_start = fresh.sums_start;
        host.last = [None; MAX_SEATS];
        host.lobby_dirty = true;
        let tick = host.state.tick();
        for r in &mut host.remotes {
            r.ready = false;
            r.need = tick;
            r.ack = tick;
            r.heard = tick;
            r.pads.clear();
        }
        let addrs: Vec<SocketAddr> = host.remotes.iter().map(|r| r.addr).collect();
        for a in addrs {
            self.welcome(name, a);
        }
        self.events.push(ServerEvent::Reconfigured {
            name: name.to_string(),
        });
    }

    fn drop_remote(&mut self, name: &str, addr: SocketAddr, timed_out: bool) {
        self.owners.remove(&addr);
        if let Some(host) = self.matches.get_mut(name) {
            host.remotes.retain(|r| r.addr != addr);
            host.lobby_dirty = true;
        }
        if timed_out {
            self.send(
                addr,
                &Message::Closed {
                    reason: CloseReason::TimedOut,
                },
            );
        }
        self.events.push(ServerEvent::Left {
            name: name.to_string(),
            addr,
            timed_out,
        });
    }

    /// Timeouts, the lobby, the ticks due, and the frames out, for one match.
    fn run(&mut self, name: &str) {
        let now = self.now;
        let timeout = self.cfg.timeout;
        let silent: Vec<SocketAddr> = self.matches[name]
            .remotes
            .iter()
            .filter(|r| now.saturating_sub(r.heard_at) > timeout)
            .map(|r| r.addr)
            .collect();
        for addr in silent {
            self.drop_remote(name, addr, true);
        }
        let history = self.cfg.history;
        let host = self.matches.get_mut(name).expect("listed");

        // Replay only when every seated human machine agrees. Spectators
        // retain their connection but cannot vote or hold up the restart.
        if matches!(host.state.world.phase, Phase::Over { .. })
            && host.remotes.iter().any(|r| !r.seats.is_empty())
            && host
                .remotes
                .iter()
                .filter(|r| !r.seats.is_empty())
                .all(|r| r.rematch)
            && host.round < u32::MAX
        {
            host.round += 1;
            host.state = Match::new(host.setup.world.clone(), host.setup.seat_teams);
            host.started_at = None;
            host.ticks_run = 0;
            host.history.clear();
            host.history_start = host.state.tick();
            host.sums = VecDeque::from([host.state.checksum()]);
            host.sums_start = host.state.tick();
            host.last = [None; MAX_SEATS];
            host.over_at = None;
            host.lobby_dirty = true;
            for r in &mut host.remotes {
                r.ready = false;
                r.rematch = false;
                r.restart_ack = false;
                r.restart_sent_at = None;
                r.need = host.state.tick();
                r.pads.clear();
                r.ack = host.state.tick();
                r.heard = host.state.tick();
                r.lead_now = i16::MAX;
                r.lead_before = i16::MAX;
                r.resync_floor = host.state.tick();
                r.state_sent_at = None;
                r.late = 0;
            }
        }
        let restart_round = host.round;
        let restart_setup = host.setup.clone();
        let mut restart_addrs = Vec::new();
        for r in &mut host.remotes {
            if !r.restart_ack
                && r.restart_sent_at
                    .is_none_or(|t| now.saturating_sub(t) >= Duration::from_millis(250))
            {
                r.restart_sent_at = Some(now);
                restart_addrs.push(r.addr);
            }
        }

        // Close a match that is empty or long over.
        if host.remotes.is_empty() {
            let since = *host.empty_since.get_or_insert(now);
            if now.saturating_sub(since) > timeout {
                self.close(name);
                return;
            }
        }
        // Over once the won game's replay has played (the game's results
        // wait for its end, as the original's game listener does).
        if matches!(host.state.world.phase, Phase::Over { .. })
            && !host.state.replays.manager.holding()
        {
            let since = *host.over_at.get_or_insert(now);
            if now.saturating_sub(since) > self.cfg.linger {
                self.close(name);
                return;
            }
        }

        // Start once everyone seated is ready.
        let mut just_started = false;
        if host.started_at.is_none()
            && host.remotes.iter().any(|r| !r.seats.is_empty())
            && host
                .remotes
                .iter()
                .filter(|r| !r.seats.is_empty())
                .all(|r| r.ready)
        {
            host.started_at = Some(now);
            host.lobby_dirty = true;
            just_started = true;
            self.events.push(ServerEvent::Started {
                name: name.to_string(),
            });
        }
        let lobby_due = host.lobby_dirty
            || host
                .lobby_sent_at
                .is_none_or(|t| now.saturating_sub(t) >= Duration::from_secs(1));
        let lobby = lobby_due.then(|| {
            host.lobby_dirty = false;
            host.lobby_sent_at = Some(now);
            Message::Lobby {
                seats: host.seats(),
                setup_version: host.setup_version,
            }
        });

        // The ticks due by the clock.
        let mut ran = 0;
        if let Some(start) = host.started_at {
            let due =
                (now.saturating_sub(start).as_secs_f64() * f64::from(street_sim::TICK_HZ)) as u64;
            while host.ticks_run < due && ran < MAX_CATCH_UP {
                // AI, simulation and checksum/history work all belong to this
                // referee. A panic must never unwind through the shared loop.
                let result = catch_unwind(AssertUnwindSafe(|| {
                    host.fix_tick(history);
                    // The world's own reason, then the seats'.
                    host.state
                        .world
                        .validate()
                        .and_then(|()| host.state.validate().map_err(|e| e.to_string()))
                }));
                let failure = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(format!("referee produced invalid state: {error}")),
                    Err(payload) => Some(
                        payload
                            .downcast_ref::<&str>()
                            .map(|s| (*s).to_owned())
                            .or_else(|| payload.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "referee panicked".into()),
                    ),
                };
                if let Some(reason) = failure {
                    self.events.push(ServerEvent::Faulted {
                        name: name.to_owned(),
                        reason,
                    });
                    self.close_with_reason(name, CloseReason::Fault);
                    return;
                }
                host.ticks_run += 1;
                ran += 1;
            }
            // Too far behind real time: let the clock go.
            host.ticks_run = host.ticks_run.max(due.saturating_sub(MAX_CATCH_UP));
        }

        let addrs: Vec<SocketAddr> = host.remotes.iter().map(|r| r.addr).collect();
        let tick = host.state.tick();
        for a in restart_addrs {
            self.send(
                a,
                &Message::Restart {
                    round: restart_round,
                    setup: restart_setup.clone(),
                },
            );
        }
        for &a in &addrs {
            if let Some(m) = &lobby {
                self.send(a, m);
            }
            if just_started {
                self.send(a, &Message::Start { tick });
            }
        }
        if ran > 0 {
            for a in addrs {
                self.send_frames(name, a);
            }
        }
    }

    /// The frames a client lacks, or the whole state if they are no longer
    /// kept.
    fn send_frames(&mut self, name: &str, addr: SocketAddr) {
        let host = &self.matches[name];
        let Some(r) = host.remotes.iter().find(|r| r.addr == addr) else {
            return;
        };
        let end = host.state.tick();
        if r.need < host.history_start || r.need > end {
            self.send_state(name, addr, false);
            return;
        }
        let lead = r.lead_now.min(r.lead_before);
        let first = r.need;
        let offset = (first - host.history_start) as usize;
        let mut n = ((end - first) as usize).min(MAX_RUN);
        let m = loop {
            let m = Message::Frames {
                first,
                frames: host.history.iter().skip(offset).take(n).copied().collect(),
                ack: r.ack,
                heard: r.heard,
                lead,
            };
            if n == 0 || m.encode().len() <= MAX_DATAGRAM {
                break m;
            }
            n /= 2;
        };
        self.send(addr, &m);
    }

    /// Send a client the whole match as it stands, at most every
    /// [`STATE_GAP`].
    fn send_state(&mut self, name: &str, addr: SocketAddr, drifted: bool) {
        let now = self.now;
        let Some(host) = self.matches.get_mut(name) else {
            return;
        };
        let tick = host.state.tick();
        let Some(r) = host.remotes.iter_mut().find(|r| r.addr == addr) else {
            return;
        };
        if r.state_sent_at
            .is_some_and(|t| now.saturating_sub(t) < STATE_GAP)
        {
            return;
        }
        r.state_sent_at = Some(now);
        r.resync_floor = tick;
        r.need = tick;
        let m = Message::State {
            tick,
            state: host.state.to_snapshot(),
        };
        self.send(addr, &m);
        self.events.push(ServerEvent::Resynced {
            name: name.to_string(),
            addr,
            tick,
            drifted,
        });
    }

    fn close(&mut self, name: &str) {
        self.close_with_reason(name, CloseReason::Over);
    }

    fn close_with_reason(&mut self, name: &str, reason: CloseReason) {
        if let Some(host) = self.matches.remove(name) {
            for r in &host.remotes {
                self.owners.remove(&r.addr);
                let closed = Message::Closed { reason };
                let m = if host.round > 0 {
                    Message::Round {
                        round: host.round,
                        bytes: closed.encode(),
                    }
                } else {
                    closed
                };
                self.part_id = self.part_id.wrapping_add(1);
                for d in m.datagrams(self.part_id) {
                    self.transport.send(r.addr, &d);
                }
            }
            self.events.push(ServerEvent::Closed {
                name: name.to_string(),
            });
        }
    }

    fn send(&mut self, to: SocketAddr, m: &Message) {
        let round = self
            .owners
            .get(&to)
            .and_then(|n| self.matches.get(n))
            .map_or(0, |h| h.round);
        let envelope = (round > 0 && m.round_scoped()).then(|| Message::Round {
            round,
            bytes: m.encode(),
        });
        let m = envelope.as_ref().unwrap_or(m);
        self.part_id = self.part_id.wrapping_add(1);
        for d in m.datagrams(self.part_id) {
            self.transport.send(to, &d);
        }
    }
}

/// A setup a match can run: two sides, seats on sides that have players.
fn valid_setup(s: &MatchSetup) -> bool {
    let players = &s.world.players;
    s.world.validate().is_ok()
        && players.len() == MAX_SEATS
        && (0..2).all(|team| players.iter().filter(|p| p.team == team).count() == 3)
        && s.roster.len() == players.len()
        // A roster's records and teams (`street_assets::roster_rules`):
        // the disc's 240 records and 49 teams, plus added players and the
        // custom team; what the roster itself holds is checked with it.
        && s.roster
            .iter()
            .all(|&record| usize::from(record) < street_assets::roster_rules::MAX_RECORDS)
        && s.teams
            .iter()
            .all(|&team| usize::from(team) < street_assets::roster_rules::MAX_TEAMS)
        && s.seat_teams.iter().all(|&t| t < 2)
        && (0..2).all(|team| s.seat_teams.iter().filter(|&&t| t == team).count() == 3)
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::{Conditions, Joiner, MemoryNetwork};
    use street_sim::{Action, BallState, Pass, ball::Via};

    fn addr(port: u16) -> SocketAddr {
        ([127, 0, 0, 1], port).into()
    }

    fn join(name: &str, setup: Option<MatchSetup>) -> Join {
        Join {
            match_name: name.into(),
            players: vec![Joiner {
                name: "Tester".into(),
                team: Some(0),
            }],
            create: setup,
            disc: Disc::stand_ins().digest(),
        }
    }

    fn stand_ins() -> std::sync::Arc<Disc> {
        std::sync::Arc::new(Disc::stand_ins())
    }

    #[test]
    fn invalid_nested_creation_payloads_and_bare_joins_cannot_create_matches() {
        let network = MemoryNetwork::new(Conditions::default(), 1);
        let mut server = Server::new(
            Box::new(network.endpoint(addr(7375))),
            ServerConfig::default(),
            stand_ins(),
        );
        let mutations: [fn(&mut MatchSetup); 10] = [
            |s| s.world.ball.state = BallState::Held(6),
            |s| {
                s.world.players[0].action = Action::Shooting {
                    hoop: 2,
                    elapsed: 0.0,
                    gamebreaker: 0,
                    fade: false,
                    tick: 0,
                    script: 0,
                    release: 30,
                    check: 0,
                    end: 40,
                    gate: 0,
                    latest: 0,
                    stall: 0,
                }
            },
            |s| s.world.ball.velocity[1] = f32::NAN,
            |s| s.world.give_and_go = Some((0, 6)),
            |s| {
                s.world.ball.state = BallState::Pass;
                s.world.ball.pass = Some(Pass {
                    from: 0,
                    to: 1,
                    tried: 0,
                    via: Via::Glass { hoop: 2 },
                    catch: street_sim::ball::NO_CATCH,
                    since: 0,
                });
            },
            |s| s.seat_teams[0] = 2,
            |s| s.roster.pop().map(|_| ()).unwrap(),
            |s| s.world.players[5].team = 0,
            |s| s.roster[0] = 308,
            |s| s.teams[1] = 50,
        ];
        for (index, mutate) in mutations.into_iter().enumerate() {
            let mut setup = MatchSetup::standard(Default::default(), 1);
            mutate(&mut setup);
            server.join(addr(5000 + index as u16), join("bad", Some(setup)));
            assert!(server.host("bad").is_none());
            assert!(server.owners.is_empty());
        }
        // A join with no setup makes no match: the server has none of its own.
        server.join(addr(5100), join("nothing", None));
        assert!(server.host("nothing").is_none());
        assert!(server.owners.is_empty());
        let valid = MatchSetup::standard(Default::default(), 1);
        server.join(addr(5200), join("healthy", Some(valid)));
        assert!(server.host("healthy").is_some());
    }

    #[test]
    fn a_finished_match_lingers_from_the_end_of_its_won_replay() {
        use street_sim::replay::{Gates, INSTANT_SPEED, WON_DELAY, mode};
        let network = MemoryNetwork::new(Conditions::default(), 1);
        let mut server = Server::new(
            Box::new(network.endpoint(addr(7375))),
            ServerConfig::default(),
            stand_ins(),
        );
        server.join(
            addr(5000),
            join("park", Some(MatchSetup::standard(Default::default(), 1))),
        );
        server.matches.get_mut("park").unwrap().remotes[0].ready = true;
        let mut now = Duration::ZERO;
        // The one seated keeps talking.
        let mut run = |server: &mut Server, ms: u64| {
            for _ in 0..ms / 16 {
                now += Duration::from_millis(16);
                if let Some(h) = server.matches.get_mut("park") {
                    h.remotes[0].heard_at = now;
                }
                server.update(now);
            }
        };
        run(&mut server, 2000);
        // The game is won, and its replay asked for (`FUN_003248d8`).
        let host = server.matches.get_mut("park").unwrap();
        host.state.world.phase = Phase::Over { winner: 0 };
        host.state.replays.manager.request(
            INSTANT_SPEED,
            120,
            WON_DELAY,
            mode::SHOT,
            Gates::default(),
        );
        run(&mut server, 100);
        assert!(server.host("park").unwrap().over_at.is_none());
        // The replay plays out; only then does the linger start.
        run(&mut server, 15_000);
        let host = server.host("park").unwrap();
        assert!(!host.state.replays.manager.holding());
        assert!(host.over_at.is_some());
    }

    #[test]
    fn a_joiner_on_other_disc_data_is_refused_saying_why() {
        let network = MemoryNetwork::new(Conditions::default(), 1);
        let mut server = Server::new(
            Box::new(network.endpoint(addr(7375))),
            ServerConfig::default(),
            stand_ins(),
        );
        let setup = || Some(MatchSetup::standard(Default::default(), 1));
        server.join(addr(5000), join("park", setup()));
        assert!(server.host("park").is_some());
        // Another region's disc, or a modded one: another digest.
        let mut other = join("park", None);
        other.disc = [7; 32];
        server.join(addr(5001), other.clone());
        assert!(!server.owners.contains_key(&addr(5001)));
        other.match_name = "other".into();
        other.create = setup();
        server.join(addr(5002), other);
        assert!(server.host("other").is_none());
        let events = server.update(Duration::ZERO);
        let refused: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                ServerEvent::Refused { reasons, .. } => Some(reasons.join("; ")),
                _ => None,
            })
            .collect();
        assert_eq!(refused.len(), 2, "{events:?}");
        assert!(
            refused.iter().all(|r| r.contains("not this server's")),
            "{refused:?}"
        );
        let mut client = network.endpoint(addr(5001));
        let mut why = None;
        while let Some((_, bytes)) = client.recv() {
            if let Ok(Message::Refused { reason }) = Message::decode(&bytes) {
                why = Some(reason);
            }
        }
        assert_eq!(why, Some(Refusal::Disc));
    }

    #[test]
    fn an_illegal_roster_cannot_create_a_match() {
        use street_assets::roster_share::{
            MatchRoster, SharedPlayer, SharedRoster, blank, with_fields,
        };
        let network = MemoryNetwork::new(Conditions::default(), 1);
        let mut server = Server::new(
            Box::new(network.endpoint(addr(7375))),
            ServerConfig::default(),
            stand_ins(),
        );
        // Six average records, as the standard world holds them.
        let lineup: Vec<_> = (0..6)
            .map(|k| {
                let mut f = vec![0i16; 101];
                f[8] = 78;
                f[9] = 215;
                f[10] = 78;
                f[11] = 215;
                f[16..27].fill(50);
                f[68..101].fill(-1);
                with_fields(blank(k), f)
            })
            .collect();
        let with = |players: Vec<SharedPlayer>| {
            let mut setup = MatchSetup::standard(Default::default(), 1);
            setup.roster = (0..6).collect();
            for p in &mut setup.world.players {
                let r = 50.0 / 99.0;
                p.ratings = street_sim::Ratings {
                    shots: r,
                    threes: r,
                    dunks: r,
                    rebounds: r,
                    blocks: r,
                    steals: r,
                    handles: r,
                    speed: r,
                    power: r,
                    overall: r,
                    reach: street_sim::player::reach_for_height(78.0),
                };
            }
            setup.roster_data = MatchRoster {
                roster: SharedRoster {
                    players_len: 240,
                    teams_len: 49,
                    players,
                    ..Default::default()
                },
                lineup: lineup.clone(),
            }
            .pack()
            .unwrap();
            setup
        };
        // A 96-inch giant on the roster.
        let giant = SharedPlayer {
            record: 100,
            source: 100,
            fields: vec![(8, 96), (10, 96)],
            ..Default::default()
        };
        server.join(addr(5000), join("giant", Some(with(vec![giant]))));
        assert!(server.host("giant").is_none());
        let events = server.update(Duration::ZERO);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ServerEvent::Refused { reasons, .. }
                if reasons.iter().any(|r| r.contains("height 96")))),
            "{events:?}"
        );
        // A legal roster whose world plays other numbers.
        let mut cheat = with(Vec::new());
        cheat.world.players[3].ratings.shots = 1.0;
        server.join(addr(5001), join("cheat", Some(cheat)));
        assert!(server.host("cheat").is_none());
        // The same roster played honestly.
        let fine = SharedPlayer {
            record: 100,
            source: 100,
            fields: vec![(8, 80), (10, 80), (16, 90)],
            ..Default::default()
        };
        server.join(addr(5002), join("fine", Some(with(vec![fine]))));
        assert!(server.host("fine").is_some());
    }

    #[test]
    fn a_referee_panic_closes_only_its_match_and_other_match_keeps_ticking() {
        let network = MemoryNetwork::new(Conditions::default(), 2);
        let mut server = Server::new(
            Box::new(network.endpoint(addr(7375))),
            ServerConfig::default(),
            stand_ins(),
        );
        let setup = || Some(MatchSetup::standard(Default::default(), 1));
        server.join(addr(5000), join("bad", setup()));
        server.join(addr(5001), join("healthy", setup()));
        for host in server.matches.values_mut() {
            host.remotes[0].ready = true;
        }
        server.update(Duration::ZERO);
        let healthy_before = server.host("healthy").unwrap().tick();
        // Simulate an internal referee defect AFTER all incoming validation.
        // AI indexes the player's team and deterministically panics in this
        // match. No malformed external setup can get here through validation.
        let bad = server.matches.get_mut("bad").unwrap();
        bad.state.world.phase = Phase::Live;
        bad.state.world.players[0].team = usize::MAX;
        let result = catch_unwind(AssertUnwindSafe(|| {
            server.update(Duration::from_millis(100))
        }));
        let events = result.expect("shared server loop must survive the referee panic");
        assert!(
            events
                .iter()
                .any(|e| matches!(e,ServerEvent::Faulted { name,.. } if name=="bad"))
        );
        assert!(server.host("bad").is_none());
        assert!(!server.owners.contains_key(&addr(5000)));
        assert_eq!(
            server.owners.get(&addr(5001)).map(String::as_str),
            Some("healthy")
        );
        assert!(server.host("healthy").unwrap().tick() > healthy_before);
        let before = server.host("healthy").unwrap().tick();
        server.update(Duration::from_millis(200));
        assert!(server.host("healthy").unwrap().tick() > before);
        let mut bad_client = network.endpoint(addr(5000));
        let mut fault = false;
        while let Some((_, bytes)) = bad_client.recv() {
            fault |= matches!(
                Message::decode(&bytes),
                Ok(Message::Closed {
                    reason: CloseReason::Fault
                })
            );
        }
        assert!(
            fault,
            "only the failed match's client receives a fault close"
        );
    }
}

#[cfg(test)]
mod rematch_tests {
    use super::*;
    use crate::{Client, ClientConfig, ClientEvent, Conditions, Joiner, MemoryNetwork, Status};

    fn addr(port: u16) -> SocketAddr {
        ([127, 0, 0, 1], port).into()
    }
    fn join(name: &str, spectator: bool) -> Join {
        Join {
            match_name: "rematch".into(),
            create: Some(MatchSetup::standard(Default::default(), 1)),
            disc: [0; 32],
            players: if spectator {
                vec![]
            } else {
                vec![Joiner {
                    name: name.into(),
                    team: None,
                }]
            },
        }
    }
    fn pump(
        net: &MemoryNetwork,
        server: &mut Server,
        clients: &mut [Client],
        now: Duration,
    ) -> Vec<ClientEvent> {
        net.set_time(now);
        server.update(now);
        let mut events = Vec::new();
        for client in clients {
            events.extend(client.update(now));
            for _ in 0..client.ticks_due(now) {
                client.advance();
            }
        }
        events
    }

    #[test]
    fn seated_votes_restart_and_old_round_packets_cannot_corrupt_it() {
        let net = MemoryNetwork::new(Conditions::default(), 71);
        let mut server = Server::new(
            Box::new(net.endpoint(addr(7375))),
            ServerConfig::default(),
            std::sync::Arc::new(Disc::stand_ins()),
        );
        let mut clients = vec![
            Client::new(
                Box::new(net.endpoint(addr(5001))),
                addr(7375),
                join("a", false),
                ClientConfig::default(),
                std::sync::Arc::new(Disc::stand_ins()),
            ),
            Client::new(
                Box::new(net.endpoint(addr(5002))),
                addr(7375),
                join("b", false),
                ClientConfig::default(),
                std::sync::Arc::new(Disc::stand_ins()),
            ),
            Client::new(
                Box::new(net.endpoint(addr(5003))),
                addr(7375),
                join("watch", true),
                ClientConfig::default(),
                std::sync::Arc::new(Disc::stand_ins()),
            ),
        ];
        for client in &mut clients {
            client.set_ready(true);
        }
        for step in 0..100 {
            pump(
                &net,
                &mut server,
                &mut clients,
                Duration::from_millis(step * 10),
            );
        }
        assert!(clients.iter().all(|c| c.status() == Status::Playing));
        // End only the authoritative match, then deliver it normally.
        let host = server.matches.get_mut("rematch").unwrap();
        host.state.world.phase = Phase::Over { winner: 0 };
        let old_snapshot = host.state.to_snapshot();
        let old_tick = host.tick();
        let finished = Message::State {
            tick: old_tick,
            state: old_snapshot.clone(),
        };
        for port in 5001..=5003 {
            server.send(addr(port), &finished);
        }
        pump(&net, &mut server, &mut clients, Duration::from_millis(1000));
        assert!(!clients[2].request_rematch());
        assert!(clients[0].request_rematch());
        for step in 101..110 {
            pump(
                &net,
                &mut server,
                &mut clients,
                Duration::from_millis(step * 10),
            );
        }
        assert_eq!(
            server.matches["rematch"].round, 0,
            "one machine cannot force everyone into a replay"
        );
        assert!(clients[1].request_rematch());
        let mut events = Vec::new();
        for step in 110..180 {
            events.extend(pump(
                &net,
                &mut server,
                &mut clients,
                Duration::from_millis(step * 10),
            ));
        }
        assert_eq!(server.matches["rematch"].round, 1);
        assert!(
            clients
                .iter()
                .all(|c| c.round() == 1 && c.status() == Status::Playing)
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ClientEvent::Restarted { round: 1 }))
                .count(),
            3
        );
        assert!(
            clients
                .iter()
                .all(|c| !matches!(c.world().unwrap().phase, Phase::Over { .. }))
        );
        // A delayed old snapshot, leave, ready and pad run must be ignored.
        let mut old_server = net.endpoint(addr(7375));
        old_server.send(addr(5001), &finished.encode());
        old_server.send(
            addr(5001),
            &Message::Round {
                round: 0,
                bytes: finished.encode(),
            }
            .encode(),
        );
        let mut old_player = net.endpoint(addr(5001));
        old_player.send(addr(7375), &Message::Leave.encode());
        old_player.send(addr(7375), &Message::Ready { ready: false }.encode());
        old_player.send(
            addr(7375),
            &Message::Round {
                round: 0,
                bytes: Message::Leave.encode(),
            }
            .encode(),
        );
        old_player.send(addr(7375), &Message::Rematch { round: 0 }.encode());
        for step in 180..200 {
            pump(
                &net,
                &mut server,
                &mut clients,
                Duration::from_millis(step * 10),
            );
        }
        assert_eq!(server.matches["rematch"].round, 1);
        assert_eq!(server.matches["rematch"].seats().len(), 2);
        assert!(!server.matches["rematch"].remotes[0].rematch);
        assert!(!matches!(
            clients[0].world().unwrap().phase,
            Phase::Over { .. }
        ));
    }

    #[test]
    fn restart_retries_after_loss_and_spectators_do_not_block_start() {
        let net = MemoryNetwork::new(Conditions::default(), 74);
        let mut server = Server::new(
            Box::new(net.endpoint(addr(7375))),
            ServerConfig::default(),
            std::sync::Arc::new(Disc::stand_ins()),
        );
        let mut clients = vec![Client::new(
            Box::new(net.endpoint(addr(5001))),
            addr(7375),
            join("a", false),
            ClientConfig::default(),
            std::sync::Arc::new(Disc::stand_ins()),
        )];
        clients[0].set_ready(true);
        for step in 0..100 {
            pump(
                &net,
                &mut server,
                &mut clients,
                Duration::from_millis(step * 10),
            );
        }
        let host = server.matches.get_mut("rematch").unwrap();
        host.state.world.phase = Phase::Over { winner: 0 };
        host.remotes[0].rematch = true;
        // The first restart announcement and its fragments are lost.
        net.set_conditions(Conditions {
            loss: 1.0,
            ..Default::default()
        });
        pump(&net, &mut server, &mut clients, Duration::from_millis(1000));
        assert_eq!(server.matches["rematch"].round, 1);
        assert!(!server.matches["rematch"].started());
        net.set_conditions(Conditions {
            loss: 0.15,
            duplicate: 0.2,
            jitter: Duration::from_millis(30),
            ..Default::default()
        });
        for step in 101..450 {
            pump(
                &net,
                &mut server,
                &mut clients,
                Duration::from_millis(step * 10),
            );
        }
        assert_eq!(clients[0].round(), 1);
        assert_eq!(clients[0].status(), Status::Playing);
        assert!(server.matches["rematch"].remotes[0].restart_ack);
        let session = clients[0].session().unwrap();
        assert_eq!(
            server.matches["rematch"].checksum_at(session.confirmed().tick()),
            Some(session.confirmed().checksum())
        );
    }
}

#[cfg(test)]
mod seat_tests {
    use super::*;
    use crate::Joiner;

    fn join(team: Option<u8>) -> Join {
        Join {
            match_name: "m".into(),
            create: None,
            disc: [0; 32],
            players: vec![Joiner {
                name: "p".into(),
                team,
            }],
        }
    }

    #[test]
    fn one_on_one_seats_one_a_side_and_three_on_three_three() {
        let teams = MatchSetup::standard(Default::default(), 1).seat_teams;
        let mut taken: Vec<u8> = Vec::new();
        let seat = |taken: &mut Vec<u8>, team, per_side| {
            let seats = assign_seats(&teams, taken, &join(team), per_side)?;
            taken.extend(&seats);
            Some(seats)
        };
        // One a side: the first takes home, the next away, then it is full.
        assert_eq!(seat(&mut taken, None, 1), Some(vec![0]));
        assert_eq!(seat(&mut taken, None, 1), Some(vec![1]));
        assert_eq!(seat(&mut taken, None, 1), None);
        assert_eq!(seat(&mut taken, Some(0), 1), None);
        // Three a side: six people, alternating sides.
        let mut taken: Vec<u8> = Vec::new();
        for _ in 0..6 {
            assert!(seat(&mut taken, None, 3).is_some());
        }
        taken.sort_unstable();
        assert_eq!(taken, [0, 1, 2, 3, 4, 5]);
        assert_eq!(seat(&mut taken, None, 3), None);
    }
}
