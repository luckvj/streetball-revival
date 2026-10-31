//! The wire protocol: every message, and how it is laid out in a datagram.
//!
//! A datagram starts with three bytes: [`MAGIC`] (`'S'`, never 0xE5, the
//! MyBaller sealed-datagram marker), the protocol [`VERSION`], and the
//! message kind. The fields follow, as `wire` writes them: varints for
//! counts and ticks, fixed little-endian integers for ids and checksums,
//! length-prefixed UTF-8 for text, 8 bytes per pad ([`Input::to_bytes`]).
//!
//! Runs of pads are delta-coded: per tick a bit mask says which seats'
//! pads changed since the tick before, and only those follow. The first
//! tick of a message is coded against "no input", so every datagram reads
//! on its own.
//!
//! A message longer than [`MAX_DATAGRAM`] (a match setup or a full state)
//! goes as several [`Message::Part`]s, reassembled by [`Reassembly`].

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use street_sim::{Input, Player, Rules, World};

use crate::game::{Frame, MAX_SEATS, Match};
use crate::wire::{DecodeError, Reader, Result, Writer};

/// First byte of every datagram: `'S'`.
pub const MAGIC: u8 = 0x53;
/// Bumped whenever a message's layout changes (12: the join's disc
/// digest, the refusals for another disc and for a match that isn't there;
/// a match's world without the disc's tables; 13: a staged script's replay
/// in the match's replays and the world's events, the freezes' replay
/// flag in the disc's digest, each side's city and name in the match's
/// looks; 14: the world's story weight, the scores at the meters' fills,
/// the AI's game+0x4c0 and a gamebreaker basket's dunk in the match's
/// world, snapshot 64; 15: the sides' database teams in the match's world,
/// snapshot 65; 16: an input's actors off its person's screen in its view
/// word's top bits, snapshot 66; 17: the computer's "loose ball" words in
/// the match's world, snapshot 67; 18: the computer's drive and "loose
/// ball" words and a rebound jump's path, clip and facing words, snapshot
/// 68; 19: the actors' update order in the match's world, snapshot 69;
/// 20: the separate rebound event, snapshot 70; 21: passout identity through
/// the catch, snapshot 71; 22: the head passout's defender and release
/// wait, snapshot 72; 23: the lobby before the match, people choosing a
/// side, each seat's pick, the creator's final setup and its generation;
/// 24: the match's opening intros in its setup and its state).
pub const VERSION: u8 = 24;
/// The largest datagram sent; longer messages are split into parts. This
/// leaves room under a 1280-byte IPv6 MTU for a relay's sealing.
pub const MAX_DATAGRAM: usize = 1200;
/// Longest name (player or match), in bytes.
pub const MAX_NAME: usize = 32;
/// Longest pick a seat shows the lobby ([`SeatEntry::pick`]), in bytes.
pub const MAX_PICK: usize = 48;
/// Most ticks of pads or frames in one message.
pub const MAX_RUN: usize = 64;

/// Someone joining from one machine; a machine may bring several people.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joiner {
    pub name: String,
    /// The side they want, or `None` for whichever has room.
    pub team: Option<u8>,
}

/// Client to server: join the match called `match_name`, creating it with
/// `create` if there is none (a join without one is refused then:
/// [`Refusal::NoMatch`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Join {
    pub match_name: String,
    pub players: Vec<Joiner>,
    pub create: Option<MatchSetup>,
    /// The digest of the joiner's disc data ([`crate::Disc::digest`]),
    /// which must be the server's: every machine in a match plays its own
    /// disc's tables ([`Refusal::Disc`]). [`crate::Client`] fills it in.
    pub disc: [u8; 32],
}

/// Everything a match starts from. The creating client builds it from its
/// own disc: the line-up's records and looks, and the world at tick 0. The
/// world carries the match's own values of the disc's data (the players'
/// gait profiles, the staged players' scales) and none of its tables,
/// which every machine installs from its own disc ([`crate::Disc`]).
#[derive(Clone, Debug, PartialEq)]
pub struct MatchSetup {
    /// The court's id, for drawing (`setup.rs`'s `MatchSetup::court`).
    pub court: String,
    /// The two teams' ids, for drawing.
    pub teams: [u16; 2],
    /// Each world player's database record, for drawing.
    pub roster: Vec<u16>,
    /// The seed the world was made with (the world carries its own
    /// generator; this is for display and logs).
    pub seed: u32,
    /// The side of each seat 1P..6P.
    pub seat_teams: [u8; MAX_SEATS],
    /// The world at tick 0: hoops, players and their ratings, rules.
    pub world: World,
    /// Anything else the game wants to hand every client (looks of created
    /// players, say). The server passes it on untouched.
    pub extra: Vec<u8>,
    /// The match's roster: the creator's, packed
    /// (`street_assets::roster_share::MatchRoster`); empty for the disc's.
    /// Every client plays it, and the server checks it and the world
    /// against it ([`crate::roster::check_setup`]).
    pub roster_data: Vec<u8>,
    /// The updates the match's opening intros hold play for (the game's
    /// `intro.rs`: the court's script, then the away side's), at most
    /// [`crate::game::MAX_INTRO`]; 0 for none. Every machine draws them
    /// from the seed and holds the game as long ([`Match::intro_hold`]).
    pub intro: u16,
}

