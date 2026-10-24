//! The scoring messages ([`crate::scoring`]): the combo ring the original
//! keeps, the meters' listeners and the statistics', on hand-built scenes
//! in the original's frame (team 0 attacks +x).

use super::*;
use crate::combo::{self, Slot};
use crate::scoring::BasketPost;
use crate::stats;

/// Team 0's two (0, 1) against team 1's one (2), live play, the ball held
/// by player 0 in team 0's front court, no shot clock.
fn scene() -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [20.0, 0.0], 0.0),
            Player::new(0, [24.0, 8.0], 0.0),
            Player::new(1, [22.0, 2.0], std::f32::consts::PI),
        ],
        Rules {
            shot_clock: None,
            ..Rules::default()
        },
        7,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    w.possession = 0;
    w.sync_game_phase();
    w
}

fn meter(w: &World, team: usize) -> f32 {
    w.teams[team].meter as f32 + w.teams[team].meter_fraction
}

/// The ball in the air from `from` to `to`, for the next take.
fn pass_to(w: &mut World, from: usize, to: usize) {
    w.ball.state = BallState::Pass;
    w.ball.pass = Some(Pass {
        from,
        to,
        tried: 0,
        via: crate::ball::Via::Direct,
        catch: crate::ball::NO_CATCH,
        since: w.tick,
    });
}

#[test]
fn a_steal_pays_a_thousand_and_drains_the_victim() {
    let mut w = scene();
    w.teams[0].meter = 10_000;
    w.teams[1].meter = 10_000;
    w.take_ball(2);
    let fill = [w.teams[0].meter_fill, w.teams[1].meter_fill];
    // `FUN_00144490`: the taker 1,000 (plus a total of 0), the victim
    // −1,000 (`0x43e248[8]`), each times its side's fill.
    assert!((meter(&w, 1) - (10_000.0 + 1_000.0 * fill[1])).abs() < 0.01);
    assert!((meter(&w, 0) - (10_000.0 - 1_000.0 * fill[0])).abs() < 0.01);
    assert_eq!(w.stats.player(2, stats::STEALS), 1);
    assert_eq!(w.stats.player(2, stats::TRICK_POINTS), 1_000);
    assert_eq!(w.stats.team(1, 0x1d), 1_000);
    assert_eq!(w.stats.team(0, 0x10), 1_000);
    assert_eq!(w.teams[1].trick_points, 1_000);
    // The ring: side 1's, class 20 with the class's own value.
    assert_eq!(w.combo.side, 1);
    assert_eq!(w.combo.ring.slots[5].class, 20);
    assert_eq!(w.combo.ring.slots[5].paid_value(), 1_000);
}

#[test]
fn a_steal_inside_the_window_after_its_sides_block_pays_both() {
    // The played capture's case (`fades-1`, update 3,937): a block, then a
    // steal 72 updates later — row 49, 2,500 × 1.25 + 1,000 × 1.25.
    for (gap, paid) in [(72, 4_375), (120, 4_375), (121, 0)] {
        let mut w = scene();
        w.post_block(2, false);
        w.tick += gap;
        w.combo_update();
        let before = meter(&w, 1);
        w.take_ball(2);
        let fill = w.teams[1].meter_fill;
        let expected = before + (1_000 + paid) as f32 * fill;
        assert!(
            (meter(&w, 1) - expected).abs() < 0.05,
            "gap {gap}: {} against {expected}",
            meter(&w, 1)
        );
        assert_eq!(w.stats.player(2, stats::TRICK_POINTS), 2_500 + 1_000 + paid);
        assert_eq!(w.combo.total, paid);
        // The total lives one update.
        w.combo_update();
        assert_eq!(w.combo.total, 0);
    }
}

