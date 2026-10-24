//! The game's phase machine ([`crate::game_phase`]) through `World::step`:
//! the moments the ball posts (`FUN_0014cb28`'s slot 24, the launcher's
//! slot 11, the basket's restart) and the game's update after the ball.

use super::*;
use crate::game_phase::{GAMEBREAKER_HOLD, WON_UPDATES, phase};

/// Three a side, every player the computer's.
fn six(seed: u32) -> World {
    let mut players = Vec::new();
    for team in 0..2 {
        let side = crate::restart::side(team);
        for k in 0..3 {
            players.push(Player::new(
                team,
                [side * 10.0, (k as f32 - 1.0) * 8.0],
                -side * std::f32::consts::FRAC_PI_2,
            ));
        }
    }
    World::new(hoops(), players, Rules::default(), seed)
}

fn computer(w: &mut World) {
    let inputs: Vec<Input> = (0..w.players.len()).map(|i| ai::input(w, i)).collect();
    w.step(&inputs);
}

#[test]
fn a_steal_in_the_front_court_brings_the_ball_up_from_the_back_court() {
    // Team 0 (attacking +x) holds the ball at x = 20, its front court and
    // team 1's back court; a swipe that takes the ball outright (slot 24)
    // gives team 1 the ball in phase 4 in the update it happens.
    let mut taken = 0;
    for seed in 0..40 {
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [20.0, 0.0], 0.0),
                Player::new(1, [20.0, tuning::HAND_REACH + 0.9], std::f32::consts::PI),
            ],
            Rules::default(),
            seed * 7 + 1,
        );
        w.players[1].ratings.steals = 1.0;
        w.phase = Phase::Live;
        w.ball.hold(0);
        w.step(&[Input::default(), buttons(button::TRICK)]);
        assert_eq!((w.game_phase.phase, w.game_phase.possession), (5, 0));
        for _ in 0..TICK_HZ {
            w.step(&[Input::default(), Input::default()]);
            if w.ball.state != BallState::Held(0) {
                break;
            }
            assert_eq!(w.game_phase.phase, phase::FRONT_COURT);
        }
        match w.ball.state {
            BallState::Held(1) => {
                taken += 1;
                assert_eq!(w.game_phase.phase, phase::BACK_COURT, "seed {seed}");
                assert_eq!(w.game_phase.possession, 1);
            }
            // Knocked loose: phase 2 in the same update.
            BallState::Loose => assert_eq!(w.game_phase.phase, phase::LOOSE, "seed {seed}"),
            _ => {}
        }
    }
    assert!(taken > 0);
}

#[test]
fn the_tip_is_phase_1_until_the_jumper_takes_it() {
    // `FUN_00141fd8` then slot 24 in phase 1: the taker's side has the
    // ball, in phase 4 (the jumpers stand in their own halves), from the
    // update he tips it.
    for seed in [3, 8] {
        let mut w = six(seed);
        w.start_opening_toss();
        assert_eq!((w.game_phase.phase, w.game_phase.possession), (1, -1));
        let mut tipped = false;
        for _ in 0..400 {
            computer(&mut w);
            if w.game_phase.phase != phase::TIP {
                tipped = true;
                assert_eq!(w.ball.state, BallState::Pass);
                assert_eq!(w.game_phase.phase, phase::BACK_COURT, "seed {seed}");
                let jumper = w.ball.pass.unwrap().from;
                assert_eq!(w.game_phase.possession, w.players[jumper].team as i32);
                break;
            }
        }
        assert!(tipped, "seed {seed}");
    }
}

#[test]
fn a_shot_is_phase_3_its_basket_a_restart_and_a_block_a_loose_ball() {
    let made = |w: &mut World| {
        let call = shooting::ShotCall {
            percent: 100,
            made: true,
            kind: 0,
            angle: std::f32::consts::FRAC_PI_4,
        };
        let shot = Shot {
            shooter: 0,
            hoop: 0,
            made: true,
            points: 1,
            gamebreaker: 0,
            layup: false,
            tried: 0,
            slam: None,
        };
        w.launch_shot(0, [28.0, 9.0, 0.0], call, shot, None);
    };
    let mut w = face_off(2);
    w.players[0].position = [28.0, 0.0];
    w.players[1].position = [0.0, 25.0];
    w.step(&[Input::default(), Input::default()]);
    assert_eq!(w.game_phase.phase, phase::FRONT_COURT);
    made(&mut w);
    assert_eq!(w.game_phase.phase, phase::SHOT);
    let mut scored = false;
    for _ in 0..300 {
        w.step(&[Input::default(), Input::default()]);
        if w.events.iter().any(|e| matches!(e, Event::Score { .. })) {
            scored = true;
            // The restart for the side scored on, in the basket's update.
            assert_eq!((w.game_phase.phase, w.game_phase.possession), (6, 1));
            assert!(matches!(w.phase, Phase::DeadBall { team: 1, .. }));
            break;
        }
        assert_eq!(w.game_phase.phase, phase::SHOT);
    }
    assert!(scored);
    // A block: the shot knocked away is a loose ball at once (state 1),
    // where a miss off the rim stays phase 3 until it falls below 6.
    let mut w = face_off(2);
    w.players[0].position = [28.0, 0.0];
    w.step(&[Input::default(), Input::default()]);
    made(&mut w);
    w.step(&[Input::default(), Input::default()]);
    let at = w.ball.position;
    let rim = w.rim_spot(0);
    w.swat(1, 0, at, rim);
    assert!(!w.ball.rebound);
    w.step(&[Input::default(), Input::default()]);
    assert_eq!(w.game_phase.phase, phase::LOOSE);
}

