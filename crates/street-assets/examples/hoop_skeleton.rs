//! Print every court's hoop skeletons (`<court>bs.o`, `<court>ds.o`) and
//! which bones the hoop meshes' vertices hang from.
//!
//!     cargo run -p street-assets --example hoop_skeleton -- "<path to Vol. 2 iso>" [COURT_ID | --all]

use street_assets::{big::Big, court, eagl, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let iso_path = args
        .next()
        .ok_or("usage: hoop_skeleton <iso> [COURT_ID | --all]")?;
    let which = args.next().unwrap_or_else(|| "RUCKER".into());
    let mut iso = Iso::open(&iso_path)?;
    let ids = if which == "--all" {
        court::court_ids(&mut iso)?
    } else {
        vec![which]
    };
    for id in ids {
        let bytes = iso.read(&format!("/XF/{id}.VIV"))?;
        let big = Big::parse(&bytes)?;
        let lower = id.to_ascii_lowercase();
        for letter in ['b', 'd'] {
            let Ok(data) = big.get(&format!("{lower}{letter}s.o")) else {
                println!("== {id} {letter}: no skeleton");
                continue;
            };
            let s = eagl::parse_skeleton(data)?;
            println!("== {id} {letter}s.o: {} bones", s.bones.len());
            for (k, b) in s.bones.iter().enumerate() {
                println!(
                    "  {k} {:<12} parent {:?} t {:.4?} r {:.4?} s {:.3?} id {}",
                    b.name, b.parent, b.translation, b.rotation, b.scale, b.id
                );
            }
            if let Ok(model) = big.get(&format!("{lower}{letter}.o")) {
                let m = eagl::parse(model)?;
                println!("  microcode {:?}", m.microcode);
                for mesh in &m.meshes {
                    let mut used = std::collections::BTreeMap::<u16, usize>::new();
                    for (j, w) in mesh.joints.iter().zip(&mesh.weights) {
                        for (b, x) in j.iter().zip(w) {
                            if *x > 0.0 {
                                *used.entry(*b).or_default() += 1;
                            }
                        }
                    }
                    let mut lo = [f32::MAX; 3];
                    let mut hi = [f32::MIN; 3];
                    for p in &mesh.positions {
                        for k in 0..3 {
                            lo[k] = lo[k].min(p[k]);
                            hi[k] = hi[k].max(p[k]);
                        }
                    }
                    println!(
                        "  mesh {:?} {:?}: {} verts, bones {:?} {lo:.2?}..{hi:.2?}",
                        mesh.group,
                        mesh.texture,
                        mesh.positions.len(),
                        used
                    );
                }
            }
        }
    }
    Ok(())
}
