//! The ball on the bones ([`crate::touch`]) through `World::step`: the
//! receiver's catch row and his hands, the knocks off a body, the floor's
//! ball and a rebound taken in the actor's update, the release's bars, and
//! the ball's leftovers (the basket outside a shot, the board-disable
//! record, the reversed lay-up spin). The disc's skeleton where the test
//! needs real hands (skipped without `STREET_ISO`); otherwise a made-up one
//! that stands the same in every clip.

use super::*;
use crate::pose::{Bank, ClipData, Poses, SkeletonBone};
use std::sync::Arc;

/// A made-up skeleton (the one `tests::turnovers` uses: the pelvis 3 up,
/// the neck 2 above it and the head 1 more, the hands 1 above the pelvis
/// and 1.5 in front, the legs straight down), every frame holding out the
/// hands of `state` (8 the right, 4 the left).
fn bank(state: u32) -> Bank {
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
    let clip = ClipData {
        raw_pose: false,
        bones: Vec::new(),
        rotations: vec![Vec::new(); 40],
        root: vec![[0.0, 2.5, 0.0]; 40],
        states: vec![state; 40],
    };
    Bank::Disc(Arc::new(
        Poses::new(&skeleton, vec![clip], vec![Some((0, false)); 1144]).unwrap(),
    ))
}

fn idle(w: &mut World) {
    let inputs = vec![Input::default(); w.players.len()];
    w.step(&inputs);
}

/// Passer 0 at x -6 and receiver 1 at `at` facing `heading` (team 0), a
/// defender far away, on the disc's skeleton.
fn pass_world(seed: u32, at: [f32; 2], heading: f32) -> Option<World> {
    let bank = crate::pose::disc_bank();
    bank.get()?;
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-6.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(0, at, heading),
            Player::new(1, [-30.0, 20.0], 0.0),
        ],
        Rules::default(),
        seed,
    );
    w.poses = bank;
    w.phase = Phase::Live;
    w.take_ball(0);
    Some(w)
}

#[test]
fn a_standing_receivers_hands_take_the_pass_before_it_arrives() {
    // The release aims at the catch row's hand posed where he stands
    // (`FUN_001611d8`) and starts the row's script; his tail sweeps the
    // ball's last move against his last update's bones (`FUN_00137db8`)
    // and the forearm and hand his clip holds out take it: in the update it
    // arrives or a few before (the captures' flight less progress, 0..5).
    let throws = [
        ([12.0, 0.0], -std::f32::consts::FRAC_PI_2),
        ([8.0, 6.0], std::f32::consts::PI),
        ([0.0, 10.0], 0.0),
        ([-4.0, -12.0], 0.3),
    ];
    let mut early = 0;
    for (at, heading) in throws {
        let Some(mut w) = pass_world(3, at, heading) else {
            return;
        };
        w.throw(0, 1, crate::passing::PassStyle::Normal);
        let pass = w.ball.pass.unwrap();
        assert!(usize::from(pass.catch) < crate::timing::CATCH_ROWS.len());
        assert!(w.catching.is_some_and(|c| c.player == 1));
        // The passer is barred for the flight, the receiver free.
        assert_eq!(i32::from(w.players[0].barred), w.ball.flight_ticks());
        assert_eq!(w.players[1].barred, 0);
        let mut caught = None;
        for _ in 0..80 {
            let left = w.ball.flight_ticks() - w.ball.flown_ticks();
            idle(&mut w);
            if w.events
                .iter()
                .any(|e| matches!(e, Event::Catch { player: 1 }))
            {
                caught = Some(left);
                break;
            }
        }
        let left = caught.expect("the receiver's hands take it");
        assert!((0..=5).contains(&left), "{left}");
        early += usize::from(left >= 1);
        assert_eq!(w.ball.state, BallState::Held(1));
        assert!(w.catching.is_none());
        assert!(w.catch.is_some_and(|c| c.player == 1));
    }
    assert!(early >= 2, "{early}");
}

