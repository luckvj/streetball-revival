//! Shots on the ball's states and rebounds taken in the air
//! ([`crate::rebound`]), through `World::step`: the shot's timer and its
//! states 5, 6 and 0 (`FUN_0014d1f8`), the rebound jump (`FUN_0015ce50`),
//! its aim (callback 36) and its catch (`FUN_0016ed18`).

use super::*;
use crate::game_phase::{ball as state, phase};
use crate::rebound::SCRIPTS;

/// Team 0's shooter at x = 28 (it attacks +x), team 1's man far away.
fn shooter(seed: u32) -> World {
    let mut w = face_off(seed);
    w.players[0].position = [28.0, 0.0];
    w.players[1].position = [0.0, 25.0];
    w.step(&[Input::default(), Input::default()]);
    w
}

fn miss(w: &mut World, kind: u8) {
    let call = shooting::ShotCall {
        percent: 0,
        made: false,
        kind,
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
    w.launch_shot(0, [28.0, 9.0, 0.0], call, shot, None);
}

fn idle(w: &mut World) {
    let inputs = vec![Input::default(); w.players.len()];
    w.step(&inputs);
}

#[test]
fn a_missed_shot_is_on_the_rim_until_its_timer_then_a_rebound() {
    // Off the front of the rim (kind 2): 5 until it touches the hoop, 6
    // until the timer (one more than its last contact) runs out, then a
    // rebound (0); the game stays in phase 3 meanwhile.
    for seed in 0..6 {
        let mut w = shooter(seed);
        miss(&mut w, 2);
        let timer = (w.ball.flight_left / TICK_SECONDS + 0.5).round() as u32;
        let mut states = Vec::new();
        for _ in 0..timer {
            assert_eq!(w.ball.state, BallState::Shot, "seed {seed}");
            idle(&mut w);
            states.push(w.native_ball_state());
            if w.ball.state == BallState::Shot {
                assert_eq!(w.game_phase.phase, phase::SHOT);
            }
        }
        assert_eq!(w.ball.state, BallState::Loose, "seed {seed}");
        assert!(w.ball.rebound, "seed {seed} {states:?}");
        assert_eq!(*states.last().unwrap(), state::REBOUND);
        let rim = states.iter().position(|&s| s == state::RIM).unwrap();
        assert!(states[..rim].iter().all(|&s| s == state::SHOT));
        assert!(
            states[rim..states.len() - 1]
                .iter()
                .all(|&s| s == state::RIM)
        );
        assert_eq!(w.game_phase.phase, phase::SHOT);
    }
}

#[test]
fn an_airball_is_loose_when_its_timer_runs_out() {
    // A straight miss (kind 0) beside the ring touches nothing: its timer
    // (the flight to its aim and through the tube, and one more) runs out
    // with the shot still 5, so it is loose (1), phase 2 at once.
    for seed in 0..6 {
        let mut w = shooter(seed);
        miss(&mut w, 0);
        let mut updates = 0;
        while w.ball.state == BallState::Shot && updates < 200 {
            assert!(!w.ball.touched);
            idle(&mut w);
            updates += 1;
        }
        assert_eq!(w.ball.state, BallState::Loose);
        assert!(!w.ball.rebound, "seed {seed}");
        assert_eq!(w.game_phase.phase, phase::LOOSE, "seed {seed}");
    }
}

#[test]
fn the_straight_flights_timer_is_the_flight_through_the_tube_and_one_more() {
    // `0x14bd30..0x14bda0`: a ball falling 0.4 an update at the aim after
    // 30 updates needs a ninth of an update more to pass the tube.
    let g = GRAVITY * TICK_SECONDS * TICK_SECONDS;
    let vy = -0.4 + g * 30.0;
    assert_eq!(shooting::straight_timer(30.0, vy), 32);
    assert_eq!(shooting::straight_timer(29.5, vy - g * 0.5), 31);
}

/// A rebound coming off team 0's rim (state 0) toward the court, player 0
/// (team 0) under where it comes down; team 1's man far away.
fn rebound_scene(seed: u32) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [31.0, 0.5], -std::f32::consts::FRAC_PI_2),
            Player::new(1, [-20.0, 20.0], 0.0),
        ],
        Rules::default(),
        seed,
    );
    w.phase = Phase::Live;
    w.possession = 0;
    w.game_phase.phase = phase::SHOT;
    w.game_phase.possession = 0;
    w.ball.position = [35.0, 13.5, 0.4];
    w.ball.velocity = [-7.0, 4.0, 0.0];
    w.ball.airborne = true;
    w.ball.rebound = true;
    w.stats.rebound = true;
    w
}