#[test]
fn a_caught_shot_announces_a_block_and_a_made_baskets_pickup_is_not_a_rebound() {
    let mut w = scene();
    w.ball.state = BallState::Shot;
    w.ball.shooter = Some(0);
    w.take_ball(2);
    assert!(w.events.contains(&Event::Block {
        player: 2,
        shooter: 0
    }));
    assert!(!w.events.iter().any(|e| matches!(e, Event::Rebound { .. })));
    assert_eq!(w.stats.player(2, stats::BLOCKS), 1);
    assert_eq!(w.stats.player(2, stats::REBOUNDS), 0);

    let mut w = scene();
    w.score_basket(0, Basket::Jumper { long: false });
    w.events.clear();
    w.take_ball(2);
    assert!(!w.events.iter().any(|e| matches!(e, Event::Rebound { .. })));
    assert_eq!(w.stats.player(2, stats::REBOUNDS), 0);
}

#[test]
fn a_rebound_clears_the_ring_and_pays_five_hundred() {
    let mut w = scene();
    let now = w.scoring_now();
    w.combo.own(0);
    w.combo.push(Slot::new(10, 0, now, 500), false);
    w.combo.push(Slot::new(10, 1, now, 1_000), false);
    // A shot off the rim (state 0) taken by player 1.
    w.ball.drop_loose();
    w.ball.rebound = true;
    w.take_ball(1);
    assert_eq!(w.combo.ring.depth(), 1);
    assert_eq!(w.combo.ring.slots[5].class, 16);
    assert!((meter(&w, 0) - 500.0 * w.teams[0].meter_fill).abs() < 0.01);
    assert_eq!(w.stats.player(1, stats::REBOUNDS), 1);
    assert_eq!(w.stats.player(1, stats::TRICK_POINTS), 500);
    // A dive's 500 stays out of a take the rebound paid.
    assert!(!w.teams[0].paid && !w.stats.paid);
}

#[test]
fn a_loose_ball_off_the_other_sides_shot_is_no_steal() {
    // Blocked: the ball's shooter (+0xf8) is the shooter's side's last
    // toucher, so the blocker's pickup posts nothing (slot 25).
    let mut w = scene();
    w.ball.drop_loose();
    w.ball.shooter = Some(0);
    w.ball.touch = [Some(0), Some(2)];
    w.take_ball(2);
    assert_eq!(w.stats.player(2, stats::STEALS), 0);
    assert_eq!(w.teams[1].meter, 0);
    // Knocked out of the holder's hands: the swiper's side's pickup steals.
    let mut w = scene();
    w.ball.drop_loose();
    w.ball.shooter = None;
    w.ball.touch = [Some(0), Some(2)];
    w.take_ball(2);
    assert_eq!(w.stats.player(2, stats::STEALS), 1);
}

#[test]
fn a_dunk_fills_its_finish_and_a_tip_slam_two_thousand() {
    for (finish, tip, value) in [
        (0, false, 1_000),
        (2, false, 3_000),
        (4, false, 7_500),
        (1, true, 2_000),
    ] {
        let mut w = scene();
        w.post_basket(BasketPost {
            shooter: 0,
            assister: None,
            base: 2,
            kind: 4,
            gamebreaker: 0,
            wins: false,
            tip_slam: tip,
            finish,
            style: 0,
        });
        let fill = w.teams[0].meter_fill;
        assert!(
            (meter(&w, 0) - value as f32 * fill).abs() < 0.01,
            "finish {finish}"
        );
        assert_eq!(w.stats.player(0, stats::TRICK_POINTS), value);
        let slot = w.combo.ring.slots[5];
        if tip {
            assert_eq!((slot.class, slot.paid_value()), (5, 2_000));
        } else {
            assert_eq!((slot.class, slot.extra, slot.value), (0, finish, value));
        }
        assert!(w.combo.basket, "the ring goes at the next update");
    }
}

#[test]
fn a_game_winning_dunk_pays_no_meter_and_its_own_class() {
    let mut w = scene();
    w.post_basket(BasketPost {
        shooter: 0,
        assister: None,
        base: 2,
        kind: 4,
        gamebreaker: 0,
        wins: true,
        tip_slam: false,
        finish: 1,
        style: 0,
    });
    assert_eq!(w.teams[0].meter, 0);
    assert_eq!(w.combo.ring.slots[5].class, 6);
    // The statistics' kind 14: 25,000 (`0x440838[14]`).
    assert_eq!(w.stats.player(0, stats::TRICK_POINTS), 25_000);
}

