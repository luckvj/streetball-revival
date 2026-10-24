//! Exports from the player's own disc (skipped without `STREET_ISO`).

use std::path::PathBuf;

use street_export::export::{self, Options};
use street_export::hires;

fn iso() -> Option<PathBuf> {
    std::env::var_os("STREET_ISO").map(PathBuf::from)
}

fn out(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn exports_a_few_characters_with_the_whole_catalog() {
    let Some(vol2) = iso() else { return };
    let dir = out("export-few");
    let opts = Options {
        out: dir.clone(),
        vol2,
        roster_db: None,
        hires: None,
        v3: None,
        v1: None,
        only: vec![
            "vol2:0".into(),
            "vol2:158".into(),
            "vol2:234".into(),
            "vol2:184".into(),
        ],
        all_anims: false,
        no_ui: false,
        threads: 4,
    };
    let report = export::characters(&opts, &hires::None_).unwrap();
    assert_eq!(report.characters, 4, "{:?}", report.warnings);

    let catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("catalog.json")).unwrap()).unwrap();
    assert_eq!(catalog["roster"], "original");
    assert_eq!(catalog["teams"].as_array().unwrap().len(), 49);
    assert_eq!(catalog["moves"].as_array().unwrap().len(), 93);
    let chars = catalog["characters"].as_array().unwrap();
    assert_eq!(chars.len(), 4);
    for c in chars {
        assert_eq!(c["raw"].as_array().unwrap().len(), 101);
        for (k, n) in [("layups", 6), ("dunks", 11), ("tricks", 11)] {
            assert_eq!(c["moves"][k].as_array().unwrap().len(), n);
            assert_eq!(c["moveScripts"][k].as_array().unwrap().len(), n);
        }
        assert!(c["model"].is_string());
    }
    // Every move row resolves to clips the move library holds.
    let clips = catalog["clips"].as_object().unwrap();
    for m in catalog["moves"].as_array().unwrap() {
        let list = m["clips"].as_array().unwrap();
        assert!(!list.is_empty(), "{} plays nothing", m["name"]);
        for c in list {
            assert!(clips.contains_key(c.as_str().unwrap()), "{c} missing");
        }
    }
    // Julius Erving's own second dunk replaces the default one.
    let doc = chars.iter().find(|c| c["record"] == 158).unwrap();
    let league = chars.iter().find(|c| c["record"] == 0).unwrap();
    assert_ne!(
        doc["moveScripts"]["dunks"][1],
        league["moveScripts"]["dunks"][1]
    );

    // The model: one skin over the game's bones, textures embedded.
    let (doc, buffers, images) = gltf::import(dir.join("models/vol2/0.glb")).unwrap();
    let skin = doc.skins().next().unwrap();
    let names: Vec<&str> = skin.joints().map(|j| j.name().unwrap()).collect();
    assert_eq!(names.len(), 43);
    assert_eq!(&names[..3], ["Root", "LThigh", "LLowLeg"]);
    assert!(images.len() >= 5);
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for mesh in doc.meshes() {
        for p in mesh.primitives() {
            let reader = p.reader(|b| Some(&buffers[b.index()]));
            for v in reader.read_positions().unwrap() {
                lo = lo.min(v[1]);
                hi = hi.max(v[1]);
            }
        }
    }
    assert!(hi - lo > 6.0 && hi - lo < 7.5, "rest body {lo}..{hi} units");

    // The libraries: the CAB stand and every move's clips.
    let (base, _, _) = gltf::import(dir.join("anims/vol2/base.glb")).unwrap();
    let anims: Vec<&str> = base.animations().filter_map(|a| a.name()).collect();
    for name in [
        "createplayer_stand",
        "STANDRELAX_ANTICIPATION_01_2",
        "jumpshot_r_3",
    ] {
        assert!(anims.contains(&name), "{name} missing from base.glb");
    }
    assert!(dir.join("ui/manifest.json").exists());
    assert!(dir.join("ui/backdrops/3.png").exists());
    assert!(dir.join("portraits/vol2/0.png").exists());
}

#[test]
fn every_character_assembles() {
    let Some(vol2) = iso() else { return };
    let mut iso = street_assets::iso::Iso::open(vol2).unwrap();
    let players = street_assets::db::players(&mut iso).unwrap();
    let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
    assert_eq!(players.len(), 240);
    for p in &players {
        let c = street_export::vol2::character(&mut iso, p, &skeleton, &hires::None_)
            .unwrap_or_else(|e| panic!("{}: {e}", p.name()));
        assert!(!c.primitives.is_empty(), "{} has no mesh", p.name());
        assert!(c.scale > 0.0);
    }
}
