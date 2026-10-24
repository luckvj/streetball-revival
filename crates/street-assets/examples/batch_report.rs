//! Batch sizes of one model in a BIG archive entry, for matching morph targets.
//!
//!     cargo run -p street-assets --example batch_report -- <archive> <entry> [first] [last]
//!
//! For example `/XF/XF000002 931 150 181` lists the head batches of `Small1Body`.

use street_assets::{big, eagl, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [archive, entry, rest @ ..] = &args[..] else {
        return Err("usage: batch_report <archive> <entry> [first] [last]".into());
    };
    let iso_path = std::env::var("STREET_ISO").map_err(|_| "set STREET_ISO")?;
    let mut iso = Iso::open(&iso_path)?;
    let index = big::read_index(&mut iso, archive)?;
    let bytes = big::read_entry(&mut iso, archive, &index[entry.parse::<usize>()?])?;
    let model = eagl::parse(&bytes)?;
    let first: usize = rest.first().map_or(Ok(0), |s| s.parse())?;
    let last: usize = rest.get(1).map_or(Ok(usize::MAX), |s| s.parse())?;
    println!("{}: {} batches", model.name, model.batch_sizes.len());
    for (b, &n) in model.batch_sizes.iter().enumerate().skip(first) {
        if b > last {
            break;
        }
        let mesh = model
            .meshes
            .iter()
            .find(|m| m.sources.iter().any(|s| s.0 == b as u32));
        println!(
            "  batch {b:4}: {n:3} inputs  group {:?} texture {:?}",
            mesh.and_then(|m| m.group.as_deref()),
            mesh.and_then(|m| m.texture.as_deref())
        );
    }
    for m in &model.meshes {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for uv in &m.uvs {
            for k in 0..2 {
                lo[k] = lo[k].min(uv[k]);
                hi[k] = hi[k].max(uv[k]);
            }
        }
        let lengths = m
            .normals
            .iter()
            .map(|n| (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt());
        let (nlo, nhi) = lengths.fold((f32::MAX, f32::MIN), |(a, b), l| (a.min(l), b.max(l)));
        println!(
            "  mesh {:?}/{:?}: {} vertices, uv {lo:.3?}..{hi:.3?}, {} normals of length {nlo:.3}..{nhi:.3}",
            m.group.as_deref().unwrap_or("-"),
            m.texture.as_deref().unwrap_or("-"),
            m.positions.len(),
            m.normals.len()
        );
    }
    Ok(())
}
