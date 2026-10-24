//! `street-server`: a headless match server for Streetball Revival online play.
//!
//! It hosts any number of named matches on one UDP port (see
//! [`street_net::Server`]), refereeing each on the original game disc it
//! is given (`--vol2`): the referee plays the disc's own animations, gait
//! records and staged gamebreakers, as every player's game does on theirs
//! (`street_net::disc`). Without the disc it refuses to host. Only digests
//! and pads cross the network: a joiner whose disc data differs (another
//! region's disc, a modded one) is refused.
//!
//! A match is made by the first game to join it, with that game's setup
//! (the server makes none of its own). Every setup's roster is checked
//! against the online rules (`street_assets::roster_rules`), rebuilt over
//! the disc's tables, and its world against it before the match is made
//! (`street_net::roster`): the anti-cheat. A roster built on a roster mod
//! (the Legendary Edition) is accepted only when the server holds that mod
//! (`--roster-db`, as many as it accepts), unless `--trust-roster-mods`.
//!
//! ```text
//! street-server --vol2 <iso> [--port 7375] [--bind 0.0.0.0] [--max-matches 64]
//!               [--roster-db <dir>]... [--trust-roster-mods]
//! ```

use std::process::ExitCode;
use std::time::{Duration, Instant};

use street_net::{DEFAULT_PORT, Disc, Server, ServerConfig, ServerEvent, UdpTransport};

const USAGE: &str = "\
usage: street-server --vol2 ISO [options]
  --vol2 ISO          the original game disc (required): the referee
                      plays every match on its animations, gait and staged
                      gamebreakers, and checks rosters against its tables
  --port N            UDP port (default 7375)
  --bind ADDR         address to listen on (default 0.0.0.0)
  --max-matches N     most matches at once (default 64)
  --roster-db DIR     a roster mod this server accepts (its folder, or the
                      folder holding xdbplyr.adf); repeat for more. Rosters
                      built on any other mod are refused.
  --trust-roster-mods accept rosters built on mods this server lacks
  --help";

/// Why the server won't host without the disc.
const NO_DISC: &str = "\
street-server needs the original game disc (--vol2 <iso>) to host: the
referee plays every match on the disc's animations, gait records and staged
gamebreakers, as each player's game does on theirs. Only digests and pads
cross the network, never the disc's data, so a server without the disc has
nothing to referee with.";

/// The command line.
#[derive(Debug, PartialEq)]
struct Options {
    bind: String,
    port: u16,
    max_matches: usize,
    vol2: Option<std::path::PathBuf>,
    roster_dbs: Vec<std::path::PathBuf>,
    trust_roster_mods: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        bind: "0.0.0.0".into(),
        port: DEFAULT_PORT,
        max_matches: 64,
        vol2: None,
        roster_dbs: Vec::new(),
        trust_roster_mods: false,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--help" || flag == "-h" {
            return Err(String::new());
        }
        if flag == "--trust-roster-mods" {
            o.trust_roster_mods = true;
            continue;
        }
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} needs a value"))?
            .as_str();
        let number = |v: &str| {
            v.parse::<u64>()
                .map_err(|_| format!("{flag}: not a number"))
        };
        match flag.as_str() {
            "--port" => o.port = u16::try_from(number(value)?).map_err(|_| "bad port")?,
            "--bind" => o.bind = value.to_string(),
            // The match's creator sets its rules; the server makes none.
            "--mode" | "--play-to" | "--shot-clock" | "--difficulty" | "--seed" => {
                return Err(format!(
                    "{flag} is gone: a match takes the rules of the game that makes it"
                ));
            }
            "--max-matches" => o.max_matches = number(value)? as usize,
            "--vol2" => o.vol2 = Some(value.into()),
            "--roster-db" => o.roster_dbs.push(value.into()),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok(o)
}

fn log(clock: Instant, line: &str) {
    let t = clock.elapsed().as_secs();
    println!("[{:02}:{:02}:{:02}] {line}", t / 3600, t / 60 % 60, t % 60);
}

fn describe(e: &ServerEvent) -> String {
    match e {
        ServerEvent::Created { name } => format!("match {name:?} created"),
        ServerEvent::Joined { name, addr, seats } => {
            let seats: Vec<String> = seats.iter().map(|s| format!("{}P", s + 1)).collect();
            format!("{addr} joined {name:?} as {}", seats.join(" "))
        }
        ServerEvent::Left {
            name,
            addr,
            timed_out,
        } => {
            let how = if *timed_out { "timed out of" } else { "left" };
            format!("{addr} {how} {name:?}")
        }
        ServerEvent::Started { name } => format!("match {name:?} started"),
        ServerEvent::Reconfigured { name } => {
            format!("match {name:?} set up as its lobby picked it")
        }
        ServerEvent::Resynced {
            name,
            addr,
            tick,
            drifted,
        } => {
            let why = if *drifted { "drifted" } else { "behind" };
            format!("{addr} in {name:?} {why}: sent the state at tick {tick}")
        }
        ServerEvent::Closed { name } => format!("match {name:?} closed"),
        ServerEvent::Faulted { name, reason } => {
            format!("match {name:?} stopped after a fault: {reason}")
        }
        ServerEvent::Refused {
            name,
            addr,
            reasons,
        } => format!("{addr} refused for {name:?}: {}", reasons.join("; ")),
    }
}

