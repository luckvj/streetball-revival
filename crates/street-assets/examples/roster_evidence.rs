//! The evidence behind `street_assets::roster_rules`: the ranges of the
//! players' sizes, ratings and fields on the disc and in a roster mod.
//!
//!     cargo run -p street-assets --example roster_evidence -- <iso> [<roster mod dir>]

use std::collections::BTreeMap;

use street_assets::{
    db::{self, Player},
    iso::Iso,
};

fn kind(r: usize) -> &'static str {
    match r {
        0..145 => "league",
        145..158 => "street",
        158..184 => "legend",
        184..234 => "generic",
        _ => "created",
    }
}

fn report(label: &str, players: &[Player]) {
    println!("== {label}: {} records", players.len());
    let mut by_kind: BTreeMap<&str, Vec<&Player>> = BTreeMap::new();
    for p in players {
        by_kind.entry(kind(p.index)).or_default().push(p);
    }
    for (k, ps) in &by_kind {
        let ext = |f: &dyn Fn(&Player) -> i32| {
            let lo = ps.iter().min_by_key(|p| f(p)).unwrap();
            let hi = ps.iter().max_by_key(|p| f(p)).unwrap();
            format!(
                "{} ({} #{}) .. {} ({} #{})",
                f(lo),
                lo.name(),
                lo.index,
                f(hi),
                hi.name(),
                hi.index
            )
        };
        println!("  [{k}] {} records", ps.len());
        println!("    height  {}", ext(&|p| i32::from(p.height_in)));
        println!("    weight  {}", ext(&|p| i32::from(p.weight_lb)));
        println!(
            "    lb/in   {}",
            ext(&|p| i32::from(p.weight_lb) * 100 / i32::from(p.height_in.max(1)))
        );
        println!(
            "    mdl-ht  {}",
            ext(&|p| i32::from(p.model_height_in) - i32::from(p.height_in))
        );
        println!(
            "    mdl-wt  {}",
            ext(&|p| i32::from(p.model_weight_lb) - i32::from(p.weight_lb))
        );
        for (a, name) in ["x", "y", "z"].iter().enumerate() {
            println!("    scale{name} {}", ext(&|p| i32::from(p.body_scale[a])));
        }
        for (r, name) in db::RATING_NAMES.iter().enumerate() {
            println!("    {name:9} {}", ext(&|p| i32::from(p.ratings[r])));
        }
        println!(
            "    total9  {}",
            ext(&|p| p.ratings[..9].iter().map(|&v| i32::from(v)).sum())
        );
        println!(
            "    total11 {}",
            ext(&|p| p.ratings.iter().map(|&v| i32::from(v)).sum())
        );
        for f in [0usize, 6, 15, 27, 28, 30, 31, 33, 34, 35, 37] {
            let mut values: Vec<i16> = ps.iter().map(|p| p.fields[f]).collect();
            values.sort();
            values.dedup();
            println!("    field{f:<3} {values:?}");
        }
    }
    let mut totals: Vec<(i32, String, usize)> = players
        .iter()
        .map(|p| {
            (
                p.ratings[..9].iter().map(|&v| i32::from(v)).sum(),
                p.name(),
                p.index,
            )
        })
        .collect();
    totals.sort();
    println!(
        "  top totals (ratings 0..8): {:?}",
        &totals[totals.len() - 6..]
    );
    let mut ratios: Vec<(i32, String, i16, i16)> = players
        .iter()
        .filter(|p| p.index < 234)
        .map(|p| {
            (
                i32::from(p.weight_lb) * 100 / i32::from(p.height_in.max(1)),
                p.name(),
                p.height_in,
                p.weight_lb,
            )
        })
        .collect();
    ratios.sort();
    println!("  lightest for height: {:?}", &ratios[..4]);
    println!("  heaviest for height: {:?}", &ratios[ratios.len() - 4..]);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut iso = Iso::open(args.next().ok_or("usage: roster_evidence <iso> [<mod>]")?)?;
    let disc = db::original_players(&mut iso)?;
    report("disc", &disc);
    let moves = db::moves(&mut iso)?;
    let mut kinds: BTreeMap<(i16, i16), usize> = BTreeMap::new();
    for m in &moves {
        *kinds.entry((m.kind, m.level)).or_default() += 1;
    }
    println!("moves (kind, level) -> count: {kinds:?}");
    if let Some(dir) = args.next() {
        let dir = db::find_roster_dir(dir.as_ref()).ok_or("no roster there")?;
        db::set_roster_override(Some(&dir))?;
        let modded = db::players(&mut iso)?;
        report("mod", &modded);
        let mut changed: BTreeMap<usize, usize> = BTreeMap::new();
        for (a, b) in modded.iter().zip(&disc) {
            for f in 0..a.fields.len() {
                if a.fields[f] != b.fields[f] {
                    *changed.entry(f).or_default() += 1;
                }
            }
        }
        println!("fields the mod changes (field -> records): {changed:?}");
        let teams = db::teams(&mut iso)?;
        let base = db::original_teams(&mut iso)?;
        let diff = teams.iter().zip(&base).filter(|(a, b)| a != b).count();
        println!("teams the mod changes: {diff} of {}", teams.len());
    }
    Ok(())
}
