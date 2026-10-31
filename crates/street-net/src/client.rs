//! A game's connection to a match: joining, the lobby, and a rollback
//! [`Session`] once the match is under way, with time sync.
//!
//! The client plays on its own disc's tables ([`Disc`]): the join carries
//! their digest, and every world the server sends (the setup's, a full
//! state) has them installed under the match's own values before it is
//! played.
//!
//! Time sync works on how early this client's pads reach the server. The
//! server measures it ([`Message::Frames`]'s `lead`: the worst over the last
//! fraction of a second) and the client aims for a small lead, running a few
//! percent fast or slow to get there, or jumping when it is far off (on
//! joining, after a stall).
//!
//! A pad that reaches the server after its tick is lost: the server repeats
//! the seat's last one, and this machine's own player is pulled back to
//! what the server played. So the lead aimed for follows the connection: it
//! grows at once when the worst report comes in with no tick to spare, and
//! shrinks a tick at a time after a steady stretch with room over. The
//! input delay is picked from the round trip when the match starts: more of
//! it on a long connection keeps the other machines' guesses of our pads
//! short.

use std::net::SocketAddr;
use std::time::Duration;

use street_sim::{Input, World};

use crate::disc::Disc;
use crate::game::Match;
use crate::protocol::{
    CloseReason, Join, MAX_DATAGRAM, MatchSetup, Message, Reassembly, Refusal, SeatEntry,
};
use crate::session::{Advanced, Session, SessionConfig, SessionStats};
use crate::transport::Transport;
use crate::wire::DecodeError;

/// Tuning for a [`Client`].
#[derive(Clone, Copy, Debug)]
pub struct ClientConfig {
    /// The session's tuning; its input delay is the least used (see
    /// [`ClientConfig::max_input_delay`]).
    pub session: SessionConfig,
    /// How many ticks early our pads should reach the server, at least.
    pub target_lead: i16,
    /// The most the lead aimed for grows to over a jittery connection.
    pub max_lead: i16,
    /// The most input delay a long round trip is given (from
    /// [`SessionConfig::input_delay`] up to this).
    pub max_input_delay: u32,
    /// Steady: the input delay covers the whole round trip, so the others'
    /// pads are in before their ticks are drawn and hardly a guess is made
    /// (lockstep's feel, with a rollback instead of a freeze when a pad is
    /// late). Otherwise half the round trip: quicker to the press, more
    /// guessed and corrected.
    pub steady: bool,
    /// Give up after hearing nothing from the server for this long.
    pub timeout: Duration,
    /// The same once the match is under way, when no loading can stall a
    /// game: the most a dead server's world stands frozen.
    pub play_timeout: Duration,
    /// Resend the join this often until answered.
    pub join_retry: Duration,
    /// Ping (and, in the lobby, repeat the ready flag) this often.
    pub ping_every: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            session: SessionConfig::default(),
            target_lead: 2,
            max_lead: 12,
            max_input_delay: 15,
            steady: true,
            timeout: Duration::from_secs(5),
            play_timeout: Duration::from_secs(5),
            join_retry: Duration::from_millis(250),
            ping_every: Duration::from_millis(500),
        }
    }
}

/// Where the client is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Waiting for an answer to the join.
    Joining,
    /// Seated, waiting for the match to start.
    Lobby,
    /// Playing: [`Client::advance`] runs the session.
    Playing,
    /// Refused, closed, timed out or left.
    Ended,
}

/// Something the game may want to show.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientEvent {
    /// Our rematch vote was sent and will be retried until restart.
    Rematch {
        round: u32,
    },
    /// The server reset the match; setup() contains its new initial state.
    Restarted {
        round: u32,
    },
    /// Seated: these seats are ours (again, after moving side).
    Welcome {
        seats: Vec<u8>,
    },
    /// The creator replaced the match's setup in the lobby: load
    /// [`Client::setup`] again.
    Setup {
        version: u32,
    },
    Refused(Refusal),
    /// The occupied seats changed.
    Lobby(Vec<SeatEntry>),
    /// The match is under way; the session starts at `tick`.
    Started {
        tick: u64,
    },
    /// The server replaced our state with its own at `tick`.
    Resynced {
        tick: u64,
    },
    Closed(CloseReason),
    /// Nothing from the server for [`ClientConfig::timeout`].
    TimedOut,
}

