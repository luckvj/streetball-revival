//! Checks of player assembly against a real USA disc image of the original game.
//! Skipped unless STREET_ISO points at the image, since the repo ships no game data.

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::BufReader,
};

use street_assets::{
    big,
    db::{self, PlayerKind, PlayerModel},
    eagl,
    iso::Iso,
    morph,
    player::{self, Side},
};

const PLAYERS: &str = "/XF/XF000002";

fn disc() -> Option<Iso<BufReader<File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

/// Every league player's three morph targets fit their build's three bodies batch
/// for batch, and only touch the head (`HEAD`, `HEADBAND`, `MOUTH` and the
/// `zdreds` hair, which Steve Nash's reshapes).
#[test]
fn every_morph_target_fits_its_body() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let entries = big::read_index(&mut iso, PLAYERS).unwrap();
    let mut bodies: HashMap<usize, eagl::Model> = HashMap::new();
    let mut checked = 0;
    // Largest move of any vertex, by group, and whose morph it was.
    let mut moved: BTreeMap<String, (f32, String)> = BTreeMap::new();
    for p in db::players(&mut iso).unwrap() {
        let Some(PlayerModel::League(m)) = p.model() else {
            continue;
        };
        for (&body, &target) in m.bodies.iter().zip(&m.morphs) {
            let model = bodies.entry(body).or_insert_with(|| {
                eagl::parse(&big::read_entry(&mut iso, PLAYERS, &entries[body]).unwrap()).unwrap()
            });
            let morph =
                morph::parse(&big::read_entry(&mut iso, PLAYERS, &entries[target]).unwrap())
                    .unwrap_or_else(|e| panic!("{} morph {target}: {e}", p.name()));
            assert!(!morph.positions.is_empty(), "{}: empty morph", p.name());
            let mut shaped = model.clone();
            morph
                .apply(&mut shaped, 1.0)
                .unwrap_or_else(|e| panic!("{} morph {target} on body {body}: {e}", p.name()));
            for (before, after) in model.meshes.iter().zip(&shaped.meshes) {
                let group = before.group.clone().unwrap_or_default();
                let most = moved.entry(group).or_insert((0.0, String::new()));
                for (a, b) in before.positions.iter().zip(&after.positions) {
                    let d = (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max);
                    if d > most.0 {
                        *most = (d, p.name());
                    }
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 145 * 3);
    println!("{moved:#?}");
    for (group, (d, who)) in &moved {
        let limit = match group.as_str() {
            "HEAD" | "HEADBAND" | "MOUTH" => 0.7,
            "zdreds" => 1.5,
            _ => 0.0,
        };
        assert!(*d <= limit, "{who}'s morph moves {group} by {d}");
    }
}

/// Everyone but created players assembles with every texture slot filled
/// (except the headband's on players without one, which is hidden).
#[test]
fn players_assemble_with_their_textures() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let players = db::players(&mut iso).unwrap();
    // A sample across the kinds: Abdur-Rahim, Iverson (headband), Sprewell
    // (braids), Shaq (large), a streetballer, a legend and a generic baller.
    for index in [0, 100, 90, 55, 145, 158, 184] {
        let p = &players[index];
        for side in [Side::Road, Side::Home] {
            let look =
                player::look(&mut iso, p, side).unwrap_or_else(|e| panic!("{}: {e}", p.name()));
            assert!(look.scale > 1.0 && look.scale < 1.4, "{}", p.name());
            for model in &look.models {
                assert_eq!(model.stats.skipped_batches, 0, "{}", p.name());
                for mesh in &model.meshes {
                    let shown = mesh.group.as_deref().is_none_or(|g| look.shows(g));
                    if let (true, Some(slot)) = (shown, &mesh.texture) {
                        assert!(
                            look.textures.contains_key(slot),
                            "{} ({side:?}): no image for {slot}",
                            p.name()
                        );
                    }
                }
            }
        }
        let extra = &player::look(&mut iso, p, Side::Road).unwrap().extra_groups;
        match index {
            100 => assert_eq!(extra, &["HEADBAND"]),
            90 => assert_eq!(extra, &["zdreds"]),
            _ => {}
        }
        assert_ne!(p.kind(), PlayerKind::Created);
    }
}

/// Every league player's pads, braces and bands load: one model per accessory
/// with a model, after the body, skinned to the body's bones, each texture
/// slot its own (never one of the body's) and bound.
#[test]
fn league_players_wear_their_accessories() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let players = db::players(&mut iso).unwrap();
    let mut worn = 0;
    for p in players.iter().filter(|p| p.kind() == PlayerKind::League) {
        let Some(PlayerModel::League(m)) = p.model() else {
            panic!("{}: no league model", p.name());
        };
        let pieces = m.accessories.iter().filter(|a| a.model.is_some()).count();
        let look =
            player::look(&mut iso, p, Side::Road).unwrap_or_else(|e| panic!("{}: {e}", p.name()));
        assert_eq!(look.models.len(), 1 + pieces, "{}", p.name());
        let body_slots: Vec<&String> = look.models[0]
            .meshes
            .iter()
            .filter_map(|m| m.texture.as_ref())
            .collect();
        for piece in &look.models[1..] {
            assert_eq!(piece.stats.skipped_batches, 0, "{}", p.name());
            for mesh in &piece.meshes {
                assert!(
                    !mesh.joints.is_empty(),
                    "{}: {} unskinned",
                    p.name(),
                    piece.name
                );
                let slot = mesh.texture.as_ref().expect("accessories are textured");
                assert!(!body_slots.contains(&slot), "{}: {slot}", p.name());
                assert!(look.textures.contains_key(slot), "{}: {slot}", p.name());
            }
        }
        worn += pieces;
    }
    // The disc dresses twenty across the league.
    assert_eq!(worn, 20);
}

