//! Decode the models in the shared archive (`/XF/XF000003`: balls, skybox,
//! indicators) and report them.
//!
//!     cargo run -p street-assets --example shared_report -- <iso>

use street_assets::{big::Big, eagl, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut iso = Iso::open(args.next().ok_or("usage: shared_report <iso>")?)?;
    let bytes = iso.read("/XF/XF000003")?;
    let big = Big::parse(&bytes)?;
    for e in big.entries().iter().filter(|e| e.name.ends_with(".o")) {
        let data = big.bytes(e);
        match eagl::parse(data) {
            Ok(m) if m.name.is_empty() => match eagl::parse_skeleton(data) {
                Ok(s) => println!(
                    "{:10} skeleton: {:?}",
                    e.name,
                    s.bones.iter().map(|b| b.name.as_str()).collect::<Vec<_>>()
                ),
                Err(err) => println!("{:10} not a model or skeleton: {err}", e.name),
            },
            Ok(m) => {
                println!(
                    "{:10} {:12} microcode {:?}, {} tris ({} of {} batches skipped), bounds {:.2?}",
                    e.name,
                    m.name,
                    m.microcode,
                    m.stats.triangles,
                    m.stats.skipped_batches,
                    m.stats.batches,
                    m.bounds()
                );
                for mesh in &m.meshes {
                    println!(
                        "      {:14} {:6} {} tris, skinned {}",
                        mesh.group.as_deref().unwrap_or("-"),
                        mesh.texture.as_deref().unwrap_or("-"),
                        mesh.indices.len() / 3,
                        !mesh.joints.is_empty()
                    );
                }
            }
            Err(err) => println!("{:10} error: {err}", e.name),
        }
    }
    Ok(())
}
