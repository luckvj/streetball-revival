//! The person's presses as the original turns them into moves: the pass
//! target and the passout specials (`FUN_00146278`), the shoot press near
//! the rim (`FUN_0016b4b8`, `FUN_001693b8`), the pump fake's end and the
//! lay-ups in the air (`FUN_0016b1e8`). In the original's frame: team 0
//! attacks +x.

use super::*;
use crate::passing::{PassChoice, stick_word};

/// Three on three, the ball with player 0: team 0's at `ours`, team 1's at
/// `theirs` (slot order: player 3 guards 0, 4 guards 1, 5 guards 2).
fn three(ours: [[f32; 2]; 3], theirs: [[f32; 2]; 3], seed: u32) -> World {
    let mut players = Vec::new();
    for at in ours {
        players.push(Player::new(0, at, std::f32::consts::FRAC_PI_2));
    }
    for at in theirs {
        players.push(Player::new(1, at, -std::f32::consts::FRAC_PI_2));
    }
    let mut w = World::new(hoops(), players, Rules::default(), seed);
    w.phase = Phase::Live;
    w.ball.hold(0);
    w
}

fn press(w: &mut World, input: Input) -> PassChoice {
    let start = w.floor_positions();
    w.pass_press(0, &input, false, &start)
}

#[test]
fn the_pass_goes_to_the_teammate_best_lined_up_by_fun_00146278s_scores() {
    // The stick at a teammate: the angle decides (the bonuses come to at
    // most 38 of 1024).
    let ours = [[20.0, 0.0], [30.0, 6.0], [30.0, -6.0]];
    let far = [[0.0, 20.0], [0.0, -20.0], [-10.0, 0.0]];
    let mut w = three(ours, far, 1);
    let at = |w: &World, k: usize| sub(w.players[k].position, w.players[0].position);
    let to_two = Input::moving(normalize2(at(&w, 2)));
    assert_eq!(
        press(&mut w, to_two),
        PassChoice::Pass {
            to: 2,
            style: PassStyle::Normal
        }
    );
    // Straight up the court, between the two: their angles tie, both
    // lanes are open within 14 of the rim, both are unguarded (their
    // defenders and nearest opponents more than 15 off); player 1 is the
    // side's nearest the rim (the first of the equals), 9 more.
    assert_eq!(
        press(&mut w, Input::moving([1.0, 0.0])),
        PassChoice::Pass {
            to: 1,
            style: PassStyle::Normal
        }
    );
    // His defender comes within 15 of him (behind him, off his lane): he
    // loses that 9 and his nearest opponent's 11; player 2 gets it.
    let mut w = three(ours, [[0.0, 20.0], [22.0, 8.0], [-10.0, 0.0]], 1);
    assert_eq!(
        press(&mut w, Input::moving([1.0, 0.0])),
        PassChoice::Pass {
            to: 2,
            style: PassStyle::Normal
        }
    );
    // At rest the facing word aims it; L3 makes it a kick.
    let mut w = three(ours, far, 1);
    w.players[0].wanted = std::f32::consts::FRAC_PI_2 + 0.3;
    assert_eq!(
        press(&mut w, Input::default().with(button::L3)),
        PassChoice::Pass {
            to: 2,
            style: PassStyle::Kick
        }
    );
}

#[test]
fn a_pass_aimed_away_from_both_goes_by_angle_and_distance() {
    // Past 228 of the best angle the sum of angle and distance (game+0x1ac)
    // picks: stick back down the court, the nearer teammate.
    let w = three(
        [[20.0, 0.0], [24.0, 12.0], [34.0, -14.0]],
        [[0.0, 20.0], [0.0, -20.0], [-10.0, 0.0]],
        1,
    );
    let start = w.floor_positions();
    let word = stick_word([-1.0, 0.0]);
    let s = |k: usize| {
        let toward = crate::contact::angle(start[0], start[k]);
        crate::timing::angle_diff(word, toward).abs()
    };
    assert!(s(1) >= 228 && s(2) >= 228, "{} {}", s(1), s(2));
    assert_eq!(w.lined_up(0, word, &start), Some(1));
}

