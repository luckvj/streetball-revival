//! Opt-in test against the user's disc: STREET_ISO=/path/to/disc.iso.
use street_assets::{iso::Iso, moves::Kind, scripts::MoveScripts};

#[test]
fn self_toss_schedules_come_from_the_script_bytecode() {
    // G05: the sim's launch and closing waits are the scripts' own bytes.
    let Some(path) = std::env::var_os("STREET_ISO") else {
        return;
    };
    let mut iso = Iso::open(path).unwrap();
    let elf = iso.read("/SLUS_206.51").unwrap();
    let scripts = MoveScripts::from_executable(&elf).unwrap();
    for (script, address) in [
        (21u16, 0x44de67u32),
        (23, 0x44dedc),
        (36, 0x44d840),
        (60, 0x44e044),
        (61, 0x44e0b9),
        (69, 0x44e365),
    ] {
        assert_eq!(
            scripts
                .script(Kind::Trick, usize::from(script))
                .unwrap()
                .address,
            address
        );
        let timing = street_assets::scripts::self_toss_timing(&elf, address)
            .unwrap()
            .expect("a self-toss script");
        let schedule = street_sim::self_toss::schedule_for_script(script).unwrap();
        let cue = street_sim::self_toss::cue_for_script(script).unwrap();
        assert_eq!(u32::from(timing.blend_updates), schedule.blend_updates);
        assert_eq!(
            u32::from(timing.pre_callback_wait_updates),
            schedule.pre_callback_wait_updates
        );
        assert_eq!(
            u32::from(timing.followthrough_updates),
            schedule.followthrough_updates
        );
        assert_eq!(
            timing.callback,
            [
                cue.flight_frames_at_60_hz as u32,
                cue.sample_frame as u32,
                u32::from(cue.bounce)
            ],
            "script {script}"
        );
    }
}
