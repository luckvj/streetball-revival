//! Blocks ([`crate::block`]) and the held ball's dribble
//! ([`crate::dribble`]) through `World::step`, on a made-up skeleton that
//! stands the same in every clip: the pelvis 3 up, the hands 1 above it and
//! 1.5 in front. The block records (437..465) play a clip whose frames
//! from 4 on carry the hands' block flags (0x240 with the right hand, 8).

use super::*;
use crate::block::{Jump, SCRIPTS, TURBO};
use crate::pose::{Bank, ClipData, Poses, SkeletonBone};
use std::sync::Arc;

fn bank() -> Bank {
    bank_dropping(0.0)
}

/// [`bank`] with the block clip's root `drop` lower from frame 5 on.
fn bank_dropping(drop: f32) -> Bank {
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
    // The animations face -z.
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
    let gait = still(vec![8; 40]);
    let mut block = still((0..50).map(|f| if f < 4 { 8 } else { 0x248 }).collect());
    for (f, root) in block.root.iter_mut().enumerate() {
        if f >= 5 {
            root[1] -= drop;
        }
    }
    let records = (0..1144)
        .map(|r| Some((u16::from((437..=465).contains(&r)), false)))
        .collect();
    Bank::Disc(Arc::new(
        Poses::new(&skeleton, vec![gait, block], records).unwrap(),
    ))
}

/// Team 0's holder at x 20 (it attacks +x), team 1's defender `ahead` in
/// front of him (+x) facing back.
fn world(seed: u32, ahead: f32) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [20.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(1, [20.0 + ahead, 0.0], -std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        seed,
    );
    w.poses = bank();
    w.phase = Phase::Live;
    w.take_ball(0);
    assert!(w.place_held(0, None));
    w
}

fn idle(w: &mut World) {
    let inputs = vec![Input::default(); w.players.len()];
    w.step(&inputs);
}

fn press_jump(w: &mut World, i: usize, buttons: u16) {
    let mut inputs = vec![Input::default(); w.players.len()];
    w.step(&inputs);
    inputs[i].buttons = buttons;
    w.step(&inputs);
}

fn block(w: &World, i: usize) -> Jump {
    match w.players[i].action {
        Action::Jumping { block: Some(j), .. } => j,
        ref a => panic!("{a:?}"),
    }
}

#[test]
fn a_block_jump_at_a_dribbler_leaves_the_floor_at_its_callback_35_and_lands_with_its_script() {
    // A holder in his gait (no shot-like bits): kind 0, 0x441db0, whose
    // callback 35 runs at tick 8 (a 4-update blend and a 4-update wait);
    // the jumper turns toward the ball 46 an update from the update after
    // the press, and the script ends when he lands.
    let mut w = world(1, 4.0);
    w.players[1].heading = 0.0;
    press_jump(&mut w, 1, button::JUMP);
    let j = block(&w, 1);
    assert_eq!(SCRIPTS[usize::from(j.script)].address, 0x441db0);
    assert_eq!(j.tick, 0);
    let target = j.word;
    let b = w.ball.position;
    assert_eq!(
        target,
        crate::contact::angle(w.players[1].position, [b[0], b[2]])
    );
    let mut words = vec![crate::timing::facing_word(w.players[1].heading)];
    for tick in 1..8 {
        idle(&mut w);
        assert_eq!(block(&w, 1).tick, tick);
        assert_eq!(w.players[1].height, 0.0, "tick {tick}");
        words.push(crate::timing::facing_word(w.players[1].heading));
    }
    for pair in words.windows(2) {
        let d = (pair[1] - pair[0] + 512).rem_euclid(1024) - 512;
        assert!(d.abs() <= crate::block::TURN, "{words:?}");
    }
    assert_eq!(*words.last().unwrap(), target, "{words:?}");
    idle(&mut w);
    let j = block(&w, 1);
    assert!(j.up && j.vy > 0.0 && j.meet >= 15, "{j:?}");
    assert!(w.players[1].height > 0.0);
    let mut updates = 0;
    while matches!(w.players[1].action, Action::Jumping { .. }) && updates < 200 {
        idle(&mut w);
        updates += 1;
    }
    assert_eq!(w.players[1].action, Action::None);
    assert_eq!(w.players[1].height, 0.0);
    assert!(updates > 10, "{updates}");
}

