//! Decode player models from the disc and report what was understood.
//!
//!     cargo run -p street-assets --example player_report -- <iso> [MODEL ...]
//!
//! Without model names, lists how many body and head models there are.

use street_assets::{iso::Iso, player};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut iso = Iso::open(
        args.next()
            .ok_or("usage: player_report <iso> [MODEL ...]")?,
    )?;
    let parts = player::Parts::load(&mut iso)?;
    let names: Vec<String> = args.collect();
    if names.is_empty() {
        let all: Vec<&str> = parts.model_names().collect();
        let bodies = all.iter().filter(|n| n.starts_with("body")).count();
        println!(
            "{} models: {bodies} bodies, {} heads",
            all.len(),
            all.len() - bodies
        );
        println!("players with textures: {}", player::player_count(&mut iso)?);
        return Ok(());
    }
    for name in names {
        let m = parts.model(&mut iso, &name)?;
        let s = m.stats;
        println!(
            "{name}: microcode {:?}, {} meshes, {} batches ({} skipped), {} triangles",
            m.microcode,
            m.meshes.len(),
            s.batches,
            s.skipped_batches,
            s.triangles
        );
        println!("   bounds {:.2?}\n   __BBOX {:.2?}", m.bounds(), m.bbox);
        for mesh in &m.meshes {
            println!(
                "   {:10} {:5} {:4} tris, {} skinned vertices",
                mesh.group.as_deref().unwrap_or("-"),
                mesh.texture.as_deref().unwrap_or("-"),
                mesh.indices.len() / 3,
                mesh.joints.len()
            );
        }
    }
    Ok(())
}