#[test]
fn two_turbos_at_rest_bounce_the_pass_off_the_defenders_head_within_12() {
    // Off the Heezay (`0x146a44..0x146c04`): two turbos held (0x40000), on
    // the floor, the stick at rest: off his defender's head (`FUN_0016e8f8`)
    // within 12, back to himself, for 0.2 of the bar.
    let ours = [[10.0, 0.0], [25.0, 10.0], [25.0, -10.0]];
    let input = Input::default().with(button::L1 | button::R1);
    let mut w = three(ours, [[18.0, 1.0], [0.0, 20.0], [0.0, -20.0]], 1);
    let bar = w.turbo_of(0);
    assert_eq!(
        press(&mut w, input),
        PassChoice::Heezay { defender: 3, to: 0 }
    );
    assert!((bar - w.turbo_of(0) - 0.2).abs() < 1e-4);
    // Beyond 12: no pass at all (`FUN_00171e30(actor, 0)`).
    let mut w = three(ours, [[23.0, 1.0], [0.0, 20.0], [0.0, -20.0]], 1);
    assert_eq!(press(&mut w, input), PassChoice::Refused);
    // Short of 0.2 of the bar: no pass either.
    let mut w = three(ours, [[18.0, 1.0], [0.0, 20.0], [0.0, -20.0]], 1);
    w.set_turbo(0, 0.15);
    assert_eq!(press(&mut w, input), PassChoice::Refused);
    // In the air: an ordinary pass (`debug_passoutOnGround`).
    let mut w = three(ours, [[18.0, 1.0], [0.0, 20.0], [0.0, -20.0]], 1);
    w.players[0].height = 1.0;
    assert!(matches!(press(&mut w, input), PassChoice::Pass { .. }));
}

#[test]
fn a_self_head_pass_waits_for_its_passout_rows_release() {
    // FUN_0015f930's passout rows and callback 53 (FUN_001741a8),
    // observed in School lesson 18: the head press does not throw at once.
    // Only board kind +/-1 floors the route distance at 25 (0x16065c).
    for bank in [crate::pose::Bank::StandIns, crate::pose::disc_bank()] {
        for gap in [2.0, 8.0] {
            for seed in 0..4 {
                let mut w = three(
                    [[10.0, 0.0], [25.0, 10.0], [25.0, -10.0]],
                    [[10.0 + gap, 0.0], [0.0, 20.0], [0.0, -20.0]],
                    seed,
                );
                w.poses = bank.clone();
                let mut inputs = vec![Input::default(); 6];
                inputs[0] = Input::default().with(button::PASS | button::L1 | button::R1);
                w.step(&inputs);
                let windup = w.windup.expect("the head pass waits for callback 53");
                let release = crate::passout::toss(windup.row).unwrap();
                let rows = if gap == 2.0 { 83..=84 } else { 85..=86 };
                assert!(rows.contains(&windup.row), "{gap}: {windup:?}");
                assert_eq!(windup.style, PassStyle::Heezay);
                assert_eq!(w.ball.state, BallState::Held(0));
                inputs.fill(Input::default());
                for _ in 1..release.tick {
                    w.step(&inputs);
                    assert_eq!(w.ball.state, BallState::Held(0));
                    assert_eq!(w.validate(), Ok(()));
                    let restored = World::from_snapshot(&w.to_snapshot()).unwrap();
                    assert_eq!(restored.checksum(), w.checksum());
                }
                w.step(&inputs);
                assert!(
                    w.ball.pass.is_some_and(|p| {
                        p.from == 0 && p.to == 0 && p.via == crate::ball::Via::Head { defender: 3 }
                    }),
                    "gap {gap}, seed {seed}, disc {}: ball {:?}, windup {:?}, special {:?}",
                    bank.get().is_some(),
                    w.ball,
                    w.windup,
                    w.passout,
                );
                assert_eq!(w.validate(), Ok(()));
            }
        }
    }
}