#[test]
fn a_catching_receiver_stops_steering_and_coasts_on_a_running_row() {
    // Every catch row takes the receiver from his stick: a standing row's
    // `97` zeroes his velocity; on a running row (callback 30) he coasts
    // on the velocity he had (the played captures' receivers keep theirs
    // to the catch).
    let Some(mut w) = pass_world(5, [10.0, 0.0], std::f32::consts::FRAC_PI_2) else {
        return;
    };
    w.throw(0, 1, crate::passing::PassStyle::Normal);
    let row = usize::from(w.ball.pass.unwrap().catch);
    w.catching = None;
    w.start_catching(1, row);
    let drift = [0.25, 0.0];
    if let Some(c) = &mut w.catching {
        c.row = 1; // catch_run_1: callback 30, no `97`
    }
    w.players[1].speed = 15.0;
    w.players[1].course = std::f32::consts::FRAC_PI_2;
    let before = w.players[1].position;
    // Steer him hard the other way: the stick no longer counts.
    let mut inputs = vec![Input::default(); 3];
    inputs[1].stick = [-127, 0];
    w.step(&inputs);
    let c = w.catching.expect("still catching");
    assert!(!c.holds());
    assert!(
        (c.drift[0] - drift[0]).abs() < 1e-4 && c.drift[1].abs() < 1e-4,
        "{:?}",
        c.drift
    );
    w.step(&inputs);
    let moved = w.players[1].position[0] - before[0];
    assert!((moved - 0.5).abs() < 1e-3, "{moved}");
}

/// The made-up world for the receiver's tail: passer 0 at x -12 and
/// receiver 1 at x 12 facing him (team 0), a defender far off, the pass
/// thrown and the receiver's row started (its bit 9 set), every clip
/// holding out the hands of `state`.
fn tail_world(state: u32) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-12.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(0, [12.0, 0.0], -std::f32::consts::FRAC_PI_2),
            Player::new(1, [0.0, 20.0], 0.0),
        ],
        Rules::default(),
        3,
    );
    w.poses = bank(state);
    w.phase = Phase::Live;
    w.take_ball(0);
    w.throw(0, 1, crate::passing::PassStyle::Normal);
    for _ in 0..2 {
        idle(&mut w);
    }
    assert!(
        w.catching
            .is_some_and(|c| c.player == 1 && c.catching && !c.parked)
    );
    assert_eq!(w.ball.state, BallState::Pass);
    w
}

/// Puts the ball where its last move crosses receiver 1's left forearm
/// (the capsule from bone 14 to the left hand, pushed 1.0 past it) more
/// than [`crate::touch::NEAR`] from his pelvis and both hands.
fn past_the_left_hand(w: &mut World) {
    let pose = w.actor_pose(1).unwrap();
    let (elbow, hand) = (
        pose.joints[14].position,
        pose.joints[crate::pose::LEFT_HAND].position,
    );
    let d: [f32; 3] = std::array::from_fn(|k| hand[k] - elbow[k]);
    let l = crate::pose::length(d);
    let d = d.map(|c| c / l);
    let side = [-d[2], 0.0, d[0]];
    let side = side.map(|c| c / (side[0] * side[0] + side[2] * side[2]).sqrt());
    let at: [f32; 3] = std::array::from_fn(|k| hand[k] + d[k] * 0.9 + side[k] * 0.65);
    for k in [
        crate::pose::PELVIS,
        crate::pose::LEFT_HAND,
        crate::pose::RIGHT_HAND,
    ] {
        let gap: [f32; 3] = std::array::from_fn(|q| at[q] - pose.joints[k].position[q]);
        assert!(crate::pose::length(gap) > crate::touch::NEAR, "{k}");
    }
    w.ball.position = at;
    w.ball.moved = side.map(|c| -c * 0.02);
    w.ball.velocity = w.ball.moved.map(|c| c * TICK_HZ as f32);
}