#[test]
fn a_gamebreakers_value_follows_fun_001439e8() {
    use crate::scoring::gamebreaker_value;
    assert_eq!(gamebreaker_value(1, 2, 2), 25_000);
    assert_eq!(gamebreaker_value(1, 4, 2), 50_000);
    assert_eq!(gamebreaker_value(2, 2, 2), 100_000);
    assert_eq!(gamebreaker_value(2, 2, 3), 50_000);
    assert_eq!(gamebreaker_value(2, 3, 2), 250_000);
}

#[test]
fn passes_enter_the_ring_at_the_catch_by_the_throw() {
    let mut w = scene();
    // The match's first pass is skipped (+0x16c).
    w.post_pass_thrown(0);
    pass_to(&mut w, 0, 1);
    w.take_ball(1);
    assert!(w.combo.ring.is_empty());
    // A plain pass from the floor: nothing.
    w.post_pass_thrown(1);
    pass_to(&mut w, 1, 0);
    w.take_ball(0);
    assert!(w.combo.ring.is_empty());
    // A kick pass (pass kind 5) from the floor: class 9.
    w.players[0].pass_kind = 5;
    w.post_pass_thrown(0);
    pass_to(&mut w, 0, 1);
    w.take_ball(1);
    assert_eq!(w.combo.ring.slots[5].class, 9);
    // From the air: class 8, worth 750 to the next payout and nothing now.
    w.players[1].height = 3.0;
    let before = w.teams[0].meter;
    w.post_pass_thrown(1);
    pass_to(&mut w, 1, 0);
    w.take_ball(0);
    assert_eq!(w.combo.ring.slots[5].class, 8);
    assert_eq!(w.teams[0].meter, before, "a pass fills nothing");
}

#[test]
fn a_passout_special_fills_three_thousand_and_keys_the_repeat_ring() {
    let mut w = scene();
    w.combo.ready = true;
    w.teams[0].possession_seen = 0;
    pass_to(&mut w, 0, 0);
    w.take_ball(0);
    let fill = w.teams[0].meter_fill;
    assert!((meter(&w, 0) - 3_000.0 * fill).abs() < 0.01);
    assert_eq!(w.stats.player(0, stats::TRICK_POINTS), 3_000);
    assert_eq!(w.teams[0].recent_moves[0], -3);
    let slot = w.combo.ring.slots[5];
    assert_eq!((slot.class, slot.extra, slot.value), (17, 0, 3_000));
    // A second passout halves (`FUN_00143a88`'s ring count) and flags the
    // extra (team+0x5c != 3,000); the meter still takes a flat 3,000.
    pass_to(&mut w, 0, 0);
    w.take_ball(0);
    assert_eq!(w.stats.player(0, stats::TRICK_POINTS), 4_500);
    let slot = w.combo.ring.slots[5];
    assert_eq!((slot.class, slot.extra, slot.value), (17, 1, 1_500));
    assert!((meter(&w, 0) - 6_000.0 * fill).abs() < 0.01);
}

#[test]
fn no_gamebreakers_and_back_to_basics_fill_no_meter() {
    for mode in [Mode::NoGamebreakers, Mode::BackToBasics] {
        let mut w = scene();
        w.rules = Rules {
            shot_clock: None,
            ..Rules::mode(mode)
        };
        w.take_ball(2);
        assert_eq!(w.teams[1].meter, 0, "{mode:?}");
        assert_eq!(
            w.rules.game_type(),
            Mode::ALL.iter().position(|&m| m == mode).unwrap() as u32
        );
    }
    for mode in Mode::ALL {
        let rules = Rules::mode(mode);
        assert_eq!(
            rules.game_type() as usize,
            Mode::ALL.iter().position(|&m| m == mode).unwrap()
        );
    }
}

