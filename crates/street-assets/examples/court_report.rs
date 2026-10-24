//! Decode a court straight from the disc and report what was and was not understood.
//!
//!     cargo run -p street-assets --example court_report -- "<path to Vol. 2 iso>" [COURT_ID | --all] [TEXTURE]
//!
//! With TEXTURE, also prints the bounds of every mesh that uses that texture.

use street_assets::{court, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let iso_path = args
        .next()
        .ok_or("usage: court_report <iso> [COURT_ID | --all]")?;
    let which = args.next().unwrap_or_else(|| "RUCKER".into());
    let texture = args.next();
    let mut iso = Iso::open(&iso_path)?;
    let ids = if which == "--all" {
        court::court_ids(&mut iso)?
    } else {
        vec![which]
    };
    for id in ids {
        let c = court::load(&mut iso, &id)?;
        let (mut chains, mut batches, mut skipped, mut tris) = (0, 0, 0, 0);
        println!(
            "== {} : {} models, {} textures",
            c.id,
            c.models.len(),
            c.textures.len()
        );
        for m in &c.models {
            let s = m.stats;
            chains += s.chains;
            batches += s.batches;
            skipped += s.skipped_batches;
            tris += s.triangles;
            let bounds = m
                .bounds()
                .map(|(lo, hi)| format!("{lo:.1?}..{hi:.1?}"))
                .unwrap_or_else(|| "-".into());
            println!(
                "   {:10} meshes={:3} chains={:4} batches={:4} skipped={:4} tris={:6}  {bounds}",
                m.name,
                m.meshes.len(),
                s.chains,
                s.batches,
                s.skipped_batches,
                s.triangles
            );
        }
        println!("   total: chains={chains} batches={batches} skipped={skipped} triangles={tris}");
        for m in &c.models {
            for mesh in m
                .meshes
                .iter()
                .filter(|mesh| texture.is_some() && mesh.texture == texture)
            {
                let (lo, hi) =
                    mesh.positions
                        .iter()
                        .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
                            (
                                std::array::from_fn(|i| lo[i].min(p[i])),
                                std::array::from_fn(|i| hi[i].max(p[i])),
                            )
                        });
                println!(
                    "   {} uses {:?}: {} tris, {lo:.1?}..{hi:.1?}",
                    m.name,
                    mesh.texture,
                    mesh.indices.len() / 3
                );
            }
        }
        println!("   missing textures: {:?}", c.missing_textures());
        if !c.skipped_textures.is_empty() {
            println!("   undecoded texture formats: {:?}", c.skipped_textures);
        }
    }
    Ok(())
}