impl MatchSetup {
    /// A match for tests: the Vol. 2 hoop placement, three average players
    /// a side lined up as `street-game`'s `setup.rs` lines them up, home
    /// seats 1P 3P 5P and away seats 2P 4P 6P. Its world has none of the
    /// match's own disc values (no gait profiles), so only the tests'
    /// stand-ins play it: a server on a real disc refuses it.
    pub fn standard(rules: Rules, seed: u32) -> Self {
        let hoops = street_sim::Hoop::native();
        let mut players = Vec::new();
        for team in 0..2 {
            // In its own half, facing the basket it attacks.
            let side = street_sim::restart::side(team);
            let x = -12.0 * side;
            let heading = 1.57 * side;
            for k in 0..3 {
                players.push(Player::new(team, [x, (k as f32 - 1.0) * 9.0], heading));
            }
        }
        Self {
            court: String::new(),
            teams: [0, 1],
            roster: vec![0; players.len()],
            seed,
            seat_teams: [0, 1, 0, 1, 0, 1],
            world: World::new(hoops, players, rules, seed),
            extra: Vec::new(),
            roster_data: Vec::new(),
            intro: 0,
        }
    }

    /// The match at its start: the world at tick 0, every seat empty, the
    /// intros holding play.
    pub fn start(&self) -> Match {
        Match::new(self.world.clone(), self.seat_teams).with_intro(self.intro)
    }

    /// The match's rules (they travel inside the world).
    pub fn rules(&self) -> &Rules {
        &self.world.rules
    }
}

/// One occupied seat, as the lobby shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeatEntry {
    pub seat: u8,
    pub team: u8,
    pub name: String,
    pub ready: bool,
    /// What the seat's machine has picked so far in the lobby (its team,
    /// players, court): the game's own bytes, which the server passes on.
    pub pick: Vec<u8>,
}

/// A public match listing; it contains no player definitions or save data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchInfo {
    pub name: String,
    pub court: String,
    pub players: u8,
    pub started: bool,
    pub finished: bool,
}

/// Why a join was turned down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The server speaks another protocol version.
    Version,
    /// No free seat on the side asked for.
    Full,
    /// The server hosts as many matches as it will.
    Busy,
    /// The join or its setup did not make sense.
    Invalid,
    /// The match's roster breaks the online rules
    /// (`street_assets::roster_rules`), or its world does not hold it.
    Roster,
    /// The match's roster is built on a roster mod the server does not
    /// hold (`street-server --roster-db`).
    RosterMod,
    /// The joiner's disc data differs from the server's (its digest,
    /// [`Join::disc`]): another region's disc, or a modded disc.
    Disc,
    /// No match by that name, and the join brought no setup to make one.
    NoMatch,
}

/// Why the server ended a client's part in a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The game is over.
    Over,
    /// Nothing heard from the client for too long.
    TimedOut,
    /// The server is shutting down.
    Shutdown,
    /// Only this match's referee failed; the server still runs other matches.
    Fault,
}

/// Every message. Client-to-server first, then server-to-client.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// Vote to replay the completed round; spectators cannot vote.
    Rematch {
        round: u32,
    },
    /// Reliable reset announcement, repeated until RestartAck.
    Restart {
        round: u32,
        setup: MatchSetup,
    },
    RestartAck {
        round: u32,
    },
    /// Generation envelope for stateful traffic after the first round.
    /// Keeping generations outside ticks rejects delayed old-round packets.
    Round {
        round: u32,
        bytes: Vec<u8>,
    },
    /// Ask a server for its named matches.
    List {
        id: u32,
    },
    Matches {
        id: u32,
        matches: Vec<MatchInfo>,
    },
    /// Join (or create) a match. Resent until a `Welcome` or `Refused`.
    Join(Join),
    /// Ready (or not) to start; the match starts once everyone is.
    Ready {
        ready: bool,
    },
    /// In the lobby: move this machine's people to `team`'s free seats.
    Side {
        team: u8,
    },
    /// In the lobby: what this machine's people have picked
    /// ([`SeatEntry::pick`]).
    Pick {
        pick: Vec<u8>,
    },
    /// In the lobby, from the machine that created the match: the match as
    /// everyone picked it, replacing the setup it was created with. The
    /// server checks it as it checks a creation, and welcomes everyone to
    /// it again.
    Setup {
        setup: MatchSetup,
    },
    /// Local pads: `ticks[n][j]` is `seats[j]`'s pad at tick `start + n`.
    /// `need` is the first confirmed frame the client lacks. Recent ticks
    /// are repeated in every message until the server acknowledges them.
    Inputs {
        need: u64,
        start: u64,
        seats: Vec<u8>,
        ticks: Vec<Vec<Input>>,
    },
    /// The client's checksums of confirmed states, `(tick, checksum)`.
    Checksums {
        reports: Vec<(u64, u64)>,
    },
    /// For the round trip time; `id` comes back in `Pong`.
    Ping {
        id: u32,
    },
    /// Leaving; the seats go to the computer.
    Leave,

    /// Joined: the seats are the client's, in the order of `Join::players`.
    /// Sent again when the seats or the setup change in the lobby; `setup_version`
    /// counts the setup's changes.
    Welcome {
        seats: Vec<u8>,
        setup: MatchSetup,
        setup_version: u32,
    },
    Refused {
        reason: Refusal,
    },
    /// The match's occupied seats (sent on change and once a second), and
    /// the setup's generation (a client behind it joins again for it).
    Lobby {
        seats: Vec<SeatEntry>,
        setup_version: u32,
    },
    /// The match is under way from `tick`.
    Start {
        tick: u64,
    },
    /// Confirmed frames from tick `first`. `ack` is the first tick whose
    /// pads the server still wants from this client; `heard` the newest tick
    /// it has a pad for; `lead` how many ticks early this client's pads
    /// arrive at worst lately (negative: late), [`i16::MAX`] if unknown.
    Frames {
        first: u64,
        frames: Vec<Frame>,
        ack: u64,
        heard: u64,
        lead: i16,
    },
    /// The whole match ([`crate::Match::to_snapshot`]) at the start of
    /// `tick`, for a client that drifted or joined late.
    State {
        tick: u64,
        state: Vec<u8>,
    },
    /// Answer to `Ping`, with the server's next tick.
    Pong {
        id: u32,
        tick: u64,
    },
    /// The client's part in the match is over.
    Closed {
        reason: CloseReason,
    },

    /// One piece of a message too long for a datagram.
    Part {
        id: u16,
        index: u8,
        count: u8,
        bytes: Vec<u8>,
    },
}

