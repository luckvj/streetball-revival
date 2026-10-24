//! The gamebreaker meter's hold and decay (`FUN_0013eff8`,
//! `FUN_00143c20`, `FUN_00143d38`) and the match's statistics
//! (`FUN_00186ef8` and the stats listeners), through `World::step`.

use super::*;
use crate::stats;
use crate::street::{METER_DECAY, METER_HOLD, MeterClock};

/// Two players face to face, nobody moving, no shot clock: player 0 (team
/// 0) holds the ball in his own back court (x < 0: team 0 attacks +x).
fn quiet() -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-5.0, 0.0], std::f32::consts::PI),
            Player::new(1, [-5.0, 6.0], 0.0),
        ],
        Rules {
            shot_clock: None,
            ..Rules::default()
        },
        3,
    );
    w.phase = Phase::Live;
    w.ball.drop_loose();
    w.ball.position = [-5.0, 1.0, 0.0];
    w
}

fn idle(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.step(&[Input::default(), Input::default()]);
    }
}

#[test]
fn the_constants_are_the_boot_resets() {
    // 0x144c30..0x144c40: fps (60) × 30; 0x142e04..0x142f04: 50000.0 /
    // (60.0 × 600.0) in f32.
    assert_eq!(METER_HOLD, 1_800);
    assert_eq!(METER_DECAY.to_bits(), (50_000.0f32 / 36_000.0f32).to_bits());
    assert_eq!(METER_DECAY.to_bits(), 0x3fb1_c71c);
}

#[test]
fn the_meters_wait_for_a_team_to_have_the_ball_in_its_back_court() {
    use crate::game_phase::phase;
    let mut w = quiet();
    // Team 1 holds it in its front court (x < 0: it attacks -x): phase 5.
    w.take_ball(1);
    w.award(0, 10_000, false);
    assert_eq!(w.meter_clock, MeterClock::Off);
    idle(&mut w, 30);
    assert_eq!(w.game_phase.phase, phase::FRONT_COURT);
    assert_eq!(w.meter_clock, MeterClock::Off);
    assert_eq!(w.teams[0].meter, 10_000, "no decay before game+0x4a4");
    // Taken back in the half team 0 defends (a steal, slot 15: its 1,000
    // and the victim's −1,000): phase 4, and the game's update sets
    // game+0x4a4 (`FUN_0013e4c8` case 4), the meters running in the same
    // update.
    w.take_ball(0);
    assert_eq!(w.game_phase.phase, phase::BACK_COURT);
    assert_eq!(w.meter_clock, MeterClock::Off);
    let taken = w.teams[0].meter;
    assert!(taken > 10_000, "the steal's 1,000 times the fill");
    idle(&mut w, 1);
    assert_eq!(w.meter_clock, MeterClock::On);
    assert!(w.teams[0].meter < taken);
    // A restart clears it until the next catch of live play.
    w.start_restart(1);
    assert_eq!(w.meter_clock, MeterClock::Inbound);
}

#[test]
fn an_unfilled_meter_decays_by_the_fill_each_live_update() {
    let mut w = quiet();
    w.take_ball(0);
    // Medium, a person on side 0: fill 0.79375 (`meter::fill`).
    w.configure_meter_fill([true, false], None);
    w.award(0, 10_000, false);
    let fill = w.teams[0].meter_fill;
    let mut expected = 10_000.0f32 * fill;
    for _ in 0..90 {
        expected -= METER_DECAY * fill * 1.0;
        if expected < 0.0 {
            expected = 0.0;
        }
    }
    idle(&mut w, 90);
    let t = &w.teams[0];
    let got = t.meter as f32 + t.meter_fraction;
    assert!((got - expected).abs() < 0.01, "{got} vs {expected}");
    // The other side's empty meter stays at 0.
    assert_eq!(w.teams[1].meter, 0);
    assert_eq!(w.teams[1].meter_fraction, 0.0);
}

#[test]
fn a_full_meter_holds_thirty_seconds_then_empties_with_its_pocket() {
    let mut w = quiet();
    w.take_ball(0);
    w.teams[0].pocketed = true;
    w.award(0, 60_000, false);
    assert_eq!(w.teams[0].meter, street::METER_FULL);
    assert_eq!(w.teams[0].meter_hold, METER_HOLD);
    assert!(w.events.contains(&Event::GamebreakerReady { team: 0 }));
    // While the hold runs nothing lands: no drain, no fill.
    w.drain(0, 2_500);
    w.award(0, 1_000, false);
    assert_eq!(w.teams[0].meter, street::METER_FULL);
    idle(&mut w, METER_HOLD as u32 - 1);
    assert_eq!(w.teams[0].meter, street::METER_FULL);
    assert_eq!(w.teams[0].meter_hold, 1);
    assert!(w.teams[0].pocketed);
    idle(&mut w, 1);
    assert_eq!(w.teams[0].meter, 0);
    assert_eq!(w.teams[0].meter_hold, 0);
    assert!(!w.teams[0].pocketed, "the expiry takes the pocket (+0x20)");
    assert!(w.events.contains(&Event::MeterDropped { team: 0 }));
    assert_eq!(w.stats.team(0, stats::METER_FILLS_POCKETED), 1);
    assert_eq!(w.stats.team(0, stats::METER_DROPS_POCKETED), 1);
}

#[test]
fn the_shot_clock_running_out_costs_twelve_thousand_five_hundred() {
    let mut w = quiet();
    w.rules.shot_clock = Some(24.0);
    w.take_ball(0);
    // The clock has seen team 0 with the ball (`DAT_003d30c4`).
    w.clock.side = 0;
    w.shot_clock = 0.01;
    w.award(0, 20_000, false);
    w.step(&[Input::default(), Input::default()]);
    assert!(w.events.contains(&Event::ShotClockViolation { team: 0 }));
    assert!(w.teams[0].meter <= 20_000 - 12_500, "{}", w.teams[0].meter);
    assert!(w.teams[0].meter >= 20_000 - 12_500 - 5);
}