#[test]
fn a_gamebreakers_basket_holds_the_restart_through_its_replays() {
    // `FUN_00141818`: game+0x104 = 240 after a gamebreaker's basket that
    // does not win; `FUN_00140840` stays false until it has counted down,
    // so nobody throws the inbound and phase 6 lasts at least as long.
    for (level, held) in [(0, false), (1, true)] {
        let mut w = six(5);
        w.phase = Phase::Live;
        w.possession = 0;
        w.ball.hold(0);
        computer(&mut w);
        let basket = if level == 0 {
            street::Basket::Jumper { long: false }
        } else {
            street::Basket::Gamebreaker {
                level,
                long: false,
                dunk: false,
            }
        };
        w.score_basket(0, basket);
        assert_eq!(w.game_phase.phase, phase::RESTART);
        assert_eq!(w.game_phase.hold, if held { GAMEBREAKER_HOLD } else { 0 });
        let mut updates = 0;
        while w.restarting() && updates < 2_000 {
            computer(&mut w);
            updates += 1;
            if updates < GAMEBREAKER_HOLD && held {
                assert!(!w.restart_ready());
                assert!(w.windup.is_none(), "an inbound pressed in the hold");
            }
        }
        assert_eq!(w.game_phase.phase, phase::BACK_COURT);
        assert!(!held || GAMEBREAKER_HOLD <= updates, "{updates}");
    }
}

#[test]
fn a_won_game_goes_to_phase_9_the_update_after_its_basket() {
    // The hoop posts the basket after the game's update: the winning
    // basket's update leaves the phase where it was (the net's catch is
    // not yet a loose ball), the next is phase 9 with its countdown.
    let mut w = face_off(2);
    w.players[0].position = [28.0, 0.0];
    w.players[1].position = [0.0, 25.0];
    let target = match w.rules.goal {
        Goal::Points(n) => n,
        _ => unreachable!(),
    };
    w.score[0] = target - 1;
    w.step(&[Input::default(), Input::default()]);
    let call = shooting::ShotCall {
        percent: 100,
        made: true,
        kind: 0,
        angle: std::f32::consts::FRAC_PI_4,
    };
    let shot = Shot {
        shooter: 0,
        hoop: 0,
        made: true,
        points: 1,
        gamebreaker: 0,
        layup: false,
        tried: 0,
        slam: None,
    };
    w.launch_shot(0, [28.0, 9.0, 0.0], call, shot, None);
    for _ in 0..300 {
        w.step(&[Input::default(), Input::default()]);
        if matches!(w.phase, Phase::Over { .. }) {
            break;
        }
    }
    assert_eq!(w.phase, Phase::Over { winner: 0 });
    assert_eq!(w.game_phase.phase, phase::SHOT);
    w.step(&[Input::default(), Input::default()]);
    assert_eq!(w.game_phase.phase, phase::WON);
    assert_eq!(w.game_phase.countdown, WON_UPDATES - 1);
    assert!(!w.game_phase.live());
}

#[test]
fn the_phase_survives_a_snapshot_and_a_bad_one_is_refused() {
    let mut w = six(4);
    w.start_opening_toss();
    for _ in 0..300 {
        computer(&mut w);
    }
    let back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back.game_phase, w.game_phase);
    assert_eq!(back.ball.rebound, w.ball.rebound);
    let mut bad = w.clone();
    bad.game_phase.phase = 7;
    assert!(bad.validate().is_err());
    let mut bad = w.clone();
    bad.game_phase.possession = 2;
    assert!(bad.validate().is_err());
    let mut bad = w;
    bad.ball.hold(0);
    bad.ball.rebound = true;
    assert!(bad.validate().is_err());
}
