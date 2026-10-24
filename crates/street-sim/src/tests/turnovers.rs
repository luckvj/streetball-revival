//! The swipe's hand ([`crate::swipe`]) and the limbs a pass meets
//! ([`crate::pose::sweep`]) through `World::step`, on a made-up skeleton
//! that stands the same in every clip: the pelvis 3 up, the neck 2 above
//! it and the head 1 more, the hands 1 above the pelvis and 1.5 in front,
//! the legs straight down; every frame holds both hands out (state 0xc).

use super::*;
use crate::pose::{Bank, ClipData, Poses, SkeletonBone};
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
    // Every bone somewhere of its own, so no limb is empty.
    for (i, b) in skeleton.iter_mut().enumerate().skip(1) {
        b.translation = [0.0, -0.05 * i as f32, 0.0];
    }
    skeleton[20].translation = [0.0, 2.0, 0.0];
    skeleton[21] = SkeletonBone {
        parent: Some(20),
        translation: [0.0, 1.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
    };
    // The animations face -z.
    skeleton[15].translation = [0.3, 1.0, -1.5];
    skeleton[19].translation = [-0.3, 1.0, -1.5];
    for (k, b) in [1usize, 2, 3, 4].into_iter().enumerate() {
        skeleton[b].translation = [0.3, -0.6 * (k + 1) as f32, 0.0];
        skeleton[b + 4].translation = [-0.3, -0.6 * (k + 1) as f32, 0.0];
    }
    let clip = ClipData {
        raw_pose: false,
        bones: Vec::new(),
        rotations: vec![Vec::new(); 40],
        root: vec![[0.0, 2.5, 0.0]; 40],
        states: vec![0xc; 40],
    };
    Bank::Disc(Arc::new(
        Poses::new(&skeleton, vec![clip], vec![Some((0, false)); 1144]).unwrap(),
    ))
}

/// Holder 0 (team 0) at the middle facing +z, defender 1 (team 1) `ahead`
/// in front of him facing back.
fn swipe_world(seed: u32, ahead: f32) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(1, [0.0, ahead], std::f32::consts::PI),
        ],
        Rules::default(),
        seed,
    );
    w.poses = bank();
    w.phase = Phase::Live;
    w.take_ball(0);
    // Where the last ball update left it (ball+0x30), as a step would.
    assert!(w.place_held(0, None));
    w
}

fn press_steal(w: &mut World, i: usize) {
    let mut inputs = vec![Input::default(); w.players.len()];
    w.step(&inputs);
    inputs[i].buttons = button::TRICK;
    w.step(&inputs);
}

#[test]
fn a_scripted_swipe_tries_its_hand_every_update_until_it_reaches() {
    // Where the swiping hand meets the held ball: the defender's hands are
    // 1.5 in front at scale 1.2, the ball 1.5 in front of the holder.
    let reach = |ahead: f32| {
        let mut w = swipe_world(1, ahead);
        let s = w.start_swipe(1).unwrap();
        let poses = w.poses.arc().unwrap();
        let body = w.players[1].body();
        let pose = s.pose(&poses, body, w.place(1)).unwrap();
        let shown = s.shown();
        let state = poses.state(shown.record.unwrap(), shown.frame);
        crate::swipe::distance(&pose, shown, state, body, w.ball.position)
    };
    assert!(reach(3.6) < 1.0, "{}", reach(3.6));
    assert!(reach(9.0) > 1.5);
    // Out of reach: the hand is tried (the roll drawn each time) from the
    // update after the press, the script's first step having set bit 6,
    // to the update after its last, whose step clears it: every test in
    // the side's update, on the clock the last update left. It never
    // reaches; the swipe ends with its script.
    let mut w = swipe_world(3, 9.0);
    let draws = |w: &World| w.rng.clone();
    press_steal(&mut w, 1);
    // The swipe's look settled (its press began bit 6).
    w.settle_looks();
    let Action::Stealing { swipe: Some(s), .. } = w.players[1].action else {
        panic!("{:?}", w.players[1].action);
    };
    assert_eq!(s.tick, 1);
    assert!(!s.tested);
    let span = s.span();
    for _ in 1..=span {
        assert!(
            matches!(w.players[1].action, Action::Stealing { swipe: Some(s), .. } if !s.tested)
        );
        let mut rolled = draws(&w);
        rolled.next_u32();
        w.step(&[Input::default(); 2]);
        if w.players[1].action == Action::None {
            // The swipe's end (bit 6 falls) turns the swiper's look
            // (event 6: the target's draw and its time's).
            rolled.next_u32();
            rolled.next_u32();
        }
        assert_eq!(draws(&w), rolled, "one roll a test");
        // Every update of it is a valid world, its last (the clock at the
        // span) too: an online referee validates every update.
        w.validate().unwrap();
        World::from_snapshot(&w.to_snapshot()).unwrap();
    }
    assert_eq!(w.players[1].action, Action::None);
    assert_eq!(w.ball.state, BallState::Held(0));
    // In reach: the first test, the update after the press, uses the swipe
    // up, and with 99 Steals (84 in 100 land) it mostly knocks the ball
    // loose or takes it.
    let mut landed = 0;
    for seed in 0..40 {
        let mut w = swipe_world(seed, 3.6);
        w.players[1].ratings.steals = 1.0;
        press_steal(&mut w, 1);
        assert_eq!(w.ball.state, BallState::Held(0), "not on the press");
        if let Action::Stealing { swipe: Some(s), .. } = w.players[1].action {
            assert!(!s.tested);
        }
        w.step(&[Input::default(); 2]);
        if let Action::Stealing { swipe: Some(s), .. } = w.players[1].action {
            assert!(s.tested);
        }
        if w.ball.state != BallState::Held(0) {
            landed += 1;
        }
    }
    assert!((25..=40).contains(&landed), "{landed}");
}