/// A shot in flight from player 0 (state 5), the ball set where player
/// 1's hand was posed last update (tick `tick` of `script`, in the air).
fn shot_at_hand(seed: u32, script: u8, offset: [f32; 3]) -> World {
    let mut w = world(seed, 2.0);
    let call = shooting::ShotCall {
        percent: 0,
        made: false,
        kind: 0,
        angle: std::f32::consts::FRAC_PI_4,
    };
    let shot = Shot {
        shooter: 0,
        hoop: 0,
        made: false,
        points: 1,
        gamebreaker: 0,
        layup: false,
        tried: 0,
        slam: None,
    };
    w.launch_shot(0, [20.0, 9.0, 0.0], call, shot, None);
    w.players[1].height = 2.0;
    let jump = Jump {
        script,
        branch: false,
        from: None,
        tick: 12,
        word: crate::timing::facing_word(w.players[1].heading),
        vy: 0.1,
        drift: [0.0; 2],
        up: true,
        meet: 15,
        airtime: 40,
        flown: 4,
        turbo: false,
    };
    w.players[1].action = Action::Jumping {
        elapsed: 0.2,
        high: false,
        block: Some(jump),
    };
    let poses = w.poses.arc().unwrap();
    let body = w.players[1].body();
    let pose = jump.pose(&poses, body, w.place(1)).unwrap();
    let shown = jump.shown();
    let hand = crate::pose::hand_point(
        &pose,
        i32::from(shown.record.unwrap()),
        shown.frame as i32,
        1,
        body,
        None,
    );
    w.ball.position = std::array::from_fn(|k| hand[k] + offset[k]);
    w.ball.moved = [0.0; 3];
    w
}

#[test]
fn a_hand_in_reach_knocks_a_shot_loose_and_counts_the_block() {
    // `FUN_0016d888`: the posed right hand within 1.0 of the shot (early
    // in its flight): knocked loose 0.35 an update along the jumper's
    // facing (turned up to 0x80) and 0.125 up, the block counted (slot
    // 16), the jumper and the shooter barred 12..26 updates (or the
    // jumper's airtime left).
    let mut knocked = 0;
    for seed in 0..12 {
        let mut w = shot_at_hand(seed, 5, [0.0, 0.5, 0.0]);
        idle(&mut w);
        assert!(
            w.events.iter().any(|e| matches!(
                e,
                Event::Block {
                    player: 1,
                    shooter: 0
                }
            )),
            "{:?}",
            w.events
        );
        knocked += 1;
        assert_eq!(w.ball.state, BallState::Loose);
        let v = w.ball.velocity.map(|c| c * TICK_SECONDS);
        assert!((crate::math::len(v[0], v[2]) - 0.35).abs() < 1e-3, "{v:?}");
        let g = crate::GRAVITY * TICK_SECONDS * TICK_SECONDS;
        assert!((v[1] - (0.125 - g)).abs() < 1e-3, "{v:?}");
        assert_eq!(w.stats.player(1, stats::BLOCKS), 1);
        assert!(
            (11..=40).contains(&w.players[1].barred),
            "{}",
            w.players[1].barred
        );
        assert!(
            (11..=26).contains(&w.players[0].barred),
            "{}",
            w.players[0].barred
        );
    }
    assert_eq!(knocked, 12);
    // Out of reach (1.5 off the hand): nothing.
    let mut w = shot_at_hand(1, 5, [0.0, 1.5, 0.0]);
    idle(&mut w);
    assert_eq!(w.ball.state, BallState::Shot);
    assert!(!w.events.iter().any(|e| matches!(e, Event::Block { .. })));
}

