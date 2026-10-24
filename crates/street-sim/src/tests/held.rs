//! The held ball on the holder's clip ([`crate::held`]) and the scoop
//! ([`crate::touch::Scooping`]) through `World::step`, on a made-up
//! skeleton that stands the same in every clip: the pelvis 3 up, the hands
//! 1 above it and 1.5 in front, 0.6 apart. The gait's record (the gait
//! channel's first row) holds the ball in the right hand (state 8); every
//! other record in the left (4).

use super::*;
use crate::pose::{Bank, ClipData, Frame, Poses, SkeletonBone};
use std::sync::Arc;

fn bank() -> Bank {
    let mut skeleton = vec![
        SkeletonBone {
            parent: Some(0),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        crate::pose::BONES
    ];
    skeleton[0].parent = None;
    for (i, b) in skeleton.iter_mut().enumerate().skip(1) {
        b.translation = [0.0, -0.05 * i as f32, 0.0];
    }
    skeleton[20].translation = [0.0, 2.0, 0.0];
    skeleton[21] = SkeletonBone {
        parent: Some(20),
        translation: [0.0, 1.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
    };
    skeleton[15].translation = [0.3, 1.0, -1.5];
    skeleton[19].translation = [-0.3, 1.0, -1.5];
    for (k, b) in [1usize, 2, 3, 4].into_iter().enumerate() {
        skeleton[b].translation = [0.3, -0.6 * (k + 1) as f32, 0.0];
        skeleton[b + 4].translation = [-0.3, -0.6 * (k + 1) as f32, 0.0];
    }
    let still = |states: Vec<u32>| ClipData {
        raw_pose: false,
        bones: Vec::new(),
        rotations: vec![Vec::new(); states.len()],
        root: vec![[0.0, 2.5, 0.0]; states.len()],
        states,
    };
    let gait = crate::ordinary::PlayerGaitChannel::default().row;
    let records = (0..1144)
        .map(|r| Some((u16::from(r != gait), false)))
        .collect();
    Bank::Disc(Arc::new(
        Poses::new(
            &skeleton,
            vec![still(vec![8; 40]), still(vec![4; 90])],
            records,
        )
        .unwrap(),
    ))
}

/// Team 0's holder at x 20 facing +x, the ball in his gait's hand.
fn world() -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [20.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(1, [-20.0, 0.0], -std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        1,
    );
    w.poses = bank();
    w.phase = Phase::Live;
    w.take_ball(0);
    assert!(w.place_held(0, None));
    w
}

/// The point `FUN_0014e060` holds the ball at on `record`'s clip, with
/// `w`'s holder 0 standing as he stands.
fn hand_on(w: &World, record: u16, state: u32) -> [f32; 3] {
    let poses = w.poses.get().unwrap();
    let body = w.players[0].body();
    let pose = poses
        .pose(
            Frame {
                record,
                time: 0.0,
                looping: false,
            },
            body,
            w.place(0),
        )
        .unwrap();
    crate::dribble::in_hand(&pose, record, 0, state, body, w.ball.radius)
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    crate::pose::length(std::array::from_fn(|k| a[k] - b[k]))
}

fn step(w: &mut World, buttons: u16) {
    let mut inputs = vec![Input::default(); w.players.len()];
    inputs[0] = Input::default().with(buttons);
    w.step(&inputs);
}

#[test]
fn a_shot_holds_the_ball_on_its_clip_easing_through_the_blend() {
    // The gait holds it in the right hand; the shot's clip in the left.
    // The blend's first update stores the offset and eases from it; by
    // the blend's end the ball is in the shot clip's hand, and stays there
    // (the shooter stands) until the release, which leaves from it.
    let mut w = world();
    let gait = w.gait_frame(0).record;
    let right = hand_on(&w, gait, 8);
    assert!(distance(w.ball.position, right) < 1e-3);
    step(&mut w, button::SHOOT);
    let Action::Shooting { script, .. } = w.players[0].action else {
        panic!("{:?}", w.players[0].action);
    };
    let c = w.channel(0).unwrap();
    assert!(!c.gait && c.shown.blend.is_some(), "{c:?}");
    let left = hand_on(&w, c.shown.record.unwrap(), 4);
    // Turned to face the rim at the press: the offset turns with him.
    let first = w.ball.position;
    assert!(distance(first, left) > 0.1, "{first:?} {left:?}");
    let mut held = Vec::new();
    for _ in 0..60 {
        step(&mut w, button::SHOOT);
        if w.ball.state != BallState::Held(0) {
            break;
        }
        held.push((w.channel(0).unwrap(), w.ball.position));
    }
    let after = held
        .iter()
        .find(|(c, _)| c.shown.blend.is_none())
        .expect("the blend ends before the release");
    assert!(distance(after.1, left) < 1e-3, "{after:?} {left:?}");
    // Every update of the blend lies between where it began and the hand.
    for (c, at) in held.iter().filter(|(c, _)| c.shown.blend.is_some()) {
        assert!(distance(*at, left) < distance(first, left) + 1e-3, "{c:?}");
    }
    let _ = script;
}

#[test]
fn the_release_leaves_from_the_held_ball() {
    // Callback 21 launches the ball from where the last ball update held
    // it: the first free step is the launch velocity from there.
    let mut w = world();
    let mut last = w.ball.position;
    for _ in 0..90 {
        step(&mut w, button::SHOOT);
        if w.ball.state == BallState::Shot {
            break;
        }
        last = w.ball.position;
    }
    assert_eq!(w.ball.state, BallState::Shot);
    let dt = crate::TICK_SECONDS;
    let v = w.ball.velocity;
    // The step just flown: position - velocity (after gravity) less the
    // half step of gravity the update adds.
    let back = [
        w.ball.position[0] - v[0] * dt,
        w.ball.position[2] - v[2] * dt,
    ];
    assert!(
        (back[0] - last[0]).abs() < 0.05 && (back[1] - last[2]).abs() < 0.05,
        "{back:?} {last:?}"
    );
}

#[test]
fn a_left_hander_takes_the_handed_blends_alternate() {
    // `8e f2` (actor+0xc0 2): the shot script 0 (`jumpshot_1`) blends to
    // 307, a left-hander's to 306.
    let mut w = world();
    let s = &timing::SHOT_SCRIPTS[0];
    for (left_handed, record) in [(false, 307), (true, 306)] {
        w.players[0].left_handed = left_handed;
        w.players[0].channel_start = None;
        w.players[0].action = Action::Shooting {
            hoop: 0,
            elapsed: 0.0,
            gamebreaker: 0,
            fade: false,
            tick: 10,
            script: 0,
            release: s.release,
            check: s.check,
            end: s.end,
            gate: s.gate,
            latest: s.latest,
            stall: 0,
        };
        assert_eq!(w.channel(0).unwrap().shown.record, Some(record));
    }
}

#[test]
fn the_live_hand_follows_the_clip_the_ball_is_held_on() {
    // A pass's clip holds it in the left hand (state 4): the live hand
    // (actor+0x25c) becomes 2 while it is held there.
    let mut w = world();
    assert_eq!(w.players[0].gait_channel.hand, 1);
    w.players[0].action = Action::Passing {
        elapsed: 0.0,
        row: 0,
        tick: 12,
    };
    assert!(w.place_held(0, None));
    assert_eq!(w.players[0].gait_channel.hand, 2);
}

#[test]
fn the_catch_row_sets_the_receivers_live_hand() {
    // `FUN_001611d8`: the row's 0x400 the right hand, 0x200 the left.
    let mut w = world();
    let right = timing::CATCH_ROWS
        .iter()
        .position(|r| r.flags & 0x400 != 0)
        .unwrap();
    let left = timing::CATCH_ROWS
        .iter()
        .position(|r| r.flags & 0x600 == 0x200)
        .unwrap();
    w.start_catching(1, right);
    assert_eq!(w.players[1].gait_channel.hand, 1);
    w.start_catching(1, left);
    assert_eq!(w.players[1].gait_channel.hand, 2);
}

#[test]
fn a_low_slow_loose_ball_is_scooped_and_taken_at_callback_0() {
    // Within reach on the floor, at most 2.5 up and slow: the ball goes
    // free toward him at a fifteenth of the gap an update, he plays the
    // pickup clip (73), and callback 0 takes it 13 updates on.
    let mut w = world();
    w.ball.drop_loose();
    w.ball.position = [22.0, 0.5, 1.0];
    w.ball.velocity = [0.0; 3];
    w.ball.airborne = false;
    step(&mut w, 0);
    let Action::Scoop { play } = w.players[0].action else {
        panic!("{:?}", w.players[0].action);
    };
    assert_eq!(play.run.tick, 0);
    assert_eq!(w.ball.state, BallState::Loose);
    let hz = crate::TICK_HZ as f32;
    assert!(
        (w.ball.velocity[0] - (20.0 - 22.0) / 15.0 * hz).abs() < 0.5,
        "{:?}",
        w.ball.velocity
    );
    let mut taken = None;
    for k in 1..20 {
        step(&mut w, 0);
        if taken.is_none() && w.ball.state == BallState::Held(0) {
            taken = Some(k);
        }
    }
    assert_eq!(taken, Some(13));
    assert_eq!(w.players[0].action, Action::None);
}

#[test]
fn the_held_ball_state_survives_a_snapshot() {
    // The blend's stored offset, the script's start and a scoop under way.
    let mut w = world();
    step(&mut w, button::SHOOT);
    step(&mut w, button::SHOOT);
    assert!(w.held_anchor.is_some() || w.players[0].channel_start.is_some());
    let back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back.held_anchor, w.held_anchor);
    assert_eq!(back.players[0].channel_start, w.players[0].channel_start);
    let mut w = world();
    w.ball.drop_loose();
    w.ball.position = [22.0, 0.5, 1.0];
    w.ball.velocity = [0.0; 3];
    w.ball.airborne = false;
    step(&mut w, 0);
    assert!(matches!(w.players[0].action, Action::Scoop { .. }));
    let back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back.players[0].action, w.players[0].action);
}
