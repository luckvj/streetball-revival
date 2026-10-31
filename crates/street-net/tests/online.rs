//! A server and several clients in one process, over an in-memory network
//! that drops, duplicates, delays and reorders datagrams (and once over real
//! loopback UDP). The clock is simulated, so the runs are exact and fast.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use street_net::{
    Client, ClientConfig, ClientEvent, Conditions, Disc, Join, Joiner, MatchSetup, MemoryNetwork,
    Server, ServerConfig, ServerEvent, Status, Transport, UdpTransport,
};
use street_sim::{Goal, Input, Rules, button, math};

const SERVER: &str = "10.0.0.1:7375";

#[test]
fn browsing_and_spectating_leave_player_seats_free() {
    let net = MemoryNetwork::new(Conditions::default(), 7);
    let mut server = Server::new(
        Box::new(net.endpoint(addr(SERVER))),
        ServerConfig::default(),
        disc(),
    );
    let mut player = Player::new(
        Box::new(net.endpoint(addr("10.0.0.2:5000"))),
        join("A", Some(0), true),
        1,
    );
    let mut spectator_join = join("Spectator", None, false);
    spectator_join.players.clear();
    let mut spectator = Player::new(
        Box::new(net.endpoint(addr("10.0.0.3:5000"))),
        spectator_join,
        2,
    );
    // A spectator's readiness has no bearing on the players' lobby.
    spectator.client.set_ready(false);
    let mut browser =
        street_net::Browser::new(Box::new(net.endpoint(addr("10.0.0.4:5000"))), addr(SERVER));
    for step in 0..2000 {
        let now = Duration::from_millis(step * 4);
        net.set_time(now);
        server.update(now);
        player.frame(now);
        spectator.frame(now);
        browser.update(now);
    }
    assert!(browser.answered());
    assert_eq!(browser.matches().len(), 1);
    assert_eq!(browser.matches()[0].name, "park");
    assert_eq!(browser.matches()[0].players, 1);
    assert!(browser.matches()[0].started);
    assert!(spectator.client.seats().is_empty());
    assert_eq!(spectator.client.status(), Status::Playing);
    assert_eq!(server.host("park").unwrap().seats().len(), 1);
    assert_agrees(&server, &spectator, "spectator");
}

/// A machine with no disc: these tests play the rules' stand-ins.
fn disc() -> std::sync::Arc<Disc> {
    std::sync::Arc::new(Disc::stand_ins())
}

