//! Print each in-game song's PathFinder map: what every event starts on
//! each track, the transitions, and the stinger banks' sounds.
//!
//!     cargo run -p street-audio --release --example path_report -- <iso> [song]

use street_assets::iso::Iso;
use street_audio::{bank::Bank, music, path, schl};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(iso_path) = args.first() else {
        eprintln!("usage: path_report <iso> [song folder]");
        std::process::exit(2);
    };
    let mut iso = Iso::open(iso_path)?;
    let songs = music::songs(&String::from_utf8_lossy(&iso.read(music::PATHSONG)?));
    for song in songs
        .iter()
        .filter(|s| args.get(1).is_none_or(|f| s.folder.eq_ignore_ascii_case(f)))
    {
        let map = path::Map::parse(&iso.read(&song.map())?)?;
        println!(
            "== {} ({}): {} nodes, {} segments",
            song.folder,
            song.title,
            map.nodes.len(),
            map.segments.len()
        );
        for e in 0..map.events {
            let mut line = format!("  event {e:2}:");
            for t in 0..map.tracks {
                let Some(r) = map.router(t, e) else { continue };
                // The segments the event's path plays first, up to a loop.
                let mut p = path::Player::new(map.clone(), 1);
                let mut segs = Vec::new();
                if r.node.is_some() {
                    p.event(e);
                    for _ in 0..12 {
                        match p.next(t) {
                            path::Cue::Play(n) => segs.push(map.nodes[usize::from(n)].segment),
                            path::Cue::Stop => break,
                        }
                    }
                }
                line += &format!("  t{t} {:02x}->{:?} {:?}", r.flags, r.node, segs);
            }
            println!("{line}");
        }
        for (k, list) in map.transitions.iter().enumerate() {
            println!("  transitions {}: {list:?}", k + 1);
        }
        for (path, name) in [(song.whoosh(), "whoosh"), (song.tricks(), "tricks")] {
            let bank = Bank::parse(iso.read(&path)?)?;
            let lens: Vec<String> = bank
                .sounds
                .iter()
                .map(|p| {
                    p.as_ref().map_or("-".into(), |p| {
                        format!(
                            "{}ms/{}ch",
                            p.samples as u64 * 1000 / u64::from(p.rate),
                            p.channels
                        )
                    })
                })
                .collect();
            println!("  {name}: {}", lens.join(" "));
        }
        let main = iso.read(&song.main())?;
        let streams = schl::streams(&main)?;
        let segs: Vec<String> = map
            .segments
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let real = streams
                    .get(k)
                    .filter(|_| s.track == 0)
                    .map_or(String::new(), |st| {
                        format!(
                            "/{}",
                            st.patch.samples as u64 * 1000 / u64::from(st.patch.rate)
                        )
                    });
                format!("{}:{}{real}", k + 1, s.millis)
            })
            .collect();
        println!("  segments {}", segs.join(" "));
    }
    Ok(())
}
