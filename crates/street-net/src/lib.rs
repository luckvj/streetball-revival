//! Online play: the wire protocol, rollback sessions for game clients, and
//! the match server. No Bevy here, so the game and the headless
//! `street-server` share it.
//!
//! # How it works
//!
//! Every player connects to one server (a player's own PC or a rented box)
//! over UDP. Every machine plays on its own disc's data ([`Disc`]: the
//! animations, gait records and staged gamebreakers the rules read), the
//! server too, and they compare its digest at the join. The simulation is
//! deterministic, so only pads (and digests) cross the network:
//!
//! 1. Each client sends its pads, tick by tick, repeating the recent ticks
//!    in every datagram until the server acknowledges them.
//! 2. The server fixes every seat's pad for each tick (a pad that is late
//!    counts as a repeat of the seat's last one), runs the game itself as the
//!    referee, and sends the fixed frames to every client.
//! 3. Clients run a little ahead of the server on guessed pads and roll back
//!    when a fixed frame differs from the guess ([`Session`]).
//! 4. Clients report checksums of their confirmed states; the server sends
//!    the whole state ([`Match::to_snapshot`]) to a client that drifts.
//!
//! What every machine simulates is a [`Match`]: the [`street_sim::World`]
//! plus the six seats (1P..6P) and which player each drives. The messages
//! are in [`protocol`] (and in `docs/FORMATS.md`, "Online protocol").
//!
//! # Hosting
//!
//! [`Server`] is the whole server; `street-server --vol2 <iso>` is a command
//! line around it. The referee holds the disc: a server is made with one
//! ([`Server::new`]) and won't host without it. A game that hosts on its own
//! PC runs a `Server` on its own disc, on a [`UdpTransport`] in its own
//! update, and joins it through a [`Client`] at `127.0.0.1`, like everyone
//! else.
//!
//! # Driving a client from the game
//!
//! Joining and the lobby:
//!
//! ```no_run
//! use std::sync::Arc;
//! use std::time::{Duration, Instant};
//! use street_net::{Client, ClientConfig, ClientEvent, Disc, Join, Joiner, MatchSetup, UdpTransport};
//! # fn setup_from_the_disc() -> MatchSetup { unimplemented!() }
//!
//! // This machine's disc: every world of the match is played on its tables.
//! let mut iso = street_assets::iso::Iso::open("vol2.iso").unwrap();
//! let disc = Arc::new(Disc::read(&mut iso).unwrap());
//! let socket = UdpTransport::bind("0.0.0.0:0").unwrap();
//! let join = Join {
//!     match_name: "park".into(),
//!     players: vec![Joiner { name: "Player".into(), team: Some(0) }],
//!     // The creator's setup, which the game builds from its disc as
//!     // setup.rs does; a joiner brings `None`.
//!     create: Some(setup_from_the_disc()),
//!     // The client sends its disc's digest.
//!     disc: [0; 32],
//! };
//! let server = "203.0.113.5:7375".parse().unwrap();
//! let mut client = Client::new(Box::new(socket), server, join, ClientConfig::default(), disc);
//! let clock = Instant::now();
//! client.set_ready(true); // when the person presses Ready
//! loop {
//!     for event in client.update(clock.elapsed()) {
//!         match event {
//!             ClientEvent::Welcome { seats } => { /* our seats, 0-based (1P = 0) */ }
//!             ClientEvent::Lobby(seats) => { /* who is in, and ready */ }
//!             ClientEvent::Started { .. } => { /* load the court from client.setup() */ }
//!             _ => {}
//!         }
//!     }
//!     # break;
//! }
//! ```
//!
//! Each frame of the match (replacing `controls::fill_inputs` and
//! `game::step_sim`'s stepping):
//!
//! ```no_run
//! # use std::time::Instant;
//! # use street_net::Client;
//! # fn pad_for(seat: u8) -> street_sim::Input { Default::default() }
//! # fn play_sounds(events: &[street_sim::Event]) {}
//! # fn frame(client: &mut Client, clock: Instant) {
//! client.update(clock.elapsed()); // network in and out, events
//! for _ in 0..client.ticks_due(clock.elapsed()) {
//!     for &seat in client.seats().to_vec().iter() {
//!         // The raw pad in court axes, as `controls::read` builds it, but
//!         // with the pass button left in: the Match turns a pass press on
//!         // defence into a switch itself.
//!         client.add_local_input(seat, pad_for(seat));
//!     }
//!     if client.advance().is_some() {
//!         // The newest tick's events: sounds, effects, the HUD.
//!         play_sounds(&client.world().unwrap().events);
//!     }
//! }
//! // Draw client.world(); client.session().unwrap().current().seats[k].player
//! // is who seat k drives (for the 1P..6P markers).
//! # }
//! ```
//!
//! Notes for the game:
//!
//! - Run the network from `Update` with [`Client::ticks_due`], the clock
//!   already nudged for time sync; or, on a step already fixed at the tick
//!   rate (`FixedUpdate`), with [`Client::ticks_due_fixed`], never both
//!   clocks at once.
//! - The world drawn is a prediction. A rollback can move players a little
//!   between frames; [`Client::take_correction`] says how far (a
//!   [`Correction`]), for the renderer to ease out rather than jump. Events
//!   of re-simulated ticks are not repeated, so a sound may play for a
//!   guess that did not stand.
//! - A game that hosts should run its [`Server`] on a thread of its own:
//!   in the game's frame loop, its ticks and frames go out at the host's
//!   frame rate, and every pad that misses its tick costs a player's own
//!   move.
//! - The server never pauses, and replays would step the world outside the
//!   session, so online play disables pausing and instant replays. The
//!   story-mode hooks in `step_sim` (lessons, stats) should read the
//!   confirmed state ([`Session::confirmed`]) if they must not see guesses.
//! - Leaving: [`Client::leave`]; the seats go to the computer.

pub mod browser;
pub mod client;
pub mod code;
pub mod disc;
pub mod game;
pub mod lan;
pub mod protocol;
pub mod roster;
pub mod server;
pub mod session;
pub mod transport;
pub mod wire;

pub use browser::Browser;
pub use client::{Client, ClientConfig, ClientEvent, Status};
pub use disc::Disc;
pub use game::{Frame, MAX_SEATS, Match, Seat};
pub use protocol::{
    CloseReason, Join, Joiner, MAGIC, MatchSetup, Message, Refusal, SeatEntry, VERSION,
};
pub use roster::{DiscTables, Known, ModTables, RosterError};
pub use server::{Host, Server, ServerConfig, ServerEvent};
pub use session::{Advanced, Correction, Session, SessionConfig, SessionStats};
pub use transport::{Conditions, MemoryNetwork, MemoryTransport, Transport, UdpTransport};

/// The server's default UDP port.
pub const DEFAULT_PORT: u16 = 7375;