#[test]
fn a_lesson_stops_the_meter_short_but_the_gamebreaker_lessons() {
    for (lesson, full) in [
        (1u8, false),
        (0xe, true),
        (0x17, true),
        (0x19, true),
        (0x1a, false),
    ] {
        let mut w = scene();
        w.lesson = Some(lesson);
        w.teams[0].meter_fill = 1.0;
        w.fill_meter(0, 60_000.0);
        if full {
            assert!(w.teams[0].meter_full(), "lesson {lesson}");
            assert_eq!(w.teams[0].meter_hold, street::METER_HOLD);
        } else {
            assert_eq!(w.teams[0].meter, 49_999, "lesson {lesson}");
            assert_eq!(w.teams[0].meter_hold, 0);
        }
    }
}

#[test]
fn an_assist_is_the_passer_a_held_ball_keeps_two_seconds() {
    let mut w = scene();
    pass_to(&mut w, 0, 1);
    w.take_ball(1);
    assert_eq!(w.assist, Some((0, crate::scoring::ASSIST_UPDATES)));
    w.score_basket(1, Basket::Jumper { long: false });
    assert_eq!(w.stats.player(0, stats::ASSISTS), 1);
    assert_eq!(w.stats.team(0, stats::ASSISTS), 1);
    // Held 120 updates after the catch, the passer is forgotten.
    let mut w = scene();
    pass_to(&mut w, 0, 1);
    w.take_ball(1);
    for _ in 0..crate::scoring::ASSIST_UPDATES {
        w.step(&[Input::default(); 3]);
    }
    assert_eq!(w.assist, None);
    // A take by the other side keeps nobody.
    let mut w = scene();
    pass_to(&mut w, 0, 2);
    w.take_ball(2);
    assert_eq!(w.assist, None);
}

#[test]
fn the_repeat_ring_clears_at_a_catch_after_the_possession_changes() {
    let mut w = scene();
    w.teams[0].recent_moves = [5, 5, 5, 5];
    w.teams[0].possession_seen = 0;
    w.take_ball(1);
    assert_eq!(
        w.teams[0].recent_moves,
        [5, 5, 5, 5],
        "the same side's catch"
    );
    w.take_ball(2);
    assert_eq!(w.teams[0].recent_moves, [-1; 4]);
    assert_eq!(w.teams[0].possession_seen, 1);
}

#[test]
fn a_pump_fake_at_a_jumper_pays_its_side_five_hundred() {
    let mut w = scene();
    w.post_pump_fake();
    w.post_pump_fake_points(2);
    let fill = w.teams[0].meter_fill;
    assert!((meter(&w, 0) - 500.0 * fill).abs() < 0.01);
    assert_eq!(w.stats.player(0, stats::TRICK_POINTS), 500);
    assert_eq!(w.stats.team(0, 0x20), 500);
    assert_eq!(w.combo.ring.slots[5].class, 13);
    // Unarmed (no pump fake since), nothing more.
    w.post_pump_fake_points(2);
    assert_eq!(w.stats.player(0, stats::TRICK_POINTS), 500);
    assert_eq!(w.combo.ring.depth(), 1);
}

#[test]
fn a_screen_on_the_holders_own_defender_by_the_arc_is_a_pick() {
    let mut w = scene();
    // The holder 20 from the rim team 0 attacks (+36.75), his defender
    // (player 2, the one in his slot) 15 from it and 5 from him.
    w.players[0].position = [16.75, 0.0];
    w.players[2].position = [21.75, 0.0];
    assert_eq!(w.screen_kind(2), 3);
    w.post_contact(1, 3);
    let fill = w.teams[0].meter_fill;
    assert!((meter(&w, 0) - 500.0 * fill).abs() < 0.01);
    assert_eq!(w.stats.player(1, stats::TRICK_POINTS), 500);
    assert_eq!(w.combo.ring.slots[5].class, 12);
    // Too close to the rim: kind 2, nothing.
    w.players[0].position = [26.75, 0.0];
    assert_eq!(w.screen_kind(2), 2);
}