#[test]
fn heezay_catches_keep_their_five_thousand_value_and_separate_repeat_key() {
    let mut w = three(
        [[10.0, 0.0], [25.0, 10.0], [25.0, -10.0]],
        [[18.0, 1.0], [0.0, 20.0], [0.0, -20.0]],
        1,
    );
    w.rules.shot_clock = None;
    w.players[0].ratings.handles = 1.0;
    w.combo.ready = true;
    w.teams[0].possession_seen = 0;
    w.settle_looks();
    // Board and head specials use different keys in FUN_00143a88.
    w.teams[0].recent_moves[0] = -3;
    let mut total = 0;
    for value in [5_000, 2_500] {
        let mut inputs = vec![Input::default(); 6];
        inputs[0] = Input::default().with(button::PASS | button::L1 | button::R1);
        w.step(&inputs);
        let release = w.windup.expect("head pass release wait").left;
        inputs.fill(Input::default());
        for _ in 0..release {
            w.step(&inputs);
        }
        assert!(
            w.ball
                .pass
                .is_some_and(|p| matches!(p.via, crate::ball::Via::Head { .. }))
        );
        let mut returned = false;
        for _ in 0..100 {
            w.step(&inputs);
            if w.ball
                .pass
                .is_some_and(|p| p.via == crate::ball::Via::Direct)
            {
                assert!(
                    w.passout
                        .is_some_and(|po| po.kind == crate::passout::Kind::Head)
                );
                let restored = World::from_snapshot(&w.to_snapshot()).unwrap();
                assert_eq!(restored.checksum(), w.checksum());
            }
            if w.ball.state == BallState::Held(0) {
                returned = true;
                break;
            }
        }
        assert!(returned, "the deterministic head pass did not return");
        total += value;
        assert_eq!(w.stats.player(0, crate::stats::TRICK_POINTS), total);
        let slot = w.combo.ring.slots[5];
        assert_eq!((slot.class, slot.extra, slot.value), (17, 1, value));
        assert_eq!(w.teams[0].recent_moves[0], -2);
        for _ in 0..120 {
            w.step(&inputs);
        }
        assert!(
            w.passout.is_none(),
            "finished head identity must be cleared"
        );
    }
}

#[test]
fn two_turbos_toward_the_rim_put_the_pass_off_the_board() {
    // Back 2 Papa (`0x146c04..0x147434`): the stick within 128 of the line
    // to the rim, in the front court within 35 of the middle and inside
    // the board's wedge; from within 8 across and 29 along to himself,
    // else to the teammate across the lane.
    let input = |s: [f32; 2]| Input::moving(s).with(button::L1 | button::R1);
    let theirs = [[0.0, 20.0], [0.0, -20.0], [-10.0, 0.0]];
    let mut w = three([[20.0, 2.0], [10.0, 15.0], [12.0, -15.0]], theirs, 1);
    let PassChoice::Glass { to, point } = press(&mut w, input([1.0, 0.0])) else {
        panic!("off the board");
    };
    assert_eq!(to, 0);
    let r = w.ball.radius;
    assert!((point[0] - (38.0 - r)).abs() < 1e-4, "{point:?}");
    // From the wing (|z| 8 or more): the teammate across the lane.
    let mut w = three([[25.0, 12.0], [10.0, 0.0], [24.0, -10.0]], theirs, 1);
    let to_rim = normalize2(sub(w.rim_spot(0), w.players[0].position));
    assert!(matches!(
        press(&mut w, input(to_rim)),
        PassChoice::Glass { to: 2, .. }
    ));
    // The stick away from the rim, or in the back court: no pass.
    let mut w = three([[20.0, 2.0], [10.0, 15.0], [12.0, -15.0]], theirs, 1);
    assert_eq!(press(&mut w, input([0.0, 1.0])), PassChoice::Refused);
    let mut w = three([[-5.0, 2.0], [10.0, 15.0], [12.0, -15.0]], theirs, 1);
    assert_eq!(press(&mut w, input([1.0, 0.0])), PassChoice::Refused);
    // Outside the wedge (deep in the corner): no pass.
    let mut w = three([[34.0, 20.0], [10.0, 15.0], [12.0, -15.0]], theirs, 1);
    let to_rim = normalize2(sub(w.rim_spot(0), w.players[0].position));
    assert_eq!(press(&mut w, input(to_rim)), PassChoice::Refused);
}