/// The disc (`--vol2`): what the referee plays every match on, and the
/// disc's own player and team tables for the roster mods.
fn read_disc(
    path: &std::path::Path,
) -> Result<(Disc, (street_assets::db::Table, street_assets::db::Table)), String> {
    use street_assets::db;
    let what = |e: &dyn std::fmt::Display| format!("--vol2 {}: {e}", path.display());
    let mut iso = street_assets::iso::Iso::open(path).map_err(|e| what(&e))?;
    let disc = Disc::read(&mut iso).map_err(|e| what(&e))?;
    let players = db::Table::read(&mut iso, db::PLAYERS).map_err(|e| what(&e))?;
    let teams = db::Table::read(&mut iso, db::TEAMS).map_err(|e| what(&e))?;
    Ok((disc, (players, teams)))
}

/// What the server checks rosters with: the disc's tables and the roster
/// mods it accepts (`--roster-db`).
fn known(
    o: &Options,
    disc: &Disc,
    original: &(street_assets::db::Table, street_assets::db::Table),
) -> Result<street_net::Known, String> {
    use street_assets::db;
    let mut mods = Vec::new();
    for dir in &o.roster_dbs {
        let found = db::find_roster_dir(dir).ok_or_else(|| {
            format!(
                "--roster-db {}: no {} there or in its database folders",
                dir.display(),
                db::PLAYERS_FILE
            )
        })?;
        let what = |e: street_assets::Error| format!("--roster-db {}: {e}", dir.display());
        let id = db::roster_mod_id(&found).map_err(what)?;
        let (players, teams) =
            db::roster_mod_tables(&found, Some((&original.0, &original.1))).map_err(what)?;
        mods.push(street_net::ModTables { id, players, teams });
    }
    Ok(street_net::Known {
        disc: Some(disc.tables().clone()),
        mods,
        trust_mods: o.trust_roster_mods,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let o = match parse(&args) {
        Ok(o) => o,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("street-server: {e}");
            }
            eprintln!("{USAGE}");
            return if e.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
    };
    // The referee holds the disc, or there is no server.
    let Some(path) = o.vol2.clone() else {
        eprintln!("{NO_DISC}");
        return ExitCode::FAILURE;
    };
    let (disc, original) = match read_disc(&path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("street-server: {e}");
            return ExitCode::FAILURE;
        }
    };
    let transport = match UdpTransport::bind((o.bind.as_str(), o.port)) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("street-server: cannot listen on {}:{}: {e}", o.bind, o.port);
            return ExitCode::FAILURE;
        }
    };
    let clock = Instant::now();
    let rosters = match known(&o, &disc, &original) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("street-server: {e}");
            return ExitCode::FAILURE;
        }
    };
    for m in &rosters.mods {
        println!(
            "accepting rosters built on roster mod {}",
            street_assets::roster_share::hex(&m.id)
        );
    }
    println!(
        "refereeing on the disc at {} (data {})",
        path.display(),
        street_assets::roster_share::hex(&disc.digest())
    );
    let cfg = ServerConfig {
        max_matches: o.max_matches,
        rosters: std::sync::Arc::new(rosters),
        ..Default::default()
    };
    let addr = transport
        .local_addr()
        .map_or_else(|_| format!("{}:{}", o.bind, o.port), |a| a.to_string());
    log(clock, &format!("street-server listening on udp {addr}"));
    let mut server = Server::new(Box::new(transport), cfg, std::sync::Arc::new(disc));
    loop {
        for e in server.update(clock.elapsed()) {
            log(clock, &describe(&e));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn defaults() {
        let o = parse(&[]).unwrap();
        assert_eq!(o.port, DEFAULT_PORT);
        assert_eq!(o.bind, "0.0.0.0");
        assert_eq!(o.vol2, None);
    }

    #[test]
    fn flags_in_any_order() {
        let o = parse(&args("--port 9000 --vol2 disc.iso --max-matches 8")).unwrap();
        assert_eq!(o.port, 9000);
        assert_eq!(o.vol2, Some("disc.iso".into()));
        assert_eq!(o.max_matches, 8);
        assert!(!o.trust_roster_mods);
        let o = parse(&args("--roster-db A --trust-roster-mods --roster-db B")).unwrap();
        assert_eq!(
            o.roster_dbs,
            vec![std::path::PathBuf::from("A"), "B".into()]
        );
        assert!(o.trust_roster_mods);
    }

    #[test]
    fn bad_lines_are_refused() {
        assert!(parse(&args("--port")).is_err());
        assert!(parse(&args("--port 70000")).is_err());
        assert!(parse(&args("--frobnicate 1")).is_err());
        assert_eq!(parse(&args("--help")), Err(String::new()));
        // The server makes no matches of its own, so it sets no rules.
        for gone in [
            "--mode normal",
            "--play-to 11",
            "--seed 5",
            "--shot-clock off",
        ] {
            assert!(
                parse(&args(gone)).unwrap_err().contains("is gone"),
                "{gone}"
            );
        }
    }

    #[test]
    fn the_server_refuses_to_host_without_the_disc() {
        assert!(NO_DISC.contains("--vol2"));
        let missing = std::path::Path::new("no such disc.iso");
        let Err(why) = read_disc(missing) else {
            panic!("a server without its disc");
        };
        assert!(why.starts_with("--vol2 no such disc.iso"), "{why}");
    }
}
