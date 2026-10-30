//! Decode the disc's audio to WAV files, to listen to them.
//!
//!     cargo run -p street-audio --release --example audio_dump -- <iso> <out dir> [what...]
//!
//! `what` is any of `banks`, `music`, `crowd`, `speech`, `menu` (default: all).
//! The court's banks and crowd are Rucker Park's, or the `XDBCOURT` record
//! `COURT` names.
//! Files are named `<bank>_<index>.wav` and so on. The output is the
//! game's own audio: keep it out of the repository.

use std::{fs, io::Write, path::Path};

use street_assets::iso::Iso;
use street_audio::{Clip, bank::Bank, courts, music, schl, speech::Commentary};

fn wav(path: &Path, clip: &Clip) -> std::io::Result<()> {
    let data_len = clip.samples.len() as u32 * 2;
    let ch = clip.channels as u32;
    let mut f = fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&(ch as u16).to_le_bytes())?;
    f.write_all(&clip.rate.to_le_bytes())?;
    f.write_all(&(clip.rate * ch * 2).to_le_bytes())?;
    f.write_all(&((ch * 2) as u16).to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    let mut bytes = Vec::with_capacity(data_len as usize);
    for s in &clip.samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    f.write_all(&bytes)
}

fn stem(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .split('.')
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(iso_path), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("usage: audio_dump <iso> <out dir> [banks|music|crowd|speech|menu ...]");
        std::process::exit(2);
    };
    let want = |w: &str| args.len() <= 2 || args[2..].iter().any(|a| a == w);
    let court: usize = std::env::var("COURT")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(9);
    let out = Path::new(out);
    fs::create_dir_all(out)?;
    let mut iso = Iso::open(iso_path)?;
    let mut written = 0;

    let mut dump_bank =
        |iso: &mut Iso<_>, path: &str, dir: &str| -> Result<(), Box<dyn std::error::Error>> {
            let bank = Bank::parse(iso.read(path)?)?;
            let dir = out.join(dir);
            fs::create_dir_all(&dir)?;
            for i in 0..bank.len() {
                match &bank.sounds[i] {
                    Some(p) if p.samples >= 200 => {
                        wav(
                            &dir.join(format!("{}_{i:02}.wav", stem(path))),
                            &bank.clip(i)?,
                        )?;
                        written += 1;
                    }
                    _ => {}
                }
            }
            Ok(())
        };

    if want("menu") {
        dump_bank(&mut iso, courts::FRONT_END, "menu")?;
    }
    if want("banks") {
        for path in courts::COMMON {
            dump_bank(&mut iso, path, "game")?;
        }
        // Rucker Park's ambience and crowd calls, as an example court.
        let (ambience, crowd, _) = courts::banks(Some(court));
        dump_bank(&mut iso, ambience, "court")?;
        dump_bank(&mut iso, crowd, "court")?;
    }
    if want("crowd") {
        let (_, _, stream) = courts::banks(Some(court));
        let data = iso.read(stream)?;
        let streams = schl::streams(&data)?;
        let dir = out.join("crowd");
        fs::create_dir_all(&dir)?;
        for (i, s) in streams.iter().enumerate().take(12) {
            let mut clip = s.decode(&data);
            // The bed runs for minutes; keep 30 seconds.
            let keep = (clip.rate * 30) as usize * clip.channels as usize;
            clip.samples.truncate(keep);
            wav(&dir.join(format!("crd_ruck_{i:02}.wav")), &clip)?;
        }
    }
    if want("music") {
        let dir = out.join("music");
        fs::create_dir_all(&dir)?;
        let data = iso.read(music::TITLE_SONG)?;
        let s = schl::streams(&data)?;
        wav(&dir.join("title.wav"), &s[0].decode(&data))?;
        let cfg = String::from_utf8_lossy(&iso.read(music::PATHSONG)?).into_owned();
        if let Some(song) = music::songs(&cfg).first() {
            let data = iso.read(&song.main())?;
            // The first minute of the in-game song, segments in file order.
            let mut all = Clip {
                rate: 36000,
                channels: 2,
                samples: Vec::new(),
                looping: None,
            };
            for seg in schl::streams(&data)? {
                let c = seg.decode(&data);
                all.rate = c.rate;
                all.samples.extend(c.samples);
                if all.samples.len() > 36000 * 2 * 60 {
                    break;
                }
            }
            wav(&dir.join(format!("{}_main.wav", song.folder)), &all)?;
        }
    }
    if want("speech") {
        let c = Commentary::open(&mut iso)?;
        let dir = out.join("speech");
        fs::create_dir_all(&dir)?;
        for prefix in [
            "Intro\\SpecIntrosA",
            "Dunk\\DunkA",
            "Block\\BlockA",
            "Steal\\StealA",
            "Shot\\ShotA",
            "GamePoint",
            "Outro\\OutroA",
        ] {
            let Some(entry) = c.files(prefix).next().cloned() else {
                continue;
            };
            let lines = c.lines(&mut iso, &entry)?;
            for i in 0..lines.len().min(5) {
                let name = format!("{}_{i}.wav", stem(&entry.name.replace('\\', "/")));
                wav(&dir.join(name), &lines.clip(i).unwrap())?;
            }
        }
    }
    println!(
        "wrote {written} bank sounds and the requested extras to {}",
        out.display()
    );
    Ok(())
}
