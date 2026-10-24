//! Pairs a Dolphin texture pack for the GameCube Vol. 2 (`GNZE69`) with the
//! PS2 images street-rs draws, and writes the table `street_assets::hires`
//! reads (`crates/street-assets/data/hires_gnze69.tsv`).
//!
//!     cargo run -p street-assets --example hires_map -- <gc disc> <ps2 iso>
//!         [--pack <Textures/GNZE69>] [--out <table.tsv>]
//!
//! With `--pack` only that pack's images are paired and the report says
//! which of them have no PS2 place; without it every GameCube image is.
//! The table holds hashes and indices only, no pixels.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use street_assets::{gcdisc::GcDisc, hires, iso::Iso};

fn pngs(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            pngs(&p, out)?;
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")) {
            out.push(p);
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: hires_map <gc disc> <ps2 iso> [--pack <dir>] [--out <table.tsv>]";
    let (Some(gc), Some(ps2)) = (args.first(), args.get(1)) else {
        return Err(usage.into());
    };
    let (mut pack, mut out) = (None, None);
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--pack" => pack = it.next().map(PathBuf::from),
            "--out" => out = it.next().map(PathBuf::from),
            _ => return Err(usage.into()),
        }
    }
    let mut disc = GcDisc::open(gc)?;
    if disc.game_id() != "GNZE69" {
        eprintln!("warning: {} is {}, not GNZE69", gc, disc.game_id());
    }
    let mut iso = Iso::open(ps2)?;

    // The pack's files, by key (a key can sit in more than one folder).
    let mut files = Vec::new();
    if let Some(dir) = &pack {
        pngs(dir, &mut files)?;
    }
    let keyed: Vec<(hires::Key, &PathBuf)> = files
        .iter()
        .filter_map(|p| Some((hires::Key::parse(&p.file_name()?.to_string_lossy())?, p)))
        .collect();
    let wanted: HashSet<hires::Key> = keyed.iter().map(|(k, _)| *k).collect();

    let (mut rows, unpaired) =
        hires::build_table(&mut disc, &mut iso, pack.as_ref().map(|_| &wanted))?;
    rows.sort_by(|a, b| (&a.ps2, a.key).cmp(&(&b.ps2, b.key)));

    // One PS2 image should have one replacement; report any that has two.
    let mut by_hash: BTreeMap<(u32, u32, u64), HashSet<hires::Key>> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.use_ == hires::Use::Image) {
        by_hash
            .entry((r.width, r.height, r.hash))
            .or_default()
            .insert(r.key);
    }
    let clashes = by_hash.values().filter(|k| k.len() > 1).count();
    let paired: HashSet<hires::Key> = rows.iter().map(|r| r.key).collect();
    let worst = rows.iter().map(|r| r.score).fold(0.0f32, f32::max);
    println!(
        "{} rows ({} PS2 images, {} keys), worst score {worst:.3}, {clashes} PS2 images with two replacements",
        rows.len(),
        by_hash.len(),
        paired.len()
    );
    if let Some(root) = &pack {
        println!(
            "pack: {} PNGs, {} keys; {} keys paired, {} not",
            files.len(),
            wanted.len(),
            wanted.intersection(&paired).count(),
            wanted.difference(&paired).count()
        );
        // Per top-level folder: PNGs paired / all.
        let mut folders: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for (k, p) in &keyed {
            let folder = p
                .strip_prefix(root)
                .ok()
                .and_then(|r| r.components().next())
                .filter(|_| p.parent() != Some(root.as_path()))
                .map_or("(top)".into(), |c| {
                    c.as_os_str().to_string_lossy().into_owned()
                });
            let f = folders.entry(folder).or_default();
            f.0 += paired.contains(k) as usize;
            f.1 += 1;
        }
        for (f, (n, all)) in folders {
            println!("  {f}: {n}/{all}");
        }
        for (k, place, why) in &unpaired.images {
            let name = keyed
                .iter()
                .find(|(kk, _)| kk == k)
                .and_then(|(_, p)| p.strip_prefix(root).ok())
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            println!("  unpaired {name}: GameCube {place} ({why})");
        }
    } else {
        let why: BTreeMap<String, usize> =
            unpaired
                .images
                .iter()
                .fold(BTreeMap::new(), |mut m, (_, _, w)| {
                    let w = w.split(" (").next().unwrap_or(w);
                    let w = if w.starts_with("PS2 image") {
                        "PS2 image differs"
                    } else {
                        w
                    };
                    *m.entry(w.to_owned()).or_insert(0) += 1;
                    m
                });
        println!("unpaired GameCube images: {why:?}");
    }
    if let Some(out) = out {
        std::fs::write(&out, hires::write_table(&rows))?;
        println!("wrote {}", out.display());
    }
    Ok(())
}
