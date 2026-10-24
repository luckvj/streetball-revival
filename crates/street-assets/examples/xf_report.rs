//! Report entries of a BIG archive (default the players', `/XF/XF000002`):
//! each model's meshes (group, texture, vertices, the bones they are skinned
//! to, bounds), each skeleton's bones and each sheet's images. Without entries, lists the archive.
//!
//!     cargo run -p street-assets --example xf_report -- <iso> [--archive /XF/XF000004] [entry ...]

use street_assets::{big::Big, eagl, iso::Iso, ssh};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).peekable();
    let mut iso = Iso::open(
        args.next()
            .ok_or("usage: xf_report <iso> [--archive <path>] [entry ...]")?,
    )?;
    let mut archive = "/XF/XF000002".to_string();
    if args.peek().is_some_and(|a| a == "--archive") {
        args.next();
        archive = args.next().ok_or("--archive needs a path")?;
    }
    let bytes = iso.read(&archive)?;
    let big = Big::parse(&bytes)?;
    let entries = big.entries();
    let wanted: Vec<usize> = args.map(|a| a.parse()).collect::<Result<_, _>>()?;
    if wanted.is_empty() {
        for (i, e) in entries.iter().enumerate() {
            println!("{i:5} {:24} {:8} bytes", e.name, big.bytes(e).len());
        }
        return Ok(());
    }
    for i in wanted {
        let e = entries.get(i).ok_or("no such entry")?;
        let bytes = big.bytes(e);
        println!("entry {i}: {} ({} bytes)", e.name, bytes.len());
        if let Ok(skeleton) = eagl::parse_skeleton(bytes) {
            let names: Vec<&str> = skeleton.bones.iter().map(|b| b.name.as_str()).collect();
            println!("  skeleton: {}", names.join(" "));
        } else if let Ok(m) = eagl::parse(bytes) {
            println!(
                "  model {:?}, half extent {:?}, bounds {:?}",
                m.name,
                m.half_extent,
                m.bounds()
            );
            for mesh in &m.meshes {
                let bones: std::collections::BTreeSet<u16> = mesh
                    .joints
                    .iter()
                    .zip(&mesh.weights)
                    .flat_map(|(j, w)| (0..4).filter(|&k| w[k] > 0.0).map(move |k| j[k]))
                    .collect();
                println!(
                    "  mesh group {:?} texture {:?}: {} vertices, {} triangles, bones {:?}",
                    mesh.group,
                    mesh.texture,
                    mesh.positions.len(),
                    mesh.indices.len() / 3,
                    bones
                );
            }
        } else if let Ok(sheet) = ssh::parse(bytes) {
            for t in &sheet.textures {
                // Each channel's mean, 0..255.
                let n = (t.rgba.len() / 4).max(1) as u64;
                let mean: Vec<u64> = (0..4)
                    .map(|c| {
                        t.rgba
                            .iter()
                            .skip(c)
                            .step_by(4)
                            .map(|&v| u64::from(v))
                            .sum::<u64>()
                            / n
                    })
                    .collect();
                println!(
                    "  image {:?} {}x{}, mean rgba {mean:?}",
                    t.name, t.width, t.height
                );
            }
        } else {
            let head: Vec<String> = bytes.iter().take(48).map(|b| format!("{b:02x}")).collect();
            println!("  neither a model nor a sheet: {}", head.join(" "));
        }
    }
    Ok(())
}
