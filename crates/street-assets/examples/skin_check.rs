//! Check how a skinned player model's bone references map onto the skeleton.
//!
//!     cargo run -p street-assets --example skin_check -- <iso> [MODEL]
//!
//! Per mesh: the bones its vertices mostly follow, with each bone's rest
//! position next to the centre of the vertices that follow it. A correct
//! mapping puts arm vertices on arm bones and so on.

use std::collections::BTreeMap;

use street_assets::{iso::Iso, player};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut iso = Iso::open(args.next().ok_or("usage: skin_check <iso> [MODEL]")?)?;
    let name = args.next().unwrap_or_else(|| "body0_outfit6".into());
    let parts = player::Parts::load(&mut iso)?;
    let model = parts.model(&mut iso, &name)?;
    let skeleton = player::skeleton(&mut iso)?;

    // Rest position of each bone: the translation of the inverse of its
    // inverse bind matrix, -R^T t.
    let rest: Vec<[f32; 3]> = skeleton
        .bones
        .iter()
        .map(|b| {
            let m = &b.inverse_bind; // column-major
            let t = [m[12], m[13], m[14]];
            std::array::from_fn(|i| -(m[4 * i] * t[0] + m[4 * i + 1] * t[1] + m[4 * i + 2] * t[2]))
        })
        .collect();
    for (i, b) in skeleton.bones.iter().enumerate().take(22) {
        println!(
            "bone {i:2} id {:2} {:12} rest {:5.2?}",
            b.id, b.name, rest[i]
        );
    }

    for mesh in &model.meshes {
        // dominant joint -> (vertex count, position sum)
        let mut by_joint: BTreeMap<u16, (usize, [f32; 3])> = BTreeMap::new();
        for ((p, joints), weights) in mesh.positions.iter().zip(&mesh.joints).zip(&mesh.weights) {
            let k = (0..4)
                .max_by(|&a, &b| weights[a].total_cmp(&weights[b]))
                .unwrap();
            let e = by_joint.entry(joints[k]).or_default();
            e.0 += 1;
            for (i, &v) in p.iter().enumerate() {
                e.1[i] += v;
            }
        }
        println!(
            "{} / {}:",
            mesh.group.as_deref().unwrap_or("-"),
            mesh.texture.as_deref().unwrap_or("-")
        );
        for (j, (n, sum)) in by_joint {
            let c: [f32; 3] = std::array::from_fn(|i| sum[i] / n as f32);
            let bone = skeleton.bones.get(j as usize);
            println!(
                "   joint {j:2} ({:12}) {n:4} verts centred {c:5.2?}, bone rest {:5.2?}",
                bone.map_or("?", |b| b.name.as_str()),
                rest.get(j as usize)
            );
        }
    }
    Ok(())
}