#[test]
fn a_receiver_on_his_row_is_not_swept_while_the_pass_flies() {
    // `0x15bf3c..0x15bf6c`: with bit 9 (his catch row's) and the pass still
    // in flight (ball+0x60 below +0x64), the tail stops before the sweep:
    // a ball crossing his left forearm is left alone.
    let mut w = tail_world(8);
    past_the_left_hand(&mut w);
    let before = w.ball.clone();
    w.touch_ball(1);
    assert_eq!(w.ball, before);
    assert!(w.events.is_empty(), "{:?}", w.events);
    assert_eq!(w.players[1].cooldown, 0);
}

#[test]
fn a_pass_meeting_another_limb_after_its_flight_is_knocked_off_the_receiver() {
    // `0x15c2fc..0x15c3c4`: past the flight's end the tail sweeps him; he
    // catches only with the forearm and hand of the hand his clip holds
    // out (here the right, state 8); the left forearm met knocks it off
    // his body (`FUN_00137ed8`) and starts his cooldown (+0x1cc).
    let mut w = tail_world(8);
    past_the_left_hand(&mut w);
    w.ball.flight_left = 0.0;
    w.touch_ball(1);
    assert!(w.events.contains(&Event::Deflection { player: 1 }));
    assert_eq!(w.ball.state, BallState::Loose);
    assert_eq!(w.players[1].cooldown, crate::touch::COOLDOWN);
    // Holding out the left hand (state 4), the same ball is caught.
    let mut w = tail_world(4);
    past_the_left_hand(&mut w);
    w.ball.flight_left = 0.0;
    w.touch_ball(1);
    assert_eq!(w.ball.state, BallState::Held(1));
    assert!(w.events.contains(&Event::Catch { player: 1 }));
}

#[test]
fn the_near_take_catches_by_the_pelvis_and_bars_the_other_side() {
    // `0x15bad0..0x15bf28`: in his catch row (bit 9) the ball within 1.0 of
    // his pelvis or either hand is his (`FUN_0014cb08`), whatever hands
    // the clip holds out; with bit 9 the other side's three are barred
    // 20 updates (+0x1a8).
    let mut w = tail_world(0);
    let pose = w.actor_pose(1).unwrap();
    let pelvis = pose.joints[crate::pose::PELVIS].position;
    w.ball.position = [pelvis[0] - 0.9, pelvis[1], pelvis[2]];
    w.ball.moved = [0.3, 0.0, 0.0];
    w.touch_ball(1);
    assert_eq!(w.ball.state, BallState::Held(1));
    assert!(w.events.contains(&Event::Catch { player: 1 }));
    assert_eq!(w.players[2].barred, crate::touch::CATCH_BAR);
    // 1.1 from the pelvis and further from the hands: not yet.
    let mut w = tail_world(0);
    let pose = w.actor_pose(1).unwrap();
    let pelvis = pose.joints[crate::pose::PELVIS].position;
    w.ball.position = [pelvis[0] - 1.1, pelvis[1] - 1.0, pelvis[2]];
    w.ball.moved = [0.3, 0.0, 0.0];
    w.touch_ball(1);
    assert_eq!(w.ball.state, BallState::Pass);
}

