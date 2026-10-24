//! Whole characters from the user's own GameCube discs. Skipped unless
//! STREET_V3_GC (Street V3) or STREET_V1_GC (the original game) names an image.

use street_assets::{
    character::Character,
    gc_character::{Kind, Library, Look},
    gcdisc::GcDisc,
};

fn open(var: &str) -> Option<(GcDisc, Library)> {
    let path = std::env::var_os(var)?;
    let mut disc = GcDisc::open(path).expect("not a readable GameCube disc");
    let lib = Library::open(&mut disc).expect("roster");
    Some((disc, lib))
}

/// Every vertex skinned to real bones with weights summing to one, the bind
/// pose reproduced by skinning with the bind matrices, all textures found.
fn check(c: &Character) {
    assert!(!c.meshes.is_empty(), "{}", c.id);
    assert!(
        !c.notes.iter().any(|n| n.starts_with("no ")),
        "{}: {:?}",
        c.id,
        c.notes
    );
    let bones = c.skeleton.bones.len();
    for m in &c.meshes {
        assert_eq!(m.joints.len(), m.positions.len(), "{} {}", c.id, m.name);
        assert_eq!(m.weights.len(), m.positions.len());
        assert_eq!(m.uvs.len(), m.positions.len());
        assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
        for (j, w) in m.joints.iter().zip(&m.weights) {
            assert!(j.iter().all(|&b| (b as usize) < bones));
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        }
        assert!(
            m.texture.is_some_and(|t| t < c.textures.len()),
            "{} {}",
            c.id,
            m.name
        );
    }
    let posed = c.skin(&c.bind_matrices());
    for (m, p) in c.meshes.iter().zip(&posed) {
        for (a, b) in m.positions.iter().zip(p) {
            assert!(
                (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3),
                "{} {}",
                c.id,
                m.name
            );
        }
    }
}

#[test]
fn v3_roster_and_teams() {
    let Some((_, lib)) = open("STREET_V3_GC") else {
        eprintln!("STREET_V3_GC not set; skipping");
        return;
    };
    assert!(lib.characters.len() >= 220, "{}", lib.characters.len());
    let count = |k: Kind| lib.characters.iter().filter(|c| c.kind == k).count();
    assert!(count(Kind::League) >= 140);
    assert_eq!(count(Kind::Legend), 25);
    let nintendo = lib.teams.iter().find(|t| t.id == 500).unwrap();
    assert_eq!(nintendo.name, "Nintendo All-Stars");
    assert_eq!(nintendo.roster, ["v3gc:mario", "v3gc:luigi", "v3gc:peach"]);
    let marion = lib.find("v3gc:shawn_marion").unwrap();
    assert_eq!(marion.team_name.as_deref(), Some("Phoenix Suns"));
    assert_eq!(
        (marion.height_in, marion.weight_lb, marion.jersey),
        (Some(79), Some(228), Some(31))
    );
    assert_eq!(marion.ratings.len(), 11);
    assert!(lib.find("nene").is_some());
    let mut keys: Vec<&str> = lib.characters.iter().map(|c| c.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), lib.characters.len(), "keys are unique");
}

#[test]
fn v3_characters_assemble_skinned() {
    let Some((mut disc, lib)) = open("STREET_V3_GC") else {
        eprintln!("STREET_V3_GC not set; skipping");
        return;
    };
    for key in [
        "mario",
        "luigi",
        "peach",
        "shawn_marion",
        "josh",
        "julius_erving",
        "bonafide",
        "ashley",
    ] {
        let c = lib.assemble(&mut disc, key, Look::Home).unwrap();
        check(&c);
    }
    // The Nintendo All-Stars keep their own 15- and 44-bone skeletons, and
    // Luigi wears Mario's gloves (his player record lists `Mhands`).
    let luigi = lib.assemble(&mut disc, "luigi", Look::Home).unwrap();
    assert_eq!(luigi.skeleton.bones.len(), 15);
    assert!(luigi.meshes.iter().any(|m| m.name.contains("mhands")));
    assert!(
        !luigi
            .meshes
            .iter()
            .any(|m| m.name.contains("eyes/luigi_eyes")),
        "blink lids left out"
    );
    let peach = lib.assemble(&mut disc, "peach", Look::Home).unwrap();
    assert_eq!(peach.skeleton.bones.len(), 44);
    assert!(street_assets::character::bind_mismatch(&peach.skeleton) < 1e-3);
    // Away uniforms differ from home ones.
    let home = lib.assemble(&mut disc, "shawn_marion", Look::Home).unwrap();
    let away = lib.assemble(&mut disc, "shawn_marion", Look::Away).unwrap();
    let tops = |c: &Character| {
        c.textures
            .iter()
            .find(|t| t.name == "tops")
            .unwrap()
            .rgba
            .clone()
    };
    assert_ne!(tops(&home), tops(&away));
    // About 4 (Mario) to 7 (league players, whose hips sit at the origin) units.
    for c in [&luigi, &peach, &home] {
        assert!(
            (4.0..8.0).contains(&c.height_units()),
            "{} {}",
            c.id,
            c.height_units()
        );
    }
}