mod kind {
    pub const REMATCH: u8 = 8;
    pub const RESTART_ACK: u8 = 9;
    pub const RESTART: u8 = 25;
    pub const ROUND: u8 = 33;
    pub const LIST: u8 = 7;
    pub const MATCHES: u8 = 24;
    pub const JOIN: u8 = 1;
    pub const READY: u8 = 2;
    pub const INPUTS: u8 = 3;
    pub const CHECKSUMS: u8 = 4;
    pub const PING: u8 = 5;
    pub const LEAVE: u8 = 6;
    pub const SIDE: u8 = 10;
    pub const PICK: u8 = 11;
    pub const SETUP: u8 = 12;
    pub const WELCOME: u8 = 16;
    pub const REFUSED: u8 = 17;
    pub const LOBBY: u8 = 18;
    pub const START: u8 = 19;
    pub const FRAMES: u8 = 20;
    pub const STATE: u8 = 21;
    pub const PONG: u8 = 22;
    pub const CLOSED: u8 = 23;
    pub const PART: u8 = 32;
}

/// Bytes of a part's own header (magic, version, kind, id, index, count).
const PART_HEADER: usize = 7;
/// Most parts in one message.
const MAX_PARTS: usize = u8::MAX as usize;

impl Message {
    pub(crate) fn round_scoped(&self) -> bool {
        matches!(
            self,
            Self::Ready { .. }
                | Self::Side { .. }
                | Self::Pick { .. }
                | Self::Setup { .. }
                | Self::Inputs { .. }
                | Self::Checksums { .. }
                | Self::Leave
                | Self::Welcome { .. }
                | Self::Lobby { .. }
                | Self::Start { .. }
                | Self::Frames { .. }
                | Self::State { .. }
                | Self::Closed { .. }
        )
    }

