//! Opt-in test against the user's disc: STREET_ISO=/path/to/disc.iso.
use std::sync::Arc;
use street_assets::{gait::GaitData, iso::Iso};
use street_sim::{
    Hoop, Input, Phase, Player, Rules, World,
    gait_select::{GaitConfig, GaitProfile, GaitRecord},
};

fn config(profiles: Vec<GaitProfile>) -> Option<GaitConfig> {
    let path = std::env::var_os("STREET_ISO")?;
    let gait = GaitData::read(&mut Iso::open(path).unwrap()).unwrap();
    Some(GaitConfig {
        records: gait
            .records
            .iter()
            .map(|r| GaitRecord {
                rows: r.rows,
                category_mask: r.category_mask,
                player: r.player_restriction,
                minimum_handles: r.minimum_handles,
                root_distance: r.root_distance,
                motion: [r.minimum_motion, r.maximum_motion],
                flags: r.flags,
            })
            .collect(),
        profiles,
        ..Default::default()
    })
}

/// The disc's animations: a match never steps without them
/// (`street_sim::pose::Bank`).
fn poses() -> street_sim::pose::Bank {
    use street_sim::pose::{Bank, ClipData, Poses, SkeletonBone};
    let path = std::env::var_os("STREET_ISO").expect("the disc");
    let data = street_assets::pose::PoseData::read(&mut Iso::open(path).unwrap()).unwrap();
    let skeleton: Vec<SkeletonBone> = data
        .skeleton
        .bones
        .iter()
        .map(|b| SkeletonBone {
            parent: b.parent.map(|p| p as u8),
            translation: b.translation,
            rotation: b.rotation,
        })
        .collect();
    let clips = data
        .clips
        .into_iter()
        .map(|c| ClipData {
            raw_pose: c.raw_pose,
            bones: c.bones,
            rotations: c.rotations,
            root: c.root,
            states: c.states,
        })
        .collect();
    Bank::Disc(Arc::new(
        Poses::new(&skeleton, clips, data.records).unwrap(),
    ))
}

fn hoops() -> [Hoop; 2] {
    [
        Hoop {
            rim: [36.95, 11.75, 0.0],
            rim_radius: 0.75,
        },
        Hoop {
            rim: [-36.96, 11.75, 0.0],
            rim_radius: 0.75,
        },
    ]
}

/// Which gait records a player running full stick plays over many
/// restarts, with the ball (from the centre) or without (far from it).
fn runs(profile: GaitProfile, config: &GaitConfig, held: bool) -> Vec<u16> {
    static POSES: std::sync::OnceLock<street_sim::pose::Bank> = std::sync::OnceLock::new();
    let poses = POSES.get_or_init(poses);
    let mut seen = Vec::new();
    // The selector draws from the shared generator: seeds spread apart.
    for k in 0..40u32 {
        let seed = k.wrapping_mul(0x9e37_79b9);
        let mut w = World::new(
            hoops(),
            vec![Player::new(
                0,
                if held { [0.0, 0.0] } else { [-30.0, 20.0] },
                0.0,
            )],
            Rules::default(),
            seed,
        );
        w.poses = poses.clone();
        w.phase = Phase::Live;

        w.gait = Some(Arc::new(GaitConfig {
            profiles: vec![profile],
            ..config.clone()
        }));
        for _ in 0..30 {
            w.step(&[Input::moving([1.0, 0.0])]);
        }
        assert_eq!(w.ball.state == street_sim::BallState::Held(0), held);
        let channel = &w.players[0].gait_channel;
        let record = channel
            .record
            .expect("a running player plays a gait record");
        // The record drives the motion range and root distance.
        let r = config.records[usize::from(record)];
        assert_eq!(channel.motion, r.motion);
        assert_eq!(channel.root_distance, r.root_distance);
        seen.push(record);
    }
    seen.sort_unstable();
    seen.dedup();
    seen
}

#[test]
fn players_run_on_the_original_gait_records() {
    let Some(config) = config(vec![]) else {
        return;
    };
    // An ordinary lighter man: the generic male runs (run_1, BPJOG,
    // run_cycle_nobal_10_4), never a legend's or a woman's.
    let man = GaitProfile {
        category: 1,
        identity: 20,
        handles: 50,
    };
    assert_eq!(runs(man, &config, false), [48, 49, 50]);
    // With the ball: dribble_run_finesse_1 and RUN_CYCLE_WBALL_05_3.
    assert_eq!(runs(man, &config, true), [103, 104]);
    // A heavier player: RUN_CYCLE_NOBALL_04_2.
    let big = GaitProfile { category: 4, ..man };
    assert_eq!(runs(big, &config, false), [45]);
    // Dr. J (record 158) always runs his own run.
    let drj = GaitProfile {
        category: 1,
        identity: 158,
        handles: 50,
    };
    assert_eq!(runs(drj, &config, false), [36]);
    assert_eq!(runs(drj, &config, true), [93]);
    // Snapshots carry the profiles and the choice; the records are the
    // disc's, which the machine reading one installs (`street_sim::disc`).
    let mut w = World::new(
        hoops(),
        vec![Player::new(0, [0.0, 0.0], 0.0)],
        Rules::default(),
        1,
    );
    w.poses = poses();
    w.phase = Phase::Live;
    w.gait = Some(Arc::new(GaitConfig {
        profiles: vec![drj],
        ..config.clone()
    }));
    for _ in 0..30 {
        w.step(&[Input::moving([1.0, 0.0])]);
    }
    let back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back.gait.as_ref().unwrap().profiles, [drj]);
    assert!(back.gait.as_ref().unwrap().records.is_empty());
    assert_eq!(back.players[0].gait_channel, w.players[0].gait_channel);
    assert_eq!(back.checksum(), w.checksum());
}