#[test]
fn the_ring_is_one_for_the_match_and_its_window_ends_a_chain() {
    let mut w = scene();
    w.post_trick(0, 2, 7, true);
    assert_eq!((w.combo.side, w.combo.ring.depth()), (0, 1));
    // A trick by the same side 200 updates later: the first one is stale
    // for the basket's matcher, which then walks the newer trick alone.
    w.tick += 200;
    w.post_trick(0, 2, 8, true);
    w.tick += 10;
    let before = w.stats.player(0, stats::TRICK_POINTS);
    w.post_basket(BasketPost {
        shooter: 0,
        assister: None,
        base: 2,
        kind: 2,
        gamebreaker: 0,
        wins: false,
        tip_slam: false,
        finish: 0,
        style: 0,
    });
    let payout = w.combo.total;
    // Two events in the window: the shot (250 × 1.25) and the fakeout of
    // value class 2 (2,000 × 1.25, row 34 column 0).
    assert_eq!(payout, (250.0f32 * 1.25 + 2_000.0 * 1.25) as i32);
    assert_eq!(
        w.stats.player(0, stats::TRICK_POINTS) - before,
        250 + payout
    );
    let _ = combo::WINDOW;
}

/// The shot clock (`FUN_0013f1f0`) on a scene that has seen team 0 with
/// the ball.
fn clocked() -> World {
    let mut w = scene();
    w.rules.shot_clock = Some(24.0);
    w.shot_clock = 24.0;
    w.clock.side = 0;
    w
}

#[test]
fn the_shot_clock_runs_in_live_play_and_refills_for_a_new_side() {
    let mut w = clocked();
    w.step_shot_clock();
    assert!((w.shot_clock - (24.0 - TICK_SECONDS)).abs() < 1e-5);
    // The other side with the ball: full again at the next look.
    w.shot_clock = 5.0;
    w.possession = 1;
    w.step_shot_clock();
    // Refilled, then run for this update.
    assert!((w.shot_clock - (24.0 - TICK_SECONDS)).abs() < 1e-5);
    assert_eq!(w.clock.side, 1);
}

#[test]
fn a_falling_shot_and_the_rim_refill_the_clock() {
    let mut w = clocked();
    w.shot_clock = 3.0;
    w.ball.drop_loose();
    w.ball.state = BallState::Shot;
    w.ball.velocity = [0.0, -1.0, 0.0];
    w.step_shot_clock();
    let ran = 24.0 - TICK_SECONDS;
    assert!((w.shot_clock - ran).abs() < 1e-5, "a shot on its way down");
    // The rim at the end team 0 attacks: full, and stopped until a hold.
    w.shot_clock = 3.0;
    w.ball.state = BallState::Loose;
    w.ball.position = [36.0, 11.9, 0.0];
    w.rim_touched();
    assert_eq!(w.shot_clock, 24.0);
    assert!(w.clock.stopped);
    w.step_shot_clock();
    assert_eq!(w.shot_clock, 24.0, "stopped");
    w.ball.hold(1);
    w.step_shot_clock();
    assert!(!w.clock.stopped);
    assert!(w.shot_clock < 24.0);
    // The rim at the other end does nothing.
    let mut w = clocked();
    w.shot_clock = 3.0;
    w.ball.position = [-36.0, 11.9, 0.0];
    w.rim_touched();
    assert_eq!(w.shot_clock, 3.0);
}

#[test]
fn a_clock_run_out_waits_for_a_shot_and_for_a_loose_ball() {
    // Nobody holding: wait for a loose ball below 12.
    let mut w = clocked();
    w.shot_clock = TICK_SECONDS * 0.5;
    w.ball.drop_loose();
    w.ball.position = [20.0, 14.0, 0.0];
    w.step_shot_clock();
    assert!(w.clock.loose);
    assert!(!w.events.contains(&Event::ShotClockViolation { team: 0 }));
    w.step_shot_clock();
    assert!(w.clock.loose, "still up at 14");
    w.ball.position[1] = 5.0;
    w.step_shot_clock();
    assert!(w.events.contains(&Event::ShotClockViolation { team: 0 }));
    assert!(matches!(w.phase, Phase::DeadBall { team: 1, .. }));
    // A shot on its way when it runs out: the rim ends the wait.
    let mut w = clocked();
    w.shot_clock = TICK_SECONDS * 0.5;
    w.clock.shot = false;
    w.ball.drop_loose();
    w.ball.state = BallState::Shot;
    w.ball.velocity = [0.0, 1.0, 0.0];
    w.step_shot_clock();
    assert!(w.clock.loose, "no holder: the loose wait");
    w.ball.position = [36.0, 11.9, 0.0];
    w.rim_touched();
    assert!(!w.clock.loose && w.shot_clock == 24.0, "the refill ends it");
}

