//! Lists the original's 108 runtime gait records with their animation
//! names: `cargo run -p street-assets --example gait_report -- <iso>`.
use street_assets::{gait::GaitData, iso::Iso, timeline::ScriptBank};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        return Err("usage: gait_report <iso>".into());
    };
    let mut iso = Iso::open(path)?;
    let gait = GaitData::read(&mut iso)?;
    let bank = ScriptBank::read(&mut iso)?;
    let name = |row: u16| {
        bank.animations
            .get(usize::from(row))
            .and_then(|a| a.name.clone())
            .unwrap_or_else(|| "-".into())
    };
    for (i, r) in gait.records.iter().enumerate() {
        println!(
            "{i:3} {:06x} rows {:4} {:4} cat {:x} player {:4} handles {:3} root {:.6} motion {:.4}..{:.4} flags {:5x}  {} | {}",
            r.script,
            r.rows[0],
            r.rows[1],
            r.category_mask,
            r.player_restriction,
            r.minimum_handles,
            r.root_distance,
            r.minimum_motion,
            r.maximum_motion,
            r.flags,
            name(r.rows[0]),
            name(r.rows[1]),
        );
    }
    Ok(())
}