/// Ticks the clock may jump at once before it is clamped to a frame's worth.
const MAX_TICKS_PER_CALL: f64 = 30.0;
/// Lead errors beyond this many ticks are corrected at once.
const JUMP_AT: i64 = 10;
/// How much faster or slower the clock runs to correct a small lead error.
const NUDGE: f64 = 0.06;
/// Most ticks of local pads in one message.
const MAX_INPUT_RUN: usize = 32;
/// Ticks of steady lead reports before the lead aimed for comes down a tick.
const LEAD_EASE: u64 = 600;
/// The worst lead over such a stretch that lets it come down.
const LEAD_ROOM: i16 = 3;
/// Ticks of late pads before the lead aimed for goes up: the server waits
/// a while for a late pad ([`crate::ServerConfig::grace`]), so one hitch
/// costs nothing here, and a lead raised for it would make every tick's
/// guess at the others longer until it came down again.
const RAISE_AFTER: u64 = 90;
/// Round trips kept for the input delay (ten seconds of pings).
const RTT_KEPT: usize = 20;
/// Ticks between steps of the input delay once the match is under way.
const DELAY_STEP: u64 = 120;

/// A connection to a match on a server.
pub struct Client {
    transport: Box<dyn Transport>,
    server: SocketAddr,
    cfg: ClientConfig,
    /// This machine's disc: the tables every world is played on.
    disc: std::sync::Arc<Disc>,
    join: Join,
    status: Status,
    seats: Vec<u8>,
    setup: Option<MatchSetup>,
    /// The setup's changes in the lobby, as last welcomed.
    setup_version: u32,
    /// The lobby showed a newer setup than ours: join again for it.
    behind: bool,
    lobby: Vec<SeatEntry>,
    /// The side asked for in the lobby, until our seats are on it.
    wanted_side: Option<u8>,
    /// What our people have picked in the lobby.
    pick: Vec<u8>,
    /// Our setup for the match as picked (we created it), resent until
    /// the server welcomes us to a newer setup than `setup_version`.
    proposed: Option<(MatchSetup, u32)>,
    proposed_at: Option<Duration>,
    session: Option<Session>,
    ready: bool,
    round: u32,
    rematch_requested: bool,
    rematch_sent_at: Option<Duration>,
    now: Duration,
    heard_at: Duration,
    join_sent_at: Option<Duration>,
    ping_sent_at: Option<Duration>,
    inputs_sent_at: Duration,
    rtt: Option<Duration>,
    /// The last [`RTT_KEPT`] round trips, for the least of them.
    rtts: std::collections::VecDeque<Duration>,
    reassembly: Reassembly,
    part_id: u16,
    /// The latest `(lead, heard)` from the server.
    lead: Option<i16>,
    /// Lead reports are ignored until the prediction reaches this tick
    /// (they describe pads sent before the last jump).
    lead_guard: u64,
    /// No step of the input delay before the prediction reaches this tick.
    delay_guard: u64,
    /// The lead aimed for now, from [`ClientConfig::target_lead`] up to
    /// [`ClientConfig::max_lead`].
    lead_target: i16,
    /// No raise of the lead aimed for before the prediction reaches this
    /// tick (the last raise has to show in the reports first).
    raise_guard: u64,
    /// The worst lead reported since `ease_from`.
    lead_lowest: i16,
    ease_from: u64,
    /// The tick the reports of late pads (a lead under 1) began.
    late_since: Option<u64>,
    budget: f64,
    frame_at: Option<Duration>,
    events: Vec<ClientEvent>,
}