fn addr(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

/// A long game, so that it is still going at the end of the test.
fn rules() -> Rules {
    Rules {
        goal: Goal::Points(1000),
        ..Rules::default()
    }
}

/// A busy, made-up pad: the stick sweeps around, buttons come and go.
fn pad(tick: u64, salt: u64) -> Input {
    let a = (tick + salt * 101) as f32 * (0.02 + salt as f32 * 0.003);
    let phase = (tick / 9 + salt * 3) % 23;
    let buttons = match phase {
        0 => button::SHOOT,
        3 => button::PASS,
        5 => button::TRICK | button::R1,
        8 => button::JUMP,
        11 => button::R1 | button::L1,
        14 => button::PASS,
        17 => button::SHOOT | button::R1,
        _ => 0,
    };
    let mut i = Input::moving([math::sin(a), math::cos(a)]).with(buttons);
    i.set_aim([0.0, if phase == 20 { 1.0 } else { 0.0 }]);
    i
}

fn join(name: &str, team: Option<u8>, create: bool) -> Join {
    Join {
        match_name: "park".into(),
        players: vec![Joiner {
            name: name.into(),
            team,
        }],
        create: create.then(|| MatchSetup::standard(rules(), 2024)),
        // The client sends its own disc's digest.
        disc: [0; 32],
    }
}

struct Player {
    client: Client,
    salt: u64,
    events: Vec<ClientEvent>,
}

impl Player {
    fn new(transport: Box<dyn Transport>, join: Join, salt: u64) -> Self {
        let mut client = Client::new(
            transport,
            addr(SERVER),
            join,
            ClientConfig::default(),
            disc(),
        );
        client.set_ready(true);
        Self {
            client,
            salt,
            events: Vec::new(),
        }
    }

    /// One frame of the game: network, then the ticks due.
    fn frame(&mut self, now: Duration) {
        self.events.extend(self.client.update(now));
        for _ in 0..self.client.ticks_due(now) {
            let tick = self.client.session().map_or(0, |s| s.tick());
            for seat in self.client.seats().to_vec() {
                self.client.add_local_input(seat, pad(tick, self.salt));
            }
            self.client.advance();
        }
    }

    /// The confirmed tick and its checksum.
    fn confirmed(&self) -> (u64, u64) {
        let s = self.client.session().expect("playing");
        (s.confirmed().tick(), s.confirmed().checksum())
    }
}

/// Drifted resyncs per client address.
fn drifted(events: &[ServerEvent]) -> HashMap<SocketAddr, usize> {
    let mut n = HashMap::new();
    for e in events {
        if let ServerEvent::Resynced {
            addr,
            drifted: true,
            ..
        } = e
        {
            *n.entry(*addr).or_default() += 1;
        }
    }
    n
}

fn assert_agrees(server: &Server, p: &Player, what: &str) {
    let host = server.host("park").expect("match");
    let (tick, sum) = p.confirmed();
    assert!(
        tick + 300 > host.tick(),
        "{what}: confirmed tick {tick}, server at {}",
        host.tick()
    );
    assert_eq!(
        Some(sum),
        host.checksum_at(tick),
        "{what}: checksum at tick {tick}"
    );
}

/// Three clients play `ticks` ticks under `conditions`; C's confirmed
/// state is corrupted a third of the way in. Every client must end in step
/// with the server, A and B without ever drifting, C after a resync.
/// Returns the pads that missed their tick.
fn bad_network_match(conditions: Conditions, seed: u32, ticks: u64) -> u64 {
    let net = MemoryNetwork::new(conditions, seed);
    let mut server = Server::new(
        Box::new(net.endpoint(addr(SERVER))),
        ServerConfig::default(),
        disc(),
    );
    let a_addr = addr("10.0.0.2:5000");
    let b_addr = addr("10.0.0.3:5000");
    let c_addr = addr("10.0.0.4:5000");
    // The joins race over the lossy network: each brings the setup, and
    // whichever arrives first makes the match (the server has none of its
    // own).
    let mut players = vec![
        Player::new(Box::new(net.endpoint(a_addr)), join("A", Some(0), true), 1),
        Player::new(Box::new(net.endpoint(b_addr)), join("B", Some(1), true), 2),
        Player::new(Box::new(net.endpoint(c_addr)), join("C", None, true), 3),
    ];
    let mut server_events = Vec::new();
    let mut tampered = false;
    let step = Duration::from_millis(4);
    let mut now = Duration::ZERO;
    while server.host("park").is_none_or(|h| h.tick() < ticks) {
        now += step;
        assert!(now < Duration::from_secs(300), "the match never got going");
        net.set_time(now);
        server_events.extend(server.update(now));
        for p in &mut players {
            p.frame(now);
        }
        // Corrupt C's confirmed state once, mid-match.
        if !tampered && server.host("park").is_some_and(|h| h.tick() >= ticks / 3) {
            players[2].client.tamper(|m| {
                m.world.score[0] += 3;
                m.world.players[4].position[0] += 2.0;
            });
            tampered = true;
        }
    }
    // Let the last confirmations land.
    for _ in 0..100 {
        now += step;
        net.set_time(now);
        server_events.extend(server.update(now));
        for p in &mut players {
            p.frame(now);
        }
    }

    let host = server.host("park").unwrap();
    // Joins race over the lossy network, so the seat numbers vary; the sides
    // do not (seats 1P 3P 5P are home, 2P 4P 6P away).
    let seats: Vec<u8> = players.iter().map(|p| p.client.seats()[0]).collect();
    assert_eq!(seats[0] % 2, 0, "A plays home");
    assert_eq!(seats[1] % 2, 1, "B plays away");
    assert!(seats[0] != seats[2] && seats[1] != seats[2]);
    for (p, name) in players.iter().zip(["A", "B", "C"]) {
        assert_eq!(p.client.status(), Status::Playing, "{name}");
        assert_agrees(&server, p, name);
        let stats = p.client.stats().unwrap();
        assert!(stats.rollbacks > 0, "{name}: guesses never missed?");
        println!(
            "seed {seed} {name}: rtt {:?}, lead {}, {stats:?}",
            p.client.rtt().unwrap_or_default(),
            p.client.lead_target()
        );
    }
    let resyncs = drifted(&server_events);
    assert_eq!(resyncs.get(&a_addr), None, "A never drifts");
    assert_eq!(resyncs.get(&b_addr), None, "B never drifts");
    assert!(
        resyncs.get(&c_addr).copied().unwrap_or(0) >= 1,
        "C resynced"
    );
    assert!(
        players[2]
            .events
            .iter()
            .any(|e| matches!(e, ClientEvent::Resynced { .. }))
    );
    let late = host.late_pads();
    println!(
        "seed {seed}: late pads {late} of {}, datagrams sent and dropped {:?}",
        3 * host.tick(),
        net.stats()
    );
    late
}

#[test]
fn clients_agree_with_the_server_over_a_bad_network() {
    let conditions = Conditions {
        loss: 0.05,
        duplicate: 0.02,
        delay: Duration::from_millis(40),
        jitter: Duration::from_millis(30),
    };
    let ticks = 4000;
    let late = bad_network_match(conditions, 7, ticks);
    // Pads that missed their tick: the start, and the odd burst of loss.
    assert!(late < ticks * 3 / 20, "{late} late pads");
}

#[test]
fn clients_agree_over_harsh_networks() {
    let conditions = Conditions {
        loss: 0.15,
        duplicate: 0.05,
        delay: Duration::from_millis(70),
        jitter: Duration::from_millis(80),
    };
    for seed in 1..=5 {
        bad_network_match(conditions, seed, 2400);
    }
}

/// A machine whose frames hitch (150 ms, every second, as a slow PC's do):
/// its pads for those ticks go out late, and the server waits for them
/// (its grace) rather than play its last pad in their place, so its own
/// player is never pulled back, and neither machine has to aim its pads
/// further ahead (which would lengthen every guess at the others).
#[test]
fn a_hitching_machine_keeps_its_pads_through_the_hitches() {
    let net = MemoryNetwork::new(
        Conditions {
            loss: 0.01,
            duplicate: 0.0,
            delay: Duration::from_millis(30),
            jitter: Duration::from_millis(15),
        },
        11,
    );
    let mut server = Server::new(
        Box::new(net.endpoint(addr(SERVER))),
        ServerConfig::default(),
        disc(),
    );
    let mut players = [
        Player::new(
            Box::new(net.endpoint(addr("10.0.0.2:5000"))),
            join("A", Some(0), true),
            1,
        ),
        Player::new(
            Box::new(net.endpoint(addr("10.0.0.3:5000"))),
            join("B", Some(1), true),
            2,
        ),
    ];
    let step = Duration::from_millis(4);
    let mut now = Duration::ZERO;
    let mut warm = None;
    while server.host("park").is_none_or(|h| h.tick() < 5400) {
        now += step;
        assert!(now < Duration::from_secs(200));
        net.set_time(now);
        server.update(now);
        let hitch = now.as_millis() % 1000 < 150;
        for (k, p) in players.iter_mut().enumerate() {
            if k != 0 || !hitch {
                p.frame(now);
            }
        }
        if warm.is_none() && server.host("park").is_some_and(|h| h.tick() >= 2400) {
            warm = Some(
                players
                    .iter()
                    .map(|p| p.client.stats().unwrap())
                    .collect::<Vec<_>>(),
            );
        }
    }
    let warm = warm.unwrap();
    let (a, b) = (
        players[0].client.stats().unwrap(),
        players[1].client.stats().unwrap(),
    );
    println!(
        "A lead {} {a:?}; B lead {} {b:?}",
        players[0].client.lead_target(),
        players[1].client.lead_target()
    );
    assert!(players[0].client.lead_target() <= 3, "A stays close");
    assert!(players[1].client.lead_target() <= 3, "B stays close");
    // Fifty seconds of hitches once settled: hardly a pad of A's lost.
    assert!(a.overridden - warm[0].overridden <= 6, "{a:?}");
    assert!(b.overridden - warm[1].overridden <= 2, "{b:?}");
    assert_agrees(&server, &players[0], "A");
    assert_agrees(&server, &players[1], "B");
}

#[test]
fn late_joiners_get_the_state_and_leavers_hand_over() {
    let net = MemoryNetwork::new(
        Conditions {
            loss: 0.03,
            duplicate: 0.0,
            delay: Duration::from_millis(25),
            jitter: Duration::from_millis(10),
        },
        99,
    );
    let mut server = Server::new(
        Box::new(net.endpoint(addr(SERVER))),
        ServerConfig::default(),
        disc(),
    );
    let mut players = vec![
        Player::new(
            Box::new(net.endpoint(addr("10.0.0.2:5000"))),
            join("A", Some(0), true),
            1,
        ),
        Player::new(
            Box::new(net.endpoint(addr("10.0.0.3:5000"))),
            join("B", Some(1), true),
            2,
        ),
    ];
    let mut server_events = Vec::new();
    let step = Duration::from_millis(5);
    let mut now = Duration::ZERO;
    let mut joined = false;
    let mut left = false;
    while server.host("park").is_none_or(|h| h.tick() < 2400) {
        now += step;
        assert!(now < Duration::from_secs(120));
        net.set_time(now);
        server_events.extend(server.update(now));
        for p in &mut players {
            if p.client.status() != Status::Ended {
                p.frame(now);
            }
        }
        let tick = server.host("park").map_or(0, |h| h.tick());
        // Past the frames the server keeps: D needs the whole state.
        if !joined && tick >= 900 {
            players.push(Player::new(
                Box::new(net.endpoint(addr("10.0.0.5:5000"))),
                join("D", Some(0), false),
                4,
            ));
            joined = true;
        }
        if !left && tick >= 1600 {
            players[1].client.leave();
            left = true;
        }
    }
    let host = server.host("park").unwrap();
    // B's seat (2P) went back to the computer; D took 3P.
    assert_eq!(host.state().seats[1].player, None);
    assert!(host.state().seats[2].player.is_some());
    assert_eq!(players[2].client.seats(), &[2]);
    assert!(
        server_events
            .iter()
            .any(|e| matches!(e, ServerEvent::Resynced { drifted: false, .. }))
    );
    assert!(server_events.iter().any(|e| matches!(
        e,
        ServerEvent::Left {
            timed_out: false,
            ..
        }
    )));
    assert!(drifted(&server_events).is_empty(), "nobody drifts");
    assert_agrees(&server, &players[0], "A");
    assert_agrees(&server, &players[2], "D");
}

#[test]
fn full_sides_and_old_versions_are_refused() {
    let net = MemoryNetwork::new(Conditions::default(), 1);
    let mut server = Server::new(
        Box::new(net.endpoint(addr(SERVER))),
        ServerConfig::default(),
        disc(),
    );
    let mut clients: Vec<Client> = (0..4)
        .map(|k| {
            Client::new(
                Box::new(net.endpoint(addr(&format!("10.0.1.{k}:5000")))),
                addr(SERVER),
                join(&format!("H{k}"), Some(0), k == 0),
                ClientConfig::default(),
                disc(),
            )
        })
        .collect();
    let mut events: Vec<Vec<ClientEvent>> = vec![Vec::new(); 4];
    // Join one at a time, so the fourth is the one turned away.
    for k in 0..4 {
        for ms in 0..50 {
            let now = Duration::from_millis(k * 100 + ms * 2);
            net.set_time(now);
            server.update(now);
            events[k as usize].extend(clients[k as usize].update(now));
        }
    }
    for e in &events[..3] {
        assert!(matches!(e[0], ClientEvent::Welcome { .. }), "{e:?}");
    }
    assert_eq!(
        events[3],
        vec![ClientEvent::Refused(street_net::Refusal::Full)]
    );
    assert_eq!(clients[3].status(), Status::Ended);

    // A datagram from a newer protocol gets a refusal the sender can read.
    let mut stranger = net.endpoint(addr("10.0.2.1:5000"));
    stranger.send(
        addr(SERVER),
        &[street_net::MAGIC, street_net::VERSION + 1, 1],
    );
    let now = Duration::from_secs(1);
    net.set_time(now);
    server.update(now);
    let (_, reply) = stranger.recv().expect("an answer");
    assert_eq!(
        street_net::Message::decode(&reply),
        Ok(street_net::Message::Refused {
            reason: street_net::Refusal::Version
        })
    );
}

#[test]
fn a_match_over_loopback_udp() {
    let server_socket = UdpTransport::bind("127.0.0.1:0").unwrap();
    let server_addr = server_socket.local_addr().unwrap();
    let mut server = Server::new(Box::new(server_socket), ServerConfig::default(), disc());
    let mut players: Vec<Player> = (0..2)
        .map(|k| {
            let socket = UdpTransport::bind("127.0.0.1:0").unwrap();
            let mut join = join(&format!("P{k}"), None, k == 0);
            join.match_name = "loopback".into();
            let mut client = Client::new(
                Box::new(socket),
                server_addr,
                join,
                ClientConfig::default(),
                disc(),
            );
            client.set_ready(true);
            Player {
                client,
                salt: k + 1,
                events: Vec::new(),
            }
        })
        .collect();
    let step = Duration::from_millis(4);
    let mut now = Duration::ZERO;
    while server.host("loopback").is_none_or(|h| h.tick() < 1200) {
        now += step;
        assert!(now < Duration::from_secs(60), "stuck");
        server.update(now);
        for p in &mut players {
            p.frame(now);
        }
        std::thread::yield_now();
    }
    let host = server.host("loopback").unwrap();
    for p in &players {
        let (tick, sum) = p.confirmed();
        assert!(tick + 120 > host.tick());
        assert_eq!(Some(sum), host.checksum_at(tick));
    }
}

#[test]
fn the_lobby_moves_sides_shows_picks_and_takes_the_creators_setup() {
    let net = MemoryNetwork::new(
        Conditions {
            loss: 0.2,
            duplicate: 0.05,
            delay: Duration::from_millis(30),
            jitter: Duration::from_millis(20),
        },
        11,
    );
    let mut server = Server::new(
        Box::new(net.endpoint(addr(SERVER))),
        ServerConfig::default(),
        disc(),
    );
    let mut host = Player::new(
        Box::new(net.endpoint(addr("10.0.0.2:5000"))),
        join("Host", Some(0), true),
        1,
    );
    host.client.set_ready(false);
    // The match is there before anyone else asks for it.
    let mut clock = 0u64;
    while host.client.status() != Status::Lobby {
        clock += 4;
        let now = Duration::from_millis(clock);
        net.set_time(now);
        server.update(now);
        host.frame(now);
    }
    let mut guest = Player::new(
        Box::new(net.endpoint(addr("10.0.0.3:5000"))),
        join("Guest", Some(0), false),
        2,
    );
    // Nobody is ready until the picks are in.
    guest.client.set_ready(false);
    let mut run = |host: &mut Player, guest: &mut Player, server: &mut Server, ms: u64| {
        for _ in 0..ms / 4 {
            clock += 4;
            let now = Duration::from_millis(clock);
            net.set_time(now);
            server.update(now);
            host.frame(now);
            guest.frame(now);
        }
    };
    run(&mut host, &mut guest, &mut server, 2000);
    assert_eq!(host.client.seats(), &[0]);
    assert_eq!(guest.client.seats(), &[2], "both home: 1P and 3P");
    // The guest moves to away, and says what they picked.
    guest.client.choose_side(1);
    guest.client.set_pick(vec![7, 9]);
    run(&mut host, &mut guest, &mut server, 3000);
    assert_eq!(guest.client.seats(), &[1], "away's first seat, 2P");
    assert!(
        guest
            .events
            .iter()
            .any(|e| matches!(e, ClientEvent::Welcome { seats } if seats == &[1]))
    );
    let lobby = host.client.lobby();
    let theirs = lobby.iter().find(|s| s.name == "Guest").expect("listed");
    assert_eq!((theirs.seat, theirs.team), (1, 1));
    assert_eq!(theirs.pick, vec![7, 9]);
    // The guest can't replace the setup; the creator can.
    let mut picked = MatchSetup::standard(rules(), 99);
    picked.court = "PHILLY".into();
    guest.client.propose_setup(picked.clone());
    run(&mut host, &mut guest, &mut server, 2000);
    assert_eq!(guest.client.setup_version(), 0);
    host.client.propose_setup(picked);

    run(&mut host, &mut guest, &mut server, 3000);
    for p in [&host, &guest] {
        assert_eq!(p.client.setup_version(), 1);
        assert_eq!(p.client.setup().unwrap().court, "PHILLY");
        assert_eq!(p.client.setup().unwrap().seed, 99);
        assert!(
            p.events
                .iter()
                .any(|e| matches!(e, ClientEvent::Setup { version: 1 }))
        );
    }
    // Then everyone readies and the match plays the picked setup.
    host.client.set_ready(true);
    guest.client.set_ready(true);
    run(&mut host, &mut guest, &mut server, 6000);
    for p in [&host, &guest] {
        assert_eq!(p.client.status(), Status::Playing);
    }
    assert_eq!(server.host("park").unwrap().state().world.rules, rules());
    assert_agrees(&server, &guest, "guest");
    assert_agrees(&server, &host, "host");
}
