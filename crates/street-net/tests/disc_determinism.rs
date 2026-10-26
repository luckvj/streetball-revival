//! Determinism on the disc's data: the server and two clients play a long
//! match on the disc's animations, gait records and staged gamebreakers,
//! each machine on its own copy of the tables (`street_net::disc`), with
//! delayed, jittered and dropped pads forcing rollbacks. Every confirmed
//! state's checksum, every tick, must be the server's.
//!
//! Opt-in test against the user's disc: STREET_ISO=/path/to/disc.iso.

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use street_assets::db;
use street_net::{
    Client, ClientConfig, Conditions, Disc, Join, Joiner, Known, MatchSetup, MemoryNetwork, Server,
    ServerConfig, ServerEvent, SessionConfig, Status,
};
use street_sim::gait_select::GaitProfile;
use street_sim::{Event, Hoop, Input, Phase, Player, Ratings, Rules, World, ai, button};

const SERVER: &str = "10.0.0.1:7375";

fn addr(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

/// One copy of the disc's data per machine, as each reads its own disc.
fn disc() -> Option<Disc> {
    static DISC: OnceLock<Option<Disc>> = OnceLock::new();
    DISC.get_or_init(|| {
        let path = std::env::var_os("STREET_ISO")?;
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        Some(Disc::read(&mut iso).unwrap())
    })
    .clone()
}

/// The simulation's view of a disc record, as `street-game`'s `setup.rs`
/// makes it for a match online (no Gym development).
fn ratings(p: &db::Player) -> Ratings {
    let r = |skill: usize| (f32::from(p.ratings[skill]) / 99.0).clamp(0.0, 1.0);
    Ratings {
        shots: r(0),
        threes: r(1),
        dunks: r(2),
        rebounds: r(3),
        blocks: r(5),
        steals: r(6),
        handles: r(7),
        speed: r(8),
        power: r(4),
        overall: r(9),
        reach: street_sim::player::reach_for_height(f32::from(p.height_in)),
    }
}

/// Two of the disc's teams, each with its roster's first three, made into
/// a match as the game makes one from its disc: the records' ratings,
/// weights, bodies and handedness, each player's gait profile and the
/// staged players' scales, the referee's toss, both sides people's.
fn setup(disc: &Disc, teams: [usize; 2], rules: Rules, seed: u32, full: bool) -> MatchSetup {
    let tables = disc.tables();
    let mut players = Vec::new();
    let mut records = Vec::new();
    let mut profiles = Vec::new();
    let mut scales = Vec::new();
    let mut widths = Vec::new();
    for (team, &t) in teams.iter().enumerate() {
        let side = street_sim::restart::side(team);
        for k in 0..3 {
            let record = tables.teams[t].roster[k];
            let p = &tables.players[record];
            let mut player = Player::new(team, [-12.0 * side, (k as f32 - 1.0) * 9.0], 1.57 * side)
                .with_ratings(ratings(p))
                .with_weight(p.weight_lb.max(1) as u16);
            player.body = p
                .native_body_scale()
                .map(|[height, width, _]| street_sim::pose::Body {
                    height,
                    width,
                    head: p.native_head_scale().map_or(1.0, |h| h[0]),
                });
            player.left_handed = p.left_handed;
            profiles.push(GaitProfile {
                category: GaitProfile::category_for(i32::from(p.weight_lb), p.female),
                identity: record as i32,
                handles: (player.ratings.handles * 99.0).round() as i32,
            });
            scales.push(p.height_scale());
            widths.push(street_assets::clip::native_body_width_ratio(
                p.native_body_scale(),
            ));
            players.push(player);
            records.push(record as u16);
        }
    }
    let mut world = World::new(Hoop::native(), players, rules, seed);
    world.start_opening_toss();
    world.configure_meter_fill([true, true], None);
    if full {
        // Both sides' Level 2 gamebreaker ready (`--full-meters`), for the
        // staged scripts.
        for team in 0..2 {
            world.teams[team].pocketed = true;
            world.set_meter(team, street_sim::street::METER_FULL as f32);
        }
        world.events.clear();
        world.stats = street_sim::stats::Stats::new(world.players.len());
    }
    world.set_match_values(profiles, scales, widths);
    MatchSetup {
        court: "RUCKER".into(),
        teams: teams.map(|t| t as u16),
        roster: records,
        seed,
        seat_teams: [0, 1, 0, 1, 0, 1],
        world,
        extra: Vec::new(),
        roster_data: Vec::new(),
    }
}

/// A person at the pad: the computer's own pad for the player the seat
/// drives (passes, shots, swipes, blocks, tricks), with turbo held at
/// times and extra presses now and then.
fn pad(world: &World, player: usize, tick: u64, salt: u64) -> Input {
    let mut input = ai::input(world, player);
    let phase = (tick / 7 + salt * 5) % 41;
    let defending = world.possession != world.players[player].team;
    input.buttons |= match phase {
        3 => button::PASS,
        9 => button::SHOOT,
        14 => button::TRICK | button::R1,
        19 => button::JUMP,
        27 => button::R1,
        33 => button::PASS | button::R1,
        // On defence, the swipe (the trick button) and the block jump.
        p if defending && p % 6 == 1 => button::TRICK,
        p if defending && p % 6 == 4 => button::SHOOT,
        _ => 0,
    };
    input
}

struct Machine {
    client: Client,
    salt: u64,
    /// What the confirmed ticks did, counted.
    passes: u32,
    /// Swipes started on the disc's scripts (in the world shown).
    swipes: u32,
    loose: u32,
    blocks: u32,
    baskets: u32,
    tricks: u32,
    gamebreakers: u32,
    /// Staged Level 2 scripts played (in the world shown).
    staged: u32,
    /// Replays started (in the match shown), and the ticks they held.
    replays: u32,
    held: u32,
    /// The match ticks a staged script's replay (mode 7) started at, in
    /// the match shown (a rollback replays the same tick).
    reruns: std::collections::BTreeSet<u64>,
    /// Ticks GB2's burst held the game (in the match shown).
    burst_held: u32,
    swiping: Vec<bool>,
    sequence: bool,
}

impl Machine {
    fn new(net: &MemoryNetwork, at: &str, disc: Disc, join: Join, salt: u64) -> Self {
        let cfg = ClientConfig {
            // Every confirmed state's checksum goes to the referee.
            session: SessionConfig {
                checksum_every: 1,
                ..SessionConfig::default()
            },
            ..ClientConfig::default()
        };
        let mut client = Client::new(
            Box::new(net.endpoint(addr(at))),
            addr(SERVER),
            join,
            cfg,
            Arc::new(disc),
        );
        client.set_ready(true);
        Self {
            client,
            salt,
            passes: 0,
            swipes: 0,
            loose: 0,
            blocks: 0,
            baskets: 0,
            tricks: 0,
            gamebreakers: 0,
            staged: 0,
            replays: 0,
            held: 0,
            reruns: Default::default(),
            burst_held: 0,
            swiping: vec![false; 6],
            sequence: false,
        }
    }

    fn frame(&mut self, now: Duration) {
        self.client.update(now);
        for _ in 0..self.client.ticks_due(now) {
            let Some(m) = self.client.session().map(|s| s.current().clone()) else {
                break;
            };
            for seat in self.client.seats().to_vec() {
                let player = m.seats[usize::from(seat)]
                    .player
                    .map_or(usize::from(seat) % 2, usize::from);
                let input = pad(&m.world, player, m.tick(), self.salt);
                self.client.add_local_input(seat, input);
            }
            self.client.advance();
            if let Some(m) = self.client.session().map(|s| s.current()) {
                self.replays += u32::from(m.replays.manager.started);
                self.held += u32::from(m.replays.manager.busy());
                if m.replays.manager.started && m.replays.manager.mode == 7 {
                    self.reruns.insert(m.tick());
                }
                // GB2's burst held the game: the world's clock stood while
                // the match's ran.
                self.burst_held += u32::from(m.burst_hold > 0);
            }
            if let Some(w) = self.client.world() {
                for (k, p) in w.players.iter().enumerate() {
                    let now = matches!(
                        p.action,
                        street_sim::Action::Stealing { swipe: Some(_), .. }
                    );
                    self.swipes += u32::from(now && !self.swiping[k]);
                    self.swiping[k] = now;
                }
                self.staged += u32::from(w.sequence.is_some() && !self.sequence);
                self.sequence = w.sequence.is_some();
            }
        }
        for (_, events) in self.client.take_confirmed_events() {
            for e in events {
                match e {
                    Event::Pass { .. } => self.passes += 1,
                    Event::Steal { .. } | Event::Deflection { .. } | Event::Fumble { .. } => {
                        self.loose += 1
                    }
                    Event::Block { .. } => self.blocks += 1,
                    Event::Score { .. } => self.baskets += 1,
                    Event::Trick { .. } => self.tricks += 1,
                    Event::Gamebreaker { .. } => self.gamebreakers += 1,
                    _ => {}
                }
            }
        }
    }
}

/// Two clients (one person a side, the computer the other four) and the
/// referee through `ticks` ticks of `teams` under `conditions`.
fn disc_match(teams: [usize; 2], seed: u32, ticks: u64, conditions: Conditions, full: bool) {
    let server_disc = disc().unwrap();
    assert!(server_disc.is_disc());
    let net = MemoryNetwork::new(conditions, seed);
    let rosters = Arc::new(Known {
        disc: Some(server_disc.tables().clone()),
        ..Known::default()
    });
    let mut server = Server::new(
        Box::new(net.endpoint(addr(SERVER))),
        ServerConfig {
            rosters,
            ..ServerConfig::default()
        },
        Arc::new(server_disc),
    );
    let rules = Rules {
        // A long match: to 1,000, so it is still going at the end.
        goal: street_sim::Goal::Points(1000),
        ..Rules::default()
    };
    let made = setup(&disc().unwrap(), teams, rules, seed, full);
    let join = |name: &str, team: u8, create: bool| Join {
        match_name: "park".into(),
        players: vec![Joiner {
            name: name.into(),
            team: Some(team),
        }],
        create: create.then(|| made.clone()),
        disc: [0; 32],
    };
    let mut machines = [
        Machine::new(
            &net,
            "10.0.0.2:5000",
            disc().unwrap(),
            join("A", 0, true),
            1,
        ),
        Machine::new(
            &net,
            "10.0.0.3:5000",
            disc().unwrap(),
            join("B", 1, true),
            2,
        ),
    ];
    let mut events = Vec::new();
    let step = Duration::from_millis(4);
    let mut now = Duration::ZERO;
    // The referee's staged replays: the match tick each started at, and
    // whether the world's clock stood still through it.
    let mut reruns = std::collections::BTreeMap::new();
    while server.host("park").is_none_or(|h| h.tick() < ticks) {
        now += step;
        assert!(
            now < Duration::from_secs(600),
            "the match never got going: {:?}",
            &events[..events.len().min(8)]
        );
        net.set_time(now);
        events.extend(server.update(now));
        if let Some(h) = server.host("park") {
            let r = &h.state().replays;
            if r.manager.started && r.manager.mode == 7 {
                reruns.insert(h.tick(), (h.state().world.tick, r.staged));
            }
            if let Some((&at, &(world, _))) = reruns.iter().next_back()
                && r.manager.busy()
                && r.manager.mode == 7
            {
                assert_eq!(h.state().world.tick, world, "the rerun from {at} held");
            }
        }
        for m in &mut machines {
            m.frame(now);
        }
    }
    for _ in 0..200 {
        now += step;
        net.set_time(now);
        events.extend(server.update(now));
        for m in &mut machines {
            m.frame(now);
        }
    }
    let refused: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, ServerEvent::Refused { .. }))
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    let host = server.host("park").unwrap();
    // The referee plays the disc's data.
    let w = &host.state().world;
    assert!(w.poses.get().is_some());
    assert_eq!(w.gait.as_ref().unwrap().records.len(), 108);
    assert!(!w.staged.as_ref().unwrap().plans.is_empty());
    assert!(!matches!(w.phase, Phase::Over { .. }));
    // No confirmed state of either client ever differed from the
    // referee's: each sent every tick's checksum, and a differing one
    // would have brought a resync.
    let drifted: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, ServerEvent::Resynced { drifted: true, .. }))
        .collect();
    assert!(drifted.is_empty(), "{drifted:?}");
    for (m, name) in machines.iter().zip(["A", "B"]) {
        assert_eq!(m.client.status(), Status::Playing, "{name}");
        let session = m.client.session().unwrap();
        let stats = session.stats();
        assert!(stats.rollbacks > 100, "{name}: {stats:?}");
        assert_eq!(stats.resyncs, 0, "{name}: {stats:?}");
        let confirmed = session.confirmed();
        assert!(confirmed.tick() + 300 > host.tick(), "{name}");
        assert_eq!(
            host.checksum_at(confirmed.tick()),
            Some(confirmed.checksum()),
            "{name} at {}",
            confirmed.tick()
        );
        assert!(confirmed.world.poses.get().is_some(), "{name}");
        println!(
            "{name}: {stats:?}; {} swipes started, {} staged scripts, {} replays holding {} \
             ticks; confirmed {} passes, {} steals, knocks and fumbles, {} blocks, {} baskets, \
             {} tricks, {} gamebreakers",
            m.swipes,
            m.staged,
            m.replays,
            m.held,
            m.passes,
            m.loose,
            m.blocks,
            m.baskets,
            m.tricks,
            m.gamebreakers
        );
        if full {
            assert!(m.staged > 0, "{name}: no staged gamebreaker played");
            assert!(
                m.burst_held >= u32::from(street_net::game::LEVEL2_BURST_HOLD),
                "{name}: no Level 2 burst held the game"
            );
            // Each staged gamebreaker's replay on this machine at the
            // referee's match tick.
            let at: Vec<u64> = reruns.keys().copied().collect();
            assert!(
                at.iter().all(|t| m.reruns.contains(t)),
                "{name}: reruns {:?}, the referee's {at:?}",
                m.reruns
            );
        }
        assert!(
            m.passes > 20 && m.baskets > 5 && m.swipes > 10 && m.blocks > 0,
            "{name}"
        );
    }
    // Replays held the world's clock while the match's ran on.
    assert!(machines.iter().all(|m| m.replays > 0), "no replay played");
    if full {
        assert!(!reruns.is_empty(), "no staged script's replay played");
    }
    println!(
        "staged replays (match tick: world tick, the script manager's): {:?}",
        reruns
    );
    assert!(host.state().world.tick < host.tick());
    println!(
        "{} ticks ({} of the world's), late pads {}, datagrams {:?}",
        host.tick(),
        host.state().world.tick,
        host.late_pads(),
        net.stats()
    );
}