impl Client {
    /// Start joining `server` with `join`, playing on `disc`; the join
    /// goes out (with the disc's digest) on the first [`Client::update`].
    pub fn new(
        transport: Box<dyn Transport>,
        server: SocketAddr,
        mut join: Join,
        cfg: ClientConfig,
        disc: std::sync::Arc<Disc>,
    ) -> Self {
        join.disc = disc.digest();
        Self {
            transport,
            server,
            cfg,
            disc,
            join,
            status: Status::Joining,
            seats: Vec::new(),
            setup: None,
            setup_version: 0,
            behind: false,
            lobby: Vec::new(),
            wanted_side: None,
            pick: Vec::new(),
            proposed: None,
            proposed_at: None,
            session: None,
            ready: false,
            round: 0,
            rematch_requested: false,
            rematch_sent_at: None,
            now: Duration::ZERO,
            heard_at: Duration::ZERO,
            join_sent_at: None,
            ping_sent_at: None,
            inputs_sent_at: Duration::ZERO,
            rtt: None,
            rtts: std::collections::VecDeque::new(),
            reassembly: Reassembly::default(),
            part_id: 0,
            lead: None,
            lead_guard: 0,
            delay_guard: 0,
            lead_target: cfg.target_lead,
            raise_guard: 0,
            lead_lowest: i16::MAX,
            ease_from: 0,
            late_since: None,
            budget: 0.0,
            frame_at: None,
            events: Vec::new(),
        }
    }

    /// Read what the server sent and send what is due. Call once a frame
    /// with a monotonic clock.
    pub fn update(&mut self, now: Duration) -> Vec<ClientEvent> {
        if self.join_sent_at.is_none() {
            self.heard_at = now;
        }
        self.now = now;
        while let Some((from, bytes)) = self.transport.recv() {
            if from != self.server {
                continue;
            }
            match Message::decode(&bytes) {
                Ok(m) => self.handle(m),
                Err(DecodeError::Version(_)) => self.end(ClientEvent::Refused(Refusal::Version)),
                Err(_) => {}
            }
        }
        if self.rematch_requested
            && self.status == Status::Playing
            && self
                .rematch_sent_at
                .is_none_or(|t| now.saturating_sub(t) >= self.cfg.join_retry)
        {
            self.rematch_sent_at = Some(now);
            self.send(&Message::Rematch { round: self.round });
        }
        match self.status {
            Status::Joining => {
                if self
                    .join_sent_at
                    .is_none_or(|t| now.saturating_sub(t) >= self.cfg.join_retry)
                {
                    self.join_sent_at = Some(now);
                    self.send(&Message::Join(self.join.clone()));
                }
            }
            Status::Lobby | Status::Playing => {
                if self
                    .ping_sent_at
                    .is_none_or(|t| now.saturating_sub(t) >= self.cfg.ping_every)
                {
                    self.ping_sent_at = Some(now);
                    self.send(&Message::Ping {
                        id: now.as_millis() as u32,
                    });
                    if self.status == Status::Lobby {
                        self.send(&Message::Ready { ready: self.ready });
                        self.send_lobby();
                    }
                }
                if self.status == Status::Lobby && self.behind {
                    self.rejoin();
                }
                // Keep acknowledgements flowing while the session waits.
                if self.status == Status::Playing
                    && now.saturating_sub(self.inputs_sent_at) >= Duration::from_millis(20)
                {
                    self.send_inputs();
                }
            }
            Status::Ended => {}
        }
        let timeout = if self.status == Status::Playing {
            self.cfg.play_timeout.min(self.cfg.timeout)
        } else {
            self.cfg.timeout
        };
        if self.status != Status::Ended && now.saturating_sub(self.heard_at) > timeout {
            self.end(ClientEvent::TimedOut);
        }
        std::mem::take(&mut self.events)
    }

    /// Vote to replay after the match ends. One vote covers this machine's
    /// local seats; every seated machine must agree. Spectators cannot vote.
    pub fn request_rematch(&mut self) -> bool {
        if self.status != Status::Playing
            || self.seats.is_empty()
            || !self
                .world()
                .is_some_and(|w| matches!(w.phase, street_sim::Phase::Over { .. }))
        {
            return false;
        }
        if !self.rematch_requested {
            self.rematch_requested = true;
            self.rematch_sent_at = Some(self.now);
            self.send(&Message::Rematch { round: self.round });
            self.events.push(ClientEvent::Rematch { round: self.round });
        }
        true
    }