#[test]
fn a_shot_on_the_rim_inside_the_ring_is_not_touched() {
    // Ball state 6 within 0.793225 across of the rim's middle: left alone,
    // however near the hand. Outside the ring the hand takes it on the
    // roll `(2 Rebounds + Blocks + Speed + Power) / 5` (here 90 in 100).
    let on_rim = |seed: u32, off: f32| {
        let mut w = shot_at_hand(seed, 5, [0.0, 0.5, 0.0]);
        w.ball.touched = true;
        w.players[1].ratings.rebounds = 1.0;
        w.players[1].ratings.blocks = 1.0;
        w.players[1].ratings.speed = 1.0;
        w.players[1].ratings.dunks = 1.0;
        let rim = w.rim_spot(0);
        let hand = |w: &World| {
            let jump = block(w, 1);
            let poses = w.poses.arc().unwrap();
            let body = w.players[1].body();
            let pose = jump.pose(&poses, body, w.place(1)).unwrap();
            let shown = jump.shown();
            crate::pose::hand_point(&pose, 446, shown.frame as i32, 1, body, None)
        };
        // His hand over the ring's middle, then `off` toward the court.
        let h = hand(&w);
        w.players[1].position[0] += rim[0] - h[0] - off;
        w.players[1].position[1] += rim[1] - h[2];
        let h = hand(&w);
        w.ball.position = [h[0], h[1] + 0.5, h[2]];
        assert_eq!(w.native_ball_state(), crate::game_phase::ball::RIM);
        w.block_test(1);
        w.ball.state
    };
    for seed in 0..10 {
        assert_eq!(on_rim(seed, 0.0), BallState::Shot, "seed {seed}");
        assert_eq!(on_rim(seed, 0.5), BallState::Shot, "seed {seed}");
    }
    let taken = (0..20)
        .filter(|&seed| on_rim(seed, 1.2) == BallState::Loose)
        .count();
    assert!((14..=20).contains(&taken), "{taken}");
}

#[test]
fn the_turbo_block_takes_half_a_bar_and_catches_what_it_reaches() {
    // Two turbos at the press: 0x441bd5 for half the bar; with less than
    // half, no jump at all.
    let turbos = button::JUMP | button::L1 | button::R1;
    let mut w = world(1, 4.0);
    w.set_turbo(1, 0.4);
    press_jump(&mut w, 1, turbos);
    assert_eq!(w.players[1].action, Action::None);
    let mut w = world(1, 4.0);
    w.set_turbo(1, 1.0);
    press_jump(&mut w, 1, turbos);
    let j = block(&w, 1);
    assert_eq!(j.script, TURBO);
    assert!(j.turbo);
    assert!(w.players[1].turbo <= 0.5 + 1e-3, "{}", w.players[1].turbo);
    // Its clip (437) with the turbo flag catches the ball it reaches.
    let mut w = shot_at_hand(2, TURBO, [0.0, 0.4, 0.0]);
    if let Action::Jumping { block: Some(j), .. } = &mut w.players[1].action {
        j.turbo = true;
    }
    let jump = block(&w, 1);
    let poses = w.poses.arc().unwrap();
    let body = w.players[1].body();
    let pose = jump.pose(&poses, body, w.place(1)).unwrap();
    let shown = jump.shown();
    let hand = crate::pose::hand_point(&pose, 437, shown.frame as i32, 1, body, None);
    w.ball.position = [hand[0], hand[1] + 0.4, hand[2]];
    idle(&mut w);
    assert_eq!(w.ball.state, BallState::Held(1));
}

#[test]
fn a_block_jump_survives_a_snapshot() {
    let mut w = world(4, 4.0);
    press_jump(&mut w, 1, button::JUMP);
    for _ in 0..10 {
        idle(&mut w);
    }
    let mut copy = World::from_snapshot(&w.to_snapshot()).expect("reads back");
    copy.poses = w.poses.clone();
    assert_eq!(copy, w);
    for _ in 0..60 {
        idle(&mut w);
        idle(&mut copy);
        assert_eq!(copy.checksum(), w.checksum());
    }
    assert_eq!(copy, w);
}