#[test]
fn a_rebound_jump_meets_the_ball_and_takes_it_in_the_air() {
    for seed in 0..4 {
        let mut w = rebound_scene(seed);
        w.step(&[buttons(button::JUMP), Input::default()]);
        assert!(w.events.contains(&Event::Jump {
            player: 0,
            block: false
        }));
        let Action::ReboundJump { script, .. } = w.players[0].action else {
            panic!("{:?}", w.players[0].action);
        };
        let jump = u16::from(SCRIPTS[usize::from(script)].jump);
        assert!(jump > 0, "seed {seed}: a standing catch");
        let mut caught = None;
        for t in 1..80 {
            let before = w.players[0].height;
            idle(&mut w);
            if u16::try_from(t).unwrap() < jump {
                assert_eq!(
                    w.players[0].height, 0.0,
                    "seed {seed}: up before callback 36"
                );
            } else if u16::try_from(t).unwrap() == jump {
                assert!(before == 0.0 && w.players[0].height > 0.0, "seed {seed}");
            }
            if w.ball.state == BallState::Held(0) {
                caught = Some(t);
                break;
            }
        }
        let t = caught.unwrap_or_else(|| panic!("seed {seed}: not caught"));
        assert!(
            w.players[0].height > 0.0,
            "seed {seed}: caught at {t} on the floor"
        );
        assert!(w.events.contains(&Event::Pickup { player: 0 }));
        // Slot 24 out of phase 3: his side's front court at once.
        assert_eq!(w.game_phase.phase, phase::FRONT_COURT);
        assert_eq!(w.stats.player(0, stats::REBOUNDS), 1);
    }
}

#[test]
fn a_shot_on_the_rim_is_taken_only_outside_the_ring_and_on_the_roll() {
    // `FUN_0016ed18`'s state 6: inside the ring (0.793225 across of the
    // rim) the hand does not take it; outside, `r % 100` at most the
    // rating mix (90 at most) takes it, else it slips him for the jump.
    let on_rim = |seed: u32, across: f32| {
        let mut w = rebound_scene(seed);
        let rim = w.hoops[0].rim;
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
        w.ball.rebound = false;
        w.ball.state = BallState::Shot;
        w.ball.shot = Some(shot);
        w.ball.touched = true;
        w.ball.position = [rim[0] - across, 12.6, rim[2]];
        // In the air under the ball, his hand on it.
        let p = &mut w.players[0];
        p.position = [rim[0] - across, rim[2]];
        p.height = 12.6 - p.ratings.reach;
        p.action = Action::ReboundJump {
            script: 0,
            tick: 12,
            vy: 0.0,
            drift: [0.0; 2],
            up: true,
            meet: 4,
            end: u16::MAX,
            barred: false,
            branch: false,
            from: None,
            airtime: 30,
            word: 0,
        };
        w.rebound_catch(0);
        w
    };
    for seed in 0..20 {
        let w = on_rim(seed, 0.4);
        assert_eq!(w.ball.state, BallState::Shot);
        assert!(!matches!(
            w.players[0].action,
            Action::ReboundJump { barred: true, .. }
        ));
    }
    let (mut taken, mut slipped) = (0, 0);
    for seed in 0..40 {
        let mut w = on_rim(seed, 1.2);
        if w.ball.state == BallState::Held(0) {
            taken += 1;
        } else {
            slipped += 1;
            assert!(matches!(
                w.players[0].action,
                Action::ReboundJump { barred: true, .. }
            ));
            w.rebound_catch(0);
            assert_eq!(
                w.ball.state,
                BallState::Shot,
                "a slipped jumper takes nothing more"
            );
        }
    }
    // Rebounds 50, Blocks 50, Speed 50, Power 50: 50.
    assert!(taken > 10 && slipped > 10, "{taken} {slipped}");
}

#[test]
fn a_jump_at_a_shot_still_flying_is_a_block_jump_and_on_the_rim_a_rebound_jump() {
    for touched in [false, true] {
        let mut w = shooter(3);
        miss(&mut w, 2);
        w.players[1].position = [33.0, 0.0];
        w.ball.touched = touched;
        w.step(&[Input::default(), buttons(button::JUMP)]);
        assert!(w.events.contains(&Event::Jump {
            player: 1,
            block: !touched
        }));
        assert_eq!(
            matches!(w.players[1].action, Action::ReboundJump { .. }),
            touched
        );
    }
}

#[test]
fn under_the_basket_the_rebound_jump_is_kind_8() {
    let mut w = rebound_scene(1);
    w.players[0].position = [37.6, 0.5];
    w.step(&[buttons(button::JUMP), Input::default()]);
    let Action::ReboundJump { script, .. } = w.players[0].action else {
        panic!("{:?}", w.players[0].action);
    };
    assert_eq!(crate::rebound::KINDS[8], [script]);
}

#[test]
fn a_rebound_jump_and_the_balls_states_survive_a_snapshot() {
    let mut w = rebound_scene(2);
    w.step(&[buttons(button::JUMP), Input::default()]);
    for _ in 0..10 {
        idle(&mut w);
    }
    assert!(matches!(
        w.players[0].action,
        Action::ReboundJump { up: true, .. }
    ));
    let back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back, w);
    // A dunk's shot carries its basket; a hoop touch shows on the ball.
    let mut d = shooter(4);
    miss(&mut d, 2);
    if let Some(shot) = &mut d.ball.shot {
        shot.slam = Some(Basket::AlleyOop { advanced: false });
    }
    d.ball.touched = true;
    let back = World::from_snapshot(&d.to_snapshot()).unwrap();
    assert_eq!(back, d);
    // Not a dunk's basket, or a script past the table: refused.
    let mut bad = d.clone();
    if let Some(shot) = &mut bad.ball.shot {
        shot.slam = Some(Basket::Layup);
    }
    assert!(bad.validate().is_err());
    let mut bad = w.clone();
    if let Action::ReboundJump { script, .. } = &mut bad.players[0].action {
        *script = SCRIPTS.len() as u8;
    }
    assert!(bad.validate().is_err());
    let mut bad = w;
    bad.ball.touched = true;
    assert!(bad.validate().is_err());
}