    pub fn round(&self) -> u32 {
        self.round
    }

    /// In the lobby: move our people to `team` (0 home, 1 away), if it has
    /// room; [`ClientEvent::Welcome`] brings the new seats.
    pub fn choose_side(&mut self, team: u8) {
        self.wanted_side = Some(team);
        if self.status == Status::Lobby {
            self.send(&Message::Side { team });
        }
    }

    /// In the lobby: what our people have picked, for everyone's lobby
    /// list (at most [`crate::protocol::MAX_PICK`] bytes).
    pub fn set_pick(&mut self, pick: Vec<u8>) {
        if pick.len() > crate::protocol::MAX_PICK || pick == self.pick {
            return;
        }
        self.pick = pick;
        if self.status == Status::Lobby {
            self.send(&Message::Pick {
                pick: self.pick.clone(),
            });
        }
    }

    /// In the lobby, as the match's creator: replace its setup with
    /// `setup`, the match as everyone picked it. Resent until the server
    /// welcomes us to it ([`ClientEvent::Setup`]).
    pub fn propose_setup(&mut self, setup: MatchSetup) {
        self.proposed = Some((setup, self.setup_version));
        self.proposed_at = None;
        self.send_lobby();
    }

    /// The setup's changes in the lobby, as last welcomed.
    pub fn setup_version(&self) -> u32 {
        self.setup_version
    }

    /// Join again (the server welcomes us again), at most every
    /// [`ClientConfig::join_retry`].
    fn rejoin(&mut self) {
        if self
            .join_sent_at
            .is_none_or(|t| self.now.saturating_sub(t) >= self.cfg.join_retry)
        {
            self.join_sent_at = Some(self.now);
            self.send(&Message::Join(self.join.clone()));
        }
    }

    /// The lobby's repeats: the side asked for, the pick, a proposed setup.
    fn send_lobby(&mut self) {
        if self.status != Status::Lobby {
            return;
        }
        if let (Some(team), Some(setup)) = (self.wanted_side, &self.setup)
            && !self
                .seats
                .iter()
                .all(|&s| setup.seat_teams[usize::from(s)] == team)
        {
            self.send(&Message::Side { team });
        }
        if !self.pick.is_empty() {
            self.send(&Message::Pick {
                pick: self.pick.clone(),
            });
        }
        let now = self.now;
        if let Some((setup, _)) = &self.proposed
            && self
                .proposed_at
                .is_none_or(|t| now.saturating_sub(t) >= Duration::from_secs(1))
        {
            let m = Message::Setup {
                setup: setup.clone(),
            };
            self.proposed_at = Some(now);
            self.send(&m);
        }
    }

    /// Say whether we are ready to start.
    pub fn set_ready(&mut self, ready: bool) {
        self.ready = ready;
        if self.status == Status::Lobby {
            self.send(&Message::Ready { ready });
        }
    }

    /// How many ticks to simulate this frame, from the real time since the
    /// last call, nudged to keep our pads arriving just in time.
    pub fn ticks_due(&mut self, now: Duration) -> u32 {
        if self.session.is_none() || self.status != Status::Playing {
            return 0;
        }
        let dt = self
            .frame_at
            .map_or(Duration::ZERO, |t| now.saturating_sub(t));
        self.frame_at = Some(now);
        self.due(dt.as_secs_f64() * f64::from(street_sim::TICK_HZ))
    }

    /// [`Client::ticks_due`] for a caller already stepped at the tick rate
    /// (the game's fixed update): one tick a call, nudged the same way. A
    /// fixed step that also measured the real time would count it twice,
    /// stepping two ticks in one frame and none in the next.
    pub fn ticks_due_fixed(&mut self) -> u32 {
        self.due(1.0)
    }

