//! Checks against the user's own Street V3 GameCube disc (`G3VE69`).
//! Skipped unless STREET_V3_GC points at the image (`.rvz`, `.iso`, `.ciso`, ...).

use street_assets::{eagl_gc, gcdisc::GcDisc, gsh};

fn disc() -> Option<GcDisc> {
    let path = std::env::var_os("STREET_V3_GC")?;
    Some(GcDisc::open(path).expect("STREET_V3_GC is not a readable GameCube disc"))
}

#[test]
fn v3_gamecube_disc_has_the_player_archive() {
    let Some(mut disc) = disc() else {
        eprintln!("STREET_V3_GC not set; skipping");
        return;
    };
    assert_eq!(disc.game_id(), "G3VE69");
    let index = disc.big_index("/xf/player.viv").unwrap();
    assert!(index.len() > 7000, "{} entries", index.len());
    for name in [
        "luigi.o",
        "peach.o",
        "bigmh.o",
        "mhands.o",
        "head_m_luigi_00_face.gsh",
    ] {
        assert!(
            index.iter().any(|e| e.name.eq_ignore_ascii_case(name)),
            "{name} missing"
        );
    }
}

#[test]
fn nintendo_all_stars_decode_completely() {
    let Some(mut disc) = disc() else {
        eprintln!("STREET_V3_GC not set; skipping");
        return;
    };
    let index = disc.big_index("/xf/player.viv").unwrap();
    let mut entry = |name: &str| {
        let e = index
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("{name} missing"))
            .clone();
        disc.big_entry("/xf/player.viv", &e).unwrap()
    };
    // (model, batches, the texture slots it binds)
    for (name, batches, slots) in [
        (
            "luigi.o",
            16,
            &["arms", "bott", "eyes", "hair", "head", "shoe", "tops"][..],
        ),
        (
            "bigmh.o",
            18,
            &["arms", "bott", "eyes", "hair", "head", "shoe", "tops"][..],
        ),
        ("peach.o", 24, &["bott", "eyes", "hair", "head", "tops"][..]),
    ] {
        let model = eagl_gc::parse(&entry(name)).unwrap();
        assert_eq!(model.stats.batches, batches, "{name}: {:?}", model.stats);
        assert_eq!(model.stats.skipped_batches, 0, "{name}");
        for slot in slots {
            assert!(
                model
                    .meshes
                    .iter()
                    .any(|m| m.texture.as_deref() == Some(*slot)),
                "{name} binds no {slot}"
            );
        }
        // Rest pose in model space: about 4 to 6 units tall, feet near the bottom.
        let (lo, hi) = model.bounds().unwrap();
        let height = hi[1] - lo[1];
        assert!((3.0..7.0).contains(&height), "{name} is {height} tall");
        let (blo, bhi) = model.bbox.unwrap();
        for k in 0..3 {
            assert!(
                (lo[k] - blo[k]).abs() < 0.05 && (hi[k] - bhi[k]).abs() < 0.05,
                "{name} bounds differ from __BBOX"
            );
        }
        for m in &model.meshes {
            assert!(
                m.uvs
                    .iter()
                    .all(|uv| uv.iter().all(|c| (-0.1..1.1).contains(c))),
                "{name} UV out of range"
            );
            assert!(
                m.normals
                    .iter()
                    .all(|n| (n[0] * n[0] + n[1] * n[1] + n[2] * n[2] - 1.0).abs() < 0.05)
            );
        }
    }
    // Their sheets decode in all three pixel formats.
    for (name, w, h) in [
        ("head_m_luigi_00_face.gsh", 128, 128),  // CMPR
        ("eyes_m_luigi_00_eyeball.gsh", 64, 64), // RGBA8
        ("tops_m_luigi_00_pants.gsh", 128, 128), // C8 + palette
    ] {
        let sheet = gsh::parse(&entry(name)).unwrap();
        assert!(sheet.skipped.is_empty(), "{name}: {:?}", sheet.skipped);
        let t = &sheet.textures[0];
        assert_eq!((t.width, t.height), (w, h), "{name}");
    }
}