    /// The message as one datagram's bytes (perhaps longer than
    /// [`MAX_DATAGRAM`]: use [`Message::datagrams`] to send).
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(Vec::with_capacity(64));
        w.u8(MAGIC);
        w.u8(VERSION);
        match self {
            Message::Rematch { round } => {
                w.u8(kind::REMATCH);
                w.u32(*round);
            }
            Message::RestartAck { round } => {
                w.u8(kind::RESTART_ACK);
                w.u32(*round);
            }
            Message::Restart { round, setup: s } => {
                w.u8(kind::RESTART);
                w.u32(*round);
                setup(&mut w, s);
            }
            Message::Round { round, bytes } => {
                w.u8(kind::ROUND);
                w.u32(*round);
                w.bytes(bytes);
            }
            Message::List { id } => {
                w.u8(kind::LIST);
                w.u32(*id);
            }
            Message::Matches { id, matches } => {
                w.u8(kind::MATCHES);
                w.u32(*id);
                w.uv(matches.len() as u64);
                for m in matches {
                    w.str(&m.name);
                    w.str(&m.court);
                    w.u8(m.players);
                    w.bool(m.started);
                    w.bool(m.finished);
                }
            }
            Message::Join(j) => {
                w.u8(kind::JOIN);
                w.str(&j.match_name);
                w.uv(j.players.len() as u64);
                for p in &j.players {
                    w.str(&p.name);
                    w.u8(p.team.unwrap_or(u8::MAX));
                }
                match &j.create {
                    None => w.u8(0),
                    Some(s) => {
                        w.u8(1);
                        setup(&mut w, s);
                    }
                }
                w.0.extend_from_slice(&j.disc);
            }
            Message::Ready { ready } => {
                w.u8(kind::READY);
                w.bool(*ready);
            }
            Message::Side { team } => {
                w.u8(kind::SIDE);
                w.u8(*team);
            }
            Message::Pick { pick } => {
                w.u8(kind::PICK);
                w.bytes(pick);
            }
            Message::Setup { setup: s } => {
                w.u8(kind::SETUP);
                setup(&mut w, s);
            }
            Message::Inputs {
                need,
                start,
                seats,
                ticks,
            } => {
                w.u8(kind::INPUTS);
                w.uv(*need);
                w.uv(*start);
                w.bytes(seats);
                w.uv(ticks.len() as u64);
                let mut prev = vec![Input::default(); seats.len()];
                for tick in ticks {
                    let mut mask = 0u8;
                    for (j, i) in tick.iter().enumerate().take(seats.len()) {
                        if *i != prev[j] {
                            mask |= 1 << j;
                        }
                    }
                    w.u8(mask);
                    for (j, i) in tick.iter().enumerate().take(seats.len()) {
                        if mask & (1 << j) != 0 {
                            w.input(i);
                        }
                        prev[j] = *i;
                    }
                }
            }
            Message::Checksums { reports } => {
                w.u8(kind::CHECKSUMS);
                w.uv(reports.len() as u64);
                for &(tick, sum) in reports {
                    w.uv(tick);
                    w.u64(sum);
                }
            }
            Message::Ping { id } => {
                w.u8(kind::PING);
                w.u32(*id);
            }
            Message::Leave => w.u8(kind::LEAVE),
            Message::Welcome {
                seats,
                setup: s,
                setup_version,
            } => {
                w.u8(kind::WELCOME);
                w.bytes(seats);
                setup(&mut w, s);
                w.u32(*setup_version);
            }
            Message::Refused { reason } => {
                w.u8(kind::REFUSED);
                w.u8(match reason {
                    Refusal::Version => 0,
                    Refusal::Full => 1,
                    Refusal::Busy => 2,
                    Refusal::Invalid => 3,
                    Refusal::Roster => 4,
                    Refusal::RosterMod => 5,
                    Refusal::Disc => 6,
                    Refusal::NoMatch => 7,
                });
            }
            Message::Lobby {
                seats,
                setup_version,
            } => {
                w.u8(kind::LOBBY);
                w.uv(seats.len() as u64);
                for s in seats {
                    w.u8(s.seat);
                    w.u8(s.team);
                    w.str(&s.name);
                    w.bool(s.ready);
                    w.bytes(&s.pick);
                }
                w.u32(*setup_version);
            }
            Message::Start { tick } => {
                w.u8(kind::START);
                w.uv(*tick);
            }
            Message::Frames {
                first,
                frames,
                ack,
                heard,
                lead,
            } => {
                w.u8(kind::FRAMES);
                w.uv(*first);
                w.uv(*ack);
                w.uv(*heard);
                w.iv((*lead).into());
                w.uv(frames.len() as u64);
                let mut prev = Frame::default();
                for f in frames {
                    let mut present = 0u8;
                    let mut changed = 0u8;
                    for k in 0..MAX_SEATS {
                        if let Some(i) = f.seats[k] {
                            present |= 1 << k;
                            if i != prev.seats[k].unwrap_or_default() {
                                changed |= 1 << k;
                            }
                        }
                    }
                    w.u8(present);
                    w.u8(changed);
                    for k in 0..MAX_SEATS {
                        if changed & (1 << k) != 0 {
                            w.input(&f.seats[k].expect("present"));
                        }
                    }
                    prev = *f;
                }
            }
            Message::State { tick, state } => {
                w.u8(kind::STATE);
                w.uv(*tick);
                w.bytes(state);
            }
            Message::Pong { id, tick } => {
                w.u8(kind::PONG);
                w.u32(*id);
                w.uv(*tick);
            }
            Message::Closed { reason } => {
                w.u8(kind::CLOSED);
                w.u8(match reason {
                    CloseReason::Over => 0,
                    CloseReason::TimedOut => 1,
                    CloseReason::Shutdown => 2,
                    CloseReason::Fault => 3,
                });
            }
            Message::Part {
                id,
                index,
                count,
                bytes,
            } => {
                w.u8(kind::PART);
                w.u16(*id);
                w.u8(*index);
                w.u8(*count);
                w.0.extend_from_slice(bytes);
            }
        }
        w.0
    }

    /// The datagrams to send: one, or the message split into parts under
    /// transfer `id` when it is too long.
    pub fn datagrams(&self, id: u16) -> Vec<Vec<u8>> {
        let whole = self.encode();
        if whole.len() <= MAX_DATAGRAM {
            return vec![whole];
        }
        let chunk = MAX_DATAGRAM - PART_HEADER;
        let count = whole.len().div_ceil(chunk);
        assert!(count <= MAX_PARTS, "message of {} bytes", whole.len());
        whole
            .chunks(chunk)
            .enumerate()
            .map(|(index, bytes)| {
                Message::Part {
                    id,
                    index: index as u8,
                    count: count as u8,
                    bytes: bytes.to_vec(),
                }
                .encode()
            })
            .collect()
    }

    /// Read one datagram.
    pub fn decode(bytes: &[u8]) -> Result<Message> {
        let mut r = Reader::new(bytes);
        if r.u8()? != MAGIC {
            return Err(DecodeError::NotOurs);
        }
        let version = r.u8()?;
        if version != VERSION {
            return Err(DecodeError::Version(version));
        }
        let m = match r.u8()? {
            kind::REMATCH => Message::Rematch { round: r.u32()? },
            kind::RESTART_ACK => Message::RestartAck { round: r.u32()? },
            kind::RESTART => Message::Restart {
                round: r.u32()?,
                setup: setup_in(&mut r)?,
            },
            kind::ROUND => Message::Round {
                round: r.u32()?,
                bytes: r.bytes(MAX_DATAGRAM * MAX_PARTS)?.to_vec(),
            },
            kind::LIST => Message::List { id: r.u32()? },
            kind::MATCHES => {
                let id = r.u32()?;
                let n = r.len(64)?;
                let matches = (0..n)
                    .map(|_| {
                        Ok(MatchInfo {
                            name: r.str(MAX_NAME)?,
                            court: r.str(MAX_NAME)?,
                            players: r.u8()?,
                            started: r.bool()?,
                            finished: r.bool()?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Message::Matches { id, matches }
            }
            kind::JOIN => {
                let match_name = r.str(MAX_NAME)?;
                let n = r.len(MAX_SEATS)?;
                let players = (0..n)
                    .map(|_| {
                        Ok(Joiner {
                            name: r.str(MAX_NAME)?,
                            team: match r.u8()? {
                                u8::MAX => None,
                                t => Some(t),
                            },
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let create = match r.u8()? {
                    0 => None,
                    _ => Some(setup_in(&mut r)?),
                };
                let disc = r.take(32)?.try_into().expect("32 bytes");
                Message::Join(Join {
                    match_name,
                    players,
                    create,
                    disc,
                })
            }
            kind::READY => Message::Ready { ready: r.bool()? },
            kind::SIDE => Message::Side { team: r.u8()? },
            kind::PICK => Message::Pick {
                pick: r.bytes(MAX_PICK)?.to_vec(),
            },
            kind::SETUP => Message::Setup {
                setup: setup_in(&mut r)?,
            },
            kind::INPUTS => {
                let need = r.uv()?;
                let start = r.uv()?;
                let seats = r.bytes(MAX_SEATS)?.to_vec();
                let n = r.len(MAX_RUN)?;
                let mut prev = vec![Input::default(); seats.len()];
                let mut ticks = Vec::with_capacity(n);
                for _ in 0..n {
                    let mask = r.u8()?;
                    for (j, p) in prev.iter_mut().enumerate() {
                        if mask & (1 << j) != 0 {
                            *p = r.input()?;
                        }
                    }
                    ticks.push(prev.clone());
                }
                Message::Inputs {
                    need,
                    start,
                    seats,
                    ticks,
                }
            }
            kind::CHECKSUMS => {
                let n = r.len(MAX_RUN)?;
                let reports = (0..n)
                    .map(|_| Ok((r.uv()?, r.u64()?)))
                    .collect::<Result<Vec<_>>>()?;
                Message::Checksums { reports }
            }
            kind::PING => Message::Ping { id: r.u32()? },
            kind::LEAVE => Message::Leave,
            kind::WELCOME => {
                let seats = r.bytes(MAX_SEATS)?.to_vec();
                let setup = setup_in(&mut r)?;
                Message::Welcome {
                    seats,
                    setup,
                    setup_version: r.u32()?,
                }
            }
            kind::REFUSED => Message::Refused {
                reason: match r.u8()? {
                    0 => Refusal::Version,
                    1 => Refusal::Full,
                    2 => Refusal::Busy,
                    3 => Refusal::Invalid,
                    4 => Refusal::Roster,
                    5 => Refusal::RosterMod,
                    6 => Refusal::Disc,
                    7 => Refusal::NoMatch,
                    _ => return Err(DecodeError::Bad("refusal")),
                },
            },
            kind::LOBBY => {
                let n = r.len(MAX_SEATS)?;
                let seats = (0..n)
                    .map(|_| {
                        Ok(SeatEntry {
                            seat: r.u8()?,
                            team: r.u8()?,
                            name: r.str(MAX_NAME)?,
                            ready: r.bool()?,
                            pick: r.bytes(MAX_PICK)?.to_vec(),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Message::Lobby {
                    seats,
                    setup_version: r.u32()?,
                }
            }
            kind::START => Message::Start { tick: r.uv()? },
            kind::FRAMES => {
                let first = r.uv()?;
                let ack = r.uv()?;
                let heard = r.uv()?;
                let lead = i16::try_from(r.iv()?).map_err(|_| DecodeError::Bad("lead"))?;
                let n = r.len(MAX_RUN)?;
                let mut prev = Frame::default();
                let mut frames = Vec::with_capacity(n);
                for _ in 0..n {
                    let present = r.u8()?;
                    let changed = r.u8()?;
                    if changed & !present != 0 {
                        return Err(DecodeError::Bad("frame mask"));
                    }
                    let mut f = Frame::default();
                    for k in 0..MAX_SEATS {
                        if changed & (1 << k) != 0 {
                            f.seats[k] = Some(r.input()?);
                        } else if present & (1 << k) != 0 {
                            f.seats[k] = Some(prev.seats[k].unwrap_or_default());
                        }
                    }
                    frames.push(f);
                    prev = f;
                }
                Message::Frames {
                    first,
                    frames,
                    ack,
                    heard,
                    lead,
                }
            }
            kind::STATE => Message::State {
                tick: r.uv()?,
                state: r.bytes(MAX_PARTS * MAX_DATAGRAM)?.to_vec(),
            },
            kind::PONG => Message::Pong {
                id: r.u32()?,
                tick: r.uv()?,
            },
            kind::CLOSED => Message::Closed {
                reason: match r.u8()? {
                    0 => CloseReason::Over,
                    1 => CloseReason::TimedOut,
                    2 => CloseReason::Shutdown,
                    3 => CloseReason::Fault,
                    _ => return Err(DecodeError::Bad("close reason")),
                },
            },
            kind::PART => {
                let id = r.u16()?;
                let index = r.u8()?;
                let count = r.u8()?;
                if count == 0 || usize::from(count) > MAX_PARTS || index >= count {
                    return Err(DecodeError::Bad("part"));
                }
                Message::Part {
                    id,
                    index,
                    count,
                    bytes: r.rest().to_vec(),
                }
            }
            _ => return Err(DecodeError::Bad("message kind")),
        };
        if !r.done() {
            return Err(DecodeError::Bad("trailing bytes"));
        }
        Ok(m)
    }
}

fn setup(w: &mut Writer, s: &MatchSetup) {
    w.str(&s.court);
    w.u16(s.teams[0]);
    w.u16(s.teams[1]);
    w.uv(s.roster.len() as u64);
    for &r in &s.roster {
        w.u16(r);
    }
    w.u32(s.seed);
    for &t in &s.seat_teams {
        w.u8(t);
    }
    w.bytes(&s.world.to_snapshot());
    w.bytes(&s.extra);
    w.bytes(&s.roster_data);
    w.u16(s.intro);
}

fn setup_in(r: &mut Reader) -> Result<MatchSetup> {
    let court = r.str(MAX_NAME)?;
    let teams = [r.u16()?, r.u16()?];
    let n = r.len(64)?;
    let roster = (0..n).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
    let seed = r.u32()?;
    let mut seat_teams = [0; MAX_SEATS];
    for t in &mut seat_teams {
        *t = r.u8()?;
    }
    // The world as the creator built it, without the disc's tables.
    let world =
        World::from_snapshot(r.bytes(192 * 1024)?).map_err(|_| DecodeError::Bad("world"))?;
    if seat_teams.iter().any(|&team| team >= 2)
        || roster.len() != world.players.len()
        || roster
            .iter()
            .any(|&record| usize::from(record) >= street_assets::roster_rules::MAX_RECORDS)
        || teams
            .iter()
            .any(|&team| usize::from(team) >= street_assets::roster_rules::MAX_TEAMS)
    {
        return Err(DecodeError::Bad("setup seat teams/roster length"));
    }
    let extra = r.bytes(1 << 16)?.to_vec();
    let roster_data = r.bytes(street_assets::roster_share::MAX_PACKED)?.to_vec();
    let intro = r.u16()?;
    if intro > crate::game::MAX_INTRO {
        return Err(DecodeError::Bad("setup intro"));
    }
    Ok(MatchSetup {
        court,
        teams,
        roster,
        seed,
        seat_teams,
        world,
        extra,
        roster_data,
        intro,
    })
}

/// Puts split messages back together, per sender.
#[derive(Default)]
pub struct Reassembly {
    pending: HashMap<(SocketAddr, u16), Pending>,
}

struct Pending {
    parts: Vec<Option<Vec<u8>>>,
    since: Duration,
}

impl Reassembly {
    /// Most messages being put together from one sender at once.
    const PER_SENDER: usize = 4;
    /// Parts older than this are dropped.
    const EXPIRY: Duration = Duration::from_secs(3);

    /// Take one part; the whole message once its last part is in.
    pub fn add(
        &mut self,
        from: SocketAddr,
        now: Duration,
        id: u16,
        index: u8,
        count: u8,
        bytes: Vec<u8>,
    ) -> Option<Vec<u8>> {
        // This public entry point also defends direct callers, rather than
        // relying solely on Message::decode having checked fragment indexes.
        if count == 0
            || usize::from(count) > MAX_PARTS
            || index >= count
            || bytes.len() > MAX_DATAGRAM - PART_HEADER
        {
            return None;
        }
        self.pending
            .retain(|_, p| now.saturating_sub(p.since) < Self::EXPIRY);
        let key = (from, id);
        if !self.pending.contains_key(&key) {
            // A sender at its limit gives up its oldest unfinished message
            // for the new one: what it sends again supersedes what a lost
            // part left behind.
            let mine: Vec<((SocketAddr, u16), Duration)> = self
                .pending
                .iter()
                .filter(|((a, _), _)| *a == from)
                .map(|(k, p)| (*k, p.since))
                .collect();
            if mine.len() >= Self::PER_SENDER
                && let Some((oldest, _)) = mine.iter().min_by_key(|(k, since)| (*since, k.1))
            {
                self.pending.remove(oldest);
            }
        }
        let p = self.pending.entry(key).or_insert_with(|| Pending {
            parts: vec![None; count.into()],
            since: now,
        });
        if p.parts.len() != usize::from(count) {
            return None;
        }
        p.parts[usize::from(index)] = Some(bytes);
        if p.parts.iter().any(Option::is_none) {
            return None;
        }
        let p = self.pending.remove(&key).expect("present");
        Some(p.parts.into_iter().flatten().flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_sim::button;

    #[test]
    fn hostile_datagrams_are_rejected_without_panicking() {
        // Anything off the network is untrusted: random bytes, and every
        // truncation and single-byte corruption of a real message, must
        // decode to an error or a message, never a panic.
        let mut seed = 0x9e37_79b9_u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for len in 0..600 {
            let bytes: Vec<u8> = (0..len % 300).map(|_| next() as u8).collect();
            let _ = Message::decode(&bytes);
        }
        let real = Message::Lobby {
            seats: vec![],
            setup_version: 0,
        }
        .encode();
        for cut in 0..real.len() {
            let _ = Message::decode(&real[..cut]);
            for flip in [0x01, 0x80, 0xff] {
                let mut bad = real.clone();
                bad[cut] ^= flip;
                let _ = Message::decode(&bad);
            }
        }
    }

    fn pad(x: i8, buttons: u16) -> Input {
        Input {
            stick: [x, -x],
            aim: [0, x / 2],
            buttons,
            view: (x as u16) & 0x3ff,
            off_screen: (x as u8) & 0x3f,
        }
    }

    fn every_message() -> Vec<Message> {
        let setup = MatchSetup {
            court: "RUCKER".into(),
            teams: [3, 17],
            roster: vec![1, 2, 3, 4, 5, 6],
            extra: vec![9, 8, 7],
            roster_data: b"RSTZ roster".to_vec(),
            intro: 1234,
            ..MatchSetup::standard(Rules::default(), 77)
        };
        let mut f1 = Frame::default();
        f1.seats[0] = Some(pad(10, button::SHOOT));
        f1.seats[3] = Some(Input::default());
        let mut f2 = f1;
        f2.seats[0] = Some(pad(-5, 0));
        f2.seats[5] = Some(pad(1, button::ALL));
        let f3 = Frame::default();
        vec![
            Message::Rematch { round: 7 },
            Message::Restart {
                round: 8,
                setup: setup.clone(),
            },
            Message::RestartAck { round: 8 },
            Message::Round {
                round: 8,
                bytes: Message::Ready { ready: true }.encode(),
            },
            Message::Join(Join {
                match_name: "park".into(),
                players: vec![
                    Joiner {
                        name: "Player".into(),
                        team: Some(1),
                    },
                    Joiner {
                        name: "Guest".into(),
                        team: None,
                    },
                ],
                create: Some(setup.clone()),
                disc: [7; 32],
            }),
            Message::Join(Join {
                match_name: String::new(),
                players: vec![],
                create: None,
                disc: [0; 32],
            }),
            Message::Ready { ready: true },
            Message::Inputs {
                need: 1234,
                start: 1200,
                seats: vec![0, 4],
                ticks: vec![
                    vec![pad(1, 0), pad(2, 0)],
                    vec![pad(1, 0), pad(3, button::PASS)],
                    vec![Input::default(), pad(3, button::PASS)],
                ],
            },
            Message::Checksums {
                reports: vec![(30, u64::MAX), (60, 12345)],
            },
            Message::Ping { id: 0xdead_beef },
            Message::Leave,
            Message::Welcome {
                seats: vec![1, 3],
                setup: setup.clone(),
                setup_version: 2,
            },
            Message::Setup { setup },
            Message::Refused {
                reason: Refusal::Full,
            },
            Message::Refused {
                reason: Refusal::Version,
            },
            Message::Refused {
                reason: Refusal::Roster,
            },
            Message::Refused {
                reason: Refusal::RosterMod,
            },
            Message::Refused {
                reason: Refusal::Disc,
            },
            Message::Refused {
                reason: Refusal::NoMatch,
            },
            Message::Lobby {
                seats: vec![SeatEntry {
                    seat: 2,
                    team: 0,
                    name: "Stretch".into(),
                    ready: true,
                    pick: vec![1, 39, 0, 7],
                }],
                setup_version: 3,
            },
            Message::Side { team: 1 },
            Message::Pick {
                pick: vec![2, 5, 1, 2, 3],
            },
            Message::Start { tick: 0 },
            Message::Frames {
                first: 99,
                frames: vec![f1, f2, f3, f1],
                ack: 120,
                heard: 125,
                lead: -3,
            },
            Message::State {
                tick: 4000,
                state: vec![1; 3000],
            },
            Message::Pong {
                id: 7,
                tick: 1 << 40,
            },
            Message::Closed {
                reason: CloseReason::TimedOut,
            },
            Message::Closed {
                reason: CloseReason::Fault,
            },
            Message::Part {
                id: 3,
                index: 1,
                count: 2,
                bytes: vec![5; 10],
            },
        ]
    }

    #[test]
    fn every_message_round_trips() {
        for m in every_message() {
            let bytes = m.encode();
            assert_eq!(bytes[0], MAGIC);
            assert_ne!(bytes[0], 0xE5);
            assert_ne!(bytes[0], 0xE6);
            for datagram in m.datagrams(7) {
                assert_eq!(datagram[0], MAGIC);
                assert_ne!(datagram[0], 0xE5);
                assert_ne!(datagram[0], 0xE6);
            }
            assert_eq!(Message::decode(&bytes).as_ref(), Ok(&m), "{m:?}");
        }
    }

    #[test]
    fn long_messages_split_and_reassemble() {
        let mut asm = Reassembly::default();
        // Each message its own id: the duplicate part left over from one
        // message must not collide with the next long one. (Each from its
        // own port: a sender's leftovers count against its four at once.)
        for (k, m) in every_message().into_iter().enumerate() {
            let from = SocketAddr::from(([127, 0, 0, 1], 9 + k as u16));
            let grams = m.datagrams(42 + k as u16);
            assert!(grams.iter().all(|g| g.len() <= MAX_DATAGRAM));
            if grams.len() == 1 {
                continue;
            }
            // Out of order, with a duplicate.
            let mut whole = None;
            for g in grams.iter().rev().chain(grams.first()) {
                let Message::Part {
                    id,
                    index,
                    count,
                    bytes,
                } = Message::decode(g).unwrap()
                else {
                    panic!("not a part");
                };
                if let Some(w) = asm.add(from, Duration::ZERO, id, index, count, bytes) {
                    whole = Some(w);
                }
            }
            assert_eq!(Message::decode(&whole.expect("reassembled")).unwrap(), m);
        }
    }

    #[test]
    fn native_trick_identity_and_large_metadata_survive_setup_transfer() {
        let mut setup = MatchSetup {
            roster: vec![1, 2, 3, 4, 5, 6],
            // Opaque setup metadata exercises more than the former64-part bound.
            extra: vec![0x5a; 60 * 1024],
            ..MatchSetup::standard(Rules::default(), 42)
        };
        setup.world.players[0].native_tricks = Some(street_sim::NativeTricks {
            slots: std::array::from_fn(|slot| street_sim::TrickIdentity {
                slot: slot as u8,
                script: if slot == 10 { 69 } else { slot as u16 },
                level: street_sim::player::TRICK_SLOT_LEVELS[slot],
                equipped_row: None,
            }),
        });
        let message = Message::Welcome {
            seats: vec![0],
            setup,
            setup_version: 0,
        };
        assert_eq!(Message::decode(&message.encode()).unwrap(), message);
        let state = Message::State {
            tick: 0,
            state: vec![0x5a; 128 * 1024],
        };
        let grams = state.datagrams(17);
        assert!(grams.len() > 64 && grams.len() <= MAX_PARTS);
        let mut assembly = Reassembly::default();
        let mut result = None;
        for datagram in grams.iter().rev() {
            let Message::Part {
                id,
                index,
                count,
                bytes,
            } = Message::decode(datagram).unwrap()
            else {
                panic!("expected fragments")
            };
            result = assembly
                .add(
                    "127.0.0.1:9".parse().unwrap(),
                    Duration::ZERO,
                    id,
                    index,
                    count,
                    bytes,
                )
                .or(result);
        }
        assert_eq!(Message::decode(&result.unwrap()).unwrap(), state);
    }

    #[test]
    fn foreign_and_damaged_datagrams_are_rejected() {
        assert_eq!(Message::decode(&[0xE5, 1, 2]), Err(DecodeError::NotOurs));
        assert_eq!(
            Message::decode(&[MAGIC, VERSION + 1, kind::LEAVE]),
            Err(DecodeError::Version(VERSION + 1))
        );
        for m in every_message() {
            let bytes = m.encode();
            for cut in 0..bytes.len() {
                if matches!(m, Message::Part { .. }) && cut >= PART_HEADER {
                    continue; // a part's payload is opaque
                }
                assert!(
                    Message::decode(&bytes[..cut]).is_err(),
                    "{m:?} cut at {cut}"
                );
            }
            let mut longer = bytes.clone();
            longer.push(0);
            if !matches!(m, Message::Part { .. }) {
                assert!(Message::decode(&longer).is_err());
            }
        }
    }

    #[test]
    fn a_quiet_frame_is_two_bytes() {
        let mut f = Frame::default();
        f.seats[0] = Some(pad(4, 0));
        f.seats[1] = Some(pad(4, 0));
        let one = Message::Frames {
            first: 0,
            frames: vec![f],
            ack: 0,
            heard: 0,
            lead: 0,
        }
        .encode()
        .len();
        let many = Message::Frames {
            first: 0,
            frames: vec![f; 33],
            ack: 0,
            heard: 0,
            lead: 0,
        }
        .encode()
        .len();
        assert_eq!(many - one, 32 * 2);
    }

    #[test]
    fn fragment_indexes_are_checked_for_direct_callers() {
        let from: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let mut reassembly = Reassembly::default();
        assert!(
            reassembly
                .add(from, Duration::ZERO, 1, 0, 0, vec![])
                .is_none()
        );
        assert!(
            reassembly
                .add(from, Duration::ZERO, 1, 3, 2, vec![])
                .is_none()
        );
        assert!(
            reassembly
                .add(from, Duration::ZERO, 1, 0, 1, vec![0; MAX_DATAGRAM])
                .is_none()
        );
        assert!(reassembly.pending.is_empty());
    }
}