/// A Vol. 2 animation drives V3's skeletons by bone name.
#[test]
fn vol2_bones_drive_v3_skeletons() {
    let (Some((mut disc, lib)), Some(iso)) = (open("STREET_V3_GC"), std::env::var_os("STREET_ISO"))
    else {
        eprintln!("STREET_V3_GC or STREET_ISO not set; skipping");
        return;
    };
    let mut iso = street_assets::iso::Iso::open(iso).unwrap();
    let vol2 = street_assets::player::skeleton(&mut iso).unwrap();
    for key in ["shawn_marion", "mario", "peach"] {
        let c = lib.assemble(&mut disc, key, Look::Home).unwrap();
        let r = street_assets::retarget::Retarget::new(&vol2, &c.skeleton);
        // Every body bone Vol. 2 animates has a partner.
        for name in [
            "Root",
            "LThigh",
            "LLowLeg",
            "LFoot",
            "UpperTorso",
            "LUpperArm",
            "LLowArm",
            "LHand",
            "Neck",
        ] {
            let i = c
                .skeleton
                .bones
                .iter()
                .position(|b| b.name == name)
                .unwrap();
            assert!(r.source_of[i].is_some(), "{key} {name}");
        }
        // Vol. 2's rest pose maps to the character's own rest pose.
        let rest: Vec<[f32; 4]> = vol2.bones.iter().map(|b| b.rotation).collect();
        let locals = r.pose(&rest);
        for (b, l) in c.skeleton.bones.iter().zip(&locals) {
            let d: f32 = (0..4).map(|k| b.rotation[k] * l.1[k]).sum();
            assert!((d.abs() - 1.0).abs() < 1e-3, "{key} {}", b.name);
        }
    }
}

