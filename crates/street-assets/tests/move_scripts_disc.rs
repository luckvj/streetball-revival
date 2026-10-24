//! Opt-in test against the user's disc: STREET_ISO=/path/to/disc.iso.
use std::collections::BTreeSet;
use street_assets::{
    anim::Bank,
    iso::Iso,
    moves::{DUNKS_SET, Kind, SHOTS_SET, TRICKS_SET},
    scripts::MoveScripts,
};

#[test]
fn original_disc_scripts_have_real_clip_targets_and_correct_slot_maps() {
    let Some(path) = std::env::var_os("STREET_ISO") else {
        return;
    };
    let mut iso = Iso::open(path).unwrap();
    let scripts = MoveScripts::read(&mut iso).unwrap();
    assert_eq!(scripts.shots.len(), 27);
    assert_eq!(scripts.dunks.len(), 60);
    assert_eq!(scripts.tricks.len(), 75);
    assert_eq!(scripts.default_shots, SHOTS_SET);
    assert_eq!(scripts.default_dunks, DUNKS_SET);
    assert_eq!(scripts.default_tricks, TRICKS_SET);
    assert_eq!(scripts.row_script(0), Some(4));
    assert_eq!(scripts.row_script(53), Some(51));
    assert_eq!(scripts.row_script(54), Some(1));
    assert_eq!(scripts.row_script(92), Some(24));
    assert_eq!(scripts.row_script(93), None);
    assert_eq!(scripts.script(Kind::Dunk, 29).unwrap().address, 0x44960a);
    assert_eq!(scripts.script(Kind::Dunk, 47).unwrap().address, 0x449f89);
    assert_eq!(scripts.script(Kind::Dunk, 59).unwrap().address, 0x44a7b3);
    assert!(scripts.script(Kind::Dunk, 60).is_none());
    assert_eq!(scripts.shots[0].clip_targets, ["jumpshot_l_1"]);
    assert_eq!(
        scripts.playback_script(Kind::Dunk, 48).unwrap().address,
        0x447ed5
    );
    assert_ne!(
        scripts
            .playback_script(Kind::Dunk, 48)
            .unwrap()
            .clip_targets,
        scripts.script(Kind::Dunk, 48).unwrap().clip_targets
    );
    assert_eq!(
        scripts.shots[0].blends[0]
            .targets
            .iter()
            .map(|c| c.record)
            .collect::<Vec<_>>(),
        [307, 306]
    );
    assert!(scripts.shots[0].blends[0].targets[0].mirrored);
    assert!(!scripts.shots[0].blends[0].targets[1].mirrored);
    assert_eq!(scripts.tricks[20].name, "juke_slipslide_revdribble");
    // Each trick row's own selection flags (`0x3d7238` +8): the first
    // three's 0xf, 0xa and 0x4a, as `street_sim::trick::SCRIPTS` has them.
    let flags: Vec<u32> = scripts.tricks[..3].iter().map(|t| t.state_mask).collect();
    assert_eq!(flags, [0xf, 0xa, 0x4a]);
    assert_eq!(scripts.tricks[0].address, 0x44cc7b);
    for kind in [Kind::Shot, Kind::Dunk, Kind::Trick] {
        let slots = if kind == Kind::Shot { 6 } else { 11 };
        assert!(scripts.default_script(kind, slots).is_none());
        for slot in 0..slots {
            assert!(
                !scripts
                    .default_script(kind, slot)
                    .unwrap()
                    .clip_targets
                    .is_empty()
            );
        }
    }
    let paths = iso
        .files()
        .map(|(p, _)| p.to_owned())
        .filter(|p| p.starts_with("/ANIM/") && p.ends_with(".BIN"))
        .collect::<Vec<_>>();
    let mut available = BTreeSet::new();
    for path in paths {
        let bank = Bank::parse(&iso.read(&path).unwrap()).unwrap();
        available.extend(bank.animations().map(str::to_owned));
    }
    let mut target_count = 0;
    for script in scripts
        .shots
        .iter()
        .chain(scripts.dunks.iter().flatten())
        .chain(scripts.tricks.iter())
        .chain(scripts.layup_finishes.iter())
    {
        assert!(
            !script.clip_targets.is_empty(),
            "empty targets: {}",
            script.name
        );
        for target in &script.clip_targets {
            target_count += 1;
            assert!(
                available.contains(target),
                "missing clip {target} in {}",
                script.name
            );
        }
    }
    eprintln!(
        "Verified {target_count} script clip references against {} disc clips",
        available.len()
    );
}

#[test]
fn advanced_dunks_are_the_ones_the_executable_flags() {
    let Some(path) = std::env::var_os("STREET_ISO") else {
        return;
    };
    let scripts = MoveScripts::read(&mut Iso::open(path).unwrap()).unwrap();
    let advanced: Vec<usize> = scripts
        .dunk_flags
        .iter()
        .enumerate()
        .filter(|(_, f)| *f & street_assets::scripts::ADVANCED_DUNK != 0)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(advanced, [30, 48, 49, 50, 51]);
    // The default set's four-turbo slot holds dunk 48.
    assert_eq!(scripts.default_dunks[0], 48);
}