#[test]
fn back_2_papa_to_himself_goes_on_a_passout_row() {
    // The pass start takes only the rows with flag 0x200000 for a passout
    // to oneself; the row's callback 53 tosses the ball at the board
    // (`FUN_001741a8`, [`crate::passout`]).
    let input = Input::moving([1.0, 0.0]).with(button::L1 | button::R1);
    let theirs = [[0.0, 20.0], [0.0, -20.0], [-10.0, 0.0]];
    let mut w = three([[20.0, 2.0], [10.0, 15.0], [12.0, -15.0]], theirs, 1);
    let start = w.floor_positions();
    w.pass_button(0, &input, false, &start);
    let wu = w.windup.expect("the ball in his hands");
    assert_eq!((wu.from, wu.to), (0, 0));
    let toss = crate::passout::toss(wu.row).expect("a passout row");
    assert_eq!(wu.left, toss.tick);
    assert!(w.passout.is_some_and(|po| po.passer == 0 && !po.tossed));
    assert_eq!(w.ball.state, BallState::Held(0));
}

#[test]
fn the_right_stick_pushed_home_is_a_give_and_go_press() {
    // `FUN_001479f8`: the right stick at 60 of 64 or more sets 0x400, and
    // its press is the pass press, aimed along it, never a passout.
    let mut w = three(
        [[20.0, 0.0], [30.0, 6.0], [30.0, -6.0]],
        [[0.0, 20.0], [0.0, -20.0], [-10.0, 0.0]],
        1,
    );
    let mut aim = Input::default().with(button::L1 | button::R1);
    aim.set_aim(normalize2(sub(
        w.players[2].position,
        w.players[0].position,
    )));
    assert!(crate::passing::aim_pressed(&aim, &Input::default()));
    let mut half = Input::default();
    half.set_aim([0.5, 0.0]);
    assert!(!crate::passing::aim_pressed(&half, &Input::default()));
    let start = w.floor_positions();
    assert_eq!(
        w.pass_press(0, &aim, true, &start),
        PassChoice::Pass {
            to: 2,
            style: PassStyle::GiveAndGo
        }
    );
    // Through World::step: the stick alone throws it.
    let mut inputs = vec![Input::default(); 6];
    inputs[0] = aim;
    w.step(&inputs);
    assert!(w.windup.is_some_and(|p| p.to == 2) || w.ball.state == BallState::Pass);
}

#[test]
fn a_leaper_takes_the_pass_unless_he_is_late_and_falling_low() {
    // One teammate leaping for an alley-oop (bit 15) in the air is the
    // target; if his meet less the pass's arrival is more than 6 short
    // while he falls below 6.5, the other teammate is.
    let ours = [[10.0, 0.0], [30.0, 4.0], [20.0, -10.0]];
    let mut w = three(ours, [[0.0, 20.0], [0.0, -20.0], [-10.0, 0.0]], 1);
    let row = timing::MOVE_ROWS
        .iter()
        .position(|r| r.mask & 0x100000 != 0)
        .unwrap() as u8;
    w.start_leap(1, row);
    if let Action::Dunking { tick, .. } = &mut w.players[1].action {
        *tick = 12;
    }
    assert!(w.players[1].leaping());
    w.players[1].height = 5.0;
    w.players[1].flight.meet = 60;
    w.players[1].flight.velocity[1] = 0.1;
    let away = Input::moving([0.0, -1.0]);
    assert_eq!(
        press(&mut w, away),
        PassChoice::Pass {
            to: 1,
            style: PassStyle::Normal
        }
    );
    w.players[1].flight.meet = 1;
    w.players[1].flight.velocity[1] = -0.1;
    assert_eq!(
        press(&mut w, away),
        PassChoice::Pass {
            to: 2,
            style: PassStyle::Normal
        }
    );
}