/// V3's animation banks decode, and each family plays on any character.
#[test]
fn v3_animations_decode_and_retarget() {
    let Some((mut disc, lib)) = open("STREET_V3_GC") else {
        eprintln!("STREET_V3_GC not set; skipping");
        return;
    };
    use street_assets::gc_character::{self as gc, V3Rigs};
    let (mut total, mut decoded) = (0, 0);
    for name in gc::v3_animation_banks(&mut disc).unwrap() {
        let bank = gc::v3_animation_bank(&mut disc, &name).unwrap();
        assert!(bank.is_big_endian());
        for a in bank.animations() {
            total += 1;
            decoded += usize::from(bank.rotations(a).ok().flatten().is_some());
        }
    }
    assert!(
        total > 2000 && decoded * 100 >= total * 95,
        "{decoded} of {total}"
    );

    let rigs = V3Rigs::load(&mut disc).unwrap();
    assert_eq!(rigs.body.bones.len(), 22);
    let fe = gc::v3_animation_bank(&mut disc, "anibfe.bin").unwrap();
    let big = gc::v3_animation_bank(&mut disc, "anibig.bin").unwrap();
    let trex = gc::v3_animation_bank(&mut disc, "anibtrex.bin").unwrap();
    assert_eq!(fe.rig("createplayer_stand").unwrap(), gc::VOL2_RIG_IN_V3);
    assert_eq!(trex.rig("celeb_short_flex").unwrap(), gc::NINTENDO_RIG);
    // At rest, each family's channels sit near its rig's rest rotations
    // (the thighs differ by half a turn between the two body rigs).
    for (bank, anim) in [
        (&fe, "createplayer_stand"),
        (&big, "bm_stand_no_ball"),
        (&trex, "breathe_1"),
    ] {
        let Ok(Some(track)) = bank.rotations(anim) else {
            continue;
        };
        let rig = rigs.for_rig(bank.rig(anim).unwrap());
        for (&b, q) in track.bones.iter().zip(&track.frames[0]) {
            let Some(rest) = rig.bones.get(b as usize) else {
                continue;
            };
            if rest.name.contains("Thigh") {
                let d: f32 = (0..4).map(|k| rest.rotation[k] * q[k]).sum();
                assert!(
                    d.abs() > 0.8,
                    "{anim} {}: {q:?} vs {:?}",
                    rest.name,
                    rest.rotation
                );
            }
        }
    }
    // Poses come out finite on every family of character.
    for key in ["shawn_marion", "mario", "peach"] {
        let c = lib.assemble(&mut disc, key, Look::Home).unwrap();
        for (bank, anim) in [(&big, "celeb_short_flex"), (&trex, "celeb_short_flex")] {
            let track = bank.rotations(anim).unwrap().unwrap();
            let rig = rigs.for_rig(bank.rig(anim).unwrap());
            let locals = gc::v3_pose(&c, rig, &track.bones, &track.frames[10]);
            let pose = street_assets::character::pose_from_locals(&c.skeleton.bones, &locals);
            let posed = c.skin(&pose);
            assert!(
                posed
                    .iter()
                    .flatten()
                    .all(|p| p.iter().all(|v| v.is_finite()))
            );
        }
    }
}

#[test]
fn v1_roster_and_characters() {
    let Some((mut disc, lib)) = open("STREET_V1_GC") else {
        eprintln!("STREET_V1_GC not set; skipping");
        return;
    };
    assert_eq!(lib.characters.len(), 175);
    let count = |k: Kind| lib.characters.iter().filter(|c| c.kind == k).count();
    assert_eq!(
        (
            count(Kind::League),
            count(Kind::Street),
            count(Kind::Generic)
        ),
        (145, 18, 12)
    );
    let shaq = lib.find("v1gc:shaquille_o_neal").unwrap();
    assert_eq!(shaq.team_name.as_deref(), Some("Los Angeles Lakers"));
    assert_eq!(
        (shaq.height_in, shaq.jersey, shaq.position),
        (Some(85), Some(34), Some(0))
    );
    // A league player, a street legend and a crew member: every part found,
    // skinned to the 43-bone skeleton Vol. 2 shares.
    for key in [
        "shaquille_o_neal",
        "carlito_vargas",
        "zoe_payne",
        "mastermind",
    ] {
        let c = lib.assemble(&mut disc, key, Look::Home).unwrap();
        check(&c);
        assert_eq!(c.skeleton.bones.len(), 43);
        assert!(
            (18.0..22.0).contains(&c.height_units()),
            "{key} {}",
            c.height_units()
        );
        assert!(
            c.meshes.iter().any(|m| m.name.contains("hand_relax")),
            "{key} has hands"
        );
    }
}

#[test]
fn v1_animations_decode() {
    let Some((mut disc, lib)) = open("STREET_V1_GC") else {
        eprintln!("STREET_V1_GC not set; skipping");
        return;
    };
    use street_assets::gc_character::V1Animations;
    let anims = V1Animations::load(&mut disc, "anibot.ani").unwrap();
    assert_eq!(anims.len(), 614);
    assert_eq!(anims.names().next(), Some("RUN_1"));
    let shaq = lib
        .assemble(&mut disc, "shaquille_o_neal", Look::Home)
        .unwrap();
    for name in ["BREATHE_1", "STAND_DANCE_1", "STANDREADY_WB_SIDESTEP_L"] {
        let i = anims.index_of(name).unwrap();
        let track = anims.rotations(i).unwrap();
        assert_eq!(track.bones.len(), 22);
        for frame in &track.frames {
            for q in frame {
                let n: f32 = q.iter().map(|v| v * v).sum();
                assert!((n - 1.0).abs() < 1e-3);
            }
        }
        // At a standing frame the hips sit about 11 units up.
        let root = anims.root(i).unwrap();
        assert!(root.iter().all(|r| (5.0..16.0).contains(&r[1])), "{name}");
        let locals = street_assets::gc_character::v3_pose(
            &shaq,
            &shaq.skeleton,
            &track.bones,
            &track.frames[0],
        );
        let pose = street_assets::character::pose_from_locals(&shaq.skeleton.bones, &locals);
        assert!(
            shaq.skin(&pose)
                .iter()
                .flatten()
                .all(|p| p.iter().all(|v| v.is_finite()))
        );
    }
    let front = V1Animations::load(&mut disc, "anifront.ani").unwrap();
    for i in 0..front.len() {
        front.rotations(i).unwrap();
    }
    // Every in-game animation decodes.
    for i in 0..anims.len() {
        anims.rotations(i).unwrap();
    }
}

