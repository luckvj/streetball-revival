//! Checks of the audio decoders against a real USA disc image of the original game.
//! Skipped unless STREET_ISO points at the image.
//!
//! Without ears, a decode is judged by two numbers. Real audio is smooth: the
//! mean step between samples is a small part of the mean level, where a
//! wrong decoder makes noise (a ratio near 1.4). And the two channels of a
//! stereo recording move together, where mis-split channels do not.

use std::{fs::File, io::BufReader};

use street_assets::iso::Iso;
use street_audio::{Clip, bank::Bank, courts, music, path, schl, speech::Commentary};

type Disc = Iso<BufReader<File>>;

fn disc() -> Option<Disc> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

/// Mean |step| over mean |level| of channel 0.
fn roughness(c: &Clip) -> f64 {
    let ch = c.channels as usize;
    let s: Vec<f64> = c.samples.iter().step_by(ch).map(|&v| v as f64).collect();
    let level = s.iter().map(|v| v.abs()).sum::<f64>() / s.len() as f64;
    let step = s.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (s.len() - 1) as f64;
    step / level
}

/// The share of samples at full scale: a wrong decoder saturates.
fn clipped(c: &Clip) -> f64 {
    c.samples
        .iter()
        .filter(|&&v| v == i16::MAX || v == i16::MIN)
        .count() as f64
        / c.samples.len() as f64
}

/// Correlation of the left and right channels.
fn stereo_correlation(c: &Clip) -> f64 {
    let (mut ll, mut rr, mut lr) = (0f64, 0f64, 0f64);
    for f in c.samples.as_chunks::<2>().0 {
        let (l, r) = (f[0] as f64, f[1] as f64);
        ll += l * l;
        rr += r * r;
        lr += l * r;
    }
    lr / (ll * rr).sqrt()
}

#[test]
fn the_title_song_decodes_as_music() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let data = iso.read(music::TITLE_SONG).unwrap();
    let streams = schl::streams(&data).unwrap();
    assert_eq!(streams.len(), 1);
    let clip = streams[0].decode(&data);
    assert_eq!((clip.rate, clip.channels), (44100, 2));
    assert_eq!(clip.frames(), streams[0].patch.samples as usize);
    let (rough, corr) = (roughness(&clip), stereo_correlation(&clip));
    assert!(rough < 0.5, "roughness {rough}");
    assert!(corr > 0.5, "stereo correlation {corr}");
}

#[test]
fn every_song_lists_and_its_segments_decode() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let cfg = String::from_utf8_lossy(&iso.read(music::PATHSONG).unwrap()).into_owned();
    let songs = music::songs(&cfg);
    assert_eq!(songs.len(), 15);
    for song in &songs {
        let data = iso.read(&song.main()).unwrap();
        let segments = schl::streams(&data).unwrap();
        assert!(
            segments.len() > 10,
            "{}: {} segments",
            song.folder,
            segments.len()
        );
        let clip = segments[0].decode(&data);
        assert_eq!(clip.frames(), segments[0].patch.samples as usize);
        assert!(roughness(&clip) < 0.5, "{}", song.folder);
    }
}

#[test]
fn every_sound_bank_decodes() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let mut paths: Vec<String> = iso
        .files()
        .map(|(p, _)| p.to_owned())
        .filter(|p| p.ends_with(".ABK") || p.ends_with(".BNK"))
        .collect();
    paths.push("/MUSIC/BLAZE5/WHOOSH.MUS".into());
    let mut sounds = 0;
    for path in &paths {
        let bank = Bank::parse(iso.read(path).unwrap()).unwrap_or_else(|e| panic!("{path}: {e}"));
        for i in 0..bank.len() {
            let Some(p) = &bank.sounds[i] else { continue };
            // A few slots are 84-sample placeholders.
            if p.samples < 1000 {
                continue;
            }
            let clip = bank.clip(i).unwrap_or_else(|e| panic!("{path} #{i}: {e}"));
            assert_eq!(clip.frames(), p.samples as usize);
            // Ambience such as traffic hiss or surf is noise by nature (up to
            // about 1.7, with a flat spectrum and normal peaks); what a broken
            // decode does is saturate, which the clipping check catches.
            let (rough, clip_share) = (roughness(&clip), clipped(&clip));
            assert!(rough < 2.0, "{path} #{i}: roughness {rough}");
            assert!(clip_share < 0.01, "{path} #{i}: {clip_share} clipped");
            sounds += 1;
        }
    }
    assert!(sounds > 300, "only {sounds} sounds");
}