/// The six created players (team 37) assemble from the kit: the outfit body
/// with its face, beard and shoe morph targets, the hair-and-head model, and
/// an image for every texture slot their shown groups use.
#[test]
fn created_players_assemble_from_the_kit() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let players = db::players(&mut iso).unwrap();
    let created: Vec<_> = players
        .iter()
        .filter(|p| p.kind() == PlayerKind::Created)
        .collect();
    assert_eq!(created.len(), 6);
    for p in created {
        let look =
            player::look(&mut iso, p, Side::Road).unwrap_or_else(|e| panic!("{}: {e}", p.name()));
        assert_eq!(look.models.len(), 2, "{}: body and head", p.name());
        for model in &look.models {
            assert_eq!(model.stats.skipped_batches, 0, "{}", p.name());
            for mesh in &model.meshes {
                let shown = mesh.group.as_deref().is_none_or(|g| look.shows(g));
                if let (true, Some(slot)) = (shown, &mesh.texture) {
                    assert!(
                        look.textures.contains_key(slot),
                        "{}: no image for {slot}",
                        p.name()
                    );
                }
            }
        }
        // Men carry a beard group with facial hair; women never do.
        assert_eq!(
            look.extra_groups.iter().any(|g| g == "zzBeard"),
            !p.female,
            "{}",
            p.name()
        );
    }
}

#[test]
fn native_body_scale_matches_original_gait_capture() {
    let Some(mut iso) = disc() else { return };
    let Some(path) = std::env::var_os("STREET_NATIVE_GAIT_CAPTURE") else {
        return;
    };
    let capture: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(capture["metadata"]["serial"], "SLUS-20651");
    let players = db::players(&mut iso).unwrap();
    let mut checked = 0;
    let mut normal = 0;
    let mut uniform = 0;
    let mut maximum = [0.0f32; 3];
    for sample in capture["samples"].as_array().unwrap() {
        let Some(address) = sample["player"]["address"].as_str() else {
            continue;
        };
        let Some(actor) = sample["actors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|actor| actor["address"].as_str() == Some(address))
        else {
            continue;
        };
        let record = actor["gait"]["player_record"].as_u64().unwrap() as usize;
        let calculated = players[record].native_body_scale().unwrap();
        if matches!(players[record].model(), Some(PlayerModel::League(_))) {
            normal += 1;
        } else {
            uniform += 1;
        }
        for (i, value) in calculated.into_iter().enumerate() {
            let observed = sample["skeleton"]["body_scale"][i].as_f64().unwrap() as f32;
            maximum[i] = maximum[i].max((value - observed).abs());
            assert!(
                (value - observed).abs() < 0.000001,
                "record {record}, component {i}: calculated {value}, observed {observed}"
            );
        }
        checked += 1;
    }
    assert!(
        checked > 0,
        "capture must include selected-player gait/body scale packets"
    );
    eprintln!(
        "native body scale: {checked} observations, normal {normal}, uniform {uniform}, max xyz error {maximum:?}"
    );
}

#[test]
fn native_prebuilt_scale_is_uniform_and_head_axes_keep_database_morphs() {
    let Some(mut iso) = disc() else { return };
    let players = db::players(&mut iso).unwrap();
    let mut normal = 0;
    let mut prebuilt = 0;
    for p in players {
        let Some(model) = p.model() else { continue };
        let Some(body) = p.native_body_scale() else {
            continue;
        };
        if matches!(model, PlayerModel::League(_)) {
            normal += 1;
        } else {
            prebuilt += 1;
            assert_eq!(body, [p.height_scale(); 3]);
        }
        // Distinct exact percentages verify the stored field-to-axis map.
        // 3153e8's chopped multiply makes 78-inch scale 3f999999;
        // 315564's nearest divide gives league head compensation 3f800001.
        // Prebuilt models take 315568's factor 1 instead.
        let mut morph = p.clone();
        morph.model_height_in = 78;
        morph.body_scale = [25, -50, 100];
        assert_eq!(morph.height_scale().to_bits(), 0x3f99_9999);
        let expected = if matches!(model, PlayerModel::League(_)) {
            [0x3f00_0001, 0x4000_0001, 0x3fa0_0001]
        } else {
            [0x3f00_0000, 0x4000_0000, 0x3fa0_0000]
        };
        // Raw19d59c writes bone21 XYZ as skeleton+8/+c/+4.
        assert_eq!(
            morph.native_head_scale().unwrap().map(f32::to_bits),
            expected
        );
    }
    assert!(normal > 0 && prebuilt > 0);
}
