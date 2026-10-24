//! The turbo bars ([`crate::turbo`]), the juke press and the counter
//! stance on its scripts ([`crate::counter`]).

use super::*;
use crate::counter::{self, COUNTERED, MISSED, Stance, TAKE, TAKE_NO_FAKE};
use crate::trick::{self, Run};

/// A handler (0) facing +x with the ball and a defender (1) `gap` in
/// front of him facing back, both a person's.
fn face_to_face(gap: f32, seed: u32) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(1, [0.0, gap], std::f32::consts::PI),
        ],
        Rules::default(),
        seed,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    w
}

/// `i` in the counter stance's first script, just pressed.
fn in_stance(w: &mut World, i: usize) {
    w.players[i].action = Action::Stealing {
        elapsed: 0.0,
        counter: true,
        swipe: None,
        stance: Some(Stance {
            script: 0,
            run: Run::new(false, 0),
            from: None,
            word: crate::timing::facing_word(w.players[i].heading),
        }),
    };
}

/// Trick `script` with its opening blend over.
fn tricking_past_the_blend(script: u16) -> Action {
    let Action::Trick { id, mut play } = trick::pressed(script, 1) else {
        unreachable!();
    };
    let steps = trick::SCRIPTS[usize::from(script)].steps;
    while play.run.tick < 0 || play.run.blend_counter().is_some() {
        play.run.step(steps, 0.0);
    }
    Action::Trick { id, play }
}

#[test]
fn a_trick_counter_meets_a_juke_after_its_blend_like_the_contact_handler() {
    // `FUN_001343e8` (raw `0x134ac8..0x134f48`): the stance touching a
    // handler in a trick, holding the ball, his channel 0 not blending, in
    // front: lands at `r % 100 <= (Steals − Handles + 50) · 0.8 + 20` (60
    // at even ratings) and takes the ball; the handler stumbles
    // (`0x44bf61`) and the defender plays his take (`0x3d0fa0`). A miss
    // stumbles the defender (`0x44bf94`) and ends the stance.
    let (mut taken, mut missed) = (0, 0);
    for seed in 0..100 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        let mut w = face_to_face(2.0, seed);
        in_stance(&mut w, 1);
        w.players[0].action = tricking_past_the_blend(0);
        w.step(&[Input::default(), Input::default()]);
        if w.ball.state == BallState::Held(1) {
            taken += 1;
            assert!(w.events.iter().any(|e| matches!(e, Event::Counter { .. })));
            assert!(matches!(
                w.players[0].action,
                Action::Stumbling { reaction: Some(r), .. } if r.script == COUNTERED
            ));
            let Action::Stealing {
                counter: true,
                stance: Some(s),
                ..
            } = w.players[1].action
            else {
                panic!("{:?}", w.players[1].action);
            };
            assert!(s.take() && matches!(s.script, TAKE | TAKE_NO_FAKE));
            assert_eq!(w.players[1].dribble, 0);
        } else {
            missed += 1;
            assert!(matches!(
                w.players[1].action,
                Action::Stumbling { reaction: Some(r), .. } if r.script == MISSED
            ));
            let Action::Trick { play, .. } = w.players[0].action else {
                panic!("{:?}", w.players[0].action);
            };
            assert_eq!(play.window, 0, "the fake window closes");
        }
    }
    assert!((45..76).contains(&taken), "{taken} {missed}");
    assert_eq!(taken + missed, 100);
}

#[test]
fn a_counter_waits_out_the_handlers_blend_and_needs_a_trick() {
    // Channel 0's 0x200000 (the trick's opening `8e`): the contact is a
    // push, the stance stands and the ball stays.
    let mut w = face_to_face(2.0, 3);
    in_stance(&mut w, 1);
    w.players[0].action = trick::pressed(0, 1);
    w.step(&[Input::default(), Input::default()]);
    assert_eq!(w.ball.state, BallState::Held(0));
    assert!(matches!(
        w.players[1].action,
        Action::Stealing { counter: true, .. }
    ));
    // No trick to meet: the stance plays its script out (bit 16 for 77
    // updates with `STEALCOUNTER_1_2`, the gait back in the 78th) and the
    // ball stays.
    let mut w = face_to_face(2.0, 3);
    in_stance(&mut w, 1);
    let mut updates = 0;
    loop {
        w.step(&[Input::default(), Input::default()]);
        if !matches!(w.players[1].action, Action::Stealing { counter: true, .. }) {
            break;
        }
        updates += 1;
        assert!(updates < 200);
        assert_eq!(w.players[1].speed, 0.0, "the stance holds him");
    }
    assert_eq!(updates, 77);
    assert_eq!(w.ball.state, BallState::Held(0));
}

