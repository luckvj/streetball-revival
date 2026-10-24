//! Fire every AEMS event of a court's match against its banks and print the
//! voices the scripts start: which bank, which sound, at what settings.
//!
//!     cargo run -p street-audio --release --example aems_report -- <iso> [court] [event]
//!
//! `court` is an `XDBCOURT` record (default 9, Rucker Park). Each event is
//! fired alone with its parameters at plausible values (straight ahead, a
//! middling distance, a kind of 1) and the scripts run for two seconds.

use street_assets::iso::Iso;
use street_audio::{
    aems::{Engine, EventMap, Mixer, Program, Sound, TICK_MS, VoiceParams},
    courts,
};

#[derive(Default)]
struct Log {
    next: u32,
    started: Vec<(u32, usize, Sound, VoiceParams)>,
    stopped: Vec<u32>,
    /// Voice, ms left.
    live: Vec<(u32, u32)>,
}

impl Mixer for Log {
    fn start(&mut self, bank: usize, sound: Sound, params: VoiceParams) -> Option<u32> {
        self.next += 1;
        self.started.push((self.next, bank, sound, params));
        self.live.push((self.next, 600));
        Some(self.next)
    }
    fn update(&mut self, voice: u32, params: VoiceParams) {
        if let Some(v) = self.started.iter_mut().find(|v| v.0 == voice)
            && params.gain() > v.3.gain()
        {
            v.3 = params;
        }
    }
    fn stop(&mut self, voice: u32) {
        self.stopped.push(voice);
        self.live.retain(|v| v.0 != voice);
    }
    fn position(&mut self, voice: u32) -> Option<(u32, u32)> {
        let v = self.live.iter_mut().find(|v| v.0 == voice)?;
        if v.1 == 0 {
            return None;
        }
        v.1 = v.1.saturating_sub(TICK_MS as u32);
        Some((600 - v.1, v.1))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(iso_path) = args.first() else {
        eprintln!("usage: aems_report <iso> [court] [event]");
        std::process::exit(2);
    };
    let court: usize = args.get(1).map_or(Ok(9), |s| s.parse())?;
    let only: Option<i32> = args
        .get(2)
        .map(|s| i32::from_str_radix(s.trim_start_matches("0x"), 16))
        .transpose()?;
    let mut iso = Iso::open(iso_path)?;
    let map = EventMap::parse(&iso.read("/SFX/GAME_EVT.AEM")?)?;
    let (ambience, crowd, _) = courts::banks(Some(court));
    let mut files: Vec<&str> = courts::COMMON.to_vec();
    files.extend([ambience, crowd, "/SPEECH/SPCH_PLY.ABK"]);
    let programs: Vec<Program> = files
        .iter()
        .map(|f| Ok(Program::parse(&iso.read(f)?)?))
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
    for (f, p) in files.iter().zip(&programs) {
        println!(
            "{f}: bank {:#06x}, {} modules, {} streams",
            p.bank,
            p.modules.len(),
            p.streams.len()
        );
    }
    for event in 0..map.events.len() as i32 {
        if only.is_some_and(|e| e != event) {
            continue;
        }
        let def = &map.events[event as usize];
        let mut engine = Engine::new(map.clone(), 7);
        for p in &programs {
            engine.load(p.clone());
        }
        // The game's default mix (0x1a2340): crowd 54/40, player speech 88,
        // reactions 90, effects 87, gamebreaker 86, environment 60.
        for (g, v) in [54, 40, 88, 90, 87, 86, 60].into_iter().enumerate() {
            engine.set_global(g, 32767 * v / 100);
        }
        let mut log = Log::default();
        // The court comes alive first (0x28, from the match intro), which
        // starts the mixer that sets every module's volume (event 0x1b).
        if event != 0x28 {
            engine.fire(&[0x28, 500], &mut log);
            for _ in 0..50 {
                engine.tick(&mut log);
            }
            log = Log {
                next: log.next,
                live: log.live,
                ..Log::default()
            };
        }
        let mut msg = vec![event, 0, 300];
        msg.extend(std::iter::repeat_n(1, def.params.len().saturating_sub(2)));
        let handle = engine.fire(&msg, &mut log);
        if let Ok(spec) = std::env::var("AEMS_TRACE") {
            // slot,module: print that module's first instance for a few passes.
            let (slot, module) = spec
                .split_once(',')
                .map(|(a, b)| (a.parse().unwrap_or(0), b.parse().unwrap_or(0)))
                .unwrap_or((0, 0));
            for t in 0..6 {
                if let Some(cells) = engine.inspect(slot, module) {
                    println!("  pass {t}");
                    for (i, (k, inp, st)) in cells.iter().enumerate() {
                        println!("    n{i:<2} k{k:02x} in {inp:?} st {:?}", &st[..4]);
                    }
                }
                engine.tick(&mut log);
            }
        }
        for _ in 0..200 {
            engine.tick(&mut log);
        }
        if log.started.is_empty() && handle.is_none() {
            continue;
        }
        let live: usize = engine.live().map(|l| l.2).sum();
        println!(
            "event {event:#04x} ({} params) handle {handle:?}, {live} instances still live",
            def.params.len()
        );
        for (_, bank, sound, p) in &log.started {
            println!(
                "    {:<22} {:?}  loudest: gain {:.2} pan {:+.2} rate {:.2} reverb {}",
                files[*bank],
                sound,
                p.gain(),
                p.pan(),
                p.rate(),
                p.reverb
            );
        }
    }
    Ok(())
}