#[test]
fn a_loose_ball_in_reach_is_taken_in_the_actors_update() {
    // `FUN_0016cd28`: within 3 across and at his body's height (his feet
    // to his head or hands, one more either way) the floor's ball is his,
    // in his own update before the ball's (the rewrite's pickup ran after
    // it, an update late). Above 2.5 (a lower, slow ball is scooped,
    // `tests::held`).
    let world = |x: f32| {
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [0.0, 0.0], 0.0),
                Player::new(1, [0.0, 20.0], 0.0),
            ],
            Rules::default(),
            1,
        );
        w.poses = bank(0xc);
        w.phase = Phase::Live;
        w.ball = crate::Ball::loose_at([x, 3.0, 0.0]);
        w.ball.velocity = [2.0, 0.0, 0.0];
        w
    };
    let mut w = world(2.5);
    idle(&mut w);
    assert_eq!(w.ball.state, BallState::Held(0));
    assert!(w.events.contains(&Event::Pickup { player: 0 }));
    assert!(!w.events.iter().any(|e| matches!(e, Event::Rebound { .. })));
    assert_eq!(w.stats.player(0, crate::stats::REBOUNDS), 0);
    // Taken where the last update left it, before the ball's update: one
    // rolling out of reach this update (2.9 across, a unit an update) is
    // still his.
    let mut w = world(2.9);
    w.ball.velocity = [60.0, 0.0, 0.0];
    idle(&mut w);
    assert_eq!(w.ball.state, BallState::Held(0));
    // 3.5 across: not his.
    let mut w = world(3.5);
    idle(&mut w);
    assert_eq!(w.ball.state, BallState::Loose);
    // In the air, not his either.
    let mut w = world(2.5);
    w.players[0].height = 1.0;
    idle(&mut w);
    assert_eq!(w.ball.state, BallState::Loose);
}

#[test]
fn a_rebound_meeting_a_body_on_the_floor_is_caught() {
    // `0x15e6f8..0x15e878`: a rebound (ball state 0) swept into the body
    // of an actor on the floor is his; the defence takes it only so (no
    // pickup reach for a rebound), and it is a rebound.
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-20.0, 0.0], 0.0),
            Player::new(1, [0.0, 0.0], 0.0),
        ],
        Rules::default(),
        2,
    );
    w.poses = bank(0xc);
    w.phase = Phase::Live;
    w.possession = 0;
    // Through the defender's trunk (the pelvis 3 up, the neck 5).
    w.ball = crate::Ball::loose_at([0.0, 4.0, 0.0]);
    w.ball.rebound = true;
    w.ball.moved = [-1.2, 0.0, 0.0];
    w.ball.velocity = [-60.0, 0.0, 0.0];
    w.stats.rebound = true;
    idle(&mut w);
    assert_eq!(w.ball.state, BallState::Held(1));
    assert!(w.events.contains(&Event::Pickup { player: 1 }));
    assert_eq!(
        w.events
            .iter()
            .filter(|e| **e == Event::Rebound { player: 1 })
            .count(),
        1
    );
    assert_eq!(w.stats.player(1, crate::stats::REBOUNDS), 1);
    assert_eq!(w.possession, 1);
}

#[test]
fn a_shot_meeting_a_defenders_body_is_knocked_off_it_and_counted() {
    // A shot (5) swept into a defender's body goes off it (`FUN_00137ed8`)
    // and free (`FUN_0014c408`: loose, the block posted for the side
    // without the ball); the shooter's own tests are barred `fps / 3`
    // updates from the release (callback 21).
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [20.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(1, [24.0, 0.0], -std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        4,
    );
    w.poses = bank(0xc);
    w.phase = Phase::Live;
    w.take_ball(0);
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
    assert_eq!(w.players[0].barred, crate::shooting::SHOOTER_BAR);
    // The ball's last move ends in the defender's trunk.
    w.ball.position = [24.0, 4.5, 0.0];
    w.ball.moved = [1.0, 0.2, 0.0];
    idle(&mut w);
    assert!(
        w.events.contains(&Event::Block {
            player: 1,
            shooter: 0
        }),
        "{:?}",
        w.events
    );
    assert_eq!(w.ball.state, BallState::Loose);
    // Set in his tests, counted down once at the end of his update
    // (`0x157300`).
    assert_eq!(w.players[1].cooldown, crate::touch::COOLDOWN - 1);
}

#[test]
fn the_release_bars_the_passers_side_and_on_a_long_pass_the_defence() {
    // `FUN_00173570`: the passer and his other teammates may not touch the
    // ball for the flight, the receiver's bar is lifted; a pass of 15 or
    // more from a passer on the floor bars the other side 35% of it.
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-12.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(0, [12.0, 0.0], -std::f32::consts::FRAC_PI_2),
            Player::new(0, [0.0, 10.0], 0.0),
            Player::new(1, [0.0, -10.0], 0.0),
        ],
        Rules::default(),
        6,
    );
    w.poses = bank(0xc);
    w.phase = Phase::Live;
    w.players[1].barred = 7;
    w.take_ball(0);
    w.throw(0, 1, crate::passing::PassStyle::Normal);
    let n = w.ball.flight_ticks();
    assert_eq!(i32::from(w.players[0].barred), n);
    assert_eq!(i32::from(w.players[2].barred), n);
    assert_eq!(w.players[1].barred, 0);
    assert_eq!(i32::from(w.players[3].barred), (n as f32 * 0.35) as i32);
}