#[test]
fn a_gamebreaker_basket_scales_the_next_fills_instead_of_filling() {
    let mut w = face_off(1);
    w.rules.shot_clock = None;
    w.players[0].position = [30.0, 0.0];
    w.players[0].ratings.shots = 1.0;
    w.configure_meter_fill([true, false], None);
    let sequence = w.teams[0].meter_sequence_fill;
    assert!(sequence != 1.0);
    w.teams[0].meter = street::METER_FULL;
    w.step(&[
        buttons(button::SHOOT | button::R1 | button::R2),
        Input::default(),
    ]);
    let mut scored = false;
    for _ in 0..3 * TICK_HZ {
        w.step(&[Input::default(), Input::default()]);
        scored |= w
            .events
            .iter()
            .any(|e| matches!(e, Event::Score { team: 0, .. }));
    }
    assert!(scored);
    // `FUN_001442b8`: +0x44 *= +0x48, and the 25,000 never reach the meter.
    assert_eq!(w.teams[0].meter_scale, sequence);
    assert_eq!(w.teams[0].meter, 0);
    assert_eq!(w.teams[0].trick_points, 25_000);
    assert_eq!(w.stats.player(0, stats::LEVEL1), 1);
    assert_eq!(w.stats.player(0, stats::TRICK_POINTS), 25_000);
    // The next fill is scaled by both.
    let fill = w.teams[0].meter_fill;
    w.award(0, 1_000, false);
    let t = &w.teams[0];
    let got = t.meter as f32 + t.meter_fraction;
    assert!((got - 1_000.0 * fill * sequence).abs() < 0.01);
}

#[test]
fn a_cancel_needs_no_pocket_of_ones_own() {
    let mut w = face_off(1);
    w.teams[0].pocketed = true;
    w.teams[1].pocketed = true;
    w.teams[0].meter = street::METER_FULL;
    w.step(&[buttons(button::SELECT | button::R1), Input::default()]);
    assert!(w.teams[1].pocketed, "FUN_00141190 refuses");
    assert!(!w.events.contains(&Event::Cancel { team: 0 }));
}

#[test]
fn baskets_shots_and_time_on_court_reach_the_statistics() {
    let mut made = None;
    for seed in 1..40 {
        let mut w = face_off(seed);
        w.rules.shot_clock = None;
        // Standing 9.75 out: no dunk on (`FUN_0016c500` takes a standing
        // shooter within 7.5), a jump shot.
        w.players[0].position = [27.0, 0.0];
        w.players[0].ratings.shots = 1.0;
        let mut points = 0;
        for k in 0..3 * TICK_HZ {
            // Held through the jump (letting go early is a pump fake).
            let held = if k < 40 {
                buttons(button::SHOOT)
            } else {
                Input::default()
            };
            w.step(&[held, Input::default()]);
            for e in &w.events {
                if let Event::Score {
                    team: 0, points: p, ..
                } = *e
                {
                    points += p;
                }
            }
        }
        assert_eq!(w.stats.player(0, stats::ATTEMPTS), 1);
        if points > 0 {
            made = Some((w, points));
            break;
        }
        assert_eq!(w.stats.player(0, stats::FIELD_GOALS), 0);
    }
    let (w, points) = made.expect("a jumper went in");
    assert_eq!(w.stats.player(0, stats::ATTEMPTS), 1);
    assert_eq!(w.stats.player(0, stats::THREE_ATTEMPTS), 0);
    assert_eq!(w.stats.player(0, stats::FIELD_GOALS), 1);
    assert_eq!(w.stats.player(0, stats::POINTS), points as i32);
    assert_eq!(w.stats.team(0, stats::POINTS), points as i32);
    assert_eq!(w.stats.player(0, stats::TRICK_POINTS), 250);
    assert_eq!(w.stats.team(0, stats::BIGGEST_LEAD), points as i32);
    // Every update of live play counts for every actor (actor+0x2a4).
    assert!(w.stats.on_court[0] > 0);
    assert_eq!(w.stats.on_court[0], w.stats.on_court[1]);
}

#[test]
fn the_star_weights_follow_fun_0012a7d0() {
    let mut s = stats::Stats::new(6);
    // Before anyone has played: the shares alone.
    let w = s.star_weights(0, &[0, 1, 2], &[60, 80, 60]);
    assert_eq!(w, vec![0.3, 0.4, 0.3]);
    // Thirty seconds each; player 1 has 6 points on 5 shots, player 0 one
    // shot and no points.
    s.on_court.fill(30 * TICK_HZ);
    s.add(1, 0, stats::POINTS, 6);
    s.add(1, 0, stats::ATTEMPTS, 5);
    s.add(0, 0, stats::ATTEMPTS, 1);
    let w = s.star_weights(0, &[0, 1, 2], &[60, 80, 60]);
    // Side: (6 + 12) / 90 = 0.2; player 0: 1/30, player 1: 17/30.
    let side = 18.0f32 / 90.0;
    assert_eq!(w[0], side * (60.0f32 / 200.0) / (1.0 / 30.0));
    assert_eq!(w[1], side * (80.0f32 / 200.0) / (17.0 / 30.0));
    assert_eq!(w[2], 60.0f32 / 200.0);
    // The best is player 0 now, not the higher rated scorer.
    let best = (0..3).max_by(|&a, &b| w[a].total_cmp(&w[b])).unwrap();
    assert_eq!(best, 0);
}