    /// Add `ticks` of time, at the rate the lead asks for, and take the
    /// whole ticks due.
    fn due(&mut self, ticks: f64) -> u32 {
        let Some(s) = &self.session else {
            return 0;
        };
        if self.status != Status::Playing {
            return 0;
        }
        let mut rate = 1.0;
        if let Some(lead) = self.lead
            && s.tick() >= self.lead_guard
        {
            let err = i64::from(lead) - i64::from(self.lead_target);
            if err.abs() > JUMP_AT {
                self.budget -= err as f64;
                self.lead = None;
                self.lead_guard = s.tick() + (-err).max(0) as u64 + self.settle_ticks();
            } else if err < 0 {
                rate = 1.0 + NUDGE;
            } else if err > 2 {
                rate = 1.0 - NUDGE;
            }
        }
        self.budget += ticks * rate;
        let n = self.budget.floor().clamp(0.0, MAX_TICKS_PER_CALL);
        self.budget -= n;
        n as u32
    }

    /// Set a local seat's pad for the next [`Client::advance`].
    pub fn add_local_input(&mut self, seat: u8, input: Input) {
        if let Some(s) = &mut self.session {
            s.add_local_input(seat, input);
        }
    }

    /// Simulate one tick (see [`Session::advance`]) and send our pads.
    pub fn advance(&mut self) -> Option<Advanced> {
        let step = self.session.as_mut()?.advance();
        self.send_inputs();
        step
    }