#[test]
fn a_loose_ball_through_the_ring_scores_for_the_side_attacking_it() {
    // `FUN_001515a8`'s post outside a shot (event 0x55): the side whose
    // ring it is scores, credited to its last player on the ball; a long
    // shot's kind (3) on that side makes it the long shot's points.
    let world = |long: bool| {
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [0.0, 0.0], 0.0),
                Player::new(0, [5.0, 0.0], 0.0),
                Player::new(1, [-10.0, 0.0], 0.0),
            ],
            Rules::default(),
            8,
        );
        w.phase = Phase::Live;
        let rim = w.hoops[target_hoop(0)].rim;
        w.ball = crate::Ball::loose_at([rim[0], rim[1] + 0.4, rim[2]]);
        w.ball.velocity = [0.0, -15.0, 0.0];
        w.ball.airborne = true;
        w.ball.touch = [Some(1), Some(2)];
        w.ball.long = [long, false];
        w
    };
    for (long, points) in [(false, 1), (true, 2)] {
        let mut w = world(long);
        for _ in 0..6 {
            idle(&mut w);
            if !w.events.is_empty() {
                break;
            }
        }
        assert!(
            w.events.iter().any(|e| matches!(
                e,
                Event::Score {
                    team: 0,
                    player: 1,
                    points: p,
                    ..
                } if *p == points
            )),
            "{:?}",
            w.events
        );
        assert_eq!(w.score, [points, 0]);
    }
}

#[test]
fn a_glass_leg_passes_through_its_sides_hoop() {
    // The board-disable record (`FUN_00148760`): while a trick pass's leg
    // off the glass of a side aims 13.25 or more high, that side's
    // backboard, collar, rim and basket meet nothing; the other side's
    // still do.
    let rims = crate::ball::native_rims();
    let rim = rims[0];
    let drop = |off: Option<f32>| {
        let mut b = crate::Ball::loose_at([rim[0], rim[1] + 0.5, rim[2]]);
        b.velocity = [0.0, -20.0, 0.0];
        b.airborne = true;
        let mut scored = false;
        for _ in 0..4 {
            scored |= b
                .step(
                    rims,
                    crate::ball::Step {
                        board_off: off,
                        ..Default::default()
                    },
                )
                .scored;
        }
        scored
    };
    assert!(drop(None));
    assert!(!drop(Some(1.0)));
    assert!(drop(Some(-1.0)));
}

#[test]
fn a_reversed_layup_spins_the_other_way() {
    // `0x14ac20..0x14ac48`: a lay-up (bit 13) of family 2 whose kind's word
    // at `0x43f6e0` has bit 0 leaves with its backspin turned round.
    let reversed: Vec<u8> = crate::touch::LAYUP_FLAGS
        .iter()
        .enumerate()
        .filter(|(_, f)| *f & 1 != 0)
        .map(|(k, _)| k as u8)
        .collect();
    assert_eq!(reversed, [3, 12, 13, 20, 22]);
    let mut w = face_off(1);
    let row = crate::timing::MOVE_ROWS
        .iter()
        .position(|r| r.family == 2 && r.id == 3)
        .unwrap();
    w.players[0].action = Action::Layup {
        hoop: 0,
        elapsed: 0.0,
        from: [0.0; 2],
        to: [1.0, 0.0],
        tick: 0,
        release: 10,
        end: 20,
        row: row as u8,
    };
    assert!(w.layup_reversed(0));
    let plain = crate::timing::MOVE_ROWS
        .iter()
        .position(|r| r.family == 2 && r.id == 1)
        .unwrap();
    if let Action::Layup { row, .. } = &mut w.players[0].action {
        *row = plain as u8;
    }
    assert!(!w.layup_reversed(0));
    let mut a = crate::Ball::loose_at([0.0, 8.0, 0.0]);
    a.velocity = [10.0, 5.0, 0.0];
    let mut b = a.clone();
    a.shot_spin(false);
    b.shot_spin(true);
    assert_eq!(a.spin, b.spin);
    assert_eq!(a.spin_axis, b.spin_axis.map(|c| -c));
}