#[test]
fn the_dribble_falls_from_the_release_hand_to_the_floor_and_rises_to_the_catch() {
    // A gait clip that lets the ball go at frame 3 (0x10) and takes it
    // back at frame 7 (0x20): between them the ball moves from hand to
    // hand, its height down to the radius at the fraction dA / (dA + dB /
    // 0.8) of the way and back up.
    let mut skeleton = vec![
        SkeletonBone {
            parent: Some(0),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        crate::pose::BONES
    ];
    skeleton[0].parent = None;
    skeleton[19].translation = [-0.3, 1.0, -1.5];
    skeleton[15].translation = [0.3, 1.0, -1.5];
    let states = vec![8, 8, 8, 8 | 0x10, 0, 0, 0, 8 | 0x20, 8, 8];
    let clip = ClipData {
        raw_pose: false,
        bones: Vec::new(),
        rotations: vec![Vec::new(); states.len()],
        root: vec![[0.0, 2.5, 0.0]; states.len()],
        states,
    };
    let poses = Poses::new(&skeleton, vec![clip], vec![Some((0, false)); 1144]).unwrap();
    let body = crate::pose::Body::uniform(1.0);
    let place = crate::pose::Place {
        position: [0.0, 0.0, 0.0],
        word: 0,
    };
    let r = ball::RADIUS;
    let at = |time: f32| crate::dribble::ball_point(&poses, 9, time, body, place, r).unwrap();
    let hand = at(3.0);
    assert!(crate::dribble::dribbles(&poses, 9));
    // In the hand on the release frame itself and from the catch on.
    assert_eq!(at(7.0), hand);
    assert_eq!(at(8.5), hand);
    // Between: straight down to the floor at 4/9 of the way (both hands at
    // the same height), then up.
    let fall = 1.0 / (1.0 + 1.0 / ball::FLOOR_RESTITUTION);
    let floor_frame = 3.0 + 4.0 * fall;
    assert!(
        (at(floor_frame)[1] - r).abs() < 1e-4,
        "{:?}",
        at(floor_frame)
    );
    let quarter = at(3.0 + 4.0 * fall * 0.5)[1];
    assert!((quarter - (hand[1] + r) * 0.5).abs() < 1e-4, "{quarter}");
    assert!(at(5.0)[1] > r && at(5.0)[1] < hand[1]);
    // Across: the hands are the same, so the ball keeps under them.
    assert!((at(5.0)[0] - hand[0]).abs() < 1e-5);
}

#[test]
fn the_takeoff_rises_by_the_clips_root_drop_until_a_whole_frame() {
    // Every block script sets channel flag 0x10000 just before its aim
    // (`91 02 00 01 00`): the channel's steps lower the jumper by the
    // clip's root rise until one reaches a new whole frame. On 0x441db0
    // the aim's update steps frame 4 to 6.4 (the meet's wait), so a clip
    // whose root drops 0.3 from frame 5 lifts him 0.3 times his height
    // scale on that update, beside his velocity; the next update moves
    // him by his velocity alone.
    let mut w = world(1, 4.0);
    w.poses = bank_dropping(0.3);
    w.players[1].heading = 0.0;
    press_jump(&mut w, 1, button::JUMP);
    assert_eq!(SCRIPTS[usize::from(block(&w, 1).script)].address, 0x441db0);
    for _ in 1..=8 {
        idle(&mut w);
    }
    let j = block(&w, 1);
    assert!(j.up && j.tick == 8, "{j:?}");
    let g = crate::toss::JUMP_GRAVITY;
    let h = w.players[1].body().height;
    let first = w.players[1].height;
    let vy = j.vy + g;
    assert!(
        (first - (vy - 0.5 * g + 0.3 * h)).abs() < 1e-4,
        "{first} {vy} {h}"
    );
    let (before, vy) = (first, j.vy);
    idle(&mut w);
    let second = w.players[1].height;
    assert!((second - (before + vy - 0.5 * g)).abs() < 1e-4, "{second}");
}

#[test]
fn the_top_bone_is_the_highest_of_the_hands_and_the_head_as_posed() {
    // `FUN_00181f50`: the highest of bones 15, 19 and 21 of the posed
    // model. On the made-up skeleton the head stands 5.5 above the feet
    // (the pelvis 2.5, the neck 2 more, the head 1) times the height
    // scale, over the hands' 3.5; a block jumper's comes from his jump's
    // clip, at his height off the floor.
    let mut w = world(1, 4.0);
    let h = w.players[1].body().height;
    let top = w.top_bone(1).unwrap();
    assert!((top - 5.5 * h).abs() < 1e-4, "{top} {h}");
    w.players[1].heading = 0.0;
    press_jump(&mut w, 1, button::JUMP);
    for _ in 1..=12 {
        idle(&mut w);
    }
    let y = w.players[1].height;
    assert!(y > 0.0);
    let top = w.top_bone(1).unwrap();
    assert!((top - (y + 5.5 * h)).abs() < 1e-4, "{top} {y} {h}");
    w.poses = crate::pose::Bank::StandIns;
    assert_eq!(w.top_bone(1), None);
}