#[test]
fn the_swipe_takes_its_script_from_the_balls_height_and_place() {
    // The ball 1.5 in front of the holder at the pelvis's height plus one
    // (3 + 1.2): within 1.5 of the swiper's pelvis, the middle swipe.
    let mut w = swipe_world(1, 3.6);
    let s = w.start_swipe(1).unwrap();
    assert_eq!(s.script, 3);
    assert_eq!(
        s.from,
        Some((
            w.players[1].gait_channel.row,
            w.players[1].gait_channel.phase
        ))
    );
    // Right behind the holder, facing his back within 5: the post-up steal.
    let mut w = swipe_world(1, -3.0);
    w.players[1].heading = 0.0;
    let s = w.start_swipe(1).unwrap();
    assert_eq!(s.script, crate::swipe::POST_UP);
    assert_eq!(s.record, 399);
    // Without the disc's animations, the stand-in.
    let mut w = swipe_world(1, 3.6);
    w.poses = Bank::StandIns;
    assert_eq!(w.start_swipe(1), None);
}

#[test]
fn a_pass_meets_a_limb_only_in_its_last_quarter_and_goes_loose() {
    // A pass from x -12 to x 12 at chest height; a defender with 99
    // Steals (15 in 100 an update) stands in its way at x 8, two thirds
    // of the way along (in its last quarter by the flight's count, the
    // pass dipping to the catch), another off it.
    let world = |seed: u32, z: f32| {
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [-12.0, 0.0], std::f32::consts::FRAC_PI_2),
                Player::new(0, [12.0, 0.0], -std::f32::consts::FRAC_PI_2),
                Player::new(1, [8.0, z], -std::f32::consts::FRAC_PI_2),
            ],
            Rules::default(),
            seed,
        );
        w.poses = bank();
        w.players[2].ratings.steals = 1.0;
        w.phase = Phase::Live;
        w.take_ball(0);
        w.throw(0, 1, PassStyle::Normal);
        w
    };
    let (mut knocked, mut caught) = (0, 0);
    for seed in 0..60 {
        let mut w = world(seed, 0.0);
        for _ in 0..120 {
            let flown = 1.0 - w.ball.flight_left / w.ball.flight_time.max(1e-6);
            let pass = w.ball.state == BallState::Pass;
            w.step(&[Input::default(); 3]);
            if w.events
                .iter()
                .any(|e| matches!(e, Event::Deflection { player: 2 }))
            {
                assert!(pass && flown > 0.75, "{flown}");
                assert_eq!(w.ball.state, BallState::Loose);
                knocked += 1;
                break;
            }
            if w.ball.state == BallState::Held(1) {
                caught += 1;
                break;
            }
        }
    }
    assert!(knocked > 0 && caught > 0, "{knocked} {caught}");
    // Off the line, never.
    for seed in 0..20 {
        let mut w = world(seed, 9.0);
        for _ in 0..120 {
            w.step(&[Input::default(); 3]);
            assert!(
                !w.events
                    .iter()
                    .any(|e| matches!(e, Event::Deflection { .. }))
            );
        }
    }
}