#[test]
fn the_dunk_rows_unlock_by_the_turbo_bit_and_the_ratings_gates() {
    // `FUN_00139850`'s table for an ordinary player: eight ids without a
    // turbo, eight with one, the slot dunks by their combo's level, the
    // rest none; `FUN_001693b8`'s gates: 21..59 drop the hard rows with no
    // draw, 60 and over draw.
    assert!(
        timing::PLAIN_DUNKS
            .iter()
            .all(|&id| timing::dunk_unlock(id) == 1)
    );
    assert!(
        timing::TURBO_DUNKS
            .iter()
            .all(|&id| timing::dunk_unlock(id) == 2)
    );
    assert_eq!(timing::dunk_unlock(48), 16);
    assert_eq!(timing::dunk_unlock(20), 4);
    assert_eq!(timing::dunk_unlock(29), 0);
    let mut w = three(
        [[30.0, 0.0], [0.0, 10.0], [0.0, -10.0]],
        [[0.0, 20.0], [0.0, -20.0], [-10.0, 0.0]],
        1,
    );
    let before = w.rng.clone();
    let g = timing::MoveGates::draw(&mut w, 40);
    assert!(g.no_hard && !g.no_easy && !g.need_easy);
    assert_eq!(w.rng, before, "no draw for 21..59");
    let g = timing::MoveGates::draw(&mut w, 70);
    assert!(g.no_hard != g.no_easy);
    assert_ne!(w.rng, before);
}

#[test]
fn past_the_rims_line_a_dunk_needs_the_rims_side_else_the_shots_from_behind() {
    // `FUN_0016b4b8` with a dunk on past |x| 36.5: beside the rim on its
    // line (sector 2..6, `FUN_0016b3e8`) 2 or more out the dunk rows; deep
    // behind it more than 3.5 off the middle a side fade by side and hand,
    // within 3.5 `layup_3_bboard`.
    let shoot = |at: [f32; 2], hand: u8| {
        let mut w = three(
            [at, [0.0, 10.0], [0.0, -10.0]],
            [[0.0, 20.0], [0.0, -20.0], [-10.0, 0.0]],
            1,
        );
        w.players[0].gait_channel.hand = hand;
        w.step(&[buttons(button::SHOOT)]);
        w.players[0].action
    };
    // Standing within 7.5 behind the line, deep behind the board.
    assert!(matches!(
        shoot([40.0, -5.0], 1),
        Action::Shooting { script, .. } if usize::from(script) == timing::SIDEFADE_RIGHT_R
    ));
    assert!(matches!(
        shoot([40.0, 5.0], 2),
        Action::Shooting { script, .. } if usize::from(script) == timing::SIDEFADE_LEFT_L
    ));
    assert!(matches!(
        shoot([40.0, 1.0], 1),
        Action::Layup { row, .. } if row == timing::BOARD_LAYUP_ROW
    ));
    // On the rim's line beside it: the dunk and lay-up rows.
    assert!(matches!(
        shoot([37.2, 5.0], 1),
        Action::Dunking { .. } | Action::Layup { .. }
    ));
}

#[test]
fn a_pump_fake_lasts_to_the_end_of_its_scripts_fall_through() {
    // `FAKE_STEPS` run to their `End`: the captured fakes' channels leave
    // the stepback at 18, `legkick_1` at 22, the heave at 8.
    assert_eq!(crate::held::fake_end(20), Some(18));
    assert_eq!(crate::held::fake_end(18), Some(22));
    assert_eq!(crate::held::fake_end(timing::HEAVE), Some(8));
    // Through World::step: a tap on the stepback.
    for k in 0..400u32 {
        let seed = k.wrapping_mul(0x9e37_79b9);
        let mut w = face_off(seed);
        w.players[0].position = [15.0, 0.0];
        w.players[1].position = [0.0, 20.0];
        w.step(&[buttons(button::SHOOT), Input::default()]);
        let Action::Shooting { script, check, .. } = w.players[0].action else {
            panic!("a shot");
        };
        if usize::from(script) != 20 {
            continue;
        }
        let mut faked = None;
        for n in 1..60 {
            w.step(&[Input::default(), Input::default()]);
            if faked.is_none() && w.events.contains(&Event::PumpFake { player: 0 }) {
                faked = Some(n);
            }
            if w.players[0].action == Action::None {
                assert_eq!(faked, Some(i32::from(check)));
                assert_eq!(n, 18, "seed {seed}");
                return;
            }
        }
        panic!("seed {seed}: the fake never ended");
    }
    panic!("no stepback drawn in 400 seeds");
}

