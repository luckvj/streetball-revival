//! Print the bounds of every draw group of a court's models.
//!
//!     cargo run -p street-assets --example group_report -- <iso> [COURT_ID]

use std::collections::BTreeMap;

use street_assets::{court, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut iso = Iso::open(args.next().ok_or("usage: group_report <iso> [COURT_ID]")?)?;
    let id = args.next().unwrap_or_else(|| "RUCKER".into());
    let c = court::load(&mut iso, &id)?;
    for m in &c.models {
        let mut groups: BTreeMap<&str, ([f32; 3], [f32; 3], usize)> = BTreeMap::new();
        for mesh in &m.meshes {
            let g = groups
                .entry(mesh.group.as_deref().unwrap_or("-"))
                .or_insert(([f32::MAX; 3], [f32::MIN; 3], 0));
            for p in &mesh.positions {
                for (i, &v) in p.iter().enumerate() {
                    g.0[i] = g.0[i].min(v);
                    g.1[i] = g.1[i].max(v);
                }
            }
            g.2 += mesh.indices.len() / 3;
        }
        for (name, (lo, hi, tris)) in groups {
            println!(
                "{:10} {name:32} {tris:6} tris  {lo:7.1?} .. {hi:7.1?}",
                m.name
            );
        }
    }
    Ok(())
}