#[test]
fn two_clients_and_the_referee_agree_on_the_discs_data_through_rollbacks() {
    if disc().is_none() {
        return;
    }
    disc_match(
        [3, 9],
        // (2025: on 2024, after the board passout's look, the second
        // client's match played no replay; 2026: 2025's blocked no shot
        // once the holders backed down.)
        2026,
        15_000,
        Conditions {
            loss: 0.04,
            duplicate: 0.02,
            delay: Duration::from_millis(45),
            jitter: Duration::from_millis(35),
        },
        false,
    );
    // Two other teams: the disc's twelfth and twentieth with a roster, on
    // a seed whose match blocks a shot (seed 7's, after the item 8 pass's
    // restarts and tip, blocks none in its 15,000 ticks; seed 9's none
    // once the dunks' knocks were ported, seed 10's none once the gait
    // selector drew from the shared generator, seed 11's none once the
    // stands, the looks and the faces drew too, seed 13's none once the
    // side with the ball stopped at once off a pass, seed 14's none once
    // the pass filter measured on its flight estimate, seed 15's none
    // once the post man's call posted him).
    let tables = disc().unwrap();
    let teams: Vec<usize> = (0..tables.tables().teams.len())
        .filter(|&t| tables.tables().teams[t].roster.len() >= 3)
        .collect();
    disc_match(
        [teams[12], teams[20]],
        16,
        15_000,
        Conditions {
            loss: 0.10,
            duplicate: 0.05,
            delay: Duration::from_millis(70),
            jitter: Duration::from_millis(60),
        },
        true,
    );
}

/// What a rollback costs on the disc's data: one match step, one world
/// clone and one match clone, averaged (opt-in: `--ignored`, with
/// STREET_ISO; run with `--release` for the shipped numbers).
#[test]
#[ignore]
fn timing_of_a_step_and_a_clone() {
    let Some(disc) = disc() else {
        return;
    };
    let mut net = setup(&disc, [3, 9], Rules::default(), 5, false);
    disc.install(&mut net.world).unwrap();
    let mut m = street_net::Match::new(net.world.clone(), net.seat_teams);
    let frame = street_net::Frame::default();
    let n = 1200;
    let t = std::time::Instant::now();
    for _ in 0..n {
        m.step(&frame);
    }
    let step = t.elapsed() / n;
    let t = std::time::Instant::now();
    for _ in 0..n {
        std::hint::black_box(m.world.clone());
    }
    let world = t.elapsed() / n;
    let t = std::time::Instant::now();
    for _ in 0..n {
        std::hint::black_box(m.clone());
    }
    let whole = t.elapsed() / n;
    println!("step {step:?}  world clone {world:?}  match clone {whole:?}");
}