/// Every character of both discs goes onto Vol. 2's skeleton: Vol. 2's
/// bones, hips at Vol. 2's height, the bind pose kept.
#[test]
fn every_character_rebinds_onto_vol2() {
    let Some(iso) = std::env::var_os("STREET_ISO") else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let mut iso = street_assets::iso::Iso::open(iso).unwrap();
    let vol2 = street_assets::player::skeleton(&mut iso).unwrap();
    use street_assets::canonical::{VOL2_HIP, rebind};
    for var in ["STREET_V3_GC", "STREET_V1_GC"] {
        let Some((mut disc, lib)) = open(var) else {
            eprintln!("{var} not set; skipping");
            continue;
        };
        for info in &lib.characters {
            let c = lib.assemble(&mut disc, &info.key, Look::Home).unwrap();
            let r = rebind(&c, &vol2).unwrap_or_else(|e| panic!("{}: {e}", info.id));
            assert_eq!(r.character.skeleton.bones.len(), 43);
            assert!((r.hip - VOL2_HIP).abs() < 1e-3, "{}", info.id);
            assert!(r.matched >= 15, "{} {}", info.id, r.matched);
            for (a, b) in r.character.skeleton.bones.iter().zip(&vol2.bones) {
                assert_eq!(
                    (&a.name, a.parent, a.rotation),
                    (&b.name, b.parent, b.rotation)
                );
            }
            let posed = r.character.skin(&r.character.bind_matrices());
            for (m, p) in r.character.meshes.iter().zip(&posed) {
                for (a, b) in m.positions.iter().zip(p) {
                    assert!((0..3).all(|k| (a[k] - b[k]).abs() < 1e-3), "{}", info.id);
                }
            }
        }
    }
}

/// The game's GameCube discs: characters load once, on Vol. 2's
/// skeleton, the Nintendo All-Stars at their own size.
#[test]
fn gc_discs_load_characters_for_the_game() {
    let (Some(iso), Some(v3)) = (
        std::env::var_os("STREET_ISO"),
        std::env::var_os("STREET_V3_GC"),
    ) else {
        eprintln!("STREET_ISO or STREET_V3_GC not set; skipping");
        return;
    };
    use street_assets::gc_discs;
    let mut iso = street_assets::iso::Iso::open(iso).unwrap();
    let vol2 = street_assets::player::skeleton(&mut iso).unwrap();
    let (game, n) = gc_discs::add(std::path::Path::new(&v3)).unwrap();
    assert_eq!((game.prefix(), n >= 220), ("v3gc", true));
    assert!(gc_discs::has("v3gc:mario") && !gc_discs::has("v1gc:anyone"));
    assert_eq!(gc_discs::info("v3gc:luigi").unwrap().jersey, Some(28));
    let mario = gc_discs::load("v3gc:mario", Look::Home, &vol2).unwrap();
    let again = gc_discs::load("V3GC:Mario", Look::Home, &vol2).unwrap();
    assert!(std::sync::Arc::ptr_eq(&mario, &again), "kept");
    assert!(
        (50.0..60.0).contains(&mario.model_height_in),
        "{}",
        mario.model_height_in
    );
    let marion = gc_discs::load("v3gc:shawn_marion", Look::Away, &vol2).unwrap();
    assert_eq!(marion.model_height_in, 79.0);
    assert!(gc_discs::load("v3gc:nobody", Look::Home, &vol2).is_err());
}