    /// Leave the match; our seats go to the computer.
    pub fn leave(&mut self) {
        if self.status != Status::Ended {
            for _ in 0..3 {
                self.send(&Message::Leave);
            }
            self.status = Status::Ended;
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// Our seats (empty until welcomed).
    pub fn seats(&self) -> &[u8] {
        &self.seats
    }

    pub fn setup(&self) -> Option<&MatchSetup> {
        self.setup.as_ref()
    }

    /// The lobby as last heard.
    pub fn lobby(&self) -> &[SeatEntry] {
        &self.lobby
    }

    /// The predicted world, to draw.
    pub fn world(&self) -> Option<&World> {
        self.session.as_ref().map(Session::world)
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn stats(&self) -> Option<SessionStats> {
        self.session.as_ref().map(Session::stats)
    }

    /// What happened in the ticks confirmed since the last call (see
    /// [`Session::take_confirmed_events`]).
    pub fn take_confirmed_events(&mut self) -> Vec<(u64, Vec<street_sim::Event>)> {
        self.session
            .as_mut()
            .map(Session::take_confirmed_events)
            .unwrap_or_default()
    }

    /// How far rollbacks moved the drawn world since the last call (see
    /// [`Session::take_correction`]).
    pub fn take_correction(&mut self) -> Option<crate::session::Correction> {
        self.session.as_mut()?.take_correction()
    }

    /// The present's events since the last call, each once (see
    /// [`Session::take_events`]).
    pub fn take_events(&mut self) -> Vec<street_sim::Event> {
        self.session
            .as_mut()
            .map(Session::take_events)
            .unwrap_or_default()
    }

    /// The smoothed round trip time.
    pub fn rtt(&self) -> Option<Duration> {
        self.rtt
    }

    /// The lead our pads are aimed to reach the server with, in ticks.
    pub fn lead_target(&self) -> i16 {
        self.lead_target
    }

    /// A lead report from the server: raise the lead aimed for by what the
    /// pads lack when they have been late (a lead under 1) for
    /// [`RAISE_AFTER`] ticks, and lower it a tick after [`LEAD_EASE`] ticks
    /// that always had [`LEAD_ROOM`] over.
    fn note_lead(&mut self, lead: i16) {
        let Some(tick) = self.session.as_ref().map(Session::tick) else {
            return;
        };
        if tick < self.lead_guard {
            return;
        }
        if lead < 1 {
            let since = *self.late_since.get_or_insert(tick);
            if tick >= self.raise_guard && tick >= since + RAISE_AFTER {
                let up = (1 - lead).clamp(1, 4);
                self.lead_target = (self.lead_target + up).min(self.cfg.max_lead);
                self.raise_guard = tick + self.settle_ticks();
                self.late_since = None;
            }
            self.lead_lowest = i16::MAX;
            self.ease_from = tick;
            return;
        }
        self.late_since = None;
        self.lead_lowest = self.lead_lowest.min(lead);
        if tick >= self.ease_from + LEAD_EASE {
            if self.lead_lowest >= LEAD_ROOM && self.lead_target > self.cfg.target_lead {
                self.lead_target -= 1;
            }
            self.lead_lowest = i16::MAX;
            self.ease_from = tick;
        }
    }

    /// Change our confirmed state behind the server's back (tests only).
    #[doc(hidden)]
    pub fn tamper(&mut self, f: impl FnOnce(&mut Match)) {
        if let Some(s) = &mut self.session {
            s.tamper(f);
        }
    }

    fn handle(&mut self, m: Message) {
        if let Message::Round { round, bytes } = m {
            let Ok(inner) = Message::decode(&bytes) else {
                return;
            };
            if !inner.round_scoped() {
                return;
            }
            if self.status == Status::Joining && matches!(inner, Message::Welcome { .. }) {
                self.round = round;
            }
            if round == self.round {
                self.handle_current(inner);
            }
            return;
        }
        if self.round > 0 && m.round_scoped() {
            return;
        }
        self.handle_current(m);
    }

    fn handle_current(&mut self, m: Message) {
        self.heard_at = self.now;
        match m {
            Message::Restart { round, setup } => {
                if self.status == Status::Ended || round < self.round {
                    return;
                }
                if round > self.round {
                    if round != self.round.saturating_add(1) || self.status == Status::Joining {
                        return;
                    }
                    let mut setup = setup;
                    if self.disc.install(&mut setup.world).is_err() {
                        return self.end(ClientEvent::Refused(Refusal::Disc));
                    }
                    self.round = round;
                    self.setup = Some(setup);
                    self.session = None;
                    self.status = Status::Lobby;
                    self.ready = true;
                    self.rematch_requested = false;
                    self.rematch_sent_at = None;
                    self.lead = None;
                    self.lead_guard = 0;
                    self.budget = 0.0;
                    self.frame_at = None;
                    self.events.push(ClientEvent::Restarted { round });
                }
                self.send(&Message::RestartAck { round });
            }
            Message::Part {
                id,
                index,
                count,
                bytes,
            } => {
                let whole = self
                    .reassembly
                    .add(self.server, self.now, id, index, count, bytes);
                if let Some(m) = whole.and_then(|b| Message::decode(&b).ok())
                    && !matches!(m, Message::Part { .. })
                {
                    self.handle(m);
                }
            }
            Message::Welcome {
                seats,
                setup,
                setup_version,
            } => {
                // Again in the lobby: our seats moved, or the setup is new.
                if self.status == Status::Lobby
                    && (seats != self.seats || setup_version != self.setup_version)
                {
                    let mut setup = setup;
                    if self.disc.install(&mut setup.world).is_err() {
                        return self.end(ClientEvent::Refused(Refusal::Disc));
                    }
                    let fresh = setup_version != self.setup_version;
                    self.setup_version = setup_version;
                    self.behind = false;
                    self.setup = Some(setup);
                    if seats != self.seats {
                        self.seats = seats.clone();
                        self.events.push(ClientEvent::Welcome { seats });
                    }
                    if fresh {
                        if self
                            .proposed
                            .as_ref()
                            .is_some_and(|&(_, before)| setup_version != before)
                        {
                            self.proposed = None;
                        }
                        self.events.push(ClientEvent::Setup {
                            version: setup_version,
                        });
                    }
                    return;
                }
                if self.status == Status::Joining {
                    self.setup_version = setup_version;
                    // The match's world on this machine's disc's tables.
                    let mut setup = setup;
                    if self.disc.install(&mut setup.world).is_err() {
                        return self.end(ClientEvent::Refused(Refusal::Disc));
                    }
                    if let Some(t) = self.join_sent_at {
                        self.sample_rtt(self.now.saturating_sub(t));
                    }
                    self.status = Status::Lobby;
                    self.seats = seats.clone();
                    self.setup = Some(setup);
                    self.events.push(ClientEvent::Welcome { seats });
                }
            }
            Message::Refused { reason } => {
                if self.status == Status::Joining {
                    self.end(ClientEvent::Refused(reason));
                }
            }
            Message::Lobby {
                seats,
                setup_version,
            } => {
                // A newer setup we missed the welcome to: ask again (and
                // keep asking, [`Client::update`]).
                if self.status == Status::Lobby && setup_version != self.setup_version {
                    self.behind = true;
                    self.rejoin();
                }
                if self.lobby != seats {
                    self.lobby = seats.clone();
                    self.events.push(ClientEvent::Lobby(seats));
                }
            }
            Message::Start { tick } => {
                if self.status == Status::Lobby {
                    self.start(tick, None);
                }
            }
            Message::Frames {
                first,
                frames,
                ack,
                heard: _,
                lead,
            } => {
                if self.status == Status::Lobby {
                    self.start(first + frames.len() as u64, None);
                }
                if let Some(s) = &mut self.session {
                    s.confirm(first, &frames);
                    s.acknowledge_inputs(ack);
                    if lead != i16::MAX {
                        self.lead = Some(lead);
                        self.note_lead(lead);
                    }
                }
            }
            Message::State { tick, state } => {
                let Ok(mut state) = Match::from_snapshot(&state) else {
                    return;
                };
                if state.tick() != tick || self.disc.install(&mut state.world).is_err() {
                    return;
                }
                match self.status {
                    Status::Lobby => self.start(tick, Some(state)),
                    Status::Playing => {
                        if let Some(s) = &mut self.session {
                            s.resync(state);
                            self.events.push(ClientEvent::Resynced { tick });
                        }
                    }
                    _ => {}
                }
            }
            Message::Pong { id, .. } => {
                let sent = Duration::from_millis(u64::from(id));
                let now = Duration::from_millis(u64::from(self.now.as_millis() as u32));
                if now >= sent {
                    self.sample_rtt(now - sent);
                    self.adapt_delay();
                }
            }
            Message::Closed { reason } => self.end(ClientEvent::Closed(reason)),
            // Client-to-server messages.
            Message::Join(_)
            | Message::Rematch { .. }
            | Message::RestartAck { .. }
            | Message::Round { .. }
            | Message::List { .. }
            | Message::Matches { .. }
            | Message::Ready { .. }
            | Message::Side { .. }
            | Message::Pick { .. }
            | Message::Setup { .. }
            | Message::Inputs { .. }
            | Message::Checksums { .. }
            | Message::Ping { .. }
            | Message::Leave => {}
        }
    }

    /// The match is under way and the server is at about `server_tick`:
    /// start the session (from the setup's world, or `state`) and aim the
    /// clock so our pads arrive `target_lead` ticks early.
    fn start(&mut self, server_tick: u64, state: Option<Match>) {
        let Some(setup) = &self.setup else {
            return;
        };
        let state = state.unwrap_or_else(|| setup.start());
        let mut cfg = self.cfg.session;
        cfg.input_delay = self.input_delay();
        let session = Session::new(state, self.seats.clone(), cfg);
        let aim = server_tick as i64 + self.rtt_ticks() as i64 + i64::from(self.lead_target)
            - i64::from(cfg.input_delay);
        let ahead = (aim - session.tick() as i64).max(0);
        self.budget = ahead as f64;
        self.lead_guard = session.tick() + ahead as u64 + self.settle_ticks();
        let tick = session.tick();
        self.session = Some(session);
        self.status = Status::Playing;
        self.events.push(ClientEvent::Started { tick });
    }

    fn send_inputs(&mut self) {
        let Some(s) = &mut self.session else {
            return;
        };
        let reports = s.take_reports();
        let need = s.need();
        let seats = s.local_seats().to_vec();
        let (start, mut ticks) = s.local_inputs(s.input_ack(), MAX_INPUT_RUN);
        loop {
            let m = Message::Inputs {
                need,
                start,
                seats: seats.clone(),
                ticks,
            };
            if m.encode().len() <= MAX_DATAGRAM {
                self.send(&m);
                break;
            }
            let Message::Inputs { ticks: t, .. } = m else {
                unreachable!()
            };
            ticks = t;
            ticks.truncate(ticks.len() / 2);
        }
        if !reports.is_empty() {
            self.send(&Message::Checksums { reports });
        }
        self.inputs_sent_at = self.now;
    }

    fn send(&mut self, m: &Message) {
        let envelope = (self.round > 0 && m.round_scoped()).then(|| Message::Round {
            round: self.round,
            bytes: m.encode(),
        });
        let m = envelope.as_ref().unwrap_or(m);
        self.part_id = self.part_id.wrapping_add(1);
        for d in m.datagrams(self.part_id) {
            self.transport.send(self.server, &d);
        }
    }

    fn end(&mut self, why: ClientEvent) {
        self.status = Status::Ended;
        self.events.push(why);
    }

    fn sample_rtt(&mut self, sample: Duration) {
        if self.rtts.len() == RTT_KEPT {
            self.rtts.pop_front();
        }
        self.rtts.push_back(sample);
        self.rtt = Some(match self.rtt {
            None => sample,
            Some(r) => r.mul_f64(0.8) + sample.mul_f64(0.2),
        });
    }

    /// The round trip in ticks (6, a tenth of a second, before measuring).
    fn rtt_ticks(&self) -> u64 {
        self.rtt.map_or(6, |r| {
            (r.as_secs_f64() * f64::from(street_sim::TICK_HZ)).ceil() as u64
        })
    }

    /// The input delay for a match starting now, from the least recent
    /// round trip in ticks (the least, as a frame that stalls, loading the
    /// match, only ever adds to a round trip): steady, the round trip and
    /// the lead aimed for less one, which leaves the prediction a tick
    /// ahead of the confirmations (a delay past that would put it behind
    /// them, drawn as each frame lands); else half the round trip. Within
    /// the config's least and most.
    fn input_delay(&self) -> u32 {
        let least = self.cfg.session.input_delay;
        let delay = self.rtts.iter().min().map_or(0, |r| {
            let rtt = (r.as_secs_f64() * f64::from(street_sim::TICK_HZ)).ceil() as u32;
            if self.cfg.steady {
                rtt + self.cfg.target_lead.max(1) as u32 - 1
            } else {
                rtt.div_ceil(2)
            }
        });
        delay.clamp(least, self.cfg.max_input_delay.max(least))
    }

    /// Move the input delay toward the one the round trips now ask for: at
    /// once while the intros hold the world (the match's start, when
    /// loading has just inflated the round trips), else a tick at a time
    /// every [`DELAY_STEP`] ticks, down as soon as asked but up only when
    /// asked for two more (a round trip on a tick's edge would flip it
    /// back and forth, each flip a pad dropped or held). The clock moves by
    /// the change, so the pads still reach the server as early as before.
    fn adapt_delay(&mut self) {
        let want = self.input_delay();
        let Some(s) = &mut self.session else {
            return;
        };
        let have = s.config().input_delay;
        let tick = s.tick();
        let next = if s.current().intro_hold > 0 {
            want
        } else if tick >= self.delay_guard {
            if want > have + 1 {
                have + 1
            } else if want < have {
                have - 1
            } else {
                have
            }
        } else {
            have
        };
        if next == have {
            return;
        }
        s.set_input_delay(next);
        self.budget += f64::from(have) - f64::from(next);
        self.delay_guard = tick + DELAY_STEP;
    }

    /// Ticks for a clock change to show in the server's lead reports.
    fn settle_ticks(&self) -> u64 {
        self.rtt_ticks() + 45
    }
}