#[test]
fn each_court_has_its_crowd() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    for court in [None, Some(0), Some(6), Some(9), Some(10)] {
        let (ambience, crowd, stream) = courts::banks(court);
        Bank::parse(iso.read(ambience).unwrap()).unwrap();
        Bank::parse(iso.read(crowd).unwrap()).unwrap();
        let data = iso.read(stream).unwrap();
        let streams = schl::streams(&data).unwrap();
        // The bed is the longest stream, stereo and minutes long; the crowd
        // calls follow it.
        let bed = streams.iter().max_by_key(|s| s.patch.samples).unwrap();
        assert_eq!(bed.patch.channels, 2, "{stream}");
        assert!(bed.patch.samples > 36000 * 60, "{stream}");
        assert!(streams.len() > 20, "{stream}");
    }
}

#[test]
fn the_commentary_lines_decode() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let c = Commentary::open(&mut iso).unwrap();
    assert_eq!(c.entries.len(), 156);
    for prefix in [
        "Dunk\\",
        "Steal\\",
        "Block\\",
        "Shot\\",
        "Intro\\",
        "Outro\\",
        "GamePoint",
    ] {
        let files: Vec<_> = c.files(prefix).cloned().collect();
        assert!(!files.is_empty(), "{prefix}");
        let lines = c.lines(&mut iso, &files[0]).unwrap();
        assert!(!lines.is_empty(), "{}", lines.name);
        let clip = lines.clip(0).unwrap();
        assert_eq!(clip.channels, 1);
        assert!(
            clip.seconds() > 0.3 && clip.seconds() < 15.0,
            "{} {}",
            lines.name,
            clip.seconds()
        );
        assert!(roughness(&clip) < 0.6, "{}", lines.name);
    }
}

#[test]
fn every_song_map_matches_its_music() {
    let Some(mut iso) = disc() else { return };
    let cfg = iso.read(music::PATHSONG).unwrap();
    let songs = music::songs(&String::from_utf8_lossy(&cfg));
    assert!(!songs.is_empty());
    for song in &songs {
        let map = path::Map::parse(&iso.read(&song.map()).unwrap()).unwrap();
        assert_eq!(
            (map.tracks, map.sections, map.events),
            (3, 1, 16),
            "{}",
            song.folder
        );
        // Track 0's segments are MAIN.MUS's streams, in order.
        let main = iso.read(&song.main()).unwrap();
        let starts: Vec<u32> = schl::streams(&main)
            .unwrap()
            .iter()
            .map(|s| s.start as u32)
            .collect();
        let mains: Vec<u32> = map
            .segments
            .iter()
            .filter(|s| s.track == 0)
            .map(|s| s.at)
            .collect();
        assert_eq!(mains, starts, "{}: MAIN.MUS segments", song.folder);
        // The banks' segments are sounds in them.
        for (track, path) in [(1, song.whoosh()), (2, song.tricks())] {
            let bank = Bank::parse(iso.read(&path).unwrap()).unwrap();
            for s in map.segments.iter().filter(|s| s.track == track) {
                assert!(
                    (s.at as usize) < bank.len(),
                    "{}: {path} sound {}",
                    song.folder,
                    s.at
                );
            }
        }
        // Every node's segment, branch and router points inside the map.
        let n = map.nodes.len();
        for node in &map.nodes {
            assert!(node.segment == 0xffff || usize::from(node.segment) <= map.segments.len());
            for b in &node.branches {
                assert!(
                    b.next == 0xffff || usize::from(b.next) < n,
                    "{}",
                    song.folder
                );
            }
        }
        assert!(
            map.routers
                .iter()
                .all(|r| r.node.is_none_or(|k| usize::from(k) < n))
        );
        // The intro plays: event 0 starts a segment on the main track.
        let mut p = path::Player::new(map, 1);
        assert!(p.event(path::event::INTRO).contains(&0), "{}", song.folder);
        assert!(matches!(p.next(0), path::Cue::Play(_)), "{}", song.folder);
    }
}

