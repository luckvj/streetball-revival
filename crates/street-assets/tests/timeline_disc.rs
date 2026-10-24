use street_assets::{
    iso::Iso,
    timeline::{Payload, ScriptBank},
};

#[test]
fn original_timelines_have_actor_camera_ball_and_real_aems_records() {
    let Some(path) = std::env::var_os("STREET_ISO") else {
        return;
    };
    let mut iso = Iso::open(path).unwrap();
    let bank = ScriptBank::read(&mut iso).unwrap();
    assert_eq!(bank.scripts.len(), 70);
    assert_eq!(bank.animations.len(), 1144);
    let finisher = bank.finishers.iter().find(|f| f.animation == 965).unwrap();
    assert_eq!(finisher.address, 0x44b4c2);
    assert_eq!(finisher.release_tick, Some(97));
    assert_eq!(
        finisher
            .spans
            .iter()
            .map(|s| s.duration)
            .collect::<Vec<_>>(),
        [34, 63]
    );
    assert_eq!(bank.scripts[0].name, "GamDN1a");
    assert_eq!(bank.scripts[0].frames, 237);
    let mut sounds = 0;
    for script in &bank.scripts {
        for frame in 0..script.frames {
            let state = script.sample(frame as f32, false);
            for actor in state.actors.iter().flatten() {
                assert!(
                    actor.actor.position.iter().all(|p| p.is_finite()),
                    "{} frame {}",
                    script.name,
                    frame
                );
                assert!(
                    actor.actor.animation < 1144,
                    "{} animation out of range",
                    script.name
                );
            }
            if let Some(camera) = state.camera {
                assert!(
                    camera
                        .position
                        .iter()
                        .chain(camera.target.iter())
                        .all(|p| p.is_finite())
                );
            }
        }
        for record in &script.records {
            if let Payload::Command(command) = &record.payload {
                sounds += usize::from(command.sound().is_some());
            }
        }
    }
    assert!(
        sounds > 16,
        "sound dispatches are present in the actual bank"
    );
    assert_eq!(bank.gamebreakers[0], [None; 4]);
    assert_eq!(bank.gamebreakers[9], [Some(0), Some(1), None, None]);
    assert_eq!(bank.gamebreaker_region_rows().len(), 206);
    assert_eq!(
        bank.gamebreaker_family([-35.0, 0.0], 0),
        bank.gamebreaker_family([35.0, 0.0], 1)
    );
}