#[test]
fn the_trick_counter_lesson_always_lands_without_the_roll() {
    for seed in 0..20 {
        let mut w = face_to_face(2.0, seed);
        w.lesson = Some(counter::LESSON);
        in_stance(&mut w, 1);
        w.players[0].action = tricking_past_the_blend(0);
        w.step(&[Input::default(), Input::default()]);
        assert_eq!(w.ball.state, BallState::Held(1), "{seed}");
    }
}

#[test]
fn the_counter_press_buys_its_stance_with_a_fifth_of_the_bar() {
    // `FUN_0015da28` (raw `0x15df60..0x15e0e4`): steal with two turbos held
    // and 0.2 of the bar: one of `0x3d7228`'s two stances by `r & 1`, 77
    // or 67 updates long.
    let press = Input::default().with(button::TRICK | button::L1 | button::R1);
    let mut lengths = std::collections::BTreeSet::new();
    for seed in 0..24 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        let mut w = face_to_face(6.0, seed);
        w.step(&[Input::default(), Input::default()]);
        let before = w.turbo_of(1);
        w.step(&[Input::default(), press]);
        let Action::Stealing {
            counter: true,
            stance: Some(s),
            swipe: None,
            ..
        } = w.players[1].action
        else {
            panic!("{:?}", w.players[1].action);
        };
        assert!(s.script < 2);
        // The bar's update (a turbo held drains it), then the stance's 0.2.
        let drained = (before - tuning::TURBO_DRAIN).max(0.0);
        assert_eq!(w.turbo_of(1), drained - crate::turbo::MOVE);
        let mut updates = 1;
        loop {
            w.step(&[Input::default(), Input::default()]);
            if !matches!(w.players[1].action, Action::Stealing { counter: true, .. }) {
                break;
            }
            updates += 1;
        }
        assert_eq!(updates, [77, 67][usize::from(s.script)], "seed {seed}");
        lengths.insert(updates);
    }
    assert_eq!(lengths.into_iter().collect::<Vec<_>>(), vec![67, 77]);
    // Short of the bar: the "no turbo" message and nothing, not a swipe.
    let mut w = face_to_face(6.0, 1);
    w.set_turbo(1, 0.19);
    w.step(&[Input::default(), press]);
    assert_eq!(w.players[1].action, Action::None);
    // The No Counters cheat makes it a swipe.
    let mut w = face_to_face(6.0, 1);
    w.rules.no_counters = true;
    w.step(&[Input::default(), press]);
    assert!(matches!(
        w.players[1].action,
        Action::Stealing { counter: false, .. }
    ));
}

#[test]
fn the_juke_press_asks_for_and_spends_a_fifth_of_the_bar() {
    // `FUN_0015c8e8`: 0.2 of the bar or nothing ("no turbo"); every trick
    // costs the same 0.2.
    let mut w = face_to_face(6.0, 2);
    w.set_turbo(0, 0.19);
    w.step(&[Input::default().with(button::TRICK), Input::default()]);
    assert_eq!(w.players[0].action, Action::None);
    let mut w = face_to_face(6.0, 2);
    w.step(&[Input::default().with(button::TRICK), Input::default()]);
    assert!(matches!(w.players[0].action, Action::Trick { .. }));
    let refilled = (1.0 + tuning::TURBO_REFILL).min(1.0);
    assert_eq!(w.turbo_of(0), refilled - crate::turbo::MOVE);
}

#[test]
fn a_computer_sides_three_share_one_bar_that_the_holder_fills() {
    // `FUN_00142098` / `DAT_0043ded0`: a side no person plays draws on one
    // bar; actor+0x2ac: only the holder (and, on the other side, the man
    // at his slot) fills or drains it, once an update.
    let mut w = World::new(hoops(), three_on_three_players(), Rules::default(), 5);
    w.phase = Phase::Live;
    w.ball.hold(1);
    w.possession = 0;
    let computer = vec![Input::computer(); 6];
    w.step(&computer);
    assert_eq!(w.turbo_slot(0), crate::turbo::COMPUTER_SLOT[0]);
    assert_eq!(w.turbo_slot(4), crate::turbo::COMPUTER_SLOT[1]);
    assert!(w.turbo_owner[1] && !w.turbo_owner[0] && !w.turbo_owner[2]);
    assert!(w.turbo_owner[4] && !w.turbo_owner[3] && !w.turbo_owner[5]);
    w.set_turbo(2, 0.5);
    assert!((0..3).all(|i| w.players[i].turbo == 0.5));
    assert!((3..6).all(|i| w.players[i].turbo != 0.5));
    // A person on a side has his own bar, and his computer teammates share
    // it without filling it.
    let mut inputs = computer.clone();
    inputs[1] = Input::default();
    w.step(&inputs);
    assert_eq!(w.turbo_slot(1), crate::turbo::COMPUTER_SLOT[0]);
    assert!(w.turbo_owner[1] && !w.turbo_owner[0]);
}

fn three_on_three_players() -> Vec<Player> {
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
    players
}