#[test]
fn a_live_player_on_the_court_stays_within_its_bounds() {
    // `FUN_0016f900(actor, 0)`: once on the court, within ±40.5 by ±25.
    let Some(mut w) = pass_world(1, [39.0, 24.0], std::f32::consts::FRAC_PI_2) else {
        return;
    };
    let mut inputs = vec![Input::default(); 3];
    inputs[1].stick = [127, 127];
    for _ in 0..90 {
        w.step(&inputs);
    }
    let p = w.players[1].position;
    assert!(p[0] <= 40.5 + 1e-4 && p[1].abs() <= 25.0 + 1e-4, "{p:?}");
}

#[test]
fn a_row_with_its_own_count_waits_for_it_and_spends_the_receivers_count() {
    // `0x15a974..0x15ab4c`: the parked script starts once the flight left
    // is at most his +0x1b0 (here `catch_stand_5`'s own 15) and sets +0x1b0
    // to -1, which keeps the rows with a shortest flight out of his next
    // pass's draw (the filter's `0x162028`).
    let Some(mut w) = pass_world(4, [20.0, 0.0], -std::f32::consts::FRAC_PI_2) else {
        return;
    };
    w.throw(0, 1, crate::passing::PassStyle::Normal);
    let n = w.ball.flight_ticks();
    assert!(n > 16, "{n}");
    let row = crate::timing::CATCH_ROWS
        .iter()
        .position(|r| r.name == "catch_stand_5")
        .unwrap();
    w.start_catching(1, row);
    w.players[1].received_flight = i32::from(crate::timing::CATCH_ROWS[row].min_flight);
    loop {
        let left = w.ball.flight_ticks() - w.ball.flown_ticks();
        idle(&mut w);
        let Some(c) = w.catching else {
            panic!("the row went before it started");
        };
        if !c.parked {
            assert!(left <= 15, "{left}");
            assert!(c.catching);
            break;
        }
        assert!(left > 15, "{left}");
        assert_eq!(w.actor_state_bits(1) & 0x200, 0);
    }
    assert_eq!(w.players[1].received_flight, -1);
    let rows = crate::timing::catch_candidates(&w, 1, 0, [-6.0, 0.0], -1, false);
    assert!(
        rows.iter()
            .all(|&r| crate::timing::CATCH_ROWS[r].min_flight < 1),
        "{rows:?}"
    );
}