/// The replay director's basket handler (`FUN_003248d8`) draws once from
/// the shared generator after a basket that scores without winning, on
/// Always and Automatic; on Manual only with a gamebreaker ball or a full
/// meter (it offers the replay instead); never for the winning basket.
/// Then the game's listener draws for the faces (`FUN_00141818`): in this
/// scene of three, the scorer's one, his teammate's two and the scorer's
/// defender's two after a jump shot.
#[test]
fn a_basket_draws_the_directors_roll_from_the_shared_generator() {
    const FACES: usize = 5;
    let roll = |w: &World| {
        w.events
            .iter()
            .filter_map(|e| match *e {
                Event::ReplayRoll { draw } => Some(draw),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for option in [0, 1] {
        let mut w = scene();
        w.replays = option;
        w.events.clear();
        let mut expected = w.rng.clone();
        w.score_basket(0, Basket::Jumper { long: false });
        let draws = roll(&w);
        assert_eq!(draws, [expected.next_u32()], "option {option}");
        // It follows the basket's own event.
        let at = w
            .events
            .iter()
            .position(|e| matches!(e, Event::Score { .. }));
        assert!(matches!(
            w.events.get(at.unwrap() + 1),
            Some(Event::ReplayRoll { .. })
        ));
        for _ in 0..FACES {
            expected.next_u32();
        }
        assert_eq!(w.rng.state(), expected.state());
    }
    // Manual: no draw, unless a meter is full.
    let mut w = scene();
    w.replays = 2;
    let mut expected = w.rng.clone();
    w.score_basket(0, Basket::Jumper { long: false });
    assert!(roll(&w).is_empty());
    for _ in 0..FACES {
        expected.next_u32();
    }
    assert_eq!(w.rng.state(), expected.state());
    let mut w = scene();
    w.replays = 2;
    w.teams[0].meter = crate::street::METER_FULL;
    w.score_basket(0, Basket::Jumper { long: false });
    assert_eq!(roll(&w).len(), 1);
    // The winning basket: the director forces its replay without a draw.
    let mut w = scene();
    w.score = [20, 0];
    w.events.clear();
    w.score_basket(0, Basket::Jumper { long: true });
    assert!(matches!(w.phase, Phase::Over { .. }));
    assert!(roll(&w).is_empty());
}

/// `FUN_00141818` (raw `0x141968..0x141c08`): after the restart a basket
/// bars every actor 40 updates, and draws nine times for the six's faces
/// (a dunk: the scorer two, his teammates two each, the defender and the
/// other two one each).
#[test]
fn a_basket_bars_all_six_and_draws_for_their_faces() {
    let mut players = Vec::new();
    for team in 0..2 {
        for k in 0..3 {
            players.push(Player::new(team, [20.0 + k as f32, k as f32 * 4.0], 0.0));
        }
    }
    let mut w = World::new(hoops(), players, Rules::default(), 3);
    w.phase = Phase::Live;
    w.ball.hold(0);
    w.possession = 0;
    w.sync_game_phase();
    w.replays = 2;
    let mut expected = w.rng.clone();
    w.score_basket(0, Basket::Dunk { advanced: false });
    assert!(w.players.iter().all(|p| p.barred == crate::BASKET_BAR));
    for _ in 0..9 {
        expected.next_u32();
    }
    assert_eq!(w.rng.state(), expected.state());
}
