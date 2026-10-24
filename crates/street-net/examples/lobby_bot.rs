//! A person for trying an online lobby with fewer machines: joins a match,
//! takes a side, leaves the picking to the side's first person (it never
//! picks), readies once the creator's match is in, and plays its seat with
//! the computer's own pad.
//!
//!     cargo run -p street-net --example lobby_bot -- <iso> <server> <name> <home|away> [seconds]
//!
//! It shows the lobby the game's way (`street-game`'s `lobby.rs`): its
//! pick is the stage byte (1 side set, then 3 picked) and no sides.

use std::sync::Arc;
use std::time::{Duration, Instant};

use street_net::{Client, ClientConfig, ClientEvent, Disc, Join, Joiner, Status, UdpTransport};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [iso, server, name, side, rest @ ..] = &args[..] else {
        return Err("usage: lobby_bot <iso> <server> <name> <home|away> [seconds]".into());
    };
    let team = match side.as_str() {
        "home" => 0,
        "away" => 1,
        other => return Err(format!("side {other:?} is not home or away").into()),
    };
    let seconds: u64 = rest.first().map_or(Ok(120), |s| s.parse())?;
    let mut iso = street_assets::iso::Iso::open(iso)?;
    let disc = Arc::new(Disc::read(&mut iso)?);
    let addr = street_net_resolve(server)?;
    let join = Join {
        match_name: "street".into(),
        players: vec![Joiner {
            name: name.clone(),
            team: Some(team),
        }],
        create: None,
        disc: [0; 32],
    };
    let mut client = Client::new(
        Box::new(UdpTransport::bind("0.0.0.0:0")?),
        addr,
        join,
        ClientConfig {
            timeout: Duration::from_secs(10),
            ..ClientConfig::default()
        },
        disc,
    );
    let clock = Instant::now();
    let mut stage = 1u8;
    while clock.elapsed() < Duration::from_secs(seconds) {
        for e in client.update(clock.elapsed()) {
            match e {
                ClientEvent::Setup { version } => {
                    println!("{name}: the match, version {version}");
                    client.set_ready(true);
                }
                ClientEvent::Started { tick } => println!("{name}: started at {tick}"),
                ClientEvent::Refused(r) => return Err(format!("refused: {r:?}").into()),
                ClientEvent::Closed(r) => {
                    println!("{name}: closed: {r:?}");
                    return Ok(());
                }
                ClientEvent::TimedOut => return Err("timed out".into()),
                _ => {}
            }
        }
        if client.status() == Status::Lobby {
            let everyone_set = !client.lobby().is_empty()
                && client
                    .lobby()
                    .iter()
                    .all(|e| e.pick.first().copied().unwrap_or(0) >= 1);
            if everyone_set {
                stage = 3;
            }
            client.set_pick(vec![stage, 0]);
        }
        for _ in 0..client.ticks_due(clock.elapsed()) {
            let seats = client.seats().to_vec();
            for seat in seats {
                let pad = client
                    .session()
                    .and_then(|s| {
                        let m = s.current();
                        let player = m.seats.get(usize::from(seat))?.player?;
                        Some(street_sim::ai::input(&m.world, usize::from(player)))
                    })
                    .unwrap_or_default();
                client.add_local_input(seat, pad);
            }
            client.advance();
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    client.leave();
    Ok(())
}

fn street_net_resolve(server: &str) -> Result<std::net::SocketAddr, Box<dyn std::error::Error>> {
    use std::net::ToSocketAddrs;
    let with_port = if server.contains(':') {
        server.to_string()
    } else {
        format!("{server}:{}", street_net::DEFAULT_PORT)
    };
    Ok(with_port
        .to_socket_addrs()?
        .next()
        .ok_or("no address for the server")?)
}