#[test]
fn a_running_receiver_plays_his_row_over_his_run() {
    // `0x15a9d0..0x15aadc`: a receiver moving when the row starts, on the
    // floor at the release (+0x1ac), keeps his gait on channel 0 and plays
    // the row on channel 2 (bones 9..21): his legs are the gait's clip,
    // carried on at its rate, his arms the row's.
    let Some(mut w) = pass_world(6, [8.0, 8.0], 0.0) else {
        return;
    };
    let mut inputs = vec![Input::default(); 3];
    inputs[1].stick = [0, -127];
    for _ in 0..20 {
        w.step(&inputs);
    }
    assert!(w.players[1].speed > 0.0);
    // A pass that leads him (the spot pass's receiver runs to the spot and
    // stops there, [`crate::receiver`]).
    let mut aim = w.pass_aim(0, 1);
    aim.lead = true;
    let start = w.floor_positions();
    w.throw_aimed(0, 1, crate::passing::PassStyle::Normal, aim, &start);
    w.step(&inputs);
    let c = w.catching.expect("catching");
    let (record, time, rate) = c.lower.expect("over his run");
    assert!(rate > 0.0);
    w.step(&inputs);
    let c2 = w.catching.expect("still catching");
    assert!((c2.lower.unwrap().1 - (time + rate)).abs() < 1e-5);
    let poses = w.poses.arc().unwrap();
    let (body, place) = (w.players[1].body(), w.place(1));
    let pose = w.actor_pose(1).unwrap();
    let gait = poses
        .pose(
            crate::pose::Frame {
                record,
                time: time + rate,
                looping: true,
            },
            body,
            place,
        )
        .unwrap();
    for k in 0..crate::touch::UPPER_BODY {
        let d: [f32; 3] =
            std::array::from_fn(|q| pose.joints[k].position[q] - gait.joints[k].position[q]);
        assert!(crate::pose::length(d) < 1e-4, "{k}");
    }
    let mut back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back, w);
    back.poses = w.poses.clone();
    for _ in 0..10 {
        w.step(&inputs);
        back.step(&inputs);
    }
    assert_eq!(back, w);
}

#[test]
fn a_catch_row_in_play_survives_a_snapshot() {
    let Some(mut w) = pass_world(9, [10.0, 4.0], std::f32::consts::PI) else {
        return;
    };
    w.throw(0, 1, crate::passing::PassStyle::Normal);
    for _ in 0..8 {
        idle(&mut w);
    }
    assert!(w.catching.is_some());
    w.players[2].cooldown = 4;
    w.ball.touch = [Some(0), None];
    let mut back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back, w);
    back.poses = w.poses.clone();
    for _ in 0..30 {
        idle(&mut w);
        idle(&mut back);
    }
    assert_eq!(back, w);
}

#[test]
fn a_spot_pass_stops_its_receiver_at_the_spot_on_catch_stand_1() {
    // The release (`FUN_001611d8`, `0x161ee0..0x161f1c`) sets the spot run
    // for a pass to the spot and parks no row (`0x162420..0x162450`); in
    // the flight his update (`0x15a708..0x15ab50`) stops him within 1 of
    // the spot (2 moving), facing the ball, on `catch_stand_1` at once.
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-6.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(0, [10.0, 4.0], 0.0),
            Player::new(1, [-30.0, 20.0], 0.0),
        ],
        Rules::default(),
        3,
    );
    w.poses = bank(8);
    w.phase = Phase::Live;
    w.take_ball(0);
    // The spot half a unit ahead of him (within 0x33 of his facing, so
    // the release keeps it a pass to the spot, `World::release_flight`).
    let aim = crate::passing::PassAim {
        spot: [10.0, 4.5],
        lead: false,
        reach: 0.0,
    };
    // The press keeps the spot (`0x15fd3c`), the release the run.
    w.spot_at_press(0, 1, &aim);
    let start = w.floor_positions();
    w.throw_aimed(0, 1, crate::passing::PassStyle::Normal, aim, &start);
    assert_eq!(w.spot_run.count, 1);
    assert!(w.catching.is_none(), "no row parked");
    idle(&mut w);
    let c = w.catching.expect("catch_stand_1");
    assert_eq!(
        (c.player, usize::from(c.row), c.parked),
        (1, crate::receiver::STAND_ROW, false)
    );
    assert_eq!(w.spot_run.count, 0);
    assert_eq!(w.players[1].speed, 0.0);
    // Wanting to face the ball.
    let p = &w.players[1];
    let to = [
        w.ball.position[0] - p.position[0],
        w.ball.position[2] - p.position[1],
    ];
    let f = [p.wanted.sin(), p.wanted.cos()];
    assert!(f[0] * to[0] + f[1] * to[1] > 0.0, "{f:?} {to:?}");
}