/// Every bank's scripts parse, their sounds exist in the bank or its
/// stream, and each court's match events reach a module: the event map's
/// actions that name a loaded bank resolve to one of its nodes.
#[test]
fn every_bank_script_runs_against_its_sounds() {
    use street_audio::aems::{Engine, EventMap, Mixer, Program, Sound, VoiceParams};
    let Some(mut iso) = disc() else { return };
    let map = EventMap::parse(&iso.read("/SFX/GAME_EVT.AEM").unwrap()).unwrap();
    assert_eq!(map.events.len(), 0x52);

    struct Count(Vec<Sound>, u32);
    impl Mixer for Count {
        fn start(&mut self, _: usize, s: Sound, _: VoiceParams) -> Option<u32> {
            self.0.push(s);
            self.1 += 1;
            Some(self.1)
        }
        fn update(&mut self, _: u32, _: VoiceParams) {}
        fn stop(&mut self, _: u32) {}
        fn position(&mut self, _: u32) -> Option<(u32, u32)> {
            Some((0, 1000))
        }
    }

    for court in (0..11).map(Some).chain([None]) {
        let (ambience, crowd, ast) = courts::banks(court);
        let mut engine = Engine::new(map.clone(), 1);
        let mut sizes = Vec::new();
        for f in courts::COMMON.iter().copied().chain([ambience, crowd]) {
            let data = iso.read(f).unwrap();
            let p = Program::parse(&data).unwrap_or_else(|e| panic!("{f}: {e}"));
            assert!(!p.modules.is_empty(), "{f}");
            let ast_len = (f == crowd).then(|| iso.read(ast).unwrap().len());
            if let Some(len) = ast_len {
                for s in &p.streams {
                    assert!((s.offset as usize) < len, "{f}: stream past its AST");
                }
            }
            sizes.push(Bank::parse(data).unwrap().len());
            engine.load(p);
        }
        let mut mix = Count(Vec::new(), 0);
        engine.fire(&[0x28, 500], &mut mix);
        for e in [
            0x36, 0x38, 0x39, 0x3a, 0x3c, 0x3d, 0x3e, 0x40, 0x43, 0x44, 0x45, 0x48, 0x49, 0x4d,
            0x50,
        ] {
            engine.fire(&[e, 0, 300, 1000, 1], &mut mix);
            for _ in 0..30 {
                engine.tick(&mut mix);
            }
        }
        assert!(mix.0.len() > 15, "{court:?}: {} voices", mix.0.len());
        assert!(
            mix.0.iter().any(|s| matches!(s, Sound::Stream { .. })),
            "{court:?}: the crowd streams"
        );
        let largest = sizes.iter().max().copied().unwrap_or(0);
        for s in &mix.0 {
            if let Sound::Bank(i) = s {
                assert!(usize::from(*i) < largest, "{court:?}: bank sound {i}");
            }
        }
    }
}

/// The players' lines land on the streams the game's own table gives
/// (`0x1af230`; a few entries checked) and nearly all of them are placed.
#[test]
fn the_player_lines_find_their_streams() {
    use street_audio::speech::{PlayerSpeech, player_voice};
    let Some(mut iso) = disc() else { return };
    let start = std::time::Instant::now();
    let speech = PlayerSpeech::load(&mut iso).unwrap();
    let player_ast = iso.read(street_audio::speech::PLAYER_STREAMS).unwrap();
    let streams = schl::streams(&player_ast).unwrap();
    let program =
        street_audio::aems::Program::parse(&iso.read(street_audio::speech::PLAYER_BANK).unwrap())
            .unwrap();
    let play = &program
        .modules
        .iter()
        .find(|m| m.id == 0x1f6e)
        .unwrap()
        .nodes[27];
    assert_eq!(play.table.len(), 3971);
    assert_eq!(play.table[2173] & 0x3fff, 2139);
    assert_eq!(play.table[2174] & 0x3fff, 2138);
    let resolved = |selector: u16| {
        let sound = play.table[usize::from(selector)] as u16;
        assert_eq!(sound >> 14, 3);
        let entry = &program.streams[usize::from(sound & 0x3fff)];
        let stream = &streams[usize::from(sound & 0x3fff)];
        assert_eq!(entry.offset as usize, stream.start);
        stream
    };
    assert_eq!(streams.len(), 3969, "player AST stream metadata");
    eprintln!("placed in {:.2}s", start.elapsed().as_secs_f32());
    assert_eq!(speech.voices.len(), 22);
    for &voice in speech.voices.keys() {
        assert_eq!(speech.history_limit(voice), 5, "voice {voice} history ring");
    }
    assert_eq!(speech.history_key(1), Some(0));
    assert_eq!(speech.history_key(8), Some(7));
    assert_eq!(speech.history_key(145), Some(16));
    assert_eq!(speech.history_key(151), Some(21));
    assert_eq!(speech.shot_history_key(1), Some(22));
    assert_eq!(speech.shot_history_key(151), Some(43));
    for &voice in speech.shot_voices.keys() {
        assert_eq!(speech.shot_history_limit(voice), 4);
    }
    assert_eq!(speech.shot_voices.len(), 22);
    assert_eq!(speech.voices.values().map(Vec::len).sum::<usize>(), 3196);
    assert_eq!(
        speech.shot_voices.values().map(Vec::len).sum::<usize>(),
        773
    );
    let all: Vec<_> = speech
        .voices
        .values()
        .chain(speech.shot_voices.values())
        .flatten()
        .collect();
    let placed: Vec<u16> = all.iter().filter_map(|l| l.stream).collect();
    eprintln!("{} of {} lines placed", placed.len(), all.len());
    assert_eq!(
        placed.len(),
        all.len(),
        "the game's lookup resolves every line"
    );
    for line in &all {
        let stream = resolved(line.stream.unwrap());
        assert!(
            stream.patch.samples > 0,
            "empty stream metadata for {line:?}"
        );
        assert!(stream.patch.rate > 0 && stream.patch.channels > 0);
        assert!(stream.start < stream.blocks && stream.blocks < stream.end);
        assert!(stream.end <= player_ast.len());
    }
    for lines in speech.voices.values().chain(speech.shot_voices.values()) {
        let stream = resolved(lines[0].stream.unwrap());
        let clip = stream.decode(&player_ast);
        assert_eq!(
            clip.frames(),
            stream.patch.samples as usize,
            "decoded frame count matches the SCHl metadata"
        );
    }
    for category in [0u8, 1, 2, 3, 4, 5, 7, 8, 9, 10, 14] {
        let total = all.iter().filter(|l| l.category == category).count();
        let hit = all
            .iter()
            .filter(|l| l.category == category && l.stream.is_some())
            .count();
        eprintln!("category {category}: {hit}/{total} placed");
    }
    let mut seen = placed.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), placed.len(), "no selector slot used twice");
    assert_eq!(
        seen,
        (0..3969).collect::<Vec<u16>>(),
        "all native lookup selector values placed"
    );
    assert_eq!(speech.shot_voices[&1][0].stream, Some(3239));
    // From the game's table: voice 1's first lines, voice 8's first.
    assert_eq!(speech.voices[&1][0].stream, Some(3258));
    assert_eq!(speech.voices[&1][1].stream, Some(3259));
    assert_eq!(speech.voices[&8][0].stream, Some(3586));
    assert_eq!(
        (speech.voices[&1][0].start, speech.voices[&1][1].start),
        (0, 0x39)
    );
    // Voices: a legend keeps their own; two tone-5 players differ.
    assert_eq!(
        player_voice(&[0, 0, 0, 0, 0, 0, 0, 146], &[]),
        Some((146, 146))
    );
    let mut tone5 = vec![0i16; 40];
    tone5[7] = 5;
    let a = player_voice(&tone5, &[]).unwrap();
    let b = player_voice(&tone5, &[a.0]).unwrap();
    assert_ne!(a.1, b.1);
}

