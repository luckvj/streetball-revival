//! Checks against a real USA disc image of the original game image.
//! Skipped unless STREET_ISO points at the image, since the repo ships no game data.

use street_assets::{court, iso::Iso};

fn disc() -> Option<Iso<std::io::BufReader<std::fs::File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

#[test]
fn every_court_decodes_with_all_textures_present() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let ids = court::court_ids(&mut iso).unwrap();
    // 11 courts plus GREENC, a small floor-and-backdrop scene (likely menus).
    assert_eq!(ids.len(), 12, "court archives: {ids:?}");
    for id in ids {
        let c = court::load(&mut iso, &id).unwrap();
        let tris: usize = c.models.iter().map(|m| m.stats.triangles).sum();
        let expected = if id == "GREENC" { 900 } else { 10_000 };
        assert!(tris > expected, "{id}: only {tris} triangles");
        // INDOOR's `botl` slot names an image that exists nowhere on the disc.
        let known_missing: &[&str] = if id == "INDOOR" { &["botl"] } else { &[] };
        assert_eq!(
            c.missing_textures(),
            known_missing,
            "{id}: missing textures"
        );
        assert!(
            c.skipped_textures.is_empty(),
            "{id}: undecoded textures {:?}",
            c.skipped_textures
        );
        for m in &c.models {
            let name = &m.name;
            assert_eq!(m.stats.skipped_batches, 0, "{name}: undecoded batches");
            assert!(
                m.meshes.iter().all(|mesh| mesh.group.is_some()),
                "{name}: mesh outside every draw group"
            );
            // The exporter's bounding box covers exactly the decoded vertices,
            // which checks the skinned dequantisation and the static positions alike.
            let ((lo, hi), (blo, bhi)) = (m.bounds().unwrap(), m.bbox.unwrap());
            let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 0.01);
            assert!(
                close(lo, blo) && close(hi, bhi),
                "{name}: bounds {lo:?}..{hi:?}, __BBOX {blo:?}..{bhi:?}"
            );
        }
        if id != "GREENC" {
            let prefix = id.to_ascii_lowercase();
            for hoop in ["b", "d"] {
                assert!(
                    c.placements.contains_key(&format!("{prefix}{hoop}")),
                    "{id}: hoop {hoop} not placed"
                );
            }
        }
    }
}

#[test]
fn rucker_floor_cracks_are_three_strips_per_batch() {
    let Some(mut iso) = disc() else { return };
    let c = court::load(&mut iso, "RUCKER").unwrap();
    let lines = c
        .models
        .iter()
        .find(|m| m.name == "ruckerl")
        .expect("ruckerl model");
    let cracks = lines
        .meshes
        .iter()
        .find(|m| m.texture.as_deref() == Some("crck"))
        .expect("crck mesh");
    // 30 vertices in three 10-vertex strips: 3 x 8 triangles, none bridging strips.
    assert_eq!(cracks.positions.len(), 30);
    assert_eq!(cracks.indices.len() / 3, 24);
}