#[test]
fn a_teammate_board_pass_releases_on_its_row_and_keeps_the_planned_catch() {
    // Callback 38 (0x173570) plans the catch and both legs together. A
    // receiver standing on the floor does not become an alley-oop leaper
    // merely because the ball has reached the board.
    let theirs = [[-20.0, 20.0], [-20.0, -20.0], [-30.0, 0.0]];
    for bank in [crate::pose::Bank::StandIns, crate::pose::disc_bank()] {
        for seed in 0..4 {
            let mut w = three([[25.0, 12.0], [10.0, 0.0], [24.0, -10.0]], theirs, seed);
            w.poses = bank.clone();
            w.players[0].ratings.handles = 1.0;
            w.combo.ready = true;
            w.teams[0].possession_seen = 0;
            let toward = normalize2(sub(w.rim_spot(0), w.players[0].position));
            let mut inputs = vec![Input::default(); 6];
            inputs[0] = Input::moving(toward).with(button::PASS | button::L1 | button::R1);
            w.step(&inputs);
            let windup = w.windup.expect("the board pass waits for callback 38");
            assert_eq!((windup.from, windup.to, windup.row), (0, 2, 92));
            assert_eq!(windup.left, crate::timing::PASS_ROWS[92].release);
            inputs.fill(Input::default());
            for _ in 1..windup.left {
                w.step(&inputs);
                assert_eq!(w.ball.state, BallState::Held(0));
                assert_eq!(w.validate(), Ok(()));
            }
            w.step(&inputs);
            let pass = w.ball.pass.expect("released at callback 38");
            assert_eq!((pass.from, pass.to), (0, 2));
            assert!(matches!(pass.via, crate::ball::Via::Glass { .. }));
            assert!((35..=38).contains(&pass.catch), "{pass:?}");
            assert!(!w.players[2].leaping());
            let planned = w
                .passout
                .and_then(|po| po.back)
                .expect("catch planned at release");
            let first = w.ball.flight_ticks();
            assert!(first > 0 && first < planned.1);
            for _ in 0..first + 2 {
                // Restoring a network snapshot keeps the fixed second leg.
                let restored = World::from_snapshot(&w.to_snapshot()).unwrap();
                assert_eq!(restored.passout, w.passout);
                assert_eq!(w.validate(), Ok(()));
                if w.ball
                    .pass
                    .is_some_and(|p| p.via == crate::ball::Via::Direct)
                {
                    break;
                }
                assert_eq!(w.passout.and_then(|po| po.back), Some(planned));
                w.step(&inputs);
            }
            let back = w.ball.pass.expect("second leg");
            assert_eq!((back.from, back.to, back.catch), (0, 2, pass.catch));
            assert_eq!(back.via, crate::ball::Via::Direct);
            assert_eq!(first + w.ball.flight_ticks(), planned.1);
            assert!(!w.players[2].leaping(), "no forced jump at the board");
            for _ in 0..planned.1 + 10 {
                if w.ball.state != BallState::Pass {
                    break;
                }
                w.step(&inputs);
            }
            assert_eq!(w.ball.state, BallState::Held(2), "seed {seed}");
            assert!(!w.players[2].leaping());
            let slot = w.combo.ring.slots[5];
            assert_eq!((slot.class, slot.extra, slot.value), (17, 0, 3_000));
            assert_eq!(w.stats.player(2, crate::stats::TRICK_POINTS), 3_000);
        }
    }
}