#[test]
fn native_player_event_rules_select_action_and_launch_lines_on_disc() {
    use street_audio::speech::{PlayerCatalog, PlayerRequest, PlayerSelectorRandom, PlayerSpeech};
    let Some(mut iso) = disc() else { return };
    let event = iso.read("/SPEECH/PLAYERS.EVT").unwrap();
    assert_eq!(event.len(), 124);
    assert_eq!(&event[0x1c..0x1e], &0x2000u16.to_le_bytes());
    assert_eq!(&event[0x28..0x2c], &[4, 0, 14, 0]);
    for (branch, rule, header_type, selectors, metadata_mask) in [
        (0x2c, 0x38, 0x2002u16, [2, 4], 7u32),
        (0x54, 0x60, 0x2003u16, [2, 3], 0x3fu32),
    ] {
        assert_eq!(&event[branch..branch + 2], &[0x39, 100]);
        assert_eq!(event[branch + 5], 0, "mode-zero line chooser");
        assert_eq!(&event[rule..rule + 2], &header_type.to_le_bytes());
        assert_eq!(&event[rule + 8..rule + 10], &selectors);
        assert_eq!(&event[rule + 0x14..rule + 0x18], &0x7fffu32.to_le_bytes());
        assert_eq!(
            &event[rule + 0x18..rule + 0x1c],
            &metadata_mask.to_le_bytes()
        );
    }
    let speech = PlayerSpeech::load(&mut iso).unwrap();
    let mut random = PlayerSelectorRandom::new(0);
    for &voice in speech.voices.keys() {
        let request = PlayerRequest {
            voice,
            category_mask: 1 << 3,
            shot_mask: 0,
            action_mask: 2,
        };
        let selected = speech.select_request(request, &mut random).unwrap();
        assert_eq!(selected.catalog, PlayerCatalog::Action);
        let line = speech.voices[&voice][selected.ordinal];
        assert_eq!(
            (line.category, line.mood, line.stream),
            (3, 1, Some(selected.stream))
        );

        let request = PlayerRequest {
            voice,
            category_mask: 1 << 14,
            shot_mask: 1,
            action_mask: 2,
        };
        let selected = speech.select_request(request, &mut random).unwrap();
        assert_eq!(selected.catalog, PlayerCatalog::Shot);
        let line = speech.shot_voices[&voice][selected.ordinal];
        assert_eq!(
            (line.category, line.mood, line.stream),
            (14, 0, Some(selected.stream))
        );
    }
}
