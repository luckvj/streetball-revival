use super::*;

/// The baskets where the Vol. 2 scene setup puts them.
pub(crate) fn hoops() -> [Hoop; 2] {
    [
        Hoop {
            rim: [36.95, 11.75, 0.0],
            rim_radius: 0.75,
        },
        Hoop {
            rim: [-36.96, 11.75, 0.0],
            rim_radius: 0.75,
        },
    ]
}

/// A live game with these players and a loose ball at centre court.
fn live(players: Vec<Player>) -> World {
    let mut w = World::new(hoops(), players, Rules::default(), 7);
    w.phase = Phase::Live;
    w
}

fn solo(at: [f32; 2]) -> World {
    live(vec![Player::new(0, at, 0.0)])
}

/// Three on three, teams lined up in their own halves.
fn three_on_three(seed: u32) -> World {
    let mut players = Vec::new();
    for team in 0..2 {
        let side = crate::restart::side(team);
        for k in 0..3 {
            let heading = -side * std::f32::consts::FRAC_PI_2;
            players.push(Player::new(
                team,
                [side * 10.0, (k as f32 - 1.0) * 8.0],
                heading,
            ));
        }
    }
    World::new(hoops(), players, Rules::default(), seed)
}

fn run(world: &mut World, input: Input, ticks: u32) {
    for _ in 0..ticks {
        world.step(&[input]);
    }
}

fn stick(x: f32, z: f32) -> Input {
    Input::moving([x, z])
}

fn buttons(b: u16) -> Input {
    Input::default().with(b)
}

/// Step with every player driven by the AI.
fn ai_step(w: &mut World) {
    let inputs: Vec<Input> = (0..w.players.len()).map(|i| ai::input(w, i)).collect();
    w.step(&inputs);
}

#[test]
fn a_held_stick_runs_the_player_that_way() {
    let mut w = solo([0.0, 10.0]);
    run(&mut w, stick(1.0, 0.0), TICK_HZ);
    let p = &w.players[0];
    assert!(
        (p.heading - std::f32::consts::FRAC_PI_2).abs() < 1e-3,
        "faces +x"
    );
    assert_eq!(p.gait, Gait::Run);
    assert!((p.speed - tuning::RUN_SPEED).abs() < 1e-3);
    // Most of a second at run speed, less the turn and acceleration.
    assert!(p.position[0] > 10.0 && p.position[0] < tuning::RUN_SPEED);
    assert!((p.position[1] - 10.0).abs() < 1.0);

    run(&mut w, Input::default(), TICK_HZ);
    assert_eq!(w.players[0].gait, Gait::Stand);
    assert_eq!(w.players[0].speed, 0.0);
}

#[test]
fn players_stay_on_the_court() {
    let mut w = solo([40.0, 10.0]);
    let sprint = stick(1.0, 0.0).with(button::R2);
    run(&mut w, sprint, 10 * TICK_HZ);
    assert_eq!(w.players[0].position[0], tuning::COURT_HALF_LENGTH);
}

#[test]
fn walking_onto_the_ball_picks_it_up() {
    let mut w = solo([-5.0, 0.0]);
    assert_eq!(w.ball.state, BallState::Loose);
    run(&mut w, stick(1.0, 0.0), TICK_HZ);
    assert_eq!(w.ball.state, BallState::Held(0));
}

#[test]
fn the_game_starts_with_an_inbound_by_team_0() {
    let mut w = three_on_three(1);
    assert!(matches!(w.phase, Phase::DeadBall { team: 0, .. }));
    // A restart with the ball at centre court: the computer walks the six
    // (people's players too) and team 0 inbounds from row 2's first spot,
    // past the sideline at x -12 in its own half (-x, `0x3cea20`).
    let mut passer = None;
    for _ in 0..TICK_HZ * 12 {
        w.step(&[Input::default(); 6]);
        if let Some(Event::Inbound { player }) =
            w.events.iter().find(|e| matches!(e, Event::Inbound { .. }))
        {
            passer = Some(*player);
            break;
        }
    }
    assert_eq!(w.phase, Phase::Live);
    let h = passer.expect("team 0 inbounds");
    assert_eq!(w.team_of(h), 0);
    assert_eq!(w.restart_plan().row, 2);
    let at = w.players[h].position;
    assert!(at[1] > 30.0 && (at[0] + 12.0).abs() < 1.5, "{at:?}");
}

#[test]
fn shots_score_for_the_shooters_team() {
    let mut w = solo([-5.0, 0.0]);
    run(&mut w, stick(1.0, 0.0), TICK_HZ / 2);
    assert_eq!(w.ball.state, BallState::Held(0));
    let mut shots = 0;
    let mut points = 0;
    for _ in 0..40 {
        // Get the ball back if needed, by walking to it.
        for _ in 0..(10 * TICK_HZ) {
            w.phase = Phase::Live;
            if w.ball.state == BallState::Held(0) {
                break;
            }
            let b = w.ball.position;
            let p = w.players[0].position;
            let dir = normalize2([b[0] - p[0], b[2] - p[1]]);
            w.step(&[stick(dir[0], dir[1])]);
        }
        assert_eq!(w.ball.state, BallState::Held(0), "ball not recovered");
        // Line up 10 in front of team 0's (+x) rim, then shoot.
        for _ in 0..(10 * TICK_HZ) {
            let p = w.players[0].position;
            let to = [27.0 - p[0], -p[1]];
            if to[0].hypot(to[1]) < 1.0 {
                break;
            }
            let d = normalize2(to);
            w.step(&[stick(d[0], d[1])]);
        }
        // A shot needs the button held through its script's check tick
        // (callback 20, 12..15 updates in), else it is a pump fake.
        run(&mut w, buttons(button::SHOOT), 20);
        shots += 1;
        run(&mut w, Input::default(), 2 * TICK_HZ);
        for e in &w.events {
            if let Event::Score {
                points: p, team, ..
            } = e
            {
                assert_eq!(*team, 0);
                points += p;
            }
        }
        points = w.score[0];
        run(&mut w, Input::default(), 4 * TICK_HZ);
    }
    // About 70% from 10 for an average shooter, open: well over half go in.
    assert!(points > shots / 2, "{points} points from {shots} shots");
    assert!(points < shots, "some shots miss");
}

#[test]
fn shots_from_beyond_the_arc_are_worth_two() {
    let w = solo([0.0, 0.0]);
    assert!(w.is_long_range([-5.0, 0.0], 1));
    assert!(!w.is_long_range([-20.0, 0.0], 1));
    assert!(w.is_long_range([-40.0, 23.0], 1), "corner");
}

#[test]
fn passes_reach_the_teammate() {
    let mut w = live(vec![
        Player::new(0, [0.0, 0.0], 0.0),
        Player::new(0, [0.0, 20.0], 0.0),
    ]);
    w.ball.hold(0);
    w.step(&[stick(0.0, 1.0).with(button::PASS), Input::default()]);
    // The press is the pass (its event now), but the ball stays in the
    // passer's hands until the pass animation lets it go
    // (`crate::passing::Windup`): the drawn pass row's callback 38 in
    // live play (`timing::pass_row`).
    assert!(
        w.events
            .iter()
            .any(|e| matches!(e, Event::Pass { from: 0, to: 1, .. }))
    );
    assert!(matches!(w.players[0].action, Action::Passing { .. }));
    // Snapshots carry the pass in the hands.
    let back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back.windup, w.windup);
    let left = w.windup.expect("the ball in the hands").left;
    assert!((4..=46).contains(&left), "{left}");
    for k in 1..left {
        assert_eq!(w.ball.state, BallState::Held(0), "update {k} after");
        assert!(w.windup.is_some());
        w.step(&[Input::default(), Input::default()]);
    }
    assert_eq!(w.ball.state, BallState::Held(0));
    w.step(&[Input::default(), Input::default()]);
    assert_eq!(w.ball.state, BallState::Pass, "released");
    // The launcher's slot 18 (`0x14c2a4`): thrown from the holder's hands.
    assert!(w.events.contains(&Event::Launched {
        from: 0,
        to: 1,
        kick: false,
        leaper: false,
    }));
    assert!(w.windup.is_none());
    assert!(matches!(w.players[0].action, Action::Passing { .. }));
    let mut takes = Vec::new();
    for _ in 0..TICK_HZ {
        w.step(&[Input::default(), Input::default()]);
        takes.extend(
            w.events
                .iter()
                .filter(|e| matches!(e, Event::Take { .. }))
                .copied(),
        );
    }
    assert_eq!(w.ball.state, BallState::Held(1));
    // The catch is a take (`FUN_0014cb28`'s tail posts slot 24).
    assert_eq!(takes, [Event::Take { player: 1 }]);
}

#[test]
fn a_pass_ahead_plays_a_forward_row_and_its_catch_faces_the_ball() {
    // `FUN_0018d768` in the original's frame (docs/TRACE.md "The sector in
    // the original's frame"): a passer facing a receiver straight ahead
    // reads sector 4 and draws a row for the receiver ahead (the forward
    // rows' 0x38), and the receiver facing him reads the ball's way in as
    // sector 4 and draws a catch row for a ball coming at his face
    // (`catch_stand_1`'s 0x10). The sim's old reading took both as 0: the
    // behind rows, and a catch from behind.
    let mut exact = 0;
    for seed in 0..16 {
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [0.0, 0.0], 0.0),
                Player::new(0, [0.0, 20.0], std::f32::consts::PI),
            ],
            Rules::default(),
            seed,
        );
        w.phase = Phase::Live;
        w.ball.hold(0);
        w.step(&[buttons(button::PASS), Input::default()]);
        let Action::Passing { row, .. } = w.players[0].action else {
            panic!("seed {seed}: a pass");
        };
        let flags = timing::PASS_ROWS[usize::from(row)].flags;
        assert!(
            flags & 0x10 != 0 && flags & 0xff != 0xff,
            "seed {seed}: row {row} ({flags:#x})"
        );
        for _ in 0..60 {
            if w.ball.state == BallState::Pass {
                break;
            }
            w.step(&[Input::default(), Input::default()]);
        }
        let catch = w.ball.pass.as_ref().expect("in flight").catch;
        let flags = timing::CATCH_ROWS[usize::from(catch)].flags;
        assert!(
            flags & 0x10 != 0,
            "seed {seed}: catch row {catch} ({flags:#x})"
        );
        exact += usize::from(flags & 0xff == 0x10);
    }
    assert!(exact > 0, "no catch row read the ball at his face alone");
}

#[test]
fn passes_go_to_the_teammate_the_stick_points_at() {
    let w = live(vec![
        Player::new(0, [0.0, 0.0], 0.0),
        Player::new(0, [10.0, 0.0], 0.0),
        Player::new(0, [-10.0, 0.0], 0.0),
        Player::new(1, [0.0, -10.0], 0.0),
    ]);
    let start = w.floor_positions();
    let word = |s: [f32; 2]| crate::passing::stick_word(s);
    assert_eq!(w.lined_up(0, word([1.0, 0.0]), &start), Some(1));
    assert_eq!(w.lined_up(0, word([-1.0, 0.1]), &start), Some(2));
}

#[test]
fn possession_emitters_preserve_steal_origin() {
    let mut seen = [false; 2];
    for seed in 0..200 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        for intercepted_pass in [false, true] {
            // The defender faces the handler with his swiping hand on the
            // carried ball (0.9 in front), 99 Steals.
            let mut w = World::new(
                hoops(),
                vec![
                    Player::new(0, [0.0, 0.0], 0.0),
                    Player::new(0, [10.0, 0.0], 0.0),
                    Player::new(1, [0.0, tuning::HAND_REACH + 0.9], std::f32::consts::PI),
                ],
                Rules::default(),
                seed,
            );
            w.players[2].ratings.steals = 1.0;
            w.phase = Phase::Live;
            w.take_ball(0);
            if intercepted_pass {
                w.throw(0, 1, PassStyle::Normal);
                // The pass's last quarter, at the defender.
                w.ball.flight_left = w.ball.flight_time * 0.2;
                let at = w.players[2].position;
                w.ball.position = [at[0], 1.0, at[1]];
                w.step_pass();
            } else {
                let holder = w.players[0].clone();
                w.ball.carry(&holder);
                w.swipe(2);
            }
            for event in &w.events {
                if let Event::Steal {
                    player,
                    from,
                    intercepted_pass: origin,
                } = event
                {
                    assert_eq!((*player, *from, *origin), (2, 0, intercepted_pass));
                    seen[usize::from(intercepted_pass)] = true;
                }
            }
        }
    }
    assert_eq!(seen, [true, true], "both actual emitters must be exercised");
}

/// A swipe by player 1 standing `gap` in front of handler 0 (both
/// facing each other), with the defender's Steals: what became of the
/// ball a second later.
fn swipe_at(gap: f32, steals: f32, seed: u32) -> (BallState, World) {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(1, [0.0, gap], std::f32::consts::PI),
        ],
        Rules::default(),
        seed,
    );
    w.players[1].ratings.steals = steals;
    w.phase = Phase::Live;
    w.ball.hold(0);
    w.step(&[Input::default(), buttons(button::TRICK)]);
    let mut first = None;
    for _ in 0..TICK_HZ {
        w.step(&[Input::default(), Input::default()]);
        if first.is_none() && w.ball.state != BallState::Held(0) {
            first = Some(w.ball.state);
        }
    }
    (first.unwrap_or(w.ball.state), w)
}

#[test]
fn a_swipe_strips_the_ball_like_fun_0016cf30() {
    // The hand (`HAND_REACH` ahead) within 1.5 of the carried ball: with
    // 50 Steals the strip lands at `r % 100 <= 32 + 20`, about 53%, and
    // takes the ball outright only when `r % 100 <= 0` (Steals 50: (50 −
    // 50) · 1.3); the rest go loose. The handler's Handles play no part.
    let (mut stripped, mut caught) = (0, 0);
    for seed in 0..100 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        let (state, w) = swipe_at(tuning::HAND_REACH + 0.9, 0.5, seed);
        match state {
            BallState::Loose => {
                stripped += 1;
                // Knocked away along the swiper's facing at 0.35 an update.
                let v = w.ball.velocity;
                assert!(v[0].hypot(v[2]) > 0.0);
            }
            BallState::Held(1) => caught += 1,
            _ => {}
        }
    }
    assert!((38..70).contains(&stripped), "{stripped} of 100");
    assert!(caught <= 4, "{caught}");
    // 99 Steals catch it outright up to `r % 100 <= 63`.
    let (mut caught, mut loose) = (0, 0);
    for seed in 0..100 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        match swipe_at(tuning::HAND_REACH + 0.9, 1.0, seed).0 {
            BallState::Held(1) => caught += 1,
            BallState::Loose => loose += 1,
            _ => {}
        }
    }
    assert!(caught > loose && caught + loose > 70, "{caught} {loose}");
    // Out of the hand's reach nothing happens, however close.
    for gap in [1.0, 2.5, 7.5] {
        assert_eq!(swipe_at(gap, 1.0, 7).0, BallState::Held(0), "{gap}");
    }
}

/// The strip's knock (`0x16d3e0..0x16d7d4`): the standing holder reels on
/// `steal_reaction_l_1` ([`trick::STRIPPED`]) and is barred from the ball
/// `r % 5 + 25` updates, the swiper `fps × 0.84` (50), and the ball spins
/// about a unit axis at 0.075..0.15. The swiper's swing (slot 7) is
/// posted with the knock.
#[test]
fn a_strip_knocked_loose_bars_both_and_reels_the_holder() {
    let mut knocked = 0;
    for seed in 0..60 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [0.0, 0.0], 0.0),
                Player::new(1, [0.0, tuning::HAND_REACH + 0.9], std::f32::consts::PI),
            ],
            Rules::default(),
            seed,
        );
        w.players[1].ratings.steals = 0.5;
        w.phase = Phase::Live;
        w.ball.hold(0);
        w.step(&[Input::default(), buttons(button::TRICK)]);
        let mut swung = false;
        for _ in 0..TICK_HZ {
            w.step(&[Input::default(), Input::default()]);
            if w.ball.state == BallState::Loose {
                swung = w.events.contains(&Event::Swing { player: 1 });
                break;
            }
        }
        if w.ball.state != BallState::Loose {
            continue;
        }
        knocked += 1;
        assert!(swung, "the knock posts the swiper's swing");
        assert!(
            (24..=29).contains(&w.players[0].barred),
            "{}",
            w.players[0].barred
        );
        assert!(
            (49..=50).contains(&w.players[1].barred),
            "{}",
            w.players[1].barred
        );
        match w.players[0].action {
            Action::Stumbling {
                reaction: Some(r), ..
            } => assert_eq!(r.script, trick::STRIPPED),
            ref a => panic!("{a:?}"),
        }
        let axis = w.ball.spin_axis;
        assert!((axis.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 1e-4);
        assert!((0.075..=0.15).contains(&w.ball.spin));
    }
    assert!(knocked > 10, "{knocked}");
}

#[test]
fn a_jumping_defender_blocks_some_shots() {
    let mut blocks = 0;
    for seed in 0..60 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [25.0, 0.0], std::f32::consts::FRAC_PI_2),
                Player::new(1, [28.0, 0.0], -std::f32::consts::FRAC_PI_2),
            ],
            Rules::default(),
            seed,
        );
        w.phase = Phase::Live;
        w.ball.hold(0);
        w.step(&[buttons(button::SHOOT), Input::default()]);
        // Jump so the peak meets the release (the shot script's).
        let release = match w.players[0].action {
            Action::Shooting { release, .. } => u32::from(release),
            _ => panic!("a shot"),
        };
        let jump_at = release - (tuning::JUMP_PEAK * TICK_HZ as f32) as u32;
        for t in 0..2 * TICK_HZ {
            let jump = t == jump_at;
            // Held through the check (a shorter tap is a pump fake), let
            // go before the release gate.
            let hold = if t < 16 {
                buttons(button::SHOOT)
            } else {
                Input::default()
            };
            w.step(&[
                hold,
                if jump {
                    buttons(button::JUMP)
                } else {
                    Input::default()
                },
            ]);
            if w.events.iter().any(|e| matches!(e, Event::Block { .. })) {
                blocks += 1;
            }
        }
    }
    // BLOCK_BASE + BLOCK_SKILL / 2: about 45%.
    assert!((15..45).contains(&blocks), "{blocks} of 60");
}

/// Team 1's player at `at`, with the ball, running at the rim (at -x)
/// for `run` updates: a dunk is on for him once he moves
/// (`FUN_0016c500`, [`World::dunk_on`]).
fn driving(seed: u32, at: [f32; 2], run: usize) -> World {
    let mut w = World::new(
        hoops(),
        vec![Player::new(1, at, -std::f32::consts::FRAC_PI_2)],
        Rules::default(),
        seed,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    for _ in 0..run {
        w.step(&[stick(-1.0, 0.0)]);
    }
    w
}

/// `seed` draws off `w`'s generator: the first draws of `Rng::new` hardly
/// depend on the seed, so a loop over seeds needs this to see the draws
/// vary.
fn spin(w: &mut World, seed: u32) {
    for _ in 0..seed {
        w.rng.next_u32();
    }
}

/// The row a press started: a dunk's or a lay-up's.
fn press_row(w: &World) -> Option<(u8, &'static timing::MoveRow)> {
    let row = match w.players[0].action {
        Action::Dunking { row, .. } | Action::Layup { row, .. } => row,
        _ => return None,
    };
    timing::move_row(row).map(|r| (row, r))
}

#[test]
fn turbo_and_shoot_near_the_rim_draws_a_turbo_dunk_or_a_layup() {
    // `FUN_001693b8(actor, 0)`: with a turbo held (the held word's 0x2000)
    // the dunk rows whose ids an ordinary player unlocks for it
    // (`timing::TURBO_DUNKS`) and every lay-up row the filter lets
    // through, one drawn; the bar pays nothing.
    let (mut dunks, mut layups, mut made) = (0, 0, 0);
    for seed in 0..40 {
        let mut w = driving(seed, [-29.0, 2.0], 3);
        spin(&mut w, seed);
        let bar = w.turbo_of(0);
        w.step(&[stick(-1.0, 0.0).with(button::L1 | button::SHOOT)]);
        let (_, r) = press_row(&w).unwrap_or_else(|| panic!("seed {seed}: a dunk or a lay-up"));
        if r.family == 0 {
            assert!(
                timing::TURBO_DUNKS.contains(&r.id),
                "seed {seed}: {}",
                r.name
            );
            dunks += 1;
        } else {
            layups += 1;
        }
        assert!(w.turbo_of(0) >= bar - 0.05, "seed {seed}: the bar paid");
        for _ in 0..2 * TICK_HZ {
            w.step(&[Input::default()]);
        }
        made += w.score[1];
        // He ended up at the rim (his run-up can carry him just past it).
        let d = distance(w.players[0].position, w.rim_spot(1));
        assert!(d < 4.0, "seed {seed}: {d}");
    }
    assert!(dunks > 0 && layups > 0, "{dunks} dunks, {layups} lay-ups");
    assert!(made >= 25, "{made} of 40");
}

#[test]
fn a_drive_from_out_front_draws_among_the_rows_for_the_front_of_the_rim() {
    // `FUN_001693b8`'s approach sector seen from the rim (its word 0x300
    // for the rim at -x, team 1's): a drive from straight
    // out on the court is sector 4, so the rows for the front (the lay-ups
    // `layup_1` 0x3c, `layup_4` 0x40 and their like at 8 to 12.5; the
    // front dunks, `dunk_6` and the spread eagles, are not unlocked for an
    // ordinary player) are among the candidates; the old reading took it
    // as 0, behind the board, where only the rows for any approach (0xff)
    // qualify. Those lay-ups carry both gate bits (0x480000): a rating
    // from 21 up drops one or the other, under 21 the gate that keeps only
    // the 0x80000 rows lets them in.
    let mut front = 0;
    for seed in 0..40 {
        // Pressing 8..12.5 out, the band of those rows.
        let mut w = driving(seed, [-27.0, 2.0], 3);
        w.players[0].ratings.dunks = 0.1;
        spin(&mut w, seed);
        assert_eq!(timing::approach_sector(&w, 0), 4);
        w.step(&[stick(-1.0, 0.0).with(button::L1 | button::SHOOT)]);
        let Some((row, _)) = press_row(&w) else {
            panic!("seed {seed}: a dunk or a lay-up");
        };
        let mask = timing::MOVE_ROWS[usize::from(row)].mask;
        assert!(mask & 0x10 != 0, "seed {seed}: row {row} ({mask:#x})");
        front += usize::from(mask & 0xff != 0xff);
    }
    assert!(front > 0, "no row for the front drawn in 40");
}

#[test]
fn turbo_and_shoot_dunks_where_fun_0016c500_has_a_dunk_on() {
    // `FUN_0016b4b8` takes the dunk rows when `FUN_0016c500(actor, 1)` is
    // 2 (raw `0x16c500..0x16c904`): within 15 of the rim when moving at it
    // with no opponent within 4.5 in the cone to the rim, or within 7.5
    // standing; else the jump shot. The sim dunked within its own 11.
    let press = stick(-1.0, 0.0).with(button::L1 | button::SHOOT);
    // Moving, 13 out: a dunk.
    let mut w = driving(1, [-25.0, 0.0], 3);
    assert!(distance(w.players[0].position, w.rim_spot(1)) > 11.0);
    w.step(&[press]);
    assert!(
        matches!(w.players[0].action, Action::Dunking { .. }),
        "{:?}",
        w.players[0].action
    );
    // Standing 9 out: a jump shot.
    let mut w = driving(1, [-27.75, 0.0], 0);
    w.step(&[buttons(button::L1 | button::SHOOT)]);
    assert!(
        matches!(w.players[0].action, Action::Shooting { .. }),
        "{:?}",
        w.players[0].action
    );
    // Moving 9 out with a defender 3 ahead of him on the line to the rim:
    // the jump shot.
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(1, [-24.0, 0.0], -std::f32::consts::FRAC_PI_2),
            Player::new(0, [-28.5, 0.0], std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        1,
    );
    w.phase = Phase::Live;
    // The cone is read about the rim the side with the ball attacks.
    w.possession = 1;
    w.ball.hold(0);
    for _ in 0..3 {
        w.step(&[stick(-1.0, 0.0), Input::default()]);
    }
    let gap = distance(w.players[0].position, w.players[1].position);
    assert!(gap < 4.5, "{gap}");
    let before = w.clone();
    w.step(&[press, Input::default()]);
    assert!(
        matches!(w.players[0].action, Action::Shooting { .. }),
        "{:?}",
        w.players[0].action
    );
    // The Dunk lesson (game vt+0xb4 3, `0x16c6d4`) skips the cone: a dunk.
    let mut w = before;
    w.lesson = Some(3);
    w.step(&[press, Input::default()]);
    assert!(
        matches!(w.players[0].action, Action::Dunking { .. }),
        "{:?}",
        w.players[0].action
    );
}

#[test]
fn the_block_lessons_holder_shoots_one_jump_shot() {
    // `FUN_0016b4b8` (`0x16b514..0x16b574`): in the Block lessons the
    // shoot press plays `0x44ed73`, even with a dunk on.
    assert_eq!(
        timing::SHOT_SCRIPTS[timing::BLOCK_LESSON_SHOT].script,
        0x44ed73
    );
    let press = stick(-1.0, 0.0).with(button::L1 | button::SHOOT);
    let mut w = driving(1, [-25.0, 0.0], 3);
    w.lesson = Some(9);
    w.step(&[press]);
    assert!(
        matches!(w.players[0].action, Action::Shooting { .. }),
        "{:?}",
        w.players[0].action
    );
}

#[test]
fn a_tricks_fumble_comes_its_wait_after_the_press() {
    // `0x15f630..0x15f7f0`: the chance by Handles' tier less the class.
    assert_eq!(trick::fumble_chance(0, 2), 100);
    assert_eq!(trick::fumble_chance(99, 2), 0);
    assert_eq!(trick::fumble_chance(50, 3), 15);
    assert_eq!(trick::fumble_chance(20, 3), 60);
    assert_eq!(trick::fumble_chance(85, 6), 45);
    assert_eq!(trick::fumble_chance(10, 6), 100);
    // `0x15a1c8..0x15a3f0`: the wait run out, the holder falls, the ball
    // comes loose and he and his nearest opponent are barred `fps`.
    let mut w = live(vec![
        Player::new(0, [0.0, 0.0], 0.0),
        Player::new(1, [3.0, 0.0], 0.0),
    ]);
    w.ball.hold(0);
    w.players[0].fumble = true;
    w.players[0].fumble_timer = 2;
    assert!(!w.fumble_tick(0));
    w.players[0].fumble_timer = 0;
    assert!(w.fumble_tick(0));
    assert_eq!(w.ball.state, BallState::Loose);
    assert!(!w.players[0].fumble);
    assert_eq!((w.players[0].barred, w.players[1].barred), (60, 60));
    let fell = match w.players[0].action {
        Action::Stumbling {
            reaction: Some(r), ..
        } => trick::FUMBLES.contains(&r.script),
        _ => false,
    };
    assert!(fell, "{:?}", w.players[0].action);
    // The defence's update clears a pending fumble.
    w.players[1].fumble = true;
    assert!(!w.fumble_tick(1));
    assert!(!w.players[1].fumble);
}

#[test]
fn the_hands_take_the_originals_poses() {
    // `FUN_001719f0`: relaxed by default; posting stresses both; the
    // locomotion's rows clench both, the ball's hand relaxing for a holder.
    use crate::hands::*;
    let mut w = live(vec![
        Player::new(0, [0.0, 0.0], 0.0),
        Player::new(1, [10.0, 0.0], 0.0),
    ]);
    w.players[1].gait_channel.row = 65;
    w.players[1].gait_channel.record = None;
    assert_eq!(w.hand_shapes(1), (RELAX, RELAX + LEFT));
    w.players[1].posting = true;
    assert_eq!(w.hand_shapes(1), (STRESS, STRESS + LEFT));
    w.players[1].posting = false;
    w.players[1].gait_channel.row = 1;
    assert_eq!(w.hand_shapes(1), (FIST, FIST + LEFT));
    w.ball.hold(1);
    w.players[1].gait_channel.hand = 1;
    assert_eq!(w.hand_shapes(1), (FIST, RELAX + LEFT));
    w.players[1].gait_channel.hand = 2;
    assert_eq!(w.hand_shapes(1), (RELAX, FIST + LEFT));
    assert_eq!(GROUPS[usize::from(RELAX)], "ARrelax");
    assert_eq!(GROUPS[usize::from(FIST + LEFT)], "ALfist");
}

#[test]
fn holding_the_ball_too_long_is_a_shot_clock_violation() {
    let mut w = live(vec![
        Player::new(0, [0.0, 0.0], 0.0),
        Player::new(1, [-20.0, 0.0], 0.0),
    ]);
    w.ball.hold(0);
    let mut violated = false;
    for _ in 0..25 * TICK_HZ {
        w.step(&[Input::default(), Input::default()]);
        violated |= w.events.contains(&Event::ShotClockViolation { team: 0 });
    }
    assert!(violated);
    // The other team inbounds after the dead ball.
    run_two(&mut w, 2 * TICK_HZ);
    assert_eq!(w.possession, 1);
    assert_eq!(w.handler(), Some(1));
}

fn run_two(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.step(&[Input::default(), Input::default()]);
    }
}

#[test]
fn a_game_ends_at_21_won_by_two() {
    let mut w = solo([0.0, 0.0]);
    w.score = [20, 20];
    w.ball.hold(0);
    w.score_basket(0, Basket::Jumper { long: false });
    assert_eq!(
        w.phase,
        Phase::DeadBall {
            left: tuning::DEAD_BALL_TIME,
            team: 1
        }
    );
    w.phase = Phase::Live;
    w.score_basket(0, Basket::Jumper { long: false });
    assert_eq!(w.phase, Phase::Over { winner: 0 });
    assert_eq!(w.score, [22, 20]);
}

/// Six computer players play a whole game: it finishes, both teams score,
/// and the ball moves through passes, steals and rebounds.
#[test]
fn computer_players_finish_a_game() {
    for seed in [3, 11] {
        let mut w = three_on_three(seed);
        let mut counts = std::collections::BTreeMap::<&str, u32>::new();
        let mut loose_ticks = 0;
        let mut longest_loose = 0;
        for _ in 0..40 * 60 * TICK_HZ {
            ai_step(&mut w);
            for e in &w.events {
                let name = match e {
                    Event::Shot { .. } => "shots",
                    Event::Dunk { .. } => "dunks",
                    Event::Score { .. } => "baskets",
                    Event::Block { .. } => "blocks",
                    Event::Pass {
                        style: PassStyle::Oop,
                        ..
                    } => "alley-oop passes",
                    Event::Pass {
                        style: PassStyle::Heezay | PassStyle::Glass,
                        ..
                    } => "street passes",
                    Event::Pass { .. } => "passes",
                    Event::Steal { .. } => "steals",
                    Event::ShotClockViolation { .. } => "violations",
                    Event::Trick { fakeout: None, .. } => "tricks",
                    Event::Trick { .. } => "fakeouts",
                    Event::Counter { .. } => "counters",
                    Event::Fumble { .. } => "fumbles",
                    Event::Gamebreaker { level: 1, .. } => "gamebreakers 1",
                    Event::Gamebreaker { .. } => "gamebreakers 2",
                    Event::Pocket { .. } => "pockets",
                    Event::Cancel { .. } => "cancels",
                    _ => continue,
                };
                *counts.entry(name).or_default() += 1;
            }
            if w.ball.state == BallState::Loose && w.phase == Phase::Live {
                loose_ticks += 1;
                longest_loose = longest_loose.max(loose_ticks);
            } else {
                loose_ticks = 0;
            }
            for p in &w.players {
                assert!(p.position.iter().all(|v| v.is_finite()));
            }
            if matches!(w.phase, Phase::Over { .. }) {
                break;
            }
        }
        println!(
            "seed {seed}: {:?} after {:.0} s, trick points {:?}, {counts:?}, longest loose ball {:.1} s",
            w.score,
            w.tick as f32 / TICK_HZ as f32,
            w.teams.map(|t| t.trick_points),
            longest_loose as f32 / TICK_HZ as f32
        );
        let Phase::Over { winner } = w.phase else {
            panic!("no winner after 40 minutes: {:?} {counts:?}", w.score);
        };
        assert!(w.score[winner] >= 21 && w.score[winner] >= w.score[1 - winner] + 2);
        assert!(w.score[1 - winner] > 0, "both teams score");
        assert!(counts.get("passes").copied().unwrap_or(0) > 5);
        assert!(longest_loose < 20 * TICK_HZ, "a loose ball is left lying");
    }
}

#[test]
fn same_inputs_same_game() {
    let play = || {
        let mut w = three_on_three(5);
        for t in 0..3000u32 {
            let mut inputs: Vec<Input> = (0..w.players.len()).map(|i| ai::input(&w, i)).collect();
            // A scripted player 0 on top of the AI.
            let mut held = 0;
            for (on, b) in [
                (t % 200 > 120, button::R1),
                (t % 150 == 0, button::SHOOT),
                (t % 170 == 0, button::PASS),
            ] {
                if on {
                    held |= b;
                }
            }
            inputs[0] = stick(math::sin((t / 50) as f32), math::cos((t / 70) as f32)).with(held);
            w.step(&inputs);
        }
        w
    };
    assert_eq!(play(), play());
}

#[test]
fn angles_wrap() {
    use std::f32::consts::PI;
    assert!((wrap_angle(3.0 * PI) - PI).abs() < 1e-5 || (wrap_angle(3.0 * PI) + PI).abs() < 1e-5);
    assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-6);
}

/// Two players a step apart: 0 with the ball facing 1, who defends.
fn face_off(seed: u32) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(1, [0.0, 3.0], std::f32::consts::PI),
        ],
        Rules::default(),
        seed,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    // Dribbling (a fresh take standing still clears out instead).
    w.players[0].dribble = 1;
    w
}

#[test]
fn turbo_runs_out_and_fills_again() {
    let mut w = solo([0.0, 0.0]);
    run(&mut w, stick(1.0, 0.0).with(button::R1), 3 * TICK_HZ);
    // Held turbo empties a full bar in five seconds.
    let after_sprint = w.players[0].turbo;
    assert!((after_sprint - 0.4).abs() < 0.01, "{after_sprint}");
    run(&mut w, stick(-1.0, 0.0).with(button::R1), 3 * TICK_HZ);
    assert_eq!(w.players[0].turbo, 0.0);
    assert_ne!(
        w.players[0].gait,
        Gait::Turbo,
        "no turbo speed on an empty bar"
    );
    // The game's own fill: a full bar in seven and a half seconds, so half
    // a bar in 3.75 s.
    run(&mut w, Input::default(), TICK_HZ * 15 / 4);
    let refilled = w.players[0].turbo;
    assert!((refilled - 0.5).abs() < 0.01, "{refilled}");
}

#[test]
fn held_turbo_drains_standing_still_as_the_original_does() {
    // `FUN_001587d8` spends the bar on any update a turbo bit is held,
    // whatever the stick does.
    let mut w = solo([0.0, 0.0]);
    run(&mut w, Input::default().with(button::R1), TICK_HZ * 5 / 2);
    let held = w.players[0].turbo;
    assert!((held - 0.5).abs() < 0.01, "{held}");
    assert_eq!(w.players[0].gait, Gait::Stand);
}

#[test]
fn trick_moves_fake_out_defenders_they_meet_and_raise_the_multiplier() {
    // L1 + R1 is slot 5 (`0x43d408`), the ordinary row's trick 16 at
    // value class 2. Its window opens with the blend's end (update 6) and
    // the defender 2 in front meets it (without the disc's bones, within
    // the stand-in reach): Handles 50 fakes him out when `r % 100` is at
    // most 35 + trunc(30 × 50 / 99) = 50. The points come at the script's
    // end (update 48): the full 2,000 and the multiplier for a fakeout,
    // half for a plain move.
    let mut fakeouts = 0;
    for seed in 0..80 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        let mut w = face_off(seed);
        w.players[1].position = [0.0, 2.0];
        w.step(&[
            buttons(button::TRICK | button::R1 | button::L1),
            Input::default(),
        ]);
        assert!(matches!(
            w.players[0].action,
            Action::Trick { id, .. } if id.script == 16 && id.level == 2
        ));
        let mut faked = None;
        let mut ended = None;
        for t in 1..TICK_HZ {
            w.step(&[Input::default(), Input::default()]);
            if t == 6 {
                // The reaction starts on the contact, held or faked.
                assert!(matches!(
                    w.players[1].action,
                    Action::Stumbling {
                        reaction: Some(_),
                        ..
                    }
                ));
            }
            for e in &w.events {
                if let Event::Trick {
                    fakeout, level: 3, ..
                } = e
                {
                    faked = *fakeout;
                    ended = Some(t);
                }
            }
        }
        assert_eq!(ended, Some(48), "seed {seed}");
        if faked == Some(1) {
            fakeouts += 1;
            assert_eq!(w.teams[0].combo.multiplier, 2);
            // The fakeout raises the multiplier, and the move is worth
            // its whole value.
            assert_eq!(w.teams[0].combo.points, street::points::TRICK[2]);
        } else {
            assert_eq!(w.teams[0].combo.multiplier, 1);
            assert_eq!(w.teams[0].combo.points, street::points::TRICK[2] / 2);
        }
    }
    // 51 in 100 for evenly rated players.
    assert!((28..54).contains(&fakeouts), "{fakeouts} of 80");
}

#[test]
fn a_still_fresh_holders_juke_press_clears_out() {
    // `FUN_0015c778`: standing still with the ball not yet dribbled
    // (+0x1a0 0), the press plays the clearout for 0.2 of the bar; its
    // kind-2 window knocks the defender in front, and it scores nothing.
    let mut knocked = 0;
    for seed in 0..20 {
        let mut w = face_off(seed);
        w.players[0].dribble = 0;
        w.set_turbo(0, 1.0);
        w.players[1].position = [0.0, 2.0];
        w.step(&[buttons(button::TRICK), Input::default()]);
        assert!(matches!(
            w.players[0].action,
            Action::Trick { id, .. } if id.script == trick::CLEAROUT
        ));
        assert!(!w.players[0].action.juking());
        assert!(
            (w.players[0].turbo - 0.8).abs() < 0.02,
            "{}",
            w.players[0].turbo
        );
        for _ in 0..TICK_HZ {
            w.step(&[Input::default(), Input::default()]);
            knocked += usize::from(matches!(w.players[1].action, Action::Stumbling { .. }));
        }
        assert_eq!(w.teams[0].trick_points, 0);
    }
    assert!(knocked > 0);
    // Moving, or once he has dribbled, the press is a trick, which posts
    // slot 2 as it starts (the clearout posts none).
    let mut w = face_off(1);
    w.players[0].dribble = 1;
    w.step(&[buttons(button::TRICK), Input::default()]);
    assert!(w.players[0].action.juking());
    let Action::Trick { id, .. } = w.players[0].action else {
        unreachable!()
    };
    assert!(w.events.contains(&Event::TrickStart {
        player: 0,
        level: id.level + 1,
        script: id.script,
    }));
}

#[test]
fn the_air_duels_odds_slide_across_their_bands() {
    // `0x136b50..0x136e60`: the stronger's knock 10% at an even duel, 90%
    // past 200; the weaker's 10% even, 2% past 200 (and only after the
    // first past 100).
    let share = |d: i32| {
        let mut rng = crate::rng::Rng::new(9);
        let (mut a, mut b) = (0, 0);
        for _ in 0..20_000 {
            let (x, y) = trick::knock_odds(&mut rng, d);
            a += usize::from(x);
            b += usize::from(y);
        }
        (a as f32 / 20_000.0, b as f32 / 20_000.0)
    };
    let near = |x: f32, y: f32| (x - y).abs() < 0.015;
    let (a, b) = share(0);
    assert!(near(a, 0.10) && near(b, 0.10), "{a} {b}");
    let (a, b) = share(75);
    assert!(near(a, 0.40) && near(b, 0.10), "{a} {b}");
    let (a, b) = share(500);
    assert!(near(a, 0.90) && near(b, 0.9 * 0.02), "{a} {b}");
}

#[test]
fn a_still_defender_in_front_draws_the_two_person_freestyle() {
    // `FUN_0015c8e8`: a still holder, the defender standing still 3 in
    // front facing him: one of the near three at value class 2, the
    // defender faked into the row's reaction; 6 away, the far three.
    for (gap, row) in [(3.0, 0), (6.0, 1)] {
        let mut w = face_off(7);
        w.players[1].position = [0.0, gap];
        w.step(&[buttons(button::TRICK), Input::default()]);
        let Action::Trick { id, play } = w.players[0].action else {
            panic!("no move at {gap}");
        };
        assert!(
            trick::FREESTYLES[row].iter().any(|&(s, _)| s == id.script),
            "{gap}: {}",
            id.script
        );
        assert_eq!((id.level, play.faked), (2, Some(1)));
        assert!(matches!(w.players[1].action, Action::Stumbling { .. }));
    }
    // Beyond 8 (over his scale, 1.2 here), the trick press.
    let mut w = face_off(7);
    w.players[1].position = [0.0, 11.0];
    w.step(&[buttons(button::TRICK), Input::default()]);
    assert!(!matches!(w.players[1].action, Action::Stumbling { .. }));
}

#[test]
fn a_trick_alone_meets_nobody() {
    // A defender out of reach: the window closes untried, the move is
    // worth half (no fakeout qualifies its base).
    let mut w = face_off(1);
    w.players[1].position = [0.0, 8.0];
    w.step(&[
        buttons(button::TRICK | button::R1 | button::L1),
        Input::default(),
    ]);
    for _ in 0..TICK_HZ {
        w.step(&[Input::default(), Input::default()]);
        assert!(!matches!(w.players[1].action, Action::Stumbling { .. }));
    }
    assert_eq!(w.teams[0].trick_points, street::points::TRICK[2] / 2);
}

#[test]
fn all_four_turbos_start_the_class_four_slot_worth_5_000_faked() {
    let mut w = face_off(1);
    w.players[0].ratings.handles = 1.0;
    w.set_turbo(0, 1.0);
    w.players[1].position = [30.0, 0.0];
    w.step(&[
        buttons(button::TRICK | button::R1 | button::L1 | button::R2 | button::L2),
        Input::default(),
    ]);
    // Slot 0, the ordinary row's trick 20 (73 updates).
    assert!(matches!(
        w.players[0].action,
        Action::Trick { id, .. } if id.script == 20 && id.level == 4
    ));
    // The held update's own drain (`FUN_001587d8`, before his side's
    // update), then the juke press's 0.2 (`FUN_0015c8e8`), whatever the
    // trick's class.
    assert_eq!(
        w.players[0].turbo,
        1.0 - tuning::TURBO_DRAIN - crate::turbo::MOVE
    );
    for _ in 0..2 * TICK_HZ {
        w.step(&[Input::default(), Input::default()]);
    }
    // Nobody to fake: half of 5,000.
    assert_eq!(w.teams[0].combo.points, street::points::TRICK[4] / 2);
}

/// A drawn trick's identity: `script` at value `class`.
fn drawn(script: u16, class: u8) -> crate::TrickIdentity {
    crate::TrickIdentity {
        slot: crate::trick::DRAWN,
        script,
        level: class,
        equipped_row: None,
    }
}

#[test]
fn a_repeated_trick_move_is_worth_half() {
    // The game halves a move's points for each prior entry in its ring of
    // the last four (team+0x2c..0x38, keyed by script). Native
    // FUN_001438b8 only discounts value_class > 1.
    let mut w = face_off(1);
    let id = drawn(30, 2);
    // Three earlier tricks, then the repeat.
    for _ in 0..3 {
        w.score_trick(0, id, None);
    }
    let before = w.teams[0].trick_points;
    w.score_trick(0, id, None);
    // Three prior copies in the ring: 1,000 / 2 / 2 / 2 = 125.
    assert_eq!(
        w.teams[0].trick_points - before,
        street::points::TRICK[2] / 2 / 8
    );
}

#[test]
fn a_trick_mixed_with_others_repeats_only_within_the_last_four() {
    let mut w = face_off(1);
    // Script 30, then two others, then 30 again while still in the ring.
    w.score_trick(0, drawn(30, 2), None);
    let before = w.teams[0].trick_points;
    for s in [31, 32] {
        w.score_trick(0, drawn(s, 2), None);
    }
    assert_eq!(w.teams[0].trick_points - before, street::points::TRICK[2]);
    let before = w.teams[0].trick_points;
    w.score_trick(0, drawn(30, 2), None);
    assert_eq!(
        w.teams[0].trick_points - before,
        street::points::TRICK[2] / 4
    );
}

#[test]
fn a_different_script_is_not_a_repeat() {
    let mut w = face_off(1);
    for s in 30..33 {
        w.score_trick(0, drawn(s, 2), None);
    }
    let before = w.teams[0].trick_points;
    w.score_trick(0, drawn(33, 2), Some(1));
    assert_eq!(w.teams[0].trick_points - before, street::points::TRICK[2]);
}

#[test]
fn trick_moves_below_value_class_two_bypass_repeat_discount_and_recent_moves() {
    // Native FUN_001438b8 checks `if (1 < iVar3)` where iVar3 is actor+0x248 (value_class).
    // Classes 0 (500) and 1 (1,000) bypass the recent_moves ring entirely
    // and never suffer repeat point reduction (half of each, plain).
    let mut w = face_off(1);
    for class in [0u8, 1u8] {
        let expected = street::points::TRICK[usize::from(class)] / 2;
        for _ in 0..5 {
            let before = w.teams[0].trick_points;
            w.score_trick(0, drawn(7, class), None);
            assert_eq!(w.teams[0].trick_points - before, expected);
            // Ring was not modified.
            assert_eq!(w.teams[0].recent_moves, [-1; 4]);
        }
    }
}

#[test]
fn a_faked_out_defender_cannot_move() {
    let mut w = face_off(1);
    w.players[1].stumble(1.0);
    let at = w.players[1].position;
    run_two(&mut w, TICK_HZ / 2);
    for _ in 0..TICK_HZ / 2 {
        w.step(&[Input::default(), stick(1.0, 0.0)]);
    }
    assert!(distance(w.players[1].position, at) < 0.5);
}

#[test]
fn a_chain_pays_its_weighted_total_on_a_basket_and_goes_to_the_next_pusher() {
    let mut w = face_off(1);
    // A short shot (class 4), a 500-point trick (class 10, the 500-value
    // class), then the basket's own short shot: a three-event chain reads
    // the ramp's 1.5 column for the shot rows and 0.2 for the trick.
    let now = w.scoring_now();
    w.combo.own(0);
    w.combo.push(crate::combo::Slot::new(4, -1, now, -1), false);
    w.combo
        .push(crate::combo::Slot::new(10, 0, now, 500), false);
    w.teams[0].combo.points = 500;
    let before = w.teams[0].trick_points;
    w.score_basket(0, Basket::Jumper { long: false });
    // The basket's own 250 and the chain's weighted total, one credit:
    // 250×1.5 + 500×0.2 + 250×1.5 = 375 + 100 + 375 = 850.
    assert_eq!(w.teams[0].trick_points - before, 250 + 850);
    assert_eq!(w.teams[0].combo, Combo::default(), "the chain cashed");
    assert_eq!(w.combo.total, 850);
    // The next update's start drops the total and clears the ring.
    w.combo_update();
    assert_eq!(w.combo.total, 0);
    assert!(w.combo.ring.is_empty());

    // The ring stays the side's own until the other side pushes: a steal
    // of the held ball hands it over.
    let mut w = face_off(1);
    w.sync_game_phase();
    let now = w.scoring_now();
    w.combo.own(0);
    w.combo
        .push(crate::combo::Slot::new(10, 4, now, 2_500), false);
    w.take_ball(0);
    assert_eq!((w.combo.side, w.combo.ring.depth()), (0, 1));
    w.take_ball(1);
    assert_eq!(w.combo.side, 1, "the steal takes the ring");
    assert_eq!(w.combo.ring.slots[5].class, 20);
    assert_eq!(w.combo.ring.depth(), 1);
}

#[test]
fn level_one_gamebreakers_take_a_point_from_the_other_team() {
    let mut w = face_off(1);
    // Keep the shot inside the arc and require an actual scoring event.
    w.players[0].position = [30.0, 0.0];
    w.players[0].ratings.shots = 1.0;
    w.score = [10, 10];
    w.teams[0].meter = street::METER_FULL;
    assert_eq!(w.teams[0].gamebreaker(), 1);
    w.step(&[
        buttons(button::SHOOT | button::R1 | button::R2),
        Input::default(),
    ]);
    assert!(matches!(
        w.players[0].action,
        Action::Shooting { gamebreaker: 1, .. }
    ));
    // Slot 64 (`0x144708`): a Level 1 meter stays full, held, until the
    // gamebreaker is over.
    assert_eq!(w.teams[0].meter, street::METER_FULL);
    assert!(w.teams[0].meter_gamebreaker && w.teams[0].meter_hold == 1);
    let mut scored = false;
    for _ in 0..3 * TICK_HZ {
        w.step(&[Input::default(), Input::default()]);
        scored |= w
            .events
            .iter()
            .any(|e| matches!(e, Event::Score { team: 0, .. }));
    }
    assert!(scored, "gamebreaker must exercise the scoring path");
    assert_eq!(w.score, [11, 9]);
    // Over (slot 65): the meter is spent, and the basket filled none of it.
    assert_eq!(w.teams[0].meter, 0, "the meter is spent");
    assert!(!w.teams[0].meter_gamebreaker && w.teams[0].meter_hold == 0);
}

#[test]
fn the_gamebreaker_task_empties_the_meter_62_updates_after_the_press() {
    // `FUN_00144ef8`: the task's first update (in the press's step) sets
    // its wait, 60 for a shooter; at 0 it asks for its two replays, one
    // update each when none plays, then posts its end (slot 65), whether
    // or not the shot is decided (the School's Gamebreaker capture).
    let mut w = face_off(1);
    w.players[0].position = [30.0, 0.0];
    w.teams[0].meter = street::METER_FULL;
    w.step(&[
        buttons(button::SHOOT | button::R1 | button::R2),
        Input::default(),
    ]);
    assert!(w.teams[0].meter_gamebreaker);
    assert_eq!(
        w.gamebreaker_task.map(|t| (t.kind, t.state, t.count)),
        Some((1, 1, 60))
    );
    for n in 1..62 {
        w.step(&[Input::default(), Input::default()]);
        assert!(w.teams[0].meter_gamebreaker, "{n}");
    }
    assert_eq!(w.gamebreaker_task.map(|t| t.state), Some(3));
    w.step(&[Input::default(), Input::default()]);
    assert!(w.gamebreaker_task.is_none());
    assert!(!w.teams[0].meter_gamebreaker && w.teams[0].meter == 0);
}

#[test]
fn pocketing_builds_an_unblockable_level_two() {
    let mut w = face_off(1);
    // Just inside the half of the basket shot at: from x < 0 the original
    // gives a shot at the +x basket 3% (`0x1730cc`), gamebreaker or not.
    w.players[0].position = [1.0, 0.0];
    w.players[1].position = [1.0, 3.0];
    w.score = [10, 10];
    w.teams[0].meter = street::METER_FULL;
    w.step(&[buttons(button::SELECT), Input::default()]);
    assert!(w.teams[0].pocketed && w.teams[0].meter == 0);
    assert!(w.events.contains(&Event::Pocket { team: 0 }));
    w.teams[0].meter = street::METER_FULL;
    assert_eq!(w.teams[0].gamebreaker(), 2);
    run_two(&mut w, 2);
    w.step(&[
        buttons(button::SHOOT | button::L1 | button::L2),
        Input::default(),
    ]);
    // The defender jumps right into it and still cannot stop it.
    let mut blocked = false;
    for t in 0..3 * TICK_HZ {
        let jump = t == 25;
        w.step(&[
            Input::default(),
            if jump {
                buttons(button::JUMP)
            } else {
                Input::default()
            },
        ]);
        blocked |= w.events.iter().any(|e| matches!(e, Event::Block { .. }));
    }
    assert!(!blocked);
    // From beyond the arc: 2 points scored and 4 taken (the points
    // table's Level 2 row; 3 taken inside).
    assert_eq!(w.score, [12, 6]);
    assert!(!w.teams[0].pocketed);
}

#[test]
fn a_full_meter_cancels_the_other_teams_pocketed_gamebreaker() {
    let mut w = face_off(1);
    w.teams[1].pocketed = true;
    w.teams[0].meter = street::METER_FULL;
    w.step(&[buttons(button::SELECT | button::R1), Input::default()]);
    assert!(!w.teams[1].pocketed);
    // Slot 71 (`0x1447a8`): the meter is spent; the cancel's 5,000 are
    // trick points, not meter (no meter listener adds them).
    assert_eq!(w.teams[0].meter, 0);
    assert_eq!(w.teams[0].trick_points, street::points::CANCEL);
    assert_eq!(w.stats.player(0, crate::stats::TRICK_POINTS), 5_000);
    assert!(w.events.contains(&Event::Cancel { team: 0 }));
}

#[test]
fn trick_counters_take_the_ball_from_a_tricking_handler() {
    let mut countered = 0;
    for seed in 0..60 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        let mut w = face_off(seed);
        // Close enough to touch (the contact radii): the counter is the
        // contact handler's.
        w.players[1].position = [0.0, 2.0];
        // Stepping toward him (a still defender in front draws the
        // two-person freestyle instead, `FUN_0015c8e8`).
        w.step(&[buttons(button::TRICK), stick(0.0, -0.5)]);
        for t in 0..=TICK_HZ {
            if t == 0 {
                w.step(&[
                    Input::default(),
                    buttons(button::TRICK | button::R1 | button::L1),
                ]);
            } else {
                w.step(&[Input::default(), Input::default()]);
            }
            if w.events
                .iter()
                .any(|e| matches!(e, Event::Counter { player: 1, .. }))
            {
                countered += 1;
                assert_eq!(w.handler(), Some(1));
                assert!(matches!(w.players[0].action, Action::Stumbling { .. }));
            }
        }
    }
    assert!((15..50).contains(&countered), "{countered} of 60");
}

#[test]
fn the_no_counters_cheat_leaves_counters_out() {
    let mut countered = None;
    for seed in 0..60 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        let mut w = face_off(seed);
        w.rules.no_counters = true;
        w.step(&[buttons(button::TRICK), Input::default()]);
        w.step(&[
            Input::default(),
            buttons(button::TRICK | button::R1 | button::L1),
        ]);
        for _ in 0..TICK_HZ {
            w.step(&[Input::default(), Input::default()]);
            if w.events.iter().any(|e| matches!(e, Event::Counter { .. })) {
                countered = Some(seed);
            }
        }
        assert!(
            matches!(
                w.players[0].action,
                Action::None | Action::Trick { .. } | Action::Stumbling { .. }
            ) || w.handler() == Some(0)
        );
    }
    assert!(
        countered.is_none(),
        "a counter fired at seed {:?}",
        countered
    );
}

#[test]
fn two_pointer_cheats_force_the_long_range_rating_and_leave_close_shots_alone() {
    let shot = |seed, position, rules: Rules, threes| {
        let mut w = face_off(seed);
        w.players[0].position = position;
        w.players[0].ratings.threes = threes;
        w.players[1].position = [-40.0, 30.0];
        w.rules = rules;
        w.release_shot(0, 0);
        (
            w.ball.shot.expect("released shot"),
            w.ball.position,
            w.ball.velocity,
        )
    };
    let easy = Rules {
        easy_2_pointers: true,
        ..Rules::default()
    };
    let hard = Rules {
        hard_2_pointers: true,
        ..Rules::default()
    };
    let mut makes = [0u32; 3];
    for seed in 0..400 {
        let seed = (seed as u32).wrapping_mul(0x9e37_79b9);
        // Beyond the arc: each cheat is exactly equivalent to setting the
        // original Threes rating to 99/0, including the seeded trajectory.
        let long = [5.0, 0.0];
        let plain = shot(seed, long, Rules::default(), 0.5);
        let easier = shot(seed, long, easy, 0.5);
        let harder = shot(seed, long, hard, 0.5);
        assert_eq!(easier, shot(seed, long, Rules::default(), 1.0));
        assert_eq!(harder, shot(seed, long, Rules::default(), 0.0));
        assert_eq!(plain.0.points, 2);
        for (count, attempt) in makes.iter_mut().zip([plain, easier, harder]) {
            *count += u32::from(attempt.0.made);
        }
        // Inside the arc: the Shots rating and outcome are unchanged.
        let close = [28.0, 11.75];
        let plain = shot(seed, close, Rules::default(), 0.5);
        assert_eq!(plain.0.points, 1);
        assert_eq!(plain, shot(seed, close, easy, 0.5));
        assert_eq!(plain, shot(seed, close, hard, 0.5));
    }
    assert!(makes[1] > makes[0], "easy/normal makes: {makes:?}");
    assert!(makes[0] > makes[2], "normal/hard makes: {makes:?}");
}

/// Three on a side's attacking end: 0 handles beyond the arc, 1 is near
/// team 0's basket (+x), 2 is spare; 3 defends far away.
fn passing_drill(seed: u32) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [5.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(0, [24.0, 6.0], std::f32::consts::FRAC_PI_2),
            Player::new(0, [8.0, -15.0], std::f32::consts::FRAC_PI_2),
            Player::new(1, [-30.0, 0.0], 0.0),
        ],
        Rules::default(),
        seed,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    w
}

fn step_all(w: &mut World, inputs: &[(usize, Input)], ticks: u32) {
    for _ in 0..ticks {
        let mut all = vec![Input::default(); w.players.len()];
        for &(i, input) in inputs {
            all[i] = input;
        }
        w.step(&all);
    }
}

#[test]
fn alley_oops_go_up_on_the_call_and_come_down_as_dunks() {
    let mut made = 0;
    for seed in 0..20 {
        let mut w = passing_drill(seed);
        // 1, without the ball near the basket, presses shoot with a turbo:
        // the leap ([`crate::leap`]).
        step_all(&mut w, &[(1, buttons(button::SHOOT | button::R1))], 1);
        assert!(w.players[1].leaping(), "1 leaps");
        step_all(&mut w, &[], 5);
        step_all(&mut w, &[(0, buttons(button::PASS))], 1);
        assert!(w.events.iter().any(|e| matches!(
            e,
            Event::Pass {
                to: 1,
                style: PassStyle::Oop,
                ..
            }
        )));
        step_all(&mut w, &[], 2 * TICK_HZ);
        made += w.score[0];
    }
    assert!(made >= 15, "{made} of 20 alley-oops");
}

#[test]
fn a_shooter_in_the_air_passes_out_of_his_shot() {
    // `FUN_0015a060` takes the holder's pass press in his jump shot: the
    // shot gives way to an airborne pass row, which flies him down.
    let mut w = passing_drill(2);
    step_all(&mut w, &[(0, buttons(button::SHOOT))], 1);
    assert!(matches!(w.players[0].action, Action::Shooting { .. }));
    let mut up = false;
    for _ in 0..40 {
        step_all(&mut w, &[(0, buttons(button::SHOOT))], 1);
        if w.players[0].height > 1.0 {
            up = true;
            break;
        }
    }
    assert!(up, "the shooter leaves the floor");
    step_all(&mut w, &[(0, buttons(button::PASS))], 1);
    let Action::Passing { row, .. } = w.players[0].action else {
        panic!("{:?}", w.players[0].action);
    };
    assert!(
        crate::timing::PASS_ROWS[usize::from(row)].flags & 0x400 != 0,
        "an air row"
    );
    step_all(&mut w, &[], 2 * TICK_HZ);
    assert_ne!(w.handler(), Some(0), "the ball went");
    assert_eq!(w.players[0].height, 0.0, "and he came down");
    assert!(w.players[0].free());
}

/// One person on team 0 (player 0), the computer on everyone else.
fn step_person(w: &mut World, person: usize, input: Input, ticks: u32) {
    for _ in 0..ticks {
        let mut all = vec![Input::computer(); w.players.len()];
        all[person] = input;
        w.step(&all);
    }
}

#[test]
fn a_give_and_go_comes_back_when_called_for() {
    // The right stick's pass on a side one person plays holds his control
    // on the passer (team+0xc8, `0x15b900`): no hand-off at the release or
    // the catch. Cross off the ball calls for it back (`FUN_001587d8`), and
    // the computer's pass back ends the hold (`0x173e44`).
    let mut w = passing_drill(1);
    let mut aimed = Input::default();
    aimed.set_aim([0.0, -1.0]);
    step_person(&mut w, 0, Input::default(), 1);
    step_person(&mut w, 0, aimed, 1);
    assert!(w.control.hold[0], "the give 'n go holds");
    for _ in 0..TICK_HZ {
        step_person(&mut w, 0, Input::default(), 1);
        if w.handler() == Some(2) {
            break;
        }
    }
    assert_eq!(w.give_and_go, Some((0, 2)));
    assert_eq!(w.handler(), Some(2));
    assert_eq!(w.person_on(0), Some(0), "the person keeps the passer");
    // The passer calls for it back.
    step_person(&mut w, 0, buttons(button::PASS), 1);
    for _ in 0..TICK_HZ {
        step_person(&mut w, 0, Input::default(), 1);
        if w.handler() == Some(0) {
            break;
        }
    }
    assert_eq!(w.handler(), Some(0));
    assert!(!w.control.hold[0], "the computer's pass ended it");
}

/// A give 'n go thrown and caught: the person on the passer 0, the
/// computer's 2 holding.
fn given_and_gone() -> World {
    let mut w = passing_drill(1);
    let mut aimed = Input::default();
    aimed.set_aim([0.0, -1.0]);
    step_person(&mut w, 0, Input::default(), 1);
    step_person(&mut w, 0, aimed, 1);
    for _ in 0..TICK_HZ {
        step_person(&mut w, 0, Input::default(), 1);
        if w.handler() == Some(2) {
            break;
        }
    }
    assert_eq!(w.handler(), Some(2));
    w
}

#[test]
fn the_give_and_goers_calls_push_the_computer_holders_node() {
    // `FUN_001587d8`: the holder's queue cleared and one node pushed, built
    // with its draws in the caller's update: Circle without a turbo a
    // shoot node, Cross a pass node to him, Cross with two turbos a trick
    // pass node without a receiver.
    let kinds = [
        (buttons(button::SHOOT), 9),
        (buttons(button::PASS), 8),
        (buttons(button::PASS | button::L1 | button::R1), 0x17),
    ];
    for (press, kind) in kinds {
        let mut w = given_and_gone();
        step_person(&mut w, 0, press, 1);
        let q = &w.ai.brains[2].queue;
        assert_eq!(q.first().map(|n| n.act.kind()), Some(kind), "{kind:#x}");
    }
}

#[test]
fn the_persons_control_goes_to_the_receiver_as_the_ball_leaves() {
    // `FUN_00173570`'s swap at callback 38: the receiver takes the person's
    // word in the release's update, not at the catch; while the pass flies
    // the receiver runs on his AI's frame with his stick as the press left
    // it, and the passer is the computer's.
    let mut w = passing_drill(1);
    step_person(&mut w, 0, Input::default(), 1);
    step_person(&mut w, 0, stick(0.2, -1.0).with(button::PASS), 1);
    let w0 = w.windup.expect("a windup");
    assert_eq!(w.person_on(0), Some(0), "the windup keeps him");
    let mut released = None;
    for t in 0..60 {
        let me = w.person_on(0).unwrap();
        step_person(&mut w, me, Input::default(), 1);
        if w.ball.state == BallState::Pass {
            released = Some(t);
            break;
        }
    }
    assert!(released.is_some(), "the ball left");
    assert_eq!(w.person_on(0), Some(w0.to), "control went with it");
    assert!(w.pad_off(w0.to), "the receiver runs on his AI's frame");
    assert!(!w.people[0]);
    let frozen = w.control.frozen.expect("the receiver's stick is held");
    assert_eq!(frozen.player, w0.to);
}

#[test]
fn cross_on_defence_switches_to_the_teammate_nearest_the_ball() {
    // `FUN_0012fd60`'s scored pick: the computer teammate nearest the ball,
    // unless the person's player is his side's nearest already, when it is
    // the one nearest the rim the offence attacks; a pass in flight scores
    // by the receiver.
    let mut w = three_on_three(3);
    w.phase = Phase::Live;
    w.ball.hold(0);
    w.possession = 0;
    w.game_phase.possession = 0;
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    let rim = w.rim_spot(target_hoop(0));
    w.players[0].position = [rim[0] - 20.0 * rim[0].signum(), 0.0];
    let ball = w.players[0].position;
    w.ball.position = [ball[0], 4.0, ball[1]];
    // 3 the person, 4 near the ball, 5 near the rim.
    w.people = vec![false, false, false, true, false, false];
    let toward = |f: f32| [ball[0] + (rim[0] - ball[0]) * f, 2.0];
    w.players[3].position = toward(0.6);
    w.players[4].position = toward(0.2);
    w.players[5].position = toward(0.95);
    assert_eq!(w.switch_target(3), Some(4), "nearest the ball");
    w.people = vec![false, false, false, false, true, false];
    assert_eq!(w.switch_target(4), Some(5), "4 is nearest: the rim's");
    // A fallen teammate scores 3 more.
    w.people = vec![false, false, false, false, false, true];
    w.players[3].position = toward(0.2);
    w.players[4].position = [toward(0.2)[0], 2.1];
    w.players[3].action = Action::Stumbling {
        elapsed: 0.0,
        duration: 1.0,
        reaction: None,
    };
    assert_eq!(w.switch_target(5), Some(4));
    // Off the person's screen a teammate scores 20 more (`FUN_00171920`):
    // 4, nearer the ball by a little, loses to 3 when off it.
    w.players[3].action = Action::None;
    w.people = vec![false, false, false, false, false, true];
    w.players[3].position = toward(0.25);
    w.players[4].position = toward(0.2);
    assert_eq!(w.switch_target(5), Some(4));
    assert_eq!(w.switch_target_seen(5, 1 << 4), Some(3));
    assert_eq!(
        w.switch_target_seen(5, 1 << 4 | 1 << 3),
        Some(4),
        "both off"
    );
}

#[test]
fn the_action_tap_switches_on_the_last_updates_tables() {
    let mut w = three_on_three(3);
    w.phase = Phase::Live;
    w.ball.hold(0);
    w.possession = 0;
    step_person(&mut w, 3, Input::default(), 2);
    let me = w.person_on(1).expect("one person on team 1");
    let want = w.switch_target(me).expect("a teammate");
    step_person(&mut w, me, buttons(button::PASS), 1);
    assert_eq!(w.person_on(1), Some(want));
    // Cross with the ball is the pass press, no switch.
    let mut v = three_on_three(3);
    v.phase = Phase::Live;
    v.ball.hold(1);
    v.possession = 0;
    step_person(&mut v, 1, Input::default(), 2);
    let me = v.person_on(0).expect("team 0's person");
    assert_eq!(v.handler(), Some(me), "the person has the holder");
    step_person(&mut v, me, buttons(button::PASS), 1);
    assert_eq!(v.person_on(0), Some(me));
    assert!(v.windup.is_some_and(|x| x.from == me), "a pass wound up");
}

#[test]
fn kick_passes_are_kick_passes() {
    let mut w = passing_drill(1);
    step_all(
        &mut w,
        &[(0, stick(1.0, 0.3).with(button::PASS | button::L3))],
        1,
    );
    assert!(w.events.iter().any(|e| matches!(
        e,
        Event::Pass {
            style: PassStyle::Kick,
            ..
        }
    )));
}

#[test]
fn off_the_heezay_comes_back_to_the_passer() {
    let mut back = 0;
    for seed in 0..20 {
        let mut w = face_off(seed);
        step_all(
            &mut w,
            &[(0, buttons(button::PASS | button::R1 | button::L1))],
            1,
        );
        assert!(w.events.iter().any(|e| matches!(
            e,
            Event::Pass {
                style: PassStyle::Heezay,
                ..
            }
        )));
        step_all(&mut w, &[], TICK_HZ);
        if w.handler() == Some(0) {
            back += 1;
        }
    }
    // Met within the passer's reach of the head bone (`FUN_00148378`).
    assert!((10..=20).contains(&back), "{back} of 20");
}

#[test]
fn a_counter_stance_catches_the_heezay_off_its_head() {
    // `FUN_00148378`'s head leg (kind 2): a defender in the counter
    // stance (`FUN_0016e9b0`) catches the trick pass off his head, an
    // interception; out of the stance the ball goes on.
    for stance in [true, false] {
        let mut w = face_off(5);
        assert!(w.computer_trick_pass(0, 0, Some(1), [0.0; 3]));
        while w.windup.is_some() {
            w.step(&[Input::default(), Input::default()]);
        }
        // The stance must cover head contact, after the row's windup.
        if stance {
            w.players[1].action = Action::Stealing {
                elapsed: 0.0,
                counter: true,
                swipe: None,
                stance: Some(crate::counter::Stance {
                    script: 0,
                    run: crate::trick::Run::new(false, 0),
                    from: None,
                    word: 0,
                }),
            };
        }
        let mut caught = false;
        for _ in 0..TICK_HZ / 2 {
            w.step(&[Input::default(), Input::default()]);
            caught |= w.events.iter().any(|e| {
                matches!(
                    e,
                    Event::Steal {
                        player: 1,
                        intercepted_pass: true,
                        ..
                    }
                )
            });
        }
        assert_eq!(caught, stance);
        assert_eq!(w.handler() == Some(1), stance);
    }
}

#[test]
fn a_trick_pass_leaves_the_passer_on_the_bones() {
    // With the disc's bones the ball's tests run on every actor: the
    // passer of a trick pass to himself leaves its first leg (off the head
    // or the board) alone. The bones' pickup took it back the update it
    // left, so the computer's trick passes never flew.
    let poses = crate::pose::disc_bank();
    if poses.get().is_none() {
        return;
    }
    for defender in [Some(1), None] {
        let mut w = face_off(5);
        w.poses = poses.clone();
        assert!(w.computer_trick_pass(0, 0, defender, [37.53, 14.5, 0.0]));
        // Off the board the passout row holds the ball in his hands until
        // its callback 53 ([`crate::passout`]).
        for _ in 0..40 {
            if w.ball.state == BallState::Pass {
                break;
            }
            w.step(&[Input::default(), Input::default()]);
        }
        for _ in 0..4 {
            w.step(&[Input::default(), Input::default()]);
            assert_eq!(w.ball.state, BallState::Pass, "{defender:?}");
        }
    }
}

#[test]
fn the_jump_shots_slot_is_the_first_pair_the_turbos_hold() {
    // `FUN_0016b4b8` (raw `0x16b760..0x16b7b0`): the first of `0x43d460`'s
    // six two-turbo pairs the held word covers, 0..5; three or four
    // turbos take the first pair they hold. The sim indexed the shots by
    // the eleven trick masks (the pairs are 5..10 there).
    use crate::player::NativeTricks;
    let (l1, r1, l2, r2) = (button::L1, button::R1, button::L2, button::R2);
    assert_eq!(NativeTricks::shot_slot(l1 | r1), Some(0));
    assert_eq!(NativeTricks::shot_slot(l1 | l2), Some(1));
    assert_eq!(NativeTricks::shot_slot(l2 | r2), Some(5));
    assert_eq!(NativeTricks::shot_slot(l1 | r1 | l2 | r2), Some(0));
    assert_eq!(NativeTricks::shot_slot(l1 | l2 | r2), Some(1));
    assert_eq!(NativeTricks::shot_slot(l1), None);
    assert_eq!(NativeTricks::slot(l2 | r2), Some(10));
}

#[test]
fn a_pass_is_gone_for_only_in_its_last_quarter() {
    // `FUN_0015da28`'s tail: a defender under the pass goes for it only
    // once more than three quarters of its flight are flown, at `r % 100 <
    // trunc(Steals / 99 · 15)` an update (15 for 99 Steals).
    let mut early = 0;
    let mut late = 0;
    for seed in 0..200 {
        for last in [false, true] {
            let mut w = World::new(
                hoops(),
                vec![
                    Player::new(0, [0.0, 0.0], 0.0),
                    Player::new(0, [10.0, 0.0], 0.0),
                    Player::new(1, [5.0, 0.0], 0.0),
                ],
                Rules::default(),
                seed,
            );
            w.players[2].ratings.steals = 1.0;
            w.phase = Phase::Live;
            w.take_ball(0);
            w.throw(0, 1, PassStyle::Normal);
            w.ball.flight_left = w.ball.flight_time * if last { 0.2 } else { 0.5 };
            w.ball.position = [5.0, 1.0, 0.0];
            w.step_pass();
            if w.handler() == Some(2) {
                if last {
                    late += 1;
                } else {
                    early += 1;
                }
            }
        }
    }
    assert_eq!(early, 0);
    assert!((15..50).contains(&late), "{late} of 200");
}

#[test]
fn back_2_papa_to_himself_comes_back_to_his_hands_on_the_floor() {
    // The pass captures' five Back 2 Papa passes to oneself
    // (`street-played-passes-1` and `-2`): the passer stays on the floor,
    // sets bit 9 at the toss 20 updates after the press and takes the ball
    // back 54 to 58 updates after it; the one whose board point was below
    // 13.25 went loose off the board ([`crate::passout`]).
    let (mut caught, mut missed) = (0, 0);
    for seed in 0..20 {
        let mut w = passing_drill(seed);
        w.players[0].position = [25.0, 0.0];
        w.players[0].heading = std::f32::consts::FRAC_PI_2;
        w.players[0].speed = 10.0;
        step_all(
            &mut w,
            &[(
                0,
                stick(1.0, 0.0).with(button::PASS | button::R1 | button::L1),
            )],
            1,
        );
        assert!(w.events.iter().any(|e| matches!(
            e,
            Event::Pass {
                style: PassStyle::Glass,
                ..
            }
        )));
        let point = w.passout.expect("a passout to himself").point;
        let mut n = 1;
        while w.ball.state != BallState::Pass && n < TICK_HZ {
            step_all(&mut w, &[], 1);
            n += 1;
        }
        assert!((19..=33).contains(&n), "seed {seed}: tossed at {n}");
        while w.ball.state == BallState::Pass && n < 2 * TICK_HZ {
            step_all(&mut w, &[], 1);
            assert!(w.players[0].height <= 0.0, "seed {seed}: on the floor");
            n += 1;
        }
        if point[1] < crate::passout::BOARD_LOW {
            assert_eq!(w.ball.state, BallState::Loose, "seed {seed}");
            missed += 1;
        } else {
            assert_eq!(w.ball.state, BallState::Held(0), "seed {seed}");
            assert!((45..=65).contains(&n), "seed {seed}: caught at {n}");
            caught += 1;
        }
    }
    assert!(caught > 0 && missed > 0, "{caught} caught, {missed} missed");
}

/// The handler 0 at `at` facing team 0's (+x) basket, with `speed`.
fn driver(at: [f32; 2], speed: f32, seed: u32) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, at, std::f32::consts::FRAC_PI_2),
            Player::new(1, [-30.0, 20.0], 0.0),
        ],
        Rules::default(),
        seed,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    // Dribbling (a fresh take standing still clears out instead).
    w.players[0].dribble = 1;
    w.players[0].speed = speed;
    // Running: on the moving path, so his velocity is his speed
    // (`Player::moving`).
    if speed > 0.0 {
        w.players[0].gait = crate::player::Gait::Run;
    }
    w
}

#[test]
fn a_drive_without_turbo_draws_a_plain_dunk_or_a_layup() {
    // `FUN_001693b8(actor, 0)` without a turbo held: the dunk ids unlocked
    // without one (`timing::PLAIN_DUNKS`) and the lay-ups.
    let (mut dunks, mut layups, mut made) = (0, 0, 0);
    for seed in 0..40 {
        let mut w = driver([30.0, 0.0], 10.0, seed);
        spin(&mut w, seed);
        step_all(&mut w, &[(0, stick(1.0, 0.0).with(button::SHOOT))], 1);
        let (_, r) = press_row(&w).unwrap_or_else(|| panic!("seed {seed}: a dunk or a lay-up"));
        if r.family == 0 {
            assert!(
                timing::PLAIN_DUNKS.contains(&r.id),
                "seed {seed}: {}",
                r.name
            );
            assert!(w.events.contains(&Event::Dunk { player: 0, hoop: 0 }));
            dunks += 1;
        } else {
            assert!(w.events.contains(&Event::Layup { player: 0 }));
            layups += 1;
        }
        // The lay-up scripts let go 31..75 updates in.
        step_all(&mut w, &[], 3 * TICK_HZ);
        made += w.score[0];
    }
    assert!(dunks > 0 && layups > 0, "{dunks} dunks, {layups} lay-ups");
    assert!(made >= 25, "{made} of 40");
}

#[test]
fn shoot_again_on_the_way_up_turns_a_dunk_into_a_layup() {
    // A two-turbo combo's slot dunk (R1 + L2, `dunk_19`), then shoot again
    // once he is 0.5 up more than 1.5 from the rim: `FUN_0016b1e8` plays
    // one of the first six mid-air rows within 5.25 of the rim (the tip and
    // `dunk_to_layup_*`), else any of ten.
    let mut w = driver([30.0, 0.0], 10.0, 1);
    step_all(
        &mut w,
        &[(
            0,
            stick(1.0, 0.0).with(button::SHOOT | button::R1 | button::L2),
        )],
        1,
    );
    assert!(matches!(w.players[0].action, Action::Dunking { .. }));
    let mut pressed = false;
    for _ in 0..80 {
        let up = w.players[0].height > 0.5;
        let far = distance(w.players[0].position, w.rim_spot(0)) > 1.5;
        if up && far && matches!(w.players[0].action, Action::Dunking { .. }) {
            step_all(&mut w, &[(0, buttons(button::SHOOT))], 1);
            pressed = true;
            break;
        }
        step_all(&mut w, &[], 1);
    }
    assert!(pressed, "he never rose 0.5 more than 1.5 out");
    let Action::Layup { row, .. } = w.players[0].action else {
        panic!("{:?}", w.players[0].action);
    };
    let k = row - timing::MIDAIR_ROWS;
    assert!(k < 10, "row {row}");
    assert_eq!(timing::move_row(row).unwrap().family, 2);
    let mut basket = None;
    for _ in 0..2 * TICK_HZ {
        step_all(&mut w, &[], 1);
        for e in &w.events {
            if let Event::Score { basket: b, .. } = e {
                basket = Some(*b);
            }
        }
    }
    if let Some(b) = basket {
        assert_eq!(b, Basket::Layup);
    }
}

#[test]
fn a_quick_tap_of_shoot_is_a_pump_fake() {
    let mut w = driver([15.0, 0.0], 0.0, 1);
    step_all(&mut w, &[(0, buttons(button::SHOOT))], 1);
    // The fake is decided at the shot script's check tick (callback 20,
    // `FUN_00172348`: the button no longer held), 12..15 updates in.
    let check = match w.players[0].action {
        Action::Shooting { check, .. } => check,
        _ => panic!("a shot"),
    };
    assert!((12..=15).contains(&check), "{check}");
    // The press update is the script's tick 0: the fake flag rises
    // `check` updates after the press (the played captures: 12 for the
    // stepback, 14 for legkick_1, 8 for the heave).
    step_all(&mut w, &[], (check - 1).into());
    assert!(!w.events.contains(&Event::PumpFake { player: 0 }));
    step_all(&mut w, &[], 1);
    assert!(w.events.contains(&Event::PumpFake { player: 0 }));
    step_all(&mut w, &[], TICK_HZ);
    assert_eq!(w.handler(), Some(0), "still has the ball");
    assert!(w.players[0].free());
}

#[test]
fn shooting_while_moving_away_is_a_fadeaway() {
    let mut w = driver([20.0, 0.0], 8.0, 1);
    w.players[0].heading = -std::f32::consts::FRAC_PI_2; // moving away, to -x
    // The fade is the stick pushed away from the basket at the shot
    // script's check tick (callback 17, `FUN_001726c0`), not the press.
    let away = stick(-1.0, 0.0).with(button::SHOOT);
    step_all(&mut w, &[(0, away)], 1);
    assert!(matches!(
        w.players[0].action,
        Action::Shooting { fade: false, .. }
    ));
    step_all(&mut w, &[(0, away)], 15);
    assert!(matches!(
        w.players[0].action,
        Action::Shooting { fade: true, .. }
    ));
    let x0 = w.players[0].position[0];
    step_all(&mut w, &[(0, buttons(button::SHOOT))], 75);
    assert!(w.players[0].position[0] < x0 - 2.0, "drifts back");
}

/// Presses Shoot on player 0 and keeps it down `hold` updates (the press
/// update the 0th), the stick at `stick`; with `onto`, the shot the press
/// started is moved onto that script's clock after the press update.
/// Returns the script and the update the ball left.
fn hold_shot(w: &mut World, hold: u32, stick: Input, onto: Option<usize>) -> (usize, Option<u32>) {
    let mut script = None;
    let mut shot = None;
    for n in 0..150 {
        let input = if n < hold {
            stick.with(button::SHOOT)
        } else {
            stick
        };
        step_all(w, &[(0, input)], 1);
        if n == 0
            && let Action::Shooting {
                script: s,
                release,
                check,
                end,
                gate,
                latest,
                ..
            } = &mut w.players[0].action
        {
            if let Some(k) = onto {
                let t = &timing::SHOT_SCRIPTS[k];
                (*s, *release, *check, *end) = (k as u8, t.release, t.check, t.end);
                (*gate, *latest) = (t.gate, t.latest);
            }
            script = Some(usize::from(*s));
        }
        if shot.is_none()
            && w.events
                .iter()
                .any(|e| matches!(e, Event::Shot { player: 0, .. }))
        {
            shot = Some(n);
        }
    }
    (script.expect("a jump shot"), shot)
}

#[test]
fn a_held_shot_leaves_when_its_gate_opens_by_itself() {
    // A person holding Shoot through a jump shot: the script waits at its
    // release gate (`86 19`, callback 19 `FUN_00174c10`) until the jump
    // is 15 updates from landing, `latest` updates after the press
    // however long it has waited, and the ball leaves `release - gate`
    // later: the stepback at 47, `legkick_1` and `legkick_2` at 48 (the
    // played captures: the stepback held 45 left at 47). Where the sim's
    // own draw from a standing press is the stepback (after the standing
    // defender's stand draw) it leaves at 47; the other two are put on the
    // press.
    let mut drawn = 0;
    for k in 0..200u32 {
        let seed = k.wrapping_mul(0x9e37_79b9);
        let mut w = driver([20.0, 0.0], 0.0, seed);
        let (script, shot) = hold_shot(&mut w, 200, Input::default(), None);
        if script == timing::DEFAULT_SHOTS[2] {
            assert_eq!(shot, Some(47), "seed {seed}");
            drawn += 1;
        }
    }
    assert!(drawn > 0, "no stepback drawn");
    for (k, left) in timing::DEFAULT_SHOTS.into_iter().zip([48, 48, 47]) {
        let mut w = driver([20.0, 0.0], 0.0, 1);
        let (script, shot) = hold_shot(&mut w, 200, Input::default(), Some(k));
        let s = &timing::SHOT_SCRIPTS[script];
        assert_eq!(u32::from(s.latest + s.release - s.gate), left, "{}", s.name);
        assert_eq!(shot, Some(left), "{}", s.name);
        // Let go before the gate opens by itself: the button's update
        // plus the tail.
        let mut w = driver([20.0, 0.0], 0.0, 1);
        let up = u32::from(s.gate) + 5;
        let (_, shot) = hold_shot(&mut w, up, Input::default(), Some(k));
        assert_eq!(shot, Some(up + u32::from(s.release - s.gate)), "{}", s.name);
    }
}

#[test]
fn a_held_side_fade_leaves_when_callback_18s_landing_opens_its_gate() {
    // Running across the basket's line with the rim on the right and the
    // ball not in the left hand, a press plays `jumpshot_sidefade_right_l`
    // (`FUN_0016b4b8`, facing 0x2a0..0x32d). Its jump is game callback 18
    // (`FUN_00172788`, at 10, 0.25 up: landing 45 later) and its clip
    // (322) is not one of `0x3d9218`, so the gate (28) opens by itself at
    // 50, 5 before the landing: held, the ball leaves at 54. Let go at 30
    // it leaves at 34 (the played captures' two holds of 30), at 10 on
    // the table's 32.
    for (hold, left) in [(200, 54), (49, 53), (30, 34), (10, 32)] {
        let mut w = driver([20.0, 0.0], 8.0, 3);
        w.players[0].set_heading(0.0); // running to +z, the rim on the right
        w.players[0].gait_channel.hand = 2;
        let (script, shot) = hold_shot(&mut w, hold, stick(0.0, 1.0), None);
        assert_eq!(script, timing::SIDEFADE_RIGHT_L, "hold {hold}");
        assert_eq!(shot, Some(left), "hold {hold}");
    }
}

#[test]
fn the_shot_chooser_reads_the_facing_from_the_rim_in_the_originals_sense() {
    // `FUN_0016b4b8` branches on `FUN_0018d290`'s angle, 0 facing the rim
    // and 512 facing away. Running 27° off square and away from the rim
    // (team 0's, at +x) the angle is 690, in the right band: hand 2 plays
    // `right_l`; 60° from facing the rim (170, the left band) hand 1
    // plays `left_r`. From long range, running at the rim is the runner
    // and running away the default three (the 400..600 branch, state bit
    // 11 clear). Measured on the mirrored court with the sim's heading as
    // the word, the four read 845, 341, 513 and 0.
    let pick = |at: [f32; 2], heading: f32, hand: u8| {
        let mut w = driver(at, 8.0, 3);
        w.players[0].set_heading(heading);
        w.players[0].gait_channel.hand = hand;
        let ahead = stick(heading.sin(), heading.cos());
        step_all(&mut w, &[(0, ahead.with(button::SHOOT))], 1);
        match w.players[0].action {
            Action::Shooting { script, .. } => usize::from(script),
            ref other => panic!("{other:?}"),
        }
    };
    let unit = std::f32::consts::TAU / 1024.0;
    let half_pi = std::f32::consts::FRAC_PI_2;
    assert_eq!(pick([20.0, 0.0], -77.0 * unit, 2), timing::SIDEFADE_RIGHT_L);
    assert_eq!(pick([20.0, 0.0], -597.0 * unit, 1), timing::SIDEFADE_LEFT_R);
    assert_eq!(pick([10.0, 0.0], half_pi, 1), timing::RUNNER);
    assert!(timing::DEFAULT_SHOTS.contains(&pick([10.0, 0.0], -half_pi, 1)));
}

#[test]
fn the_shot_chooser_knows_the_shooter() {
    // `FUN_0016b4b8` reads the record (actor+0x288) and its field 12:
    // record 172 shoots `jumpshot_bird` where others draw the default
    // three, a woman `jumpshot_1` after the draw and her stepping set shot
    // for the runner, and records 149, 166 and 178 their own hooks off a
    // side fade's draw. A two-turbo shot plays the player's own slot.
    let pick = |shooter: crate::Shooter, at: [f32; 2], heading: f32, hand: u8, extra: u16| {
        let mut w = driver(at, 8.0, 3);
        w.players[0].shooter = Some(shooter);
        w.players[0].set_heading(heading);
        w.players[0].gait_channel.hand = hand;
        let ahead = stick(heading.sin(), heading.cos());
        step_all(&mut w, &[(0, ahead.with(button::SHOOT | extra))], 1);
        match w.players[0].action {
            Action::Shooting { script, .. } => usize::from(script),
            ref other => panic!("{other:?}"),
        }
    };
    let who = crate::Shooter::shared;
    let unit = std::f32::consts::TAU / 1024.0;
    let half_pi = std::f32::consts::FRAC_PI_2;
    let away = -half_pi;
    assert_eq!(
        pick(who(172, false), [10.0, 0.0], away, 1, 0),
        timing::BIRD_SHOT
    );
    assert_eq!(
        pick(who(3, true), [10.0, 0.0], away, 1, 0),
        timing::FEMALE_SHOT
    );
    assert_eq!(
        pick(who(3, true), [10.0, 0.0], half_pi, 1, 0),
        timing::FEMALE_RUNNER
    );
    assert_eq!(
        pick(who(3, false), [10.0, 0.0], half_pi, 1, 0),
        timing::RUNNER
    );
    for (record, hook) in timing::LEGEND_HOOKS {
        assert_eq!(
            pick(who(record, false), [20.0, 0.0], -77.0 * unit, 1, 0),
            hook
        );
    }
    // The hand check comes first: hand 2 in the right band is the fade.
    assert_eq!(
        pick(who(149, false), [20.0, 0.0], -77.0 * unit, 2, 0),
        timing::SIDEFADE_RIGHT_L
    );
    let mut own = who(3, false);
    own.slots[0] = 21;
    assert_eq!(pick(own, [10.0, 0.0], away, 1, button::L1 | button::R1), 21);
}

#[test]
fn the_state_bits_show_the_post_up_the_rim_and_the_box_out() {
    // Bits 11 (`FUN_0016ea20`), 14 (`FUN_0016ea50`), 17 (`FUN_0016e970`)
    // and 18 (`FUN_0016e980`), all inside the camera's mask `0x7ffdc`.
    let mut w = driver([20.0, 0.0], 0.0, 1);
    assert_eq!(w.actor_state_bits(0), 0);
    w.players[0].posting = true;
    w.players[0].on_rim = true;
    w.players[0].boxing = true;
    w.players[0].boxed = true;
    assert_eq!(w.actor_state_bits(0), 1 << 11 | 1 << 14 | 1 << 17 | 1 << 18);
}

#[test]
fn the_contest_skips_the_fallen_and_the_counter_stance() {
    // `0x172c8c..0x172d04`: opponents within 7 are near and those in a
    // block jump contest, but one down (flag 0x40) or in the counter
    // stance (bit 16) is not counted at all.
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [20.0, 0.0], 0.0),
            Player::new(1, [22.0, 0.0], 0.0),
            Player::new(1, [20.0, 3.0], 0.0),
            Player::new(1, [20.0, -3.0], 0.0),
        ],
        Rules::default(),
        1,
    );
    assert_eq!(w.contest(0), (3, 0));
    w.players[1].action = Action::Jumping {
        elapsed: 0.0,
        high: false,
        block: None,
    };
    assert_eq!(w.contest(0), (3, 1));
    w.players[2].action = Action::Stealing {
        elapsed: 0.0,
        counter: true,
        swipe: None,
        stance: None,
    };
    w.players[3].action = Action::Stumbling {
        elapsed: 0.0,
        duration: 1.0,
        reaction: None,
    };
    assert_eq!(w.contest(0), (1, 1));
}

#[test]
fn a_shooters_own_dunk_slots_and_unlocks() {
    // `FUN_00139d38`: a slot the record answers for unlocks its dunk by
    // the row's level; otherwise the shared table.
    let shared = crate::Shooter::shared(3, false);
    assert_eq!(shared.dunk_unlock(3), 1);
    assert_eq!(shared.dunk_unlock(48), 16);
    let mut own = shared;
    own.dunks[5] = 3;
    own.dunk_unlocks[5] = 4;
    assert_eq!(own.dunk_unlock(3), 4);
    assert_eq!(own.dunk_unlock(4), 2);
    // A two-turbo press plays the slot's own dunk.
    let mut w = driver([30.0, 0.0], 0.0, 1);
    w.players[0].shooter = Some(own);
    let row = timing::MOVE_ROWS
        .iter()
        .position(|r| r.family == 0 && r.id == 3)
        .unwrap();
    let slot5 = button::L1 | button::R1;
    assert_eq!(
        timing::press_move(&mut w, 0, slot5),
        timing::PressMove::Row {
            row: row as u8,
            advanced: false
        }
    );
}

#[test]
fn a_turbo_block_under_the_rim_goaltends_some_shots() {
    let mut goaltends = 0;
    for seed in 0..40 {
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [20.0, 0.0], std::f32::consts::FRAC_PI_2),
                Player::new(1, [36.5, 0.5], -std::f32::consts::FRAC_PI_2),
            ],
            Rules::default(),
            seed,
        );
        w.phase = Phase::Live;
        w.ball.hold(0);
        for t in 0..3 * TICK_HZ {
            // The button held through the check (12..15 after the press;
            // up before it would be a pump fake) and let go before the
            // release gate.
            let shooter = if t < 16 {
                buttons(button::SHOOT)
            } else {
                Input::default()
            };
            // Jump as the ball comes down toward the rim.
            let coming =
                w.ball.state == BallState::Shot && w.ball.velocity[1] < 0.0 && w.players[1].free();
            let defender = if coming {
                buttons(button::JUMP | button::R1 | button::L1)
            } else {
                Input::default()
            };
            w.step(&[shooter, defender]);
            if w.events.iter().any(|e| matches!(e, Event::Goaltend { .. })) {
                goaltends += 1;
            }
        }
    }
    assert!(goaltends >= 8, "{goaltends} of 40");
}

#[test]
fn a_double_tap_of_turbo_dives_for_a_loose_ball() {
    let mut w = driver([0.0, 0.0], 0.0, 1);
    w.ball.drop_loose();
    w.ball.position = [-6.0, ball::RADIUS, 0.0];
    w.ball.velocity = [0.0; 3];
    w.players[0].heading = -std::f32::consts::FRAC_PI_2;
    step_all(&mut w, &[(0, buttons(button::R1))], 1);
    step_all(&mut w, &[], 3);
    step_all(&mut w, &[(0, buttons(button::R1))], 1);
    assert!(w.events.contains(&Event::Dive { player: 0 }));
    step_all(&mut w, &[], TICK_HZ);
    assert_eq!(w.handler(), Some(0));
}

#[test]
fn a_rebound_caught_in_the_air_is_tipped_in_on_a_midair_script() {
    // In a rebound jump (bit 8) with the ball, more than 1 up in the half
    // his side attacks, shoot plays `FUN_0016b1e8`'s mid-air lay-up: within
    // 5.25 of the rim one of the first six (`rand() % 6`). A block jump's
    // catch (bit 12) has none.
    let rebound = |w: &mut World| {
        w.players[0].action = Action::ReboundJump {
            script: 0,
            tick: 20,
            vy: 0.1,
            drift: [0.0, 0.0],
            up: true,
            meet: 0,
            end: 60,
            barred: false,
            branch: false,
            from: None,
            airtime: 30,
            word: 0,
        };
        w.players[0].height = 2.5;
    };
    for seed in 0..8 {
        let mut w = driver([34.0, 0.0], 0.0, seed);
        rebound(&mut w);
        step_all(&mut w, &[(0, buttons(button::SHOOT))], 1);
        let Action::Layup { row, .. } = w.players[0].action else {
            panic!("seed {seed}: {:?}", w.players[0].action);
        };
        assert!((row - timing::MIDAIR_ROWS) < 6, "seed {seed}: row {row}");
    }
    let mut w = driver([34.0, 0.0], 0.0, 1);
    w.players[0].action = Action::Jumping {
        elapsed: 0.3,
        high: false,
        block: None,
    };
    w.players[0].height = 2.5;
    step_all(&mut w, &[(0, buttons(button::SHOOT))], 1);
    assert!(matches!(w.players[0].action, Action::Jumping { .. }));
}

fn world_with(rules: Rules) -> World {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(1, [0.0, 3.0], std::f32::consts::PI),
        ],
        rules,
        1,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    w
}

#[test]
fn scoring_systems_value_baskets_as_the_game_says() {
    let street = Rules::default();
    let league = Rules::mode(Mode::LeagueScoring);
    let old = Rules::mode(Mode::OldSchool);
    let long = Basket::Jumper { long: true };
    let short = Basket::Jumper { long: false };
    assert_eq!((short.points(&street), long.points(&street)), (1, 2));
    assert_eq!((short.points(&league), long.points(&league)), (2, 3));
    assert_eq!((short.points(&old), long.points(&old)), (1, 1));
    let dunks = Rules::mode(Mode::DunksOnly);
    assert_eq!(short.points(&dunks), 0);
    assert_eq!(Basket::AlleyOop { advanced: false }.points(&dunks), 1);
    // The points table (`0x43b618`): a Level 1 takes 1 (2 in league
    // scoring), a Level 2 scores 2 and takes 3, from beyond the arc 4
    // (league: 3, taking 4 and 7); No Gamebreakers' and the trick-point and
    // Gamebreaker Wins games' rows give a gamebreaker nothing.
    let gb = |level, long, dunk| Basket::Gamebreaker { level, long, dunk };
    let both = |b: Basket, r: &Rules| (b.points(r), b.taken(r));
    assert_eq!(both(gb(1, false, false), &street), (1, 1));
    assert_eq!(both(gb(1, true, false), &street), (2, 1));
    assert_eq!(both(gb(2, false, false), &street), (2, 3));
    assert_eq!(both(gb(2, true, false), &street), (2, 4));
    assert_eq!(both(gb(1, false, false), &league), (2, 2));
    assert_eq!(both(gb(2, true, false), &league), (3, 7));
    assert_eq!(both(gb(2, true, false), &old), (2, 3));
    // A staged Level 2 dunk scores in Dunks Only; a jumper there doesn't.
    assert_eq!(both(gb(2, false, true), &dunks), (2, 3));
    assert_eq!(both(gb(2, false, false), &dunks), (0, 0));
    let no_gb = Rules::mode(Mode::NoGamebreakers);
    assert_eq!(both(gb(2, false, false), &no_gb), (0, 0));
    for mode in [Mode::TrickPointChallenge, Mode::GamebreakerWins] {
        let r = Rules::mode(mode);
        assert_eq!(both(long, &r), (0, 0));
        assert_eq!(both(gb(2, true, false), &r), (0, 0));
    }
}

#[test]
fn spotted_points_start_on_the_board() {
    let w = world_with(Rules {
        spot: [0, 5],
        spot_trick_points: [10_000, 0],
        ..Rules::default()
    });
    assert_eq!(w.score, [0, 5]);
    assert_eq!(w.teams[0].trick_points, 10_000);
}

/// The trick-point game is won at a basket (`FUN_0018c450` on slot 26,
/// then `FUN_0018c620`): the scorer's side past the target and the other
/// side's trick points. Reaching the target alone wins nothing, nor does a
/// basket by the side behind.
#[test]
fn a_trick_point_challenge_is_won_at_a_basket_past_the_target() {
    let mut w = world_with(Rules::mode(Mode::TrickPointChallenge));
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    w.trick_points(0, 100_001);
    assert_eq!(w.stats.team(0, crate::stats::TRICK_POINTS), 100_001);
    // Past the target with no basket: play goes on.
    assert_eq!(w.phase, Phase::Live);
    // The side behind, past the target too: its basket wins nothing while
    // it is behind (the basket's own trick points counted first).
    w.trick_points(1, 50_000);
    w.score_basket(1, Basket::Jumper { long: false });
    assert!(matches!(w.phase, Phase::DeadBall { .. }), "{:?}", w.phase);
    w.phase = Phase::Live;
    w.trick_points(1, 100_000);
    w.score_basket(0, Basket::Jumper { long: false });
    assert!(matches!(w.phase, Phase::DeadBall { .. }), "{:?}", w.phase);
    w.phase = Phase::Live;
    w.score_basket(1, Basket::Jumper { long: false });
    assert_eq!(w.phase, Phase::Over { winner: 1 });
    // A basket scores no points in this game.
    assert_eq!(w.score, [0, 0]);
}

#[test]
fn gamebreaker_wins_ends_on_the_first_gamebreaker() {
    let mut w = world_with(Rules::mode(Mode::GamebreakerWins));
    w.score_basket(0, Basket::Jumper { long: false });
    assert!(matches!(w.phase, Phase::DeadBall { .. }));
    w.phase = Phase::Live;
    w.score_basket(
        1,
        Basket::Gamebreaker {
            level: 1,
            long: false,
            dunk: false,
        },
    );
    assert_eq!(w.phase, Phase::Over { winner: 1 });
}

#[test]
fn back_to_basics_has_no_trick_points_or_gamebreakers() {
    let mut w = world_with(Rules::mode(Mode::BackToBasics));
    w.award(0, 50_000, true);
    assert_eq!(w.teams[0].trick_points, 0);
    assert!(!w.teams[0].meter_full());
}

#[test]
fn unlimited_turbo_never_runs_out() {
    let mut w = world_with(Rules {
        unlimited_turbo: true,
        ..Rules::default()
    });
    for _ in 0..5 * TICK_HZ {
        w.step(&[stick(1.0, 0.0).with(button::R1), Input::default()]);
    }
    assert_eq!(w.players[0].turbo, 1.0);
}

/// Every mode plays to a finish with computer players.
#[test]
fn every_mode_finishes() {
    for mode in [
        Mode::DunksOnly,
        Mode::TrickPointChallenge,
        Mode::GamebreakerWins,
        Mode::OldSchool,
    ] {
        let mut w = three_on_three(7);
        w.rules = Rules::mode(mode);
        for _ in 0..40 * 60 * TICK_HZ {
            ai_step(&mut w);
            if matches!(w.phase, Phase::Over { .. }) {
                break;
            }
        }
        println!(
            "{}: {:?} trick points {:?} after {:.0} s",
            mode.name(),
            w.score,
            w.teams.map(|t| t.trick_points),
            w.tick as f32 / TICK_HZ as f32
        );
        assert!(
            matches!(w.phase, Phase::Over { .. }),
            "{} never ends",
            mode.name()
        );
    }
}

/// A staged plan for the tests: role 0 passes to role 1 (frames 5..15),
/// who releases a real shot at 20, on a 60-frame script staged everywhere.
fn staged_config() -> gamebreaker::Config {
    use gamebreaker::*;
    let track = |slot: u16, x: f32, z: f32| ActorTrack {
        frame: 0,
        duration: 60,
        slot,
        pose: ActorPose {
            position: [x, 0.0, z],
            heading: std::f32::consts::FRAC_PI_2,
            animation: 0,
            animation_frame: 0.0,
        },
        animation_speed: 0.5,
        playback: None,
        keep_position: false,
        keep_heading: false,
    };
    let cue = |frame: u32, duration: u32, op: u8, from: i16, to: i16| {
        let mut data = [0u8; 36];
        data[0] = op;
        data[2..4].copy_from_slice(&from.to_le_bytes());
        data[4..6].copy_from_slice(&to.to_le_bytes());
        BallCue {
            frame,
            duration,
            data,
        }
    };
    let mut config = Config {
        plans: vec![SequencePlan {
            script: 12,
            kind: 1,
            frame_count: 60,
            actors: (0..6)
                .map(|s| track(s, -30.0 + f32::from(s), f32::from(s) - 3.0))
                .collect(),
            curves: vec![Curve {
                slot: 1,
                field: 0,
                start_frame: 0,
                end_frame: 20,
                start: -29.0,
                end: -33.0,
                start_tangent: 0.0,
                end_tangent: 0.0,
                interpolation: 1,
            }],
            ball_cues: vec![cue(5, 10, 0, 0, 1), cue(20, 10, 2, 1, -1)],
            controls: Controls::default(),
            finishes: Vec::new(),
        }],
        regions: RegionMap::from_rows(&vec![vec![5u8; 320]; 206]),
        hands: vec![HandTrack {
            animation: 0,
            positions: vec![[0.0, 5.0, 0.0]],
            width_deltas: Vec::new(),
        }],
        ..Config::default()
    };
    config.families[5][0] = Some(12);
    config.validate().unwrap();
    config
}

#[test]
fn a_staged_loop_preserves_its_rate_before_a_negative_height_is_clamped() {
    for team in 0..2 {
        let mut config = staged_config();
        let plan = &mut config.plans[0];
        plan.actors[0].pose.position[1] = -0.125;
        let mut flags = [0; 16];
        flags[1] = 1;
        flags[6] = 1;
        plan.actors[0].playback = Some(gamebreaker::NativePlayback {
            frames: 40,
            speed_word: 128,
            end_fraction: 256,
            flags,
            initial: true,
            loop_distances: [Some(2.0f32.to_bits()); 2],
        });
        plan.curves.push(gamebreaker::Curve {
            slot: 0,
            field: 0,
            start_frame: 0,
            end_frame: 20,
            start: -30.0,
            end: -34.0,
            start_tangent: 0.0,
            end_tangent: 0.0,
            interpolation: 1,
        });
        config.validate().unwrap();
        let mut w = three_on_three(3);
        w.staged = Some(std::sync::Arc::new(config));
        w.phase = Phase::Live;
        w.possession = team;
        let shooter = team * 3;
        w.ball.hold(shooter);
        w.teams[team].pocketed = true;
        w.teams[team].meter = street::METER_FULL;
        let none = vec![Input::default(); 6];
        let mut shoot = none.clone();
        shoot[shooter] = buttons(button::SHOOT | button::L1 | button::L2);
        w.step(&shoot);
        let mut observed = 0;
        for _ in 0..12 {
            w.step(&none);
            if let Some(sequence) = w.sequence {
                observed += 1;
                let clock = sequence.native_clocks[0].unwrap();
                assert_eq!(w.players[shooter].height, 0.0);
                assert_eq!(
                    clock.step_bits,
                    0.5f32.to_bits(),
                    "team {team}, frame {}",
                    sequence.frame
                );
            }
        }
        assert!(observed >= 10, "the production staged path did not run");
    }
}

#[test]
fn a_staged_level_two_plays_its_script_and_scores_on_physical_shot_arrival() {
    let mut w = three_on_three(3);
    w.staged = Some(std::sync::Arc::new(staged_config()));
    w.phase = Phase::Live;
    w.ball.hold(0);
    w.score = [10, 10];
    w.teams[0].pocketed = true;
    w.teams[0].meter = street::METER_FULL;
    assert_eq!(w.teams[0].gamebreaker(), 2);
    let none = vec![Input::default(); 6];
    let mut shoot = none.clone();
    shoot[0] = buttons(button::SHOOT | button::L1 | button::L2);
    w.step(&shoot);
    // Posed two updates after the press (`World::staged_start`).
    assert!(w.sequence.is_none() && w.staged_start.is_some());
    w.step(&none);
    w.step(&none);
    let s = w.sequence.expect("the staged sequence starts");
    assert_eq!((s.script, s.shooter, s.team), (12, 0, 0));
    // The scripts are authored attacking -x: team 0 plays them mirrored.
    assert!(s.mirrored(), "team 0 plays the script mirrored");
    // Role 0 is the shooter, standing where the script puts role 0,
    // mirrored.
    assert_eq!(w.players[0].position, [30.0, -3.0]);
    // Mid-sequence the world survives a snapshot (the plans are the disc's,
    // installed again by the machine reading it: crate::disc) and keeps
    // its checksum.
    for _ in 0..10 {
        w.step(&none);
    }
    let back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back.checksum(), w.checksum());
    let back = back.with_tables_of(&w);
    assert_eq!(back, w);
    assert_eq!(back.checksum(), w.checksum());
    // The shot is the shooter's own (`FUN_00333ef8`): its flight comes
    // from the launch angle, with the launcher's half-second and 14-unit
    // floors, so this plan's low release lands after its 60 frames; the
    // ball flies on and the basket counts as the Level 2's when it drops.
    let mut scored_at = None;
    let mut ticks = 0;
    while scored_at.is_none() {
        let frame = w.sequence.map(|s| s.frame);
        w.step(&none);
        ticks += 1;
        if w.events.iter().any(|e| {
            matches!(
                e,
                Event::Score {
                    team: 0,
                    basket: Basket::Gamebreaker { level: 2, .. },
                    ..
                }
            )
        }) {
            scored_at = Some(frame);
        }
        assert!(ticks < 200, "the basket counts");
    }
    assert!(w.sequence.is_none());
    // Inside the arc: two points scored, three taken (the points table's
    // Level 2 row, as the School's staged run scored it: 20 to 22).
    assert_eq!(w.score, [12, 7]);
    assert!(matches!(w.phase, Phase::DeadBall { team: 1, .. }));
    // Play resumes: the other team inbounds (the restart's walk).
    for _ in 0..12 * TICK_HZ {
        w.step(&none);
        if w.phase == Phase::Live && w.handler().is_some() {
            break;
        }
    }
    assert_eq!(w.team_of(w.handler().expect("inbounded")), 1);
}

#[test]
fn team_one_plays_staged_scripts_as_authored() {
    let mut w = three_on_three(3);
    w.staged = Some(std::sync::Arc::new(staged_config()));
    w.phase = Phase::Live;
    w.ball.hold(3);
    w.possession = 1;
    w.teams[1].pocketed = true;
    w.teams[1].meter = street::METER_FULL;
    let none = vec![Input::default(); 6];
    let mut shoot = none.clone();
    shoot[3] = buttons(button::SHOOT | button::L1 | button::L2);
    w.step(&shoot);
    // Posed two updates after the press (`World::staged_start`).
    assert!(w.sequence.is_none() && w.staged_start.is_some());
    w.step(&none);
    w.step(&none);
    let s = w.sequence.expect("the staged sequence starts");
    // Team 1 attacks -x, the end the scripts are authored for.
    assert!(!s.mirrored());
    assert_eq!(w.players[3].position, [-30.0, -3.0]);
    // The script faces -x (a model turn of +90 degrees).
    assert!((w.players[3].forward()[0] + 1.0).abs() < 1e-4);
    // Roles 0..2 are the shooter's side, 3..5 the other.
    for (role, p) in s.actors.iter().enumerate().take(6) {
        let team = w.players[p.unwrap()].team;
        assert_eq!(team, if role < 3 { 1 } else { 0 });
    }
}

#[test]
fn a_staged_script_freezes_and_stops_where_its_commands_say() {
    let mut config = staged_config();
    config.plans[0].controls = gamebreaker::Controls {
        stop: Some(40),
        hold: Some(gamebreaker::Hold {
            frame: 12,
            duration: 6,
            camera: 1,
            holds_replays: true,
        }),
        rerun: None,
    };
    config.validate().unwrap();
    let mut w = three_on_three(3);
    w.staged = Some(std::sync::Arc::new(config));
    w.phase = Phase::Live;
    w.ball.hold(0);
    w.teams[0].pocketed = true;
    w.teams[0].meter = street::METER_FULL;
    let none = vec![Input::default(); 6];
    let mut shoot = none.clone();
    shoot[0] = buttons(button::SHOOT | button::L1 | button::L2);
    w.step(&shoot);
    w.step(&none);
    w.step(&none);
    assert!(w.sequence.is_some());
    let mut frames = Vec::new();
    while let Some(s) = w.sequence {
        frames.push((s.frame, s.held));
        if s.held > 0 {
            // Frozen mid-sequence, and a snapshot keeps the freeze.
            let back = World::from_snapshot(&w.to_snapshot()).unwrap();
            assert_eq!(back.with_tables_of(&w), w);
        }
        w.step(&none);
        assert!(frames.len() < 100);
    }
    // Frame 12 holds for six more ticks, on the freeze's camera from its
    // own frame; play shows the frame before the manager's.
    let held: Vec<u32> = frames
        .iter()
        .filter(|&&(f, _)| f == 12)
        .map(|&(_, h)| h)
        .collect();
    assert_eq!(held, [0, 1, 2, 3, 4, 5, 6]);
    let plan = w.staged.as_ref().unwrap().plan(12).unwrap();
    assert_eq!(plan.camera_at(12.0, 1.0, false), (1, 12.0));
    assert_eq!(plan.camera_at(12.0, 3.0, false), (1, 14.0));
    assert_eq!(plan.camera_at(12.0, 6.0, true), (0, 11.0));
    assert_eq!(plan.camera_at(13.0, 0.0, false), (0, 12.0));
    // The script stops at frame 40, short of its 60.
    assert_eq!(frames.last().map(|&(f, _)| f), Some(40));
}

#[test]
fn a_staged_scripts_replay_plays_as_the_games_replay_and_its_freeze_holds_the_ring() {
    // Played as a match steps its replays (`street-net`'s `Match`): the
    // world only while no replay plays, the replays every update after it.
    let mut config = staged_config();
    config.plans[0].controls = gamebreaker::Controls {
        stop: Some(40),
        hold: Some(gamebreaker::Hold {
            frame: 12,
            duration: 6,
            camera: 1,
            holds_replays: true,
        }),
        rerun: Some(gamebreaker::Rerun {
            from: 10,
            to: 40,
            speed: 50,
            camera: 2,
        }),
    };
    config.validate().unwrap();
    let mut w = three_on_three(3);
    w.staged = Some(std::sync::Arc::new(config));
    w.phase = Phase::Live;
    w.ball.hold(0);
    w.teams[0].pocketed = true;
    w.teams[0].meter = street::METER_FULL;
    let mut replays = crate::replay::Replays::default();
    let none = vec![Input::default(); 6];
    let mut shoot = none.clone();
    shoot[0] = buttons(button::SHOOT | button::L1 | button::L2);
    let mut frozen_ticks = Vec::new();
    let mut rerun = None;
    let mut world_steps_in_replay = 0;
    for u in 0..500 {
        let stepped = !replays.holds_world();
        let mut events = Vec::new();
        if stepped {
            w.step(if u == 0 { &shoot } else { &none });
            events = w.events.clone();
            if w.sequence.is_some_and(|s| s.held > 0) {
                frozen_ticks.push(w.tick);
            }
        }
        let had = replays.staged.is_some();
        replays.update(&mut w, &mut events, stepped, &[], 1, false);
        // The staged gamebreaker's task is the simulation's: no shot's
        // replays (mode 5) among its script.
        assert!(!replays.task.busy() && replays.manager.mode != 5);
        if let Some(e) = events
            .iter()
            .find(|e| matches!(e, Event::StagedRerun { .. }))
        {
            // The script's last frame: its sequence ends this update.
            assert!(w.sequence.is_none());
            rerun = Some((*e, w.tick));
            assert!(replays.manager.busy() && replays.manager.mode == 7);
        }
        if had && stepped {
            world_steps_in_replay += 1;
        }
        if rerun.is_some() && !replays.holds_world() {
            break;
        }
    }
    let (event, at) = rerun.expect("the rerun");
    assert_eq!(
        event,
        Event::StagedRerun {
            script: 12,
            // Team 0 attacks +x: the script mirrored.
            mirrored: true,
            from: 10,
            frames: 30,
            speed: 50,
            camera: 2,
        }
    );
    // No game update while it played.
    assert_eq!(world_steps_in_replay, 0);
    // The freeze's six ticks stayed out of the ring, which holds every
    // other tick up to the rerun's.
    assert_eq!(frozen_ticks.len(), 6);
    let ring: Vec<u64> = replays.manager.slots().map(|s| s.tick).collect();
    assert!(frozen_ticks.iter().all(|t| !ring.contains(t)), "{ring:?}");
    // Its own update recorded last (`FUN_00329248(m, 0)` at the start).
    assert_eq!(*ring.last().unwrap(), at);
    // The world after it is the world the stop left.
    assert!(w.sequence.is_none());
}

#[test]
fn reaches_and_jumps_are_announced_for_the_listener() {
    // Player 1 defends player 0, who has the ball.
    let defend = || {
        let mut w = live(vec![
            Player::new(0, [-20.0, 0.0], 0.0),
            Player::new(1, [-18.0, 0.0], 0.0),
        ]);
        w.ball.hold(0);
        w
    };
    let mut w = defend();
    step_all(&mut w, &[(1, buttons(button::TRICK))], 1);
    assert!(w.events.contains(&Event::Reach {
        player: 1,
        counter: false
    }));
    // A jump at a held ball is a block jump (the game's 0x15d3a0)...
    let mut w = defend();
    step_all(&mut w, &[(1, buttons(button::JUMP))], 1);
    assert!(w.events.contains(&Event::Jump {
        player: 1,
        block: true
    }));
    // ...and at a loose one a rebound jump (0x15ce50).
    let mut w = live(vec![Player::new(1, [-18.0, 0.0], 0.0)]);
    step_all(&mut w, &[(0, buttons(button::JUMP))], 1);
    assert!(w.events.contains(&Event::Jump {
        player: 0,
        block: false
    }));
}

#[test]
fn a_dunk_is_let_go_at_the_rim_made_or_not() {
    for seed in 0..10 {
        // A two-turbo combo's slot dunk (`dunk_19`): always a dunk.
        let mut w = driver([30.0, 0.0], 10.0, seed);
        step_all(
            &mut w,
            &[(
                0,
                stick(1.0, 0.0).with(button::SHOOT | button::R1 | button::L2),
            )],
            1,
        );
        let mut slams = 0;
        for _ in 0..2 * TICK_HZ {
            step_all(&mut w, &[], 1);
            slams += w
                .events
                .iter()
                .filter(|e| matches!(e, Event::Slam { player: 0, hoop: 0 }))
                .count();
        }
        assert_eq!(slams, 1, "seed {seed}");
    }
}

#[test]
fn restart_formation_places_all_six_players_and_inbounds_to_possessing_slot_zero() {
    let mut w = three_on_three(1);
    w.restart_formation(0);
    assert_eq!(w.possession, 0);
    assert_eq!(w.phase, Phase::Live);
    assert!(matches!(w.ball.state, BallState::Held(0)));
    assert_eq!(w.players[0].position, [-47.0, 16.0]);
    assert_eq!(w.players[3].position, [-12.0, 10.0]);
    assert_eq!(w.players[1].position, [-32.0, -10.0]);
    assert_eq!(w.players[4].position, [13.0, -10.0]);
    assert_eq!(w.players[2].position, [-35.0, 8.0]);
    assert_eq!(w.players[5].position, [-22.0, 12.0]);

    // Team 1 restart
    w.restart_formation(1);
    assert_eq!(w.possession, 1);
    assert_eq!(w.phase, Phase::Live);
    assert!(matches!(w.ball.state, BallState::Held(3)));
    assert_eq!(w.players[3].position, [47.0, 16.0]);
    assert_eq!(w.players[0].position, [12.0, 10.0]);
    assert_eq!(w.players[4].position, [32.0, -10.0]);
    assert_eq!(w.players[1].position, [-13.0, -10.0]);
    assert_eq!(w.players[5].position, [35.0, 8.0]);
    assert_eq!(w.players[2].position, [22.0, 12.0]);
}

#[test]
fn authoritative_self_toss_script69_timeline_matches_native_execution() {
    let mut w = driver([0.0, 0.0], 0.0, 1);
    // Equip script 69 to slot 10 (L2 + R2)
    let mut slots = [crate::TrickIdentity {
        slot: 0,
        script: 1,
        level: 4,
        equipped_row: None,
    }; 11];
    slots[10] = crate::TrickIdentity {
        slot: 10,
        script: 69,
        level: 2,
        equipped_row: Some(65),
    };
    w.players[0].native_tricks = Some(crate::NativeTricks { slots });

    // Initiate trick with TRICK | L2 | R2
    let trigger_input = buttons(button::TRICK | button::L2 | button::R2);
    step_all(&mut w, &[(0, trigger_input)], 1);

    // Verify player is now in Action::Trick with script 69
    assert!(matches!(
        w.players[0].action,
        Action::Trick { id, .. } if id.script == 69
    ));
    assert_eq!(w.ball.state, BallState::Held(0));

    // The script's clock (the press update its tick 0, as the played
    // capture's script 69 at frame 7989 of pads-2 runs it): the blend (6)
    // and the leadup (14) hold the ball through tick 19, the 20th update.
    for tick in 1..=19 {
        step_all(&mut w, &[], 1);
        assert!(
            matches!(w.players[0].action, Action::Trick { .. }),
            "tick {tick}"
        );
        assert_eq!(w.ball.state, BallState::Held(0), "ball held at tick {tick}");
    }

    // Tick 20: callback 53 launches the ball (the capture: ball state 4,
    // one flight update in, the update the clip shows frame 7.5).
    step_all(&mut w, &[], 1);
    assert!(matches!(w.ball.state, BallState::Pass));
    let pass = w.ball.pass.unwrap();
    assert_eq!(pass.from, 0);
    assert_eq!(pass.to, 0);
    assert_eq!(pass.via, ball::Via::Direct);

    // The ball flies 28 updates: the launch's, then ticks 21..=46.
    for tick in 21..=46 {
        step_all(&mut w, &[], 1);
        assert!(
            matches!(w.ball.state, BallState::Pass),
            "ball in flight at tick {tick}"
        );
        assert!(
            matches!(w.players[0].action, Action::Trick { .. }),
            "player tricking at tick {tick}"
        );
    }

    // Tick 47 (the 28th of the flight): the catch (the capture's 8036).
    step_all(&mut w, &[], 1);
    assert_eq!(w.ball.state, BallState::Held(0), "ball caught at tick 47");
    assert!(
        w.events
            .iter()
            .any(|e| matches!(e, Event::Catch { player: 0 }))
    );
    assert!(
        matches!(w.players[0].action, Action::Trick { .. }),
        "followthrough active at tick 47"
    );

    // Followthrough: the last wait's 6 updates, ticks 48 through 53.
    for tick in 48..=53 {
        step_all(&mut w, &[], 1);
        assert_eq!(w.ball.state, BallState::Held(0));
        assert!(
            matches!(w.players[0].action, Action::Trick { .. }),
            "followthrough at tick {tick}"
        );
    }

    // Tick 54: the script ends (bit 10 drops, the capture's 8043).
    step_all(&mut w, &[], 1);
    assert_eq!(
        w.players[0].action,
        Action::None,
        "trick complete at tick 54"
    );
    assert_eq!(
        w.ball.state,
        BallState::Held(0),
        "ball still held by player 0"
    );
}

#[test]
fn authoritative_bounce_self_toss_script23_timeline_and_rebound_match_native() {
    let mut w = driver([0.0, 0.0], 0.0, 1);
    let mut slots = [crate::TrickIdentity {
        slot: 0,
        script: 1,
        level: 4,
        equipped_row: None,
    }; 11];
    slots[5] = crate::TrickIdentity {
        slot: 5,
        script: 23,
        level: 2,
        equipped_row: Some(60),
    };
    w.players[0].native_tricks = Some(crate::NativeTricks { slots });

    // Trigger trick on slot 5 (TRICK | L1 | R1, mask 0xc000)
    let trigger_input = buttons(button::TRICK | button::L1 | button::R1);
    step_all(&mut w, &[(0, trigger_input)], 1);

    assert!(matches!(
        w.players[0].action,
        Action::Trick { id, .. } if id.script == 23
    ));
    assert_eq!(w.ball.state, BallState::Held(0));

    // G05: script 23 blends for 6 updates, waits 2, then calls 53 (its
    // 30 is the pose frame to sample, not a time): launch at tick 8 (the
    // press update its tick 0).
    for tick in 1..=7 {
        step_all(&mut w, &[], 1);
        assert!(matches!(w.players[0].action, Action::Trick { .. }));
        assert_eq!(w.ball.state, BallState::Held(0), "ball held at tick {tick}");
    }

    // Tick 8: launch fires into BallState::Pass with Via::Floor
    step_all(&mut w, &[], 1);
    assert!(matches!(w.ball.state, BallState::Pass));
    let pass = w.ball.pass.unwrap();
    assert_eq!(pass.from, 0);
    assert_eq!(pass.to, 0);
    assert_eq!(pass.via, ball::Via::Floor);
    assert!(
        w.ball.velocity[1] < 0.0,
        "initial vertical velocity must be downward toward floor"
    );

    // Flight for 54 ticks: ticks 9 through 61
    let mut floor_bounced = false;
    for tick in 9..=61 {
        step_all(&mut w, &[], 1);
        assert!(
            matches!(w.ball.state, BallState::Pass),
            "ball in flight at tick {tick}"
        );
        if w.ball.position[1] <= w.ball.radius + 1e-3 {
            floor_bounced = true;
        }
    }
    assert!(
        floor_bounced,
        "bounce self-toss ball must make floor contact"
    );

    // At tick 62 (54th flight update): catch occurs
    step_all(&mut w, &[], 1);
    assert_eq!(w.ball.state, BallState::Held(0), "ball caught at tick 62");
    assert!(
        w.events
            .iter()
            .any(|e| matches!(e, Event::Catch { player: 0 }))
    );

    // Script 23's closing wait is 2 updates: tick 63, then done at 64.
    for tick in 63..=63 {
        step_all(&mut w, &[], 1);
        assert_eq!(w.ball.state, BallState::Held(0));
        assert!(
            matches!(w.players[0].action, Action::Trick { .. }),
            "followthrough at tick {tick}"
        );
    }

    step_all(&mut w, &[], 1);
    assert_eq!(
        w.players[0].action,
        Action::None,
        "trick complete at tick 64"
    );
    assert_eq!(w.ball.state, BallState::Held(0));
}

#[test]
fn native_slot_arbitration_by_level_matches_fun_0015eff0() {
    let slots = std::array::from_fn(|i| crate::player::TrickIdentity {
        slot: i as u8,
        script: (100 + i) as u16,
        level: crate::player::TRICK_SLOT_LEVELS[i],
        equipped_row: Some(i as u8 + 54),
    });
    let mut rng = crate::rng::Rng::new(42);

    // Level 4 has exactly 1 slot (slot 0)
    let selected = World::select_slot_by_level(&mut rng, 4, &slots).unwrap();
    assert_eq!(selected.slot, 0);

    // Level 3 has 4 slots (slots 1, 2, 3, 4)
    let selected = World::select_slot_by_level(&mut rng, 3, &slots).unwrap();
    assert!((1..=4).contains(&selected.slot));

    // Level 2 has 6 slots (slots 5..=10)
    let selected = World::select_slot_by_level(&mut rng, 2, &slots).unwrap();
    assert!((5..=10).contains(&selected.slot));

    // Non-existent level returns None
    assert_eq!(World::select_slot_by_level(&mut rng, 5, &slots), None);
}

#[test]
fn jog_keeps_its_own_hysteresis_through_world_step() {
    // G04: stick 81 then 119 (intensities 41 then 60). Jog plays the walk
    // clip, but its 39/61 band holds it at Jog where Walk's 41/60 would
    // have turned it into a run.
    use crate::input::GaitFamily;
    let mut w = solo([0.0, 0.0]);
    let axis = |v: i8| Input::moving([f32::from(v) / 127.0, 0.0]);
    run(&mut w, axis(81), 1);
    assert_eq!(w.players[0].gait_channel.family, GaitFamily::Jog as u32);
    run(&mut w, axis(119), 1);
    assert_eq!(w.players[0].gait_channel.family, GaitFamily::Jog as u32);
    assert_ne!(w.players[0].gait, player::Gait::Run);
    // A full stick does leave Jog for Run.
    run(&mut w, axis(127), 1);
    assert_eq!(w.players[0].gait_channel.family, GaitFamily::Run as u32);
    // Letting go clears the family.
    run(&mut w, Input::default(), 1);
    assert_eq!(w.players[0].gait_channel.family, 0);
}

#[test]
fn the_gait_channel_clock_runs_while_standing_with_the_ball() {
    // G06: the standing dribble reads this clock, so it must not stop.
    let mut w = solo([0.0, 0.0]);
    w.ball.hold(0);
    run(&mut w, Input::default(), 1);
    let start = w.players[0].gait_channel.phase;
    run(&mut w, Input::default(), 120);
    let phase = w.players[0].gait_channel.phase;
    assert_eq!(w.players[0].gait, player::Gait::Stand);
    assert!(phase > start + 10.0, "{start} -> {phase}");
    // The view's bounce phase moves through its cycle.
    let bounces: Vec<f32> = (0..6)
        .map(|_| {
            run(&mut w, Input::default(), 7);
            (w.players[0].gait_channel.phase / 6.0).fract()
        })
        .collect();
    assert!(bounces.windows(2).any(|p| p[0] != p[1]), "{bounces:?}");
}

#[test]
fn a_planned_make_knocked_off_its_line_does_not_score() {
    // G07: a planned make cannot score a ball that never reached the
    // basket; an undisturbed make still does. The make is aimed as the
    // launcher aims one: at the middle 11.9567747 up, steep enough to drop
    // through the ring (`FUN_0014fc00`).
    let shoot = |deflect: bool| {
        let mut w = solo([-20.0, 0.0]);
        let rim = w.hoops[0].rim;
        let (from, to) = ([-20.0, 8.0, 0.0], [rim[0], NATIVE_AIM_HEIGHT, rim[2]]);
        let aim = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let angle = ball::steepen_entry(0.0, from, aim, w.ball.radius, false);
        w.ball
            .launch(from, to, ball::shot_flight_time(from, to, angle));
        w.ball.state = BallState::Shot;
        w.ball.shot = Some(Shot {
            shooter: 0,
            hoop: 0,
            made: true,
            points: 1,
            gamebreaker: 0,
            layup: false,
            tried: 0,
            slam: None,
        });
        let mut events = Vec::new();
        for tick in 0..120 {
            if deflect && tick == 20 {
                w.ball.velocity = [25.0, 0.0, 25.0];
            }
            w.step(&[Input::default()]);
            events.extend(w.events.iter().cloned());
            if w.ball.state != BallState::Shot {
                break;
            }
        }
        (w.score, events)
    };
    let (score, events) = shoot(false);
    assert_eq!(score[0], 1, "{events:?}");
    let (score, events) = shoot(true);
    assert_eq!(score, [0, 0]);
    assert!(
        events.iter().any(|e| matches!(e, Event::RimOut)),
        "{events:?}"
    );
}

#[test]
fn the_basket_and_net_follow_fun_001515a8_and_fun_00153550() {
    use crate::ball::{RIM_ENTRY, crossed_rim};
    let rim = [-36.95, 11.75, 0.0];
    // Down through the 11.91355 plane: inside 0.793225 of the centre at
    // the interpolated crossing point scores, outside does not, and going
    // up never does.
    assert!(crossed_rim([-36.95, 12.2, 0.5], [-36.95, 11.6, 0.5], rim));
    assert!(!crossed_rim([-36.95, 12.2, 0.9], [-36.95, 11.6, 0.9], rim));
    assert!(!crossed_rim([-36.95, 11.6, 0.0], [-36.95, 12.2, 0.0], rim));
    // The crossing point is interpolated: start outside, end inside.
    let (a, b) = ([-36.95 + 1.0, 12.0, 0.0], [-36.95 + 0.4, 11.8, 0.0]);
    // f = (11.8 - 11.91355) / -0.2 = 0.56775: x = 1.0 f + 0.4 (1 - f).
    let x = 1.0 * 0.56775 + 0.4 * (1.0 - 0.56775);
    assert_eq!(crossed_rim(a, b, rim), x <= RIM_ENTRY);
    // The net cone: at y the centre may be (y - 10.5)(0.75 - r)/1.4567747
    // from the middle; past it the ball is put back and slowed to a tenth
    // outside a shot.
    let mut ball = Ball::loose_at([-36.95 + 0.25, 11.0, 0.0]);
    ball.velocity = [4.0, -2.0, 0.0];
    let cone = (11.0 - 10.5) * (0.75 - ball.radius) / 1.456_774_7;
    assert!(ball.net(rim));
    assert!((ball.position[0] - (-36.95 + cone)).abs() < 1e-5);
    assert!((ball.velocity[0] - 0.4).abs() < 1e-6 && (ball.velocity[1] + 0.2).abs() < 1e-6);
    // Inside the cone nothing happens.
    let mut ball = Ball::loose_at([-36.95, 11.0, 0.0]);
    assert!(!ball.net(rim));
}

/// Release a shot by `player` at `hoop` from where they stand and fly it
/// out: (the release callback's make roll, the basket scored, the shot).
fn release_and_fly(w: &mut World, player: usize, layup: bool) -> (bool, bool, Shot) {
    let hoop = target_hoop(w.players[player].team);
    let before = w.score;
    if layup {
        w.release_layup(player, hoop);
    } else {
        w.release_shot(player, hoop);
    }
    let shot = w.ball.shot.expect("released");
    for _ in 0..(5 * TICK_HZ) {
        if w.ball.state != BallState::Shot {
            break;
        }
        w.step_ball(None);
    }
    (shot.made, w.score != before, shot)
}

#[test]
fn shots_go_in_exactly_when_the_release_callback_rolls_a_make() {
    // Every jump shot and lay-up meets the rim and the backboard and
    // scores only by dropping through the ring (`FUN_001515a8`); the
    // launcher's aims make that agree with the callback's roll every time.
    // The make rates are the traced percentages: an average (50) open
    // shooter, nobody within 7, level game.
    for (at, layup, rating, percent) in [
        // Lay-ups: 50/100 × 15 + 80 = 87, + 50/99 × 20 = 10.
        ([34.0, 0.0], true, 0.5, 97),
        ([35.15, 0.0], true, 0.5, 97),
        // 10 out: 70 − trunc(10/22 × 15) = 64, + 10.
        ([27.0, 0.0], false, 0.5, 74),
        ([30.0, 7.0], false, 0.5, 74),
        // 18 out: 70 − 12 + 10.
        ([19.0, 0.0], false, 0.5, 68),
        // 27 out (long): 22 − trunc(5/12 × 10) + trunc(50/99 × 31).
        ([10.0, 0.0], false, 0.5, 33),
        // A 99 Threes rating: 35 − 4 + 31.
        ([10.0, 0.0], false, 1.0, 62),
    ] {
        let (mut rolled, mut scored, n) = (0, 0, 400u32);
        for seed in 0..n {
            let mut w = face_off(seed.wrapping_mul(0x9e37_79b9));
            w.players[0].position = at;
            w.players[0].ratings.shots = rating;
            w.players[0].ratings.threes = rating;
            w.players[1].position = [-40.0, 30.0];
            w.ball.hold(0);
            let (made, went_in, shot) = release_and_fly(&mut w, 0, layup);
            assert_eq!(made, went_in, "{at:?} seed {seed}: {shot:?}");
            assert_eq!(shot.layup, layup);
            rolled += u32::from(made);
            scored += u32::from(went_in);
        }
        assert_eq!(rolled, scored);
        let rate = f64::from(scored) * 100.0 / f64::from(n);
        assert!(
            (rate - f64::from(percent)).abs() < 6.0,
            "{at:?}: {rate}% against {percent}%"
        );
    }
}

#[test]
fn the_release_callback_reads_the_court_the_ratings_and_the_score() {
    let call = |at: [f32; 2], defender: [f32; 2], jumping: bool, score: [u32; 2]| {
        let mut w = face_off(5);
        w.players[0].position = at;
        w.players[1].position = defender;
        if jumping {
            w.players[1].action = Action::Jumping {
                elapsed: 0.0,
                high: false,
                block: None,
            };
        }
        w.score = score;
        let from = [at[0], 9.0, at[1]];
        w.shot_call(0, 0, from, false, 0, false).percent
    };
    // 10 out, average Shots, nobody near: 64 + 10.
    assert_eq!(call([26.95, 0.0], [-10.0, 20.0], false, [0, 0]), 74);
    // Someone within 7: no open bonus; jumping (state bit 12, the sim's
    // reading) takes 5 more.
    assert_eq!(call([26.95, 0.0], [26.95, 5.0], false, [0, 0]), 64);
    assert_eq!(call([26.95, 0.0], [26.95, 5.0], true, [0, 0]), 59);
    // Behind 5 to 10 in a game to 21, mid-game (the higher score between
    // the sim's stage bounds 2 and 16): the table's +6; behind 0 to 1,
    // early: +2.
    assert_eq!(call([26.95, 0.0], [-10.0, 20.0], false, [5, 10]), 80);
    assert_eq!(call([26.95, 0.0], [-10.0, 20.0], false, [0, 1]), 76);
    // Level game late (both past 16): nothing.
    assert_eq!(call([26.95, 0.0], [-10.0, 20.0], false, [17, 17]), 74);
    // The wrong half of the court: 3, whatever else. Team 0 attacks +x,
    // and x = 0 counts as the +x half (`(x >= 0) == (side > 0)`).
    assert_eq!(call([-0.5, 0.0], [-10.0, 20.0], false, [0, 10]), 3);
    assert_eq!(call([-5.0, 0.0], [-10.0, 20.0], false, [0, 0]), 3);
    assert_ne!(call([0.0, 0.0], [-10.0, 20.0], false, [0, 10]), 3);
}

#[test]
fn every_launcher_aim_ends_the_way_it_was_asked() {
    // Straight in (0), off the glass (1), the front (2), back (3) and
    // sides (4, 5) of the rim, made and missed, from close, mid-range and
    // long, at both baskets.
    for kind in 0..6u8 {
        for made in [true, false] {
            for (team, at) in [
                (0, [28.0, 0.0]),
                (0, [31.0, -6.0]),
                (0, [14.0, 10.0]),
                (1, [-24.0, 8.0]),
            ] {
                for seed in 0..12u32 {
                    let mut w = face_off(seed);
                    w.players[0].team = team;
                    w.players[0].position = at;
                    w.players[1].position = [0.0, 30.0];
                    w.ball.hold(0);
                    let hoop = target_hoop(team);
                    let from = [at[0], 9.0, at[1]];
                    let call = shooting::ShotCall {
                        percent: 50,
                        made,
                        kind,
                        angle: std::f32::consts::FRAC_PI_4,
                    };
                    let shot = Shot {
                        shooter: 0,
                        hoop,
                        made,
                        points: 1,
                        gamebreaker: 0,
                        layup: false,
                        tried: 0,
                        slam: None,
                    };
                    w.launch_shot(hoop, from, call, shot, None);
                    let before = w.score;
                    for _ in 0..(5 * TICK_HZ) {
                        if w.ball.state != BallState::Shot {
                            break;
                        }
                        w.step_ball(None);
                    }
                    assert_ne!(w.ball.state, BallState::Shot);
                    assert_eq!(
                        w.score != before,
                        made,
                        "kind {kind} made {made} from {at:?} seed {seed}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_bank_shot_comes_off_the_glass_into_the_basket() {
    // `FUN_0014fa60`: aimed at the point mirrored behind the board with
    // restitution 0.75, the ball touches the glass and drops in.
    use crate::ball::{BOARD_FACE, RADIUS, bank_mirror};
    let x = bank_mirror(36.96, 1.0, RADIUS);
    assert!((x - (36.96 + (BOARD_FACE - RADIUS - 36.96) * (1.0 / 0.75 + 1.0))).abs() < 1e-4);
    let mut w = face_off(1);
    w.players[0].team = 1;
    w.players[0].position = [-30.0, 6.0];
    w.ball.hold(0);
    let call = shooting::ShotCall {
        percent: 100,
        made: true,
        kind: 1,
        angle: std::f32::consts::FRAC_PI_4,
    };
    let shot = Shot {
        shooter: 0,
        hoop: 1,
        made: true,
        points: 1,
        gamebreaker: 0,
        layup: false,
        tried: 0,
        slam: None,
    };
    w.launch_shot(1, [-30.0, 9.0, 6.0], call, shot, None);
    let mut glass = false;
    for _ in 0..(5 * TICK_HZ) {
        if w.ball.state != BallState::Shot {
            break;
        }
        let vx = w.ball.velocity[0];
        w.step_ball(None);
        glass |= vx < 0.0 && w.ball.velocity[0] > 0.0;
    }
    assert!(glass, "off the glass");
    assert_eq!(w.score[1], 1);
}

#[test]
fn a_shot_counts_its_flown_updates_against_its_launch_not_its_timer() {
    // The block test's late-flight let-go reads ball+0x60 over +0x64
    // (`0x16de54..0x16de6c`): the updates flown since the launch over the
    // launch's own flight. The shot's timer (ball+0x158) runs past the aim
    // (`straight_timer`), so a progress read off it came out low, and
    // negative once a timer longer than the flight was compared with it.
    let mut w = face_off(1);
    w.players[0].position = [20.0, 6.0];
    w.ball.hold(0);
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
        points: 2,
        gamebreaker: 0,
        layup: false,
        tried: 0,
        slam: None,
    };
    w.launch_shot(0, [20.0, 9.0, 6.0], call, shot, None);
    let flight = w.ball.flight_ticks();
    assert!(flight > 10, "{flight}");
    assert_eq!(w.ball.flown_ticks(), 0);
    assert!(w.ball.flight_start > w.ball.flight_time);
    for k in 1..=flight {
        w.step_ball(None);
        if w.ball.state != BallState::Shot {
            break;
        }
        assert_eq!(w.ball.flown_ticks(), k);
    }
}

#[test]
fn easy_and_hard_change_the_odds_only_with_a_person_playing() {
    // `configure_meter_fill` records which sides people play; the release
    // callback then applies Easy's +40 for the person and -10 for the
    // computer (within 33.75), and Hard drops the person's catch-up.
    let call = |difficulty: Difficulty, controlled: [bool; 2], shooter: usize| {
        let mut w = face_off(5);
        w.rules.difficulty = difficulty;
        w.configure_meter_fill(controlled, None);
        let at = if shooter == 0 {
            [26.95, 0.0]
        } else {
            [-26.96, 0.0]
        };
        w.players[shooter].position = at;
        w.players[1 - shooter].position = [0.0, 25.0];
        w.score = if shooter == 0 { [5, 10] } else { [10, 5] };
        let from = [at[0], 9.0, at[1]];
        let hoop = target_hoop(w.players[shooter].team);
        w.shot_call(shooter, hoop, from, false, 0, false).percent
    };
    // Nobody playing: Easy changes nothing (74 + the catch-up 6).
    assert_eq!(call(Difficulty::Easy, [false, false], 0), 80);
    // A person on the shooter's side: +40, catch-up doubled (capped at 98).
    assert_eq!(call(Difficulty::Easy, [true, false], 0), 98);
    // The computer shooting against a person: -10 and no catch-up.
    assert_eq!(call(Difficulty::Easy, [false, true], 0), 64);
    // Hard: the person's side loses the catch-up, the computer keeps it.
    assert_eq!(call(Difficulty::Hard, [true, false], 0), 74);
    assert_eq!(call(Difficulty::Hard, [false, true], 0), 80);
}

#[test]
fn dunks_make_by_the_release_callbacks_dunk_branch() {
    use crate::shooting::dunk_percent;
    let medium = Difficulty::Medium;
    let d = |rating, advanced| dunk_percent(rating, advanced, 0, None, medium, true, false);
    // 50: tier 3, half way; less 2 is 1: 98. 30: tier 2 less 2 is 0:
    // 70 x 0.5 + 98 x 0.5 = 84. 10: tier 1 less 2: 40 x 0.5 + 70 x 0.5.
    assert_eq!(d(50, None), 98);
    assert_eq!(d(30, None), 84);
    assert_eq!(d(10, None), 55);
    assert_eq!(d(0, None), 0);
    // An exact 40 is tier 2 with all of it: 98.
    assert_eq!(d(40, None), 98);
    // 85: tier 5 half way; an advanced dunk of level 4 leaves 1: 98; of a
    // level-6 move -1: 55.
    assert_eq!(d(85, Some(4)), 98);
    assert_eq!(d(85, Some(6)), 55);
    // 50 advanced at level 4: -1, 40 x 0.5 + 70 x 0.5 = 55.
    assert_eq!(d(50, Some(4)), 55);
    // Easy: a person's dunks +20 (capped 98), the computer's -10.
    assert_eq!(
        dunk_percent(30, None, 0, Some(true), Difficulty::Easy, true, false),
        98
    );
    assert_eq!(
        dunk_percent(30, None, 0, Some(false), Difficulty::Easy, true, false),
        74
    );
    // The wrong half: 3. A gamebreaker: 100.
    assert_eq!(dunk_percent(99, None, 0, None, medium, false, false), 3);
    assert_eq!(dunk_percent(0, None, 0, None, medium, true, true), 100);
    // Through the world: an average dunker's dunk is the 98.
    let mut w = face_off(1);
    w.players[0].position = [33.0, 0.0];
    w.players[0].ratings.dunks = 50.0 / 99.0;
    assert_eq!(w.dunk_call(0, 0, false), 98);
    // An advanced dunk takes the level of the turbo combo held, from the
    // dunk table at 0x43d3b0: all four turbos are level 4 (50: tier 3 - 4
    // = -1, 55), three are 3 (0, 84).
    use crate::button::{L1, L2, R1, R2};
    use crate::shooting::dunk_move_level;
    assert_eq!(dunk_move_level(L1 | R1 | L2 | R2), 4);
    assert_eq!(dunk_move_level(L1 | R1 | R2), 3);
    assert_eq!(dunk_move_level(R1 | L2), 2);
    w.players[0].action_buttons = L1 | R1 | L2 | R2;
    assert_eq!(w.dunk_call(0, 0, true), 55);
    w.players[0].action_buttons = L1 | R1 | L2;
    assert_eq!(w.dunk_call(0, 0, true), 84);
    // With the player's dunk set known, the dunk in the slot decides: the
    // default set's only advanced dunk is slot 0 (dunk 48, four turbos).
    w.players[0].native_dunks = Some(1);
    w.players[0].action_buttons = L1 | R1 | L2 | R2;
    assert_eq!(w.dunk_call(0, 0, false), 55);
    w.players[0].action_buttons = L1 | R1 | L2;
    assert_eq!(w.dunk_call(0, 0, true), 98);
    // An advanced dunk equipped in a three-turbo slot (slot 1) takes 3.
    w.players[0].native_dunks = Some(1 << 1);
    w.players[0].action_buttons = L1 | R1 | L2;
    assert_eq!(w.dunk_call(0, 0, false), 84);
}

/// The original's court with the ball loose and falling at `at`, nobody
/// near it.
fn loose_on_native_court(at: [f32; 3], velocity: [f32; 3]) -> World {
    let mut w = World::new(
        Hoop::native(),
        vec![
            Player::new(0, [-20.0, -20.0], 0.0),
            Player::new(1, [-20.0, 20.0], 0.0),
        ],
        Rules::default(),
        1,
    );
    w.phase = Phase::Live;
    w.ball = Ball::loose_at(at);
    w.ball.velocity = velocity;
    w.ball.airborne = true;
    w
}

/// Step the world's ball until `until` holds; the velocity it had before
/// the update that did it.
fn step_ball_until(w: &mut World, ticks: u32, until: impl Fn(&Ball) -> bool) -> [f32; 3] {
    for _ in 0..ticks {
        let before = w.ball.velocity;
        w.step_ball(None);
        if until(&w.ball) {
            return before;
        }
    }
    panic!("never happened: {:?}", w.ball);
}

#[test]
fn a_loose_ball_meets_the_collar_like_any_other() {
    // `FUN_0014a968` runs `FUN_0014d1f8` for every ball nobody holds, so a
    // loose ball dropped on the collar (`FUN_00152418`: the bracket's top,
    // 12 up, 36.75..38 out, within 0.25 across, outside the ring) lands on
    // it and bounces with the rim's restitution 0.7.
    let r = ball::RADIUS;
    let mut w = loose_on_native_court([37.52, 12.0 + r + 0.3, 0.24], [0.0; 3]);
    let before = step_ball_until(&mut w, 60, |b| b.velocity[1] > 0.0);
    let into = before[1] - GRAVITY * TICK_SECONDS;
    assert!(into < 0.0);
    assert_eq!(w.ball.position[1], 12.0 + r);
    assert!(
        (w.ball.velocity[1] + 0.7 * into).abs() < 1e-3,
        "{:?}",
        w.ball
    );
    // Straight down: nothing across, so no friction and no spin.
    assert_eq!(w.ball.spin, 0.0);
    assert_eq!(w.ball.state, BallState::Loose);
}

#[test]
fn the_backboard_is_a_box_with_faces_and_edges() {
    // `FUN_00151928`: 38.0..38.25 out, 11.5..15 up, ±3 across. A loose
    // ball rising under the board meets its bottom face and goes back down
    // at the glass's 0.75, which a plane at the face could not do.
    let r = ball::RADIUS;
    let mut w = loose_on_native_court([38.1, 10.6, 0.0], [0.0, 8.0, 0.0]);
    let before = step_ball_until(&mut w, 30, |b| b.velocity[1] < 0.0);
    let into = before[1] - GRAVITY * TICK_SECONDS;
    assert!(w.ball.position[1] <= 11.5 - r + 1e-4, "{:?}", w.ball);
    assert!(
        (w.ball.velocity[1] + 0.75 * into).abs() < 1e-3,
        "{:?}",
        w.ball
    );
    // Past its side edge, the ball comes off the corner of the box: back
    // out across and toward the court.
    let mut w = loose_on_native_court([37.9, 13.0, 3.6], [0.0, 0.0, -10.0]);
    step_ball_until(&mut w, 30, |b| b.velocity[2] > 0.0);
    assert!(w.ball.velocity[0] < 0.0, "{:?}", w.ball);
}

#[test]
fn shots_and_passes_leave_the_hand_spinning() {
    // `FUN_0014aaf8` (`0x0014bdac..0x0014be78`): a shot's backspin is
    // 3 × 2π/60 per update about velocity × up; `FUN_0014c0c0` spins a
    // pass 2 × 2π/60 about the same axis.
    let mut w = face_off(3);
    w.players[0].position = [-26.0, 0.0];
    w.players[1].position = [0.0, 25.0];
    w.ball.hold(0);
    w.release_shot(0, 0);
    assert_eq!(w.ball.state, BallState::Shot);
    assert_eq!(w.ball.spin, ball::SHOT_SPIN);
    assert!((ball::SHOT_SPIN - 3.0 * std::f32::consts::TAU / 60.0).abs() < 1e-6);
    let v = w.ball.velocity;
    let across = (v[0] * v[0] + v[2] * v[2]).sqrt();
    let axis = [-v[2] / across, 0.0, v[0] / across];
    for (got, want) in w.ball.spin_axis.into_iter().zip(axis) {
        assert!((got - want).abs() < 1e-5, "{:?}", w.ball.spin_axis);
    }
    let mut w = face_off(3);
    w.players[1].team = 0;
    w.players[1].position = [-12.0, 0.0];
    w.throw(0, 1, PassStyle::Normal);
    assert_eq!(w.ball.state, BallState::Pass);
    assert_eq!(w.ball.spin, ball::PASS_SPIN);
}

#[test]
fn backspin_takes_speed_off_a_bounce() {
    // `FUN_00150d88` works on the speed of the ball's surface where it
    // touches (`FUN_001507c8`): with backspin the bottom of a ball moving
    // forward slides faster over the floor, so friction takes more of its
    // speed. Both come off rolling: spin `|v_t|/r` about `n × v_t`, -z for
    // a ball going +x.
    let bounce = |spin: f32| {
        let mut w = loose_on_native_court([0.0, ball::RADIUS + 0.05, 0.0], [12.0, -10.0, 0.0]);
        w.ball.spin_axis = [0.0, 0.0, 1.0];
        w.ball.spin = spin;
        step_ball_until(&mut w, 10, |b| b.velocity[1] > 0.0);
        w.ball
    };
    let plain = bounce(0.0);
    let spun = bounce(ball::SHOT_SPIN);
    assert!(spun.velocity[0] < plain.velocity[0], "{spun:?} {plain:?}");
    assert!(plain.velocity[0] < 12.0);
    for b in [plain, spun] {
        let rolling = b.velocity[0] * TICK_SECONDS / b.radius;
        assert!((b.spin - rolling).abs() < 1e-4, "{b:?}");
        assert!((b.spin_axis[2] + 1.0).abs() < 1e-5, "{b:?}");
    }
}

#[test]
fn a_loose_ball_rolls_on_as_a_hollow_ball_does() {
    // On the floor the ball rolls (`FUN_001511f8`): friction 0.9 of its
    // weight slows its bottom and spins it up until it rolls without
    // slipping, at 3/5 of its speed for a hollow ball (spin from friction
    // is 1.5/r per unit, `0x00151478`). Nothing slows a rolling ball after
    // that; the sim's old per-update drag of 0.985 is gone.
    let mut w = loose_on_native_court([-30.0, ball::RADIUS, 0.0], [10.0, 0.0, 0.0]);
    w.ball.airborne = false;
    for _ in 0..90 {
        w.step_ball(None);
    }
    assert!(!w.ball.airborne);
    assert_eq!(w.ball.position[1], ball::RADIUS);
    let speed = w.ball.velocity[0];
    assert!((speed - 6.0).abs() < 0.01, "{:?}", w.ball);
    let rolling = speed * TICK_SECONDS / ball::RADIUS;
    assert!((w.ball.spin - rolling).abs() < 1e-4, "{:?}", w.ball);
    for _ in 0..60 {
        w.step_ball(None);
    }
    assert!((w.ball.velocity[0] - speed).abs() < 1e-3, "{:?}", w.ball);
}

#[test]
fn a_pass_that_meets_the_rim_is_knocked_loose() {
    // A pass is a free ball too: one that meets the rim or the collar
    // leaves the pass (state 4 goes to 1 at `0x0014d74c..0x0014d784`) and
    // is loose; off the glass or the floor it flies on.
    let mut w = World::new(
        Hoop::native(),
        vec![
            Player::new(0, [30.0, 0.0], 0.0),
            Player::new(0, [-20.0, 20.0], 0.0),
        ],
        Rules::default(),
        1,
    );
    w.phase = Phase::Live;
    let pass = ball::Pass {
        from: 0,
        to: 1,
        tried: u8::MAX,
        via: ball::Via::Direct,
        catch: ball::NO_CATCH,
        since: 0,
    };
    w.ball
        .throw([33.0, 11.95, 0.0], [37.0, 11.95, 0.0], 0.25, pass);
    let mut loose = false;
    for _ in 0..14 {
        let vx = w.ball.velocity[0];
        w.step_ball(None);
        if w.ball.state == BallState::Loose {
            loose = true;
            assert!(w.ball.velocity[0] < vx, "{:?}", w.ball);
            assert!(w.ball.flight_left > 0.0, "{:?}", w.ball);
            assert!(w.ball.position[0] < 36.75 - ball::RIM_ENTRY, "{:?}", w.ball);
            break;
        }
    }
    assert!(loose, "{:?}", w.ball);
}

#[test]
fn a_ball_coming_up_through_the_ring_is_stopped_under_it() {
    // `FUN_001515a8`: while the ball has not scored since it was held, it
    // cannot come up through a ring: it stops just under the rim plane.
    let mut w = loose_on_native_court([36.75, 11.4, 0.1], [0.0, 9.0, 0.0]);
    step_ball_until(&mut w, 10, |b| b.velocity[1] <= 0.0);
    let ball = &w.ball;
    assert_eq!(ball.position[1], f32::from_bits(0x413d_ecda), "{ball:?}");
    assert!(!ball.through);
    // Once it has gone through, it may come back up.
    let mut w = loose_on_native_court([36.75, 11.4, 0.1], [0.0, 9.0, 0.0]);
    w.ball.armed = false;
    for _ in 0..10 {
        w.step_ball(None);
    }
    assert!(w.ball.position[1] > NATIVE_RIM_HEIGHT, "{:?}", w.ball);
}

#[test]
fn a_slow_ball_on_the_rim_settles_and_rolls_off() {
    // `FUN_001526d0`: a ball coming off the rim slower than five gravity
    // steps settles onto it (`FUN_00151550`) and rolls on the tube
    // (`FUN_001511f8`) until it falls off; here just outside the ring, so
    // it rolls off the front and drops, never through.
    let r = ball::RADIUS;
    let top = NATIVE_AIM_HEIGHT + ball::RIM_TUBE + r;
    let mut w = loose_on_native_court([36.75 - ball::RIM_ENTRY - 0.03, top + 0.004, 0.0], [0.0; 3]);
    let mut rolled = false;
    for _ in 0..120 {
        w.step_ball(None);
        rolled |= !w.ball.airborne && w.ball.position[1] > NATIVE_RIM_HEIGHT;
        assert!(!w.ball.through, "{:?}", w.ball);
    }
    assert!(rolled);
    assert!(w.ball.airborne, "{:?}", w.ball);
    assert!(w.ball.position[1] < NATIVE_RIM_HEIGHT);
    assert!(w.ball.position[0] < 36.75 - ball::RIM_ENTRY);
}

#[test]
fn the_net_slows_a_made_shot_the_update_after_it_goes_through() {
    // `FUN_00153550` runs only for a ball that went through (`ball+0x18c`),
    // and not in the update it did: the next one pulls it to the net's cone
    // and keeps a tenth of its speed.
    let mut w = face_off(2);
    w.players[0].position = [-28.0, 0.0];
    w.players[1].position = [0.0, 25.0];
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
    w.launch_shot(0, [-28.0, 9.0, 0.0], call, shot, None);
    step_ball_until(&mut w, 300, |b| b.state == BallState::Loose);
    assert_eq!(w.score[0], 1);
    assert!(w.ball.through);
    let falling = w.ball.velocity[1] - GRAVITY * TICK_SECONDS;
    w.step_ball(None);
    assert!(!w.ball.through);
    assert!(
        (w.ball.velocity[1] - 0.1 * falling).abs() < 1e-3,
        "{:?}",
        w.ball
    );
}

#[test]
fn the_balls_spin_and_flags_survive_a_snapshot() {
    let mut w = face_off(4);
    w.ball = Ball::loose_at([36.0, 12.5, 0.4]);
    w.ball.velocity = [1.0, -2.0, 0.5];
    w.ball.spin_axis = [0.0, 0.6, 0.8];
    w.ball.spin = 0.25;
    w.ball.airborne = true;
    w.ball.through = true;
    w.ball.bank = true;
    w.ball.armed = false;
    let back = World::from_snapshot(&w.to_snapshot()).unwrap();
    assert_eq!(back.ball, w.ball);
    let mut a = w.clone();
    let mut b = back;
    for _ in 0..30 {
        a.step_ball(None);
        b.step_ball(None);
    }
    assert_eq!(a.ball, b.ball);
}

#[test]
fn a_court_with_a_ceiling_sends_a_high_ball_back_down() {
    // `FUN_00153428`: court 8's ceiling at 33.77 turns a rising loose ball
    // back down (restitution 0.5); courts without one let it fly on.
    let fly = |ceiling: f32| {
        let mut w = solo([0.0, 20.0]);
        w.players[0].position = [30.0, 20.0];
        w.ball = Ball::loose_at([0.0, 30.0, 0.0]);
        w.ball.airborne = true;
        w.ball.velocity = [0.0, 30.0, 0.0];
        w.ball.ceiling = ceiling;
        let mut top: f32 = 0.0;
        for _ in 0..60 {
            w.step(&[Input::default()]);
            top = top.max(w.ball.position[1]);
        }
        top
    };
    assert_eq!(crate::ball::ceiling_for_court(8), 33.77);
    assert_eq!(crate::ball::ceiling_for_court(6), 25.58);
    assert_eq!(crate::ball::ceiling_for_court(0), -1.0);
    assert!(fly(33.77) <= 33.77 - crate::ball::RADIUS + 1e-4);
    assert!(fly(-1.0) > 35.0);
}

#[test]
fn a_dunk_goes_down_through_the_ring_the_update_after_it_is_let_go() {
    // The launcher's dunk path (`FUN_0014aaf8`): the dunk is put in the
    // ring at r + 12 and goes down at the hand's pace, so the game is in
    // phase 3 for the launch's update and the basket (and the restart's
    // phase 6) comes the next, as in all 36 made dunks of the attract
    // captures; the net keeps half a dunked ball's speed.
    let mut made = 0;
    let mut missed = 0;
    for seed in 0..40 {
        let mut w = driver([30.0, 0.0], 10.0, seed);
        // Dunks 10: the release callback's 55%. A two-turbo combo's slot
        // dunk (`dunk_19`).
        w.players[0].ratings.dunks = 0.1;
        step_all(
            &mut w,
            &[(
                0,
                stick(1.0, 0.0).with(button::SHOOT | button::R1 | button::L2),
            )],
            1,
        );
        let mut slammed = false;
        for _ in 0..2 * TICK_HZ {
            step_all(&mut w, &[], 1);
            if w.events.contains(&Event::Slam { player: 0, hoop: 0 }) {
                slammed = true;
                break;
            }
        }
        assert!(slammed, "seed {seed}");
        if w.ball.state != BallState::Shot {
            continue; // blocked: the rewrite's own block at the rim
        }
        let shot = w.ball.shot.unwrap();
        assert_eq!(
            shot.slam,
            Some(Basket::Dunk { advanced: false }),
            "seed {seed}"
        );
        assert_eq!(w.game_phase.phase, crate::game_phase::phase::SHOT);
        assert!(!w.events.iter().any(|e| matches!(e, Event::Score { .. })));
        assert!(w.ball.dunked);
        // Put at r + 12 within 0.75 - r of the middle (the launch update's
        // move undone: the step, half the gravity off the height and all
        // of it off the rise), at the pace (0.4889 an update).
        let rim = w.hoops[0].rim;
        let dt = TICK_SECONDS;
        let v = w.ball.velocity;
        let vy = v[1] + GRAVITY * dt;
        let b = w.ball.position;
        let at = [
            b[0] - v[0] * dt,
            b[1] - vy * dt + 0.5 * GRAVITY * dt * dt,
            b[2] - v[2] * dt,
        ];
        let across = ((at[0] - rim[0]).powi(2) + (at[2] - rim[2]).powi(2)).sqrt();
        assert!(across <= 0.75 - ball::RADIUS + 1e-4, "{across}");
        assert!((at[1] - (12.0 + ball::RADIUS)).abs() < 1e-3, "{at:?}");
        if shot.made {
            let speed = (v[0] * v[0] + vy * vy + v[2] * v[2]).sqrt() * dt;
            assert!(speed > 0.45 && speed < 0.5, "{speed}");
        }
        step_all(&mut w, &[], 1);
        if shot.made {
            made += 1;
            assert!(
                w.events.iter().any(|e| matches!(
                    e,
                    Event::Score {
                        team: 0,
                        basket: Basket::Dunk { advanced: false },
                        ..
                    }
                )),
                "seed {seed}"
            );
            assert_eq!(w.game_phase.phase, crate::game_phase::phase::RESTART);
            // The net: speed into the cone is cut to half, not a tenth.
            let mut b = w.ball.clone();
            b.position = [w.hoops[0].rim[0] + 0.6, 11.0, 0.0];
            b.velocity = [10.0, -10.0, 0.0];
            assert!(b.net(w.hoops[0].rim));
            assert_eq!(b.velocity, [5.0, -5.0, 0.0]);
        } else {
            missed += 1;
            assert!(!w.events.iter().any(|e| matches!(e, Event::Score { .. })));
        }
    }
    assert!(made > 0 && missed > 0, "{made} {missed}");
}

#[test]
fn the_opening_toss_waits_at_the_tip_off_spots_then_goes_up_like_fun_0014c858() {
    let mut w = three_on_three(5);
    w.start_opening_toss();
    // FUN_0018af20 / 0x3cfbb0: slot k of each team, x times the side
    // (the way it attacks: team 0 +x), the jumper and the man behind him
    // in the half the team defends.
    let spots: Vec<[f32; 2]> = w.players.iter().map(|p| p.position).collect();
    assert_eq!(
        spots,
        [
            [-3.25, 0.0],
            [3.25, 7.0],
            [-7.0, 0.0],
            [3.25, 0.0],
            [-3.25, 7.0],
            [7.0, 0.0]
        ]
    );
    // Facing the centre (FUN_00170608 toward the origin).
    for p in &w.players {
        let [fx, fz] = p.forward();
        let to_centre = normalize2([-p.position[0], -p.position[1]]);
        assert!(fx * to_centre[0] + fz * to_centre[1] > 0.999, "{p:?}");
    }
    assert_eq!(w.opening_toss, OpeningToss::Waiting(toss::WAIT_UPDATES));
    assert_eq!(w.phase, Phase::Live);
    // Nothing moves and no button counts while the referee holds it.
    let before = w.players.clone();
    let push = Input::moving([1.0, 0.0]).with(button::SHOOT | button::JUMP);
    for _ in 1..toss::WAIT_UPDATES {
        w.step(&[push; 6]);
        assert!(w.events.is_empty());
    }
    assert_eq!(w.players, before);
    assert_eq!(w.ball.position, toss::RELEASE);
    assert_eq!(w.opening_toss, OpeningToss::Waiting(1));
    // The last update of the wait launches it: up to the apex and back
    // down on the centre spot. The retained attract capture's ball one
    // update after its launch (sample 584), per update:
    w.step(&[Input::default(); 6]);
    assert_eq!(w.tick, u64::from(toss::WAIT_UPDATES));
    assert_eq!(w.opening_toss, OpeningToss::Flight);
    assert_eq!(w.ball.state, BallState::Loose);
    let observed_velocity = [0.002_974_642_7, 0.542_546_15, 0.007_225_262_5];
    let observed_position = [-0.318_738_6, 6.917_49, -0.774_200_6];
    for k in 0..3 {
        let v = w.ball.velocity[k] * TICK_SECONDS;
        assert!((v - observed_velocity[k]).abs() < 2e-6, "{k}: {v}");
        assert!(
            (w.ball.position[k] - observed_position[k]).abs() < 2e-5,
            "{k}"
        );
    }
    let mut quiet = w.clone();
    let mut top: f32 = 0.0;
    let mut landed = None;
    let mut falling = false;
    for _ in 0..300 {
        quiet
            .ball
            .step(quiet.hoops.map(|h| h.rim), ball::Step::default());
        top = top.max(quiet.ball.position[1]);
        falling |= quiet.ball.velocity[1] < 0.0;
        // The first bounce off the floor.
        if falling && quiet.ball.velocity[1] > 0.0 {
            landed = Some(quiet.ball.position);
            break;
        }
    }
    assert!((top - toss::APEX).abs() < 0.02, "apex {top}");
    // On the centre spot, within the one update of floor travel the bounce
    // test sees.
    let landed = landed.expect("the toss comes down");
    assert!(
        landed[0].abs() < 0.01 && landed[2].abs() < 0.01,
        "{landed:?}"
    );
}

#[test]
fn the_opening_toss_spins_about_a_random_axis_like_fun_0014c858() {
    let mut w = three_on_three(5);
    w.start_opening_toss();
    for _ in 1..toss::WAIT_UPDATES {
        w.step(&[Input::default(); 6]);
    }
    // The three draws the launch takes from the shared generator
    // (raw 0x14c8bc..0x14c9ec), each (d & 0x7fffffff) * 2^-31 - 0.5.
    let mut rng = w.rng.clone();
    let axis: [f32; 3] = std::array::from_fn(|_| {
        (rng.next_u32() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000) - 0.5
    });
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    w.step(&[Input::default(); 6]);
    assert_eq!(w.opening_toss, OpeningToss::Flight);
    // DAT_004b854c: (1/60) * 2pi * 0.5, half a turn a second.
    assert_eq!(w.ball.spin, toss::SPIN);
    assert!((toss::SPIN - std::f32::consts::PI / 60.0).abs() < 1e-7);
    assert_eq!(w.ball.spin_axis, axis.map(|c| c / length));
    let unit: f32 = w.ball.spin_axis.iter().map(|c| c * c).sum();
    assert!((unit - 1.0).abs() < 1e-5);
}

#[test]
fn standing_players_take_their_stands_as_the_stand_path_picks_them() {
    // `FUN_00163d40`'s stand path: the offence off the ball draws
    // `r % 15` every update and starts that stand while his row is none
    // of them; the defence draws `r & 1` (breathing or anticipation); the
    // holder's first stand while dribbling is one of the dribbler's; each
    // then loops without starting again.
    use crate::stand;
    let mut w = three_on_three(11);
    w.phase = Phase::Live;
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    w.possession = 0;
    w.ball.hold(0);
    w.players[0].dribble = 1;
    for p in &mut w.players {
        p.gait_channel.record = None;
        p.gait_channel.row = 7;
    }
    w.settle_looks();
    let order = w.actor_order();
    let mut rng = w.rng.clone();
    let mut expected = [None; 6];
    for &i in &order {
        match i {
            0 => {
                let k = if w.players[0].ratings.native(7) >= 30 {
                    rng.next_u32() % 11
                } else {
                    rng.next_u32() & 1
                };
                expected[0] = Some(stand::DRIBBLE_FIRST + k as u8);
            }
            1 | 2 => expected[i] = Some(stand::OFFENCE_STANDS[(rng.next_u32() % 15) as usize]),
            _ => {
                let pick = 3 * (rng.next_u32() & 1) as usize;
                expected[i] = Some(stand::DEFENCE_STANDS[pick]);
            }
        }
    }
    w.step(&[Input::default(); 6]);
    for (i, want) in expected.iter().enumerate() {
        assert_eq!(w.players[i].stand.map(|s| s.script), *want, "{i}");
    }
    let before: Vec<_> = w
        .players
        .iter()
        .map(|p| p.stand.map(|s| s.script))
        .collect();
    for _ in 0..30 {
        w.step(&[Input::default(); 6]);
    }
    let after: Vec<_> = w
        .players
        .iter()
        .map(|p| p.stand.map(|s| s.script))
        .collect();
    assert_eq!(before, after, "the stands loop");
    // A stick pushed ends his stand.
    let mut inputs = [Input::default(); 6];
    inputs[1] = stick(1.0, 0.0);
    w.step(&inputs);
    assert!(w.players[1].stand.is_none());
}

#[test]
fn a_heezay_to_a_teammate_leaves_at_its_rows_callback_38() {
    // The press holds the ball on a passout row (`0x146a44..`); callback
    // 38 throws it at the defender's head with the head's ratio
    // (`0x173930..0x173c64`), the receiver's leg queued.
    use crate::ball::Via;
    let mut w = three_on_three(4);
    w.phase = Phase::Live;
    w.ball.hold(0);
    assert!(w.computer_trick_pass(0, 1, Some(3), [0.0; 3]));
    assert!(w.windup.is_some_and(|wu| wu.to == 1));
    assert_eq!(w.ball.state, BallState::Held(0));
    let mut ticks = 0;
    while w.ball.state == BallState::Held(0) && ticks < TICK_HZ {
        w.step(&[Input::default(); 6]);
        ticks += 1;
    }
    assert!(ticks > 1, "thrown at the press");
    let pass = w.ball.pass.expect("a pass");
    assert_eq!((pass.from, pass.to), (0, 1));
    assert_eq!(pass.via, Via::Head { defender: 3 });
    let po = w.passout.expect("the head's legs");
    assert!(po.tossed && po.back.is_some());
}

#[test]
fn a_head_passout_meets_the_head_within_the_passers_reach() {
    // `FUN_00148378` kind 2: within `Handles × 4 / 99` of the head bone the
    // defender reacts (`r & 1`) and the leg back flies to its target; out
    // of it the legs end and the ball flies on free.
    use crate::ball::Via;
    let leg = |w: &mut World, off: f32| {
        w.ball.hold(0);
        w.players[0].ratings.handles = 0.5;
        let head = w.head_point(3);
        w.passout = Some(crate::passout::Passout {
            passer: 0,
            kind: crate::passout::Kind::Head,
            defender: Some(3),
            point: head,
            back: Some(([0.0, 4.0, 0.0], 40)),
            tossed: true,
            tail: None,
        });
        let pass = crate::Pass {
            from: 0,
            to: 0,
            tried: u8::MAX,
            via: Via::Head { defender: 3 },
            catch: crate::ball::NO_CATCH,
            since: w.tick,
        };
        w.ball.throw(head, head, 0.1, pass);
        w.ball.position = [head[0] + off, head[1], head[2]];
        w.head_leg(pass, 3);
    };
    let mut w = three_on_three(9);
    w.phase = Phase::Live;
    leg(&mut w, 0.5);
    assert_eq!(w.ball.state, BallState::Pass);
    assert!(matches!(w.ball.pass, Some(p) if p.via == Via::Direct));
    assert!(matches!(
        w.players[3].action,
        Action::Stumbling {
            reaction: Some(_),
            ..
        }
    ));
    let mut w = three_on_three(9);
    w.phase = Phase::Live;
    leg(&mut w, 3.0);
    assert_eq!(w.ball.state, BallState::Loose);
}

#[test]
fn a_boxer_takes_the_man_behind_him_instead_of_the_push() {
    // `FUN_001343e8`'s box-out (`0x134f4c..0x135198`): the boxer (bit 17,
    // nobody held) meets a standing opponent behind him: `BOX_BEHIND_4`
    // for him, a boxed front clip by `r & 1` for the other (bit 18), the
    // partners kept and nobody pushed; then the faces and the message.
    use crate::stand;
    let mut w = three_on_three(4);
    w.phase = Phase::Live;
    let (a, b) = (3, 0);
    w.players[a].position = [30.0, 0.0];
    w.players[a].heading = 0.0;
    w.players[a].boxing = true;
    w.players[b].position = [30.0, -1.2];
    let before = (w.players[a].position, w.players[b].position);
    let mut rng = w.rng.clone();
    assert!(w.box_out_contact(a, b));
    let r = rng.next_u32() & 1;
    assert_eq!(
        w.players[a].stand.map(|s| s.script),
        Some(stand::BOXER_BEHIND)
    );
    assert_eq!(
        w.players[b].stand.map(|s| s.script),
        Some(stand::BOXED_BEHIND[usize::from(r == 0)])
    );
    assert_eq!(
        (w.players[a].partner, w.players[b].boxer),
        (Some(b as u8), Some(a as u8))
    );
    assert_eq!((w.players[a].position, w.players[b].position), before);
    assert!(
        w.ai.brains
            .get(b)
            .is_none_or(|brain| brain.screened_by == a as i32)
    );
    // Holding him, the boxer takes nobody else.
    assert!(!w.box_out_contact(a, 1));
    // The boxed man's first update sets bit 18.
    w.stand_tick(b);
    assert!(w.players[b].boxed);
}

#[test]
fn the_idle_face_draws_when_its_time_runs_out() {
    // `FUN_00159240`: the face's time (fps from the actors' reset) counts
    // down; at 0 the idle face for `fps + r % fps` updates, its clip drawn.
    let mut w = three_on_three(6);
    w.idle_face(0);
    assert_eq!(w.players[0].face_wait, 59);
    w.players[0].face_wait = 1;
    let mut rng = w.rng.clone();
    w.idle_face(0);
    let r = rng.next_u32();
    rng.next_u32();
    assert_eq!(w.players[0].face_wait, (60 + r % 60) as i32);
    assert_eq!(w.rng, rng);
    // A face of its own (`FUN_00159190`) sets the time; none under 1.
    w.face(0, 120);
    assert_eq!(w.players[0].face_wait, 120);
    let before = w.rng.clone();
    w.face(0, 0);
    assert_eq!((w.players[0].face_wait, w.rng.clone()), (120, before));
}

#[test]
fn a_holder_who_picked_up_his_dribble_only_turns() {
    // `0x167020..0x167050`: actor+0x1a0 −1 leaves the holder on the stand
    // path whatever his stick asks; it turns him (`0x1687dc`).
    let mut w = driver([20.0, 0.0], 0.0, 4);
    w.players[0].dribble = -1;
    let at = w.players[0].position;
    for _ in 0..20 {
        w.step(&[stick(0.0, 1.0), Input::default()]);
    }
    assert_eq!(w.players[0].position, at);
    assert!(w.players[0].stand.is_some());
    assert_eq!(
        crate::timing::facing_word(w.players[0].heading),
        crate::timing::facing_word(crate::math::atan2(0.0, 1.0))
    );
}

#[test]
fn a_man_near_the_rim_boxes_out_while_a_shot_is_up() {
    // `0x168218..0x1682c0`: nobody holding it, the ball in a shot, within
    // 14 of the rim and the ball nearer it: the box-out (bit 17), until
    // the ball is farther from the rim than he is.
    use crate::stand;
    let mut w = three_on_three(2);
    w.phase = Phase::Live;
    w.game_phase.phase = crate::game_phase::phase::SHOT;
    w.possession = 0;
    let rim = w.rim_spot(0);
    w.players[4].position = [rim[0] - 6.0, 2.0];
    w.ball = Ball::loose_at([rim[0] - 1.0, 12.0, 0.0]);
    w.ball.state = BallState::Shot;
    w.stand_path(4, &Input::default());
    assert_eq!(w.players[4].stand.map(|s| s.script), Some(stand::BOX_OUT));
    w.stand_tick(4);
    assert!(w.players[4].boxing);
    w.ball.position = [rim[0] - 20.0, 3.0, 0.0];
    w.stand_path(4, &Input::default());
    assert!(!w.players[4].boxing);
    assert_eq!(w.players[4].stand.map(|s| s.script), Some(stand::READY));
}

#[test]
fn a_man_told_to_celebrate_does_so_in_the_restart_on_the_floor() {
    // `FUN_0015da28`'s first test and `FUN_00170e70`: the word waits for
    // the restart and the floor, then one draw of the shared generator
    // picks the row, and his wanted facing turns him to the ball.
    let mut w = three_on_three(3);
    w.ball = Ball::loose_at([36.0, 2.0, 0.0]);
    w.players[1].celebrate = true;
    w.players[1].height = 1.0;
    w.start_restart(1);
    let mut rng = w.rng.clone();
    w.celebrate_due(1);
    assert!(w.players[1].celebrate, "in the air");
    w.players[1].height = 0.0;
    w.celebrate_due(1);
    assert!(!w.players[1].celebrate);
    let k = (rng.next_u32() % 17) as u8;
    match w.players[1].action {
        Action::Stumbling {
            reaction: Some(r), ..
        } => assert_eq!(r.script, trick::CELEBRATE_FIRST + k),
        a => panic!("{a:?}"),
    }
    let ball = crate::timing::angle_units(crate::math::atan2(
        w.players[1].position[0] - 36.0,
        w.players[1].position[1],
    ));
    assert_eq!(crate::timing::facing_word(w.players[1].wanted), ball);
    // Already on its script: no second one.
    w.players[1].celebrate = true;
    w.celebrate_due(1);
    assert!(w.players[1].celebrate);
    // Out of the restart the word waits.
    let mut w = three_on_three(3);
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    w.players[0].celebrate = true;
    w.celebrate_due(0);
    assert!(w.players[0].celebrate);
}

#[test]
fn the_win_celebrates_the_scorer_and_a_loser_reacts() {
    // `0x13dbb8..0x13dee8`: the scorer's celebration facing the centre,
    // then the first loser on the floor (`FUN_00171160`, far from the
    // centre and facing it: the rows barred to facing away).
    let mut w = three_on_three(5);
    w.scorer = Some(2);
    w.players[3].height = 2.0;
    w.players[4].position = [30.0, 0.0];
    w.players[4].heading =
        crate::restart::heading_radians(crate::timing::angle_units(crate::math::atan2(30.0, 0.0)));
    let mut rng = w.rng.clone();
    w.won_celebration(0);
    let k = (rng.next_u32() % 17) as u8;
    let script = |w: &World, i: usize| match w.players[i].action {
        Action::Stumbling {
            reaction: Some(r), ..
        } => Some(r.script),
        _ => None,
    };
    assert_eq!(script(&w, 2), Some(trick::CELEBRATE_FIRST + k));
    let [x, z] = w.players[2].position;
    assert_eq!(
        crate::timing::facing_word(w.players[2].wanted),
        crate::timing::angle_units(crate::math::atan2(x, z))
    );
    assert_eq!(script(&w, 3), None, "in the air");
    let r = rng.next_u32() % 2;
    assert_eq!(script(&w, 4), Some(trick::DEJECT_FIRST + r as u8));
    assert_eq!(script(&w, 5), None);
    // A trick-point game's dunker waits for the countdown.
    let mut w = three_on_three(5);
    w.rules.goal = crate::rules::Goal::TrickPoints(1000);
    w.scorer = Some(2);
    w.players[2].action = Action::Dunking {
        hoop: 0,
        elapsed: 0.0,
        from: [0.0; 2],
        to: [0.0; 2],
        style: crate::player::DunkStyle::Normal,
        layup: false,
        tick: 0,
        release: 0,
        end: 0,
        row: 0,
        hang: None,
    };
    w.won_celebration(0);
    assert!(w.celebrate_pending);
    w.won_countdown();
    assert!(w.celebrate_pending);
    w.players[2].action = Action::None;
    w.won_countdown();
    assert!(!w.celebrate_pending);
    assert_eq!(
        script(&w, 2).map(|s| s >= trick::CELEBRATE_FIRST),
        Some(true)
    );
}

#[test]
fn a_restart_snap_stands_both_teams_at_the_strategies_spots_like_fun_001407c8() {
    let mut w = three_on_three(3);
    // Team 0 has just scored at hoop 0 (+x): team 1 restarts with the ball
    // loose under that basket, the one it defends.
    w.ball = Ball::loose_at([36.0, 2.0, 0.0]);
    w.players[3].position = [30.0, 2.0];
    w.players[4].position = [20.0, -10.0];
    w.players[5].position = [0.0, 20.0];
    w.start_restart(1);
    w.snap_formation(1);
    // FUN_0012bc40: the inbounder is team 1's player nearest the ball
    // (slot 0); the row is the one whose first spot is nearest the ball in
    // team 1's frame (team 1 attacks -x: x times minus its side), row 0
    // (47, 16) behind its own basket; the second spot goes to the nearer
    // of the others to that spot (slot 1), the third to slot 2.
    assert_eq!(w.restart_plan().row, 0);
    assert_eq!(w.restart_plan().order, [0, 1, 2]);
    let push = |i: usize| (w.players[i].height_ft().clamp(4.0, 8.0) - 4.0) * 0.25;
    assert_eq!(w.players[3].position, [47.0 + push(3), 16.0]);
    assert_eq!(w.players[4].position, [35.0, 8.0]);
    assert_eq!(w.players[5].position, [32.0, -10.0]);
    // The defenders (team 0, side +1) at row 0 of 0x3ceaf8 in the same
    // order: two up toward the inbound, one back toward their basket.
    assert_eq!(w.players[0].position, [12.0, 10.0]);
    assert_eq!(w.players[1].position, [22.0, 12.0]);
    assert_eq!(w.players[2].position, [-13.0, -10.0]);
    // Facing: the inbounder the second spot, the others the first, each
    // defender the middle of the first spot and his own.
    let heading = |at: [f32; 2], to: [f32; 2]| {
        restart::heading_radians(ordinary::native_heading(
            [at[0], 0.0, at[1]],
            [to[0], 0.0, to[1]],
        ))
    };
    let first = w.players[3].position;
    assert_eq!(w.players[3].heading, heading(first, [35.0, 8.0]));
    // In the simulation's convention he faces that spot.
    let [fx, fz] = w.players[3].forward();
    let to = normalize2(sub([35.0, 8.0], first));
    assert!(fx * to[0] + fz * to[1] > 0.99);
    assert_eq!(w.players[4].heading, heading([35.0, 8.0], first));
    let middle = |defender: usize, spot: [f32; 2]| {
        let a = [47.0 + push(defender), 16.0];
        [(a[0] + spot[0]) * 0.5, (a[1] + spot[1]) * 0.5]
    };
    assert_eq!(
        w.players[2].heading,
        heading([-13.0, -10.0], middle(2, [32.0, -10.0]))
    );
    // The inbounder has the ball; the restart goes on (phase 6) until all
    // six are ready and he has passed it in.
    assert_eq!(w.ball.state, BallState::Held(3));
    assert_eq!(w.possession, 1);
    assert!(matches!(w.phase, Phase::DeadBall { team: 1, .. }));
    assert!(!w.events.contains(&Event::Inbound { player: 3 }));
    let mut inbound = None;
    for _ in 0..4 * TICK_HZ {
        w.step(&[Input::default(); 6]);
        if let Some(Event::Inbound { player }) =
            w.events.iter().find(|e| matches!(e, Event::Inbound { .. }))
        {
            inbound = Some(*player);
            break;
        }
    }
    // Everyone already stands at his spot: the inbound pass follows at
    // once, to the second spot.
    assert_eq!(inbound, Some(3));
    assert_eq!(w.phase, Phase::Live);
    assert_eq!(w.ball.pass.map(|p| (p.from, p.to)), Some((3, 4)));
    // A carrier on the restarting team is the inbounder whatever the
    // distances (`FUN_0018c018`).
    let plan = restart::plan(&restart::Facts {
        team: 1,
        positions: [[0.0, 0.0], [-5.0, 5.0], [-9.0, 9.0]],
        ball: [0.0, 0.0],
        holder: Some(2),
        previous: restart::Plan::default(),
        fresh_clock: true,
        people: 0,
        human: [false; 3],
        heights: [6.5; 3],
        lesson: false,
    });
    // Row 2 (12, 31.5) is nearest the ball; slot 1 is nearer that spot.
    assert_eq!(plan.row, 2);
    assert_eq!(plan.order, [2, 1, 0]);
}

/// After a basket the restart is the original's: nobody is placed; the
/// computer walks all six to their spots (people's players too), the
/// inbounder fetches the ball and carries it behind the line, and the
/// inbound pass goes once all six have marked themselves ready.
#[test]
fn after_a_basket_the_six_walk_to_their_spots_and_the_inbound_waits_for_all_six() {
    for seed in [1, 4, 11] {
        let mut w = three_on_three(seed);
        w.phase = Phase::Live;
        // Team 0 scores at hoop 0; the ball drops through the net.
        w.ball = Ball::loose_at([-36.95, 6.0, 0.0]);
        w.turbo = [0.3; crate::turbo::SLOTS];
        w.sync_turbo();
        w.score_basket(0, Basket::Jumper { long: false });
        assert!(matches!(w.phase, Phase::DeadBall { team: 1, .. }));
        assert!(w.restarting());
        // FUN_001406b8 tops every turbo up.
        assert!(w.players.iter().all(|p| p.turbo == 1.0));
        // Team 1's players are a person's: the computer drives them now.
        let inputs: Vec<Input> = (0..6)
            .map(|i| {
                if i < 3 {
                    Input::computer()
                } else {
                    Input::default()
                }
            })
            .collect();
        let start: Vec<[f32; 2]> = w.players.iter().map(|p| p.position).collect();
        let mut picked = None;
        let mut inbound = None;
        for t in 0..tuning::RESTART_LIMIT as u32 * TICK_HZ {
            let ready_before = w.restart_ready();
            w.step(&inputs);
            for e in &w.events {
                match *e {
                    Event::Pickup { player } => picked = Some(player),
                    Event::Inbound { player } => inbound = Some((player, t)),
                    _ => {}
                }
            }
            if inbound.is_some() {
                assert!(ready_before || w.restart_ready());
                break;
            }
            assert!(matches!(w.phase, Phase::DeadBall { .. }), "seed {seed}");
        }
        let (passer, t) = inbound.unwrap_or_else(|| panic!("seed {seed}: no inbound"));
        // The inbounder picked the ball up and passed it in from behind
        // the baseline under the basket just scored on.
        assert_eq!(picked, Some(passer), "seed {seed}");
        assert_eq!(w.team_of(passer), 1);
        assert!(w.players[passer].position[0] < -45.0, "seed {seed}");
        assert_eq!(w.phase, Phase::Live);
        assert_eq!(w.ball.state, BallState::Pass);
        // Everyone walked: nobody stands where he was.
        for (i, p) in w.players.iter().enumerate() {
            assert!(distance(p.position, start[i]) > 1.0, "seed {seed}: {i}");
        }
        // Well within the rewrite's limit.
        assert!(t < 10 * TICK_HZ, "seed {seed}: {t} updates");
    }
}

#[test]
fn a_shot_clock_violation_restarts_with_the_clock_run_out() {
    let mut w = three_on_three(6);
    w.phase = Phase::Live;
    w.ball.hold(0);
    // The clock has seen team 0 with the ball (`DAT_003d30c4`).
    w.clock.side = 0;
    w.shot_clock = TICK_SECONDS * 0.5;
    w.step(&[Input::default(); 6]);
    assert!(w.events.contains(&Event::ShotClockViolation { team: 0 }));
    assert!(matches!(w.phase, Phase::DeadBall { team: 1, .. }));
    // Run out as the restart starts, so `FUN_0012bc40` skips rows 0 and 1
    // (the computer's start reads it at the next update's beginning); that
    // update's clock (`FUN_0013f1f0`) sees team 1 with the ball and fills
    // it (`FUN_0013f188`), and it stands through the restart.
    assert_eq!(w.shot_clock, 0.0);
    let inputs = vec![Input::computer(); 6];
    w.step(&inputs);
    let full = w.rules.shot_clock.unwrap();
    assert_eq!(w.shot_clock, full);
    for _ in 0..TICK_HZ * 12 {
        w.step(&inputs);
        if w.phase == Phase::Live {
            break;
        }
        assert_eq!(w.shot_clock, full, "the clock stands in phase 6");
    }
    assert_eq!(w.phase, Phase::Live);
    assert!(w.restart_plan().row >= 2);
    assert!(full - w.shot_clock <= 2.0 * TICK_SECONDS + 1e-4);
}

#[test]
fn the_jumpers_jump_and_the_winner_tips_the_toss_to_the_man_behind_him() {
    for seed in [9, 10, 21] {
        let mut w = three_on_three(seed);
        w.start_opening_toss();
        for _ in 0..toss::WAIT_UPDATES {
            ai_step(&mut w);
        }
        assert_eq!(w.opening_toss, OpeningToss::Flight);
        let chain_before = w.combo.ring;
        let (mut jumps, mut tip, mut catch) = (Vec::new(), None, None);
        let (mut scripts, mut highest, mut flight, mut tipped_at) = (Vec::new(), [0.0f32; 2], 0, 0);
        for k in 0..600 {
            let before = w.ball.clone();
            let hands: Vec<[f32; 3]> = w
                .players
                .iter()
                .map(|p| [p.position[0], p.height + p.ratings.reach, p.position[1]])
                .collect();
            ai_step(&mut w);
            for (j, p) in w.players.iter().enumerate() {
                if let Action::TipJump { script, .. } = p.action {
                    if !scripts.contains(&(j, script)) {
                        scripts.push((j, script));
                    }
                    highest[j / 3] = highest[j / 3].max(p.height);
                }
            }
            for e in &w.events {
                match *e {
                    Event::Jump { player, .. } => jumps.push((player, before.velocity[1])),
                    Event::Pass { from, to, .. } => {
                        let hand = hands[from];
                        let reach = (0..3)
                            .map(|c| (hand[c] - before.position[c]).powi(2))
                            .sum::<f32>()
                            .sqrt();
                        tip = Some((from, to, reach));
                        flight = w.ball.flight_ticks();
                        tipped_at = k;
                    }
                    Event::Catch { player } => catch = Some((player, k - tipped_at)),
                    _ => {}
                }
            }
            if catch.is_some() {
                break;
            }
        }
        // FUN_0015eaa0: both jumpers (each team's slot 0) jump as the toss
        // starts down; a computer side's timing is r % 20 + 1.
        assert_eq!(jumps.len(), 2, "seed {seed}: {jumps:?}");
        assert!(jumps.iter().any(|j| j.0 == 0) && jumps.iter().any(|j| j.0 == 3));
        assert!(jumps.iter().all(|j| j.1 < 0.0));
        assert!(w.tip.timing.iter().all(|t| (1..=20).contains(t)));
        // The winner is the side with the larger timing (actor 0's record
        // read for both; a tie to team 0).
        let winner = if w.tip.timing[1] > w.tip.timing[0] {
            3
        } else {
            0
        };
        assert_eq!(w.tip.winner, winner as i32);
        // Both play their scripts: the winner `tipoff_jump_2`, the loser
        // one of the other two; each `a1` lifts him well off the floor.
        let loser = 3 - winner;
        assert!(scripts.contains(&(winner, 0)), "seed {seed}: {scripts:?}");
        assert!(
            scripts.contains(&(loser, 1)) || scripts.contains(&(loser, 2)),
            "seed {seed}: {scripts:?}"
        );
        assert!(highest.iter().all(|&h| h > 2.0), "seed {seed}: {highest:?}");
        // He tips it with his hand within 1.25 of where the ball was, to
        // his slot 2 behind him: the first pass, 40 updates.
        let (from, to, reach) = tip.expect("the tip");
        assert_eq!((from, to), (winner, winner + 2));
        assert!(reach <= toss::TIP_REACH, "{reach}");
        assert_eq!(flight, crate::FIRST_PASS_FLIGHT as i32);
        assert!(!w.tip.first_pass);
        // Caught as it arrives (the original's hands take it one update
        // before: 38 in every captured opening); the catch ends the
        // opening; no rebound, the combo rings untouched.
        assert_eq!(catch, Some((winner + 2, 39)));
        assert_eq!(w.opening_toss, OpeningToss::None);
        assert_eq!(w.possession, w.players[winner].team);
        assert_eq!(chain_before, w.combo.ring);
    }
}

#[test]
fn a_person_times_the_jump_with_shoot_near_the_toss_apex() {
    let mut w = three_on_three(9);
    w.start_opening_toss();
    // Team 1 is a person's; its jumper presses shoot near the apex.
    let mut inputs = vec![Input::computer(); 6];
    for _ in 0..toss::WAIT_UPDATES {
        w.step(&inputs);
    }
    let mut pressed = false;
    for _ in 0..200 {
        inputs[3..].fill(Input::default());
        if !pressed && w.ball.velocity[1] > 0.0 && w.ball.position[1] > 19.9 {
            inputs[3] = buttons(button::SHOOT);
            pressed = true;
        }
        w.step(&inputs);
        if w.tip.winner >= 0 {
            break;
        }
    }
    assert!(pressed);
    // Pressed within a hair of the apex: the best timing, 20 or 21 capped.
    assert_eq!(w.tip.timing[1], 20);
    assert_eq!(w.tip.winner, 3);
    // A person's side that never presses gets 1.
    let mut w = three_on_three(9);
    w.start_opening_toss();
    let mut inputs = vec![Input::computer(); 6];
    inputs[0..3].fill(Input::default());
    for _ in 0..toss::WAIT_UPDATES + 120 {
        w.step(&inputs);
    }
    assert_eq!(w.tip.timing[0], 1);
}

#[test]
fn an_inbound_or_a_restart_cancels_the_opening_toss() {
    let mut w = three_on_three(2);
    w.start_opening_toss();
    w.restart_formation(1);
    assert_eq!(w.opening_toss, OpeningToss::None);
    let mut w = three_on_three(2);
    w.start_opening_toss();
    // A lesson stages a dead ball over it; the restart's end (here the
    // rewrite's limit) ends the toss.
    w.phase = Phase::DeadBall { left: 0.1, team: 0 };
    for _ in 0..10 {
        w.step(&[]);
    }
    assert_eq!(w.opening_toss, OpeningToss::None);
    assert!(matches!(w.ball.state, BallState::Held(_)));
}

#[test]
fn the_opening_toss_survives_a_snapshot() {
    let mut w = three_on_three(4);
    w.start_opening_toss();
    for _ in 0..40 {
        w.step(&[]);
    }
    let back = World::from_snapshot(&w.to_snapshot()).expect("snapshot");
    assert_eq!(
        back.opening_toss,
        OpeningToss::Waiting(toss::WAIT_UPDATES - 40)
    );
    let mut a = w.clone();
    let mut b = back;
    for _ in 0..200 {
        a.step(&[]);
        b.step(&[]);
    }
    assert_eq!(a.checksum(), b.checksum());
    let mut bad = w.clone();
    bad.opening_toss = OpeningToss::Waiting(0);
    assert!(bad.validate().is_err());
    assert!(World::from_snapshot(&bad.to_snapshot()).is_err());
    bad.opening_toss = OpeningToss::Waiting(toss::WAIT_UPDATES + 1);
    assert!(bad.validate().is_err());
}

#[test]
fn a_game_on_the_sessions_generator_draws_its_update_order() {
    // `FUN_0033cf98` puts actor 0 in first and each next one at `rand() %
    // (k + 1)` among the k in: five of the construction's draws, which
    // the game makes anyway (the generator after the construction is the
    // same), so every order comes up about as often. A world from a bare
    // seed keeps the highest index first; the order rides the snapshot
    // and a world must hold a permutation.
    let bare = World::new(hoops(), Vec::new(), Rules::default(), 9);
    assert_eq!(bare.update_order, HIGHEST_FIRST);
    let mut seen = std::collections::HashMap::new();
    for seed in 0..14_400 {
        // A session's generator, run on through a front end.
        let mut g = rng::Rng::new(seed);
        for _ in 0..1000 + seed % 977 {
            g.next_u32();
        }
        let w = World::from_generator(hoops(), Vec::new(), Rules::default(), g.clone());
        let mut sorted = w.update_order;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4, 5]);
        *seen.entry(w.update_order).or_insert(0u32) += 1;
        if seed == 0 {
            let mut plain = g;
            for _ in 0..CONSTRUCTION_DRAWS {
                plain.next_u32();
            }
            let mut after = w.generator().clone();
            let mut expect = plain;
            // The points game's pair follows the construction.
            expect.next_u32();
            expect.next_u32();
            assert_eq!(after.next_u32(), expect.next_u32());
            let back = World::from_snapshot(&w.to_snapshot()).unwrap();
            assert_eq!(back.update_order, w.update_order);
        }
    }
    assert_eq!(seen.len(), 720);
    assert!(seen.values().all(|&n| (5..=40).contains(&n)), "{seen:?}");
    let mut bad = bare;
    bad.update_order = [5, 4, 3, 2, 1, 1];
    assert!(bad.validate().is_err());
}

/// Production paths held against the shipped program (needs STREET_ISO).
mod static_parity;

/// The meter's hold and decay and the match's statistics.
mod meter_stats;

/// The scoring messages: the combo ring, the meters' and the statistics'
/// listeners.
mod scoring;

/// The game's phase machine through `World::step`.
mod game_phase;

/// Blocks and the dribble, on a made-up skeleton.
mod blocks;
/// The turbo bars, the juke press and the counter stance on its scripts.
mod counter_stance;
/// The held ball on the holder's clip and the scoop, on a made-up skeleton.
mod held;
/// The person's presses: the pass target and the passout specials, the
/// shoot press near the rim, the pump fake's end, the lay-ups in the air.
mod presses;
/// Shots on the ball's states and rebounds taken in the air.
mod rebound;
/// The ball on the bones: catches, knocks, pickups and the ball's leftovers.
mod touch;
/// The swipe's hand and the limbs a pass meets, on a made-up skeleton.
mod turnovers;

/// A screen is the original's plain contact (`FUN_001343e8`): a defender
/// running into an offensive player who stands still takes the whole
/// push and cannot get through; the screener does not move. The overlap
/// is the radii's against the distance table built before the actors
/// move (raw `0x1344ac..0x1344cc`, `0x135c34..0x135c64`).
#[test]
fn a_defender_running_into_a_still_screener_is_held_off_like_fun_001343e8() {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-30.0, 10.0], 0.0),
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(1, [-5.0, 0.0], std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        1,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    for _ in 0..2 * TICK_HZ {
        w.step(&[Input::default(), Input::default(), stick(1.0, 0.0)]);
        let gap = distance(w.players[1].position, w.players[2].position);
        assert!(gap > 2.0 * contact::RADIUS - 0.3, "{gap}");
    }
    assert_eq!(w.players[1].position, [0.0, 0.0]);
    // The push takes the overlap on the distance table built before the
    // actors move (`FUN_0018d0c0`), so the runner rests his own step (his
    // speed's update) inside the two radii.
    let step = w.players[2].speed * TICK_SECONDS;
    let x = w.players[2].position[0];
    assert!(
        (x + 2.0 * contact::RADIUS - step).abs() < 0.01,
        "{x} {step}"
    );
    // Two opponents walking into each other: neither walks through.
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-30.0, 10.0], 0.0),
            Player::new(0, [3.0, 0.0], 0.0),
            Player::new(1, [-3.0, 0.0], 0.0),
        ],
        Rules::default(),
        1,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    // Held off at two radii (later they slide past each other), before
    // the pushes make a screen (`SCREEN_CONTACTS`), whose reactions move
    // both on their clips.
    for _ in 0..30 {
        w.step(&[Input::default(), stick(-1.0, 0.0), stick(1.0, 0.0)]);
    }
    assert!(!w.down(1) && !w.down(2));
    // Each rests his own step inside the radii (the distance table's).
    let (a, b) = (w.players[2].position[0], w.players[1].position[0]);
    let steps = (w.players[1].speed + w.players[2].speed) * TICK_SECONDS;
    assert!(
        a < b && b - a > 2.0 * contact::RADIUS - steps - 0.01,
        "{a} {b} {steps}"
    );
}

/// `FUN_001343e8` tests the radii against the game's distance table, built
/// before the actors move (`FUN_0018d0c0`): a runner who closes inside the
/// radii only this update is not pushed until the next one, and then by
/// the overlap the table holds.
#[test]
fn contacts_go_by_the_distance_table_built_before_the_actors_move() {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-30.0, 10.0], 0.0),
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(1, [-12.0, 0.0], std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        1,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    let run = [Input::default(), Input::default(), stick(1.0, 0.0)];
    for _ in 0..20 {
        w.step(&run);
    }
    let reach = 2.0 * contact::RADIUS;
    w.players[2].position = [-(reach + 0.05), 0.0];
    w.step(&run);
    let step = w.players[2].speed * TICK_SECONDS;
    let x = w.players[2].position[0];
    assert!((x + reach + 0.05 - step).abs() < 1e-4, "{x} {step}");
    assert!(-x < reach);
    // The next update begins inside: pushed back by the table's overlap.
    let inside = reach + x;
    w.step(&run);
    let step = w.players[2].speed * TICK_SECONDS;
    let y = w.players[2].position[0];
    assert!(
        (y - (x + step - inside)).abs() < 1e-4,
        "{y} {x} {step} {inside}"
    );
    assert_eq!(w.players[1].position, [0.0, 0.0]);
}

/// A change of gait row blends the pose from the old row's clip, held at
/// the frame it had reached, to the new row's over 6 updates (the channel
/// step's blend, flag 0x200000, the counter over the length).
#[test]
fn a_change_of_gait_row_blends_the_pose_from_the_old_row() {
    let bank = crate::pose::disc_bank();
    let Some(poses) = bank.get() else {
        return;
    };
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-30.0, 10.0], 0.0),
            Player::new(1, [0.0, 0.0], 0.0),
        ],
        Rules::default(),
        1,
    );
    w.poses = bank.clone();
    w.phase = Phase::Live;
    w.ball.hold(0);
    for _ in 0..10 {
        w.step(&[Input::default(), Input::default()]);
    }
    let before = w.players[1].gait_channel;
    w.step(&[Input::default(), stick(0.0, 1.0)]);
    let g = w.players[1].gait_channel;
    assert_ne!(g.row, before.row);
    assert_eq!(g.blend_counter, 6);
    assert_eq!(g.blend_from, Some((before.row, before.phase)));
    let body = w.players[1].body();
    let from = crate::pose::Frame {
        record: before.row,
        time: before.phase,
        looping: true,
    };
    for k in 1..=6 {
        let g = w.players[1].gait_channel;
        assert_eq!(g.blend_counter, 7 - k);
        let want = poses
            .blend(from, w.gait_frame(1), k as f32 / 6.0, body, w.place(1))
            .unwrap();
        assert_eq!(w.actor_pose(1).unwrap(), want, "{k}");
        w.step(&[Input::default(), stick(0.0, 1.0)]);
    }
    assert_eq!(
        w.actor_pose(1).unwrap(),
        poses.pose(w.gait_frame(1), body, w.place(1)).unwrap()
    );
}

/// A computer screener's pick press (`FUN_0015a060`, `0x15b5c4..
/// 0x15b6e8`) plays a setroughpick past its callback 16
/// ([`trick::PICKS`]); its clearout's arms knock the defender he steps
/// into. Off the floor there is no pick.
#[test]
fn a_computer_screeners_pick_press_knocks_the_defender_he_steps_into() {
    let poses = crate::pose::disc_bank();
    if poses.get().is_none() {
        return;
    }
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-30.0, 10.0], 0.0),
            Player::new(0, [0.0, 0.0], -std::f32::consts::FRAC_PI_2),
            Player::new(1, [-2.4, 0.0], std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        1,
    );
    w.poses = poses;
    w.phase = Phase::Live;
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    w.ball.hold(0);
    w.players[1].height = 1.0;
    assert!(!w.pick_press(1));
    w.players[1].height = 0.0;
    assert!(w.pick_press(1));
    let script = |w: &World, i: usize| match w.players[i].action {
        Action::Stumbling {
            reaction: Some(r), ..
        } => Some(r.script),
        _ => None,
    };
    assert!(trick::PICKS.contains(&script(&w, 1).expect("the screener's pick")));
    let mut knocked = false;
    for _ in 0..40 {
        w.step(&[Input::default(); 3]);
        knocked |= script(&w, 2).is_some();
    }
    assert!(knocked, "the defender stands");
}

/// The screen reaction (raw `0x135400..0x135570`): an offensive player
/// without the ball who keeps pushing into the same defender, facing him,
/// knocks him off balance on the 31st contact (the count `0x3d0f74` from
/// 0 reaching 30), with three draws (equal power: the weight is drawn
/// too), and the count rests at -600. The contact's message
/// (`0x3c6910`) puts the screener in the defender's AI+0xb0. Both play
/// the original's scripts: the screener a setroughpick
/// ([`trick::SCREENER`]), the defender, hit from in front, the heavy or
/// the light list's quarter 2 ([`trick::SCREENED_HEAVY`]).
#[test]
fn pushing_into_a_defender_thirty_times_knocks_him_off_balance() {
    for computer in [false, true] {
        let mut w = World::new(
            hoops(),
            vec![
                Player::new(0, [-30.0, 10.0], 0.0),
                Player::new(0, [0.0, 0.0], -std::f32::consts::FRAC_PI_2),
                Player::new(1, [-2.4, 0.0], std::f32::consts::FRAC_PI_2),
            ],
            Rules::default(),
            1,
        );
        w.phase = Phase::Live;
        w.ball.hold(0);
        let first = if computer {
            Input::computer()
        } else {
            Input::default()
        };
        // With the computer on the holder and the person on 1, team 0 is a
        // side one pad plays: the game would hand the person the holder
        // unless a give 'n go holds him on 1 (team+0xc8).
        w.control.hold[0] = computer;
        let mut contacts = 0;
        let mut knocked = None;
        for t in 0..120 {
            let before = w.rng.clone();
            w.step(&[first, stick(-1.0, 0.0), Input::default()]);
            if w.contacts.with[1] == 2 {
                contacts += 1;
            }
            if matches!(w.players[2].action, Action::Stumbling { .. }) {
                knocked = Some(t);
                assert_eq!(w.contacts.count[1], contact::SCREEN_REST);
                let script = |i: usize| match w.players[i].action {
                    Action::Stumbling {
                        reaction: Some(r), ..
                    } => Some(r.script),
                    _ => None,
                };
                assert!(trick::SCREENER.contains(&script(1).expect("the screener's")));
                let defender = script(2).expect("the defender's");
                assert!(
                    trick::SCREENED_HEAVY[2].contains(&defender)
                        || trick::SCREENED_LIGHT[2].contains(&defender),
                    "{defender}"
                );
                if !computer {
                    // The contact's three and the knocked defender's
                    // stand draw earlier in the update (`r & 1`).
                    let mut r = before;
                    for _ in 0..4 {
                        r.next_u32();
                    }
                    assert_eq!(w.rng, r, "three draws and the stand's");
                } else {
                    assert_eq!(w.ai.brains[2].screened_by, 1);
                }
                break;
            }
        }
        assert!(knocked.is_some(), "no screen");
        assert_eq!(contacts, 31);
        // A world read back keeps the counts.
        let back = World::from_snapshot(&w.to_snapshot()).unwrap();
        assert_eq!(back.contacts, w.contacts);
    }
}

/// Teammates do not touch while one of them shoots (`FUN_00136218`).
#[test]
fn teammates_pass_through_a_shooter() {
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [-20.0, 0.0], -std::f32::consts::FRAC_PI_2),
            Player::new(0, [-21.0, 0.0], 0.0),
        ],
        Rules::default(),
        1,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    // Shoot held (a quick release would be a pump fake).
    for _ in 0..10 {
        w.step(&[buttons(button::SHOOT), Input::default()]);
        assert!(matches!(w.players[0].action, Action::Shooting { .. }));
    }
    assert_eq!(w.players[1].position, [-21.0, 0.0]);
    assert_eq!(w.players[0].position, [-20.0, 0.0]);
}

/// A holder 16.5 from the rim with his man 3.5 nearer it on his line, his
/// wanted facing at the man, and the stick toward the rim.
fn backdown_world(seed: u32) -> (World, Input) {
    let mut w = face_off(seed);
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    let rim = w.rim_spot(crate::target_hoop(0));
    let side = rim[0].signum();
    w.players[0].position = [rim[0] - side * 16.5, 0.0];
    w.players[1].position = [rim[0] - side * 13.0, 0.0];
    let toward = crate::contact::angle(w.players[0].position, w.players[1].position);
    w.players[0].wanted = crate::restart::heading_radians(toward);
    w.players[0].heading = w.players[0].wanted;
    (w, stick(side, 0.0))
}

#[test]
fn a_holder_walking_his_man_down_backs_down_and_posts_up() {
    // `FUN_00163b48`: 15..18 from the rim, the stick at it, the man 2.1
    // nearer, within 6 and on the line: `into_backdown`, which holds the
    // walk through its transition and runs into the side dribble's
    // `9b 0x820` (bit 11).
    let (mut w, push) = backdown_world(3);
    assert!(w.backs_down(0, crate::backdown::stick_word(&push), &push));
    let before = w.players[0].position;
    step_all(&mut w, &[(0, push)], 1);
    assert_eq!(
        w.players[0].stand.map(|s| s.script),
        Some(stand::BACKDOWN_IN)
    );
    assert!(!w.players[0].posting);
    let mut ticks = 0;
    while !w.players[0].posting && ticks < 2 * TICK_HZ {
        assert!(w.backdown_holds(0));
        step_all(&mut w, &[(0, push)], 1);
        ticks += 1;
    }
    assert!(w.players[0].posting, "never posted up");
    assert!(crate::distance(before, w.players[0].position) < 1.0);
    // The defender against him takes the post stance.
    step_all(&mut w, &[(0, push)], 1);
    assert!(w.post_stance(1));
}

#[test]
fn turbo_or_a_far_man_keeps_the_holder_from_backing_down() {
    let (mut w, push) = backdown_world(3);
    let word = crate::backdown::stick_word(&push);
    assert!(!w.backs_down(0, word, &push.with(button::R1)));
    let rim = w.rim_spot(crate::target_hoop(0));
    w.players[1].position = [rim[0] - rim[0].signum() * 9.0, 0.0];
    assert!(!w.backs_down(0, word, &push));
    let (mut w, push) = backdown_world(3);
    w.players[1].position[1] = 3.0;
    assert!(!w.backs_down(0, word, &push));
}

#[test]
fn a_post_defender_goes_round_the_posting_holder_toward_the_rim() {
    // `0x15db18..0x15dca4`: on the post rows against a posting holder he
    // keeps his distance and turns at most 14 units an update toward the
    // holder's way to the rim.
    let (mut w, _) = backdown_world(3);
    w.players[0].posting = true;
    let hp = w.players[0].position;
    let rim = w.toward_rim(0);
    // Off the holder's line by 100 units, 3 away.
    let word = (rim + 100) & 0x3ff;
    w.players[1].position = [
        hp[0] - 3.0 * crate::input::native_sin_1024(word),
        hp[1] - 3.0 * crate::input::native_cos_1024(word),
    ];
    w.start_stand(1, stand::POST_STANCE);
    w.stand_tick(1);
    while !matches!(w.channel_row(1), 718 | 719) {
        w.stand_tick(1);
    }
    w.post_slide(1);
    let at = crate::contact::angle(hp, w.players[1].position);
    let off = crate::timing::angle_diff(rim, at).abs();
    assert!((84..=88).contains(&off), "{off}");
    assert!((crate::distance(hp, w.players[1].position) - 3.0).abs() < 1e-3);
}

#[test]
fn a_man_off_the_ball_holding_juke_near_the_rim_posts_for_it() {
    // `0x15b544..0x15b5bc`: `post_call_for_ball_2` posts (bit 11) and holds
    // his walk; let go on its rows he stands ready (`0x15ad84`).
    let mut w = three_on_three(5);
    w.phase = Phase::Live;
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    w.ball.hold(0);
    w.possession = 0;
    let rim = w.rim_spot(crate::target_hoop(0));
    w.players[1].position = [rim[0] - rim[0].signum() * 8.0, 4.0];
    let juke = Input::default().with(button::TRICK);
    let mut all = vec![Input::default(); 6];
    all[1] = juke;
    w.step(&all);
    assert_eq!(w.players[1].stand.map(|s| s.script), Some(stand::POST_CALL));
    for _ in 0..10 {
        w.step(&all);
    }
    assert!(w.players[1].posting);
    assert!(w.backdown_holds(1));
    w.step(&[Input::default(); 6]);
    assert!(!w.players[1].posting);
    assert_eq!(w.players[1].stand.map(|s| s.script), Some(stand::READY));
}

#[test]
fn a_post_call_lets_go_on_defence() {
    // `0x15ad84` runs before the update splits the actors: a posting man
    // whose side lost the ball stands ready without the button.
    let mut w = three_on_three(5);
    w.phase = Phase::Live;
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    w.ball.hold(0);
    w.possession = 0;
    let rim = w.rim_spot(crate::target_hoop(0));
    w.players[1].position = [rim[0] - rim[0].signum() * 8.0, 4.0];
    let juke = Input::default().with(button::TRICK);
    w.post_call(1, &juke);
    for _ in 0..4 {
        w.stand_tick(1);
    }
    assert!(w.players[1].posting);
    w.possession = 1;
    w.post_call(1, &Input::default());
    assert!(!w.players[1].posting);
    assert_eq!(w.players[1].stand.map(|s| s.script), Some(stand::READY));
}

#[test]
fn a_computer_screener_holding_the_stance_stands_on_record_64() {
    // `0x15b7d4..0x15b85c`: held 0x400000 starts `0x44ba7b` (record 64,
    // the walk held) every update; let go on record 64 he stands ready
    // (`0x15add8..0x15ae14`). A person's pad never holds it.
    let mut w = three_on_three(5);
    w.phase = Phase::Live;
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    w.ball.hold(0);
    w.possession = 0;
    let person = Input {
        buttons: button::PICK_STANCE,
        ..Input::default()
    };
    w.pick_stance(1, &person);
    assert_eq!(w.players[1].stand.map(|s| s.script), None);
    let held = Input {
        buttons: button::PICK_STANCE | button::COMPUTER,
        ..Input::default()
    };
    for _ in 0..4 {
        w.pick_stance(1, &held);
        assert_eq!(
            w.players[1].stand.map(|s| s.script),
            Some(stand::PICK_STANCE)
        );
        assert!(w.backdown_holds(1));
        w.stand_tick(1);
    }
    assert_eq!(w.channel_row(1), 64);
    w.players[1].height = 1.0;
    w.players[1].stand = None;
    w.pick_stance(1, &held);
    assert_eq!(w.players[1].stand.map(|s| s.script), None, "off the floor");
    w.players[1].height = 0.0;
    w.pick_stance(1, &held);
    w.stand_tick(1);
    w.pick_stance(1, &Input::default());
    assert_eq!(w.players[1].stand.map(|s| s.script), Some(stand::READY));
}

#[test]
fn a_persons_strategy_press_with_both_turbos_is_his_own_pick() {
    // `0x15b5f0..0x15b7d0`: off the ball, two turbos held and half the bar
    // buy the whole setroughpick (`0x43f038[r % 3]`); short of the bar or
    // of the turbos, nothing, and the press ends his buttons either way.
    let mut w = three_on_three(5);
    w.phase = Phase::Live;
    w.game_phase.phase = crate::game_phase::phase::FRONT_COURT;
    w.ball.hold(0);
    w.possession = 0;
    let script = |w: &World| match w.players[1].action {
        Action::Stumbling {
            reaction: Some(r), ..
        } => Some(r.script),
        _ => None,
    };
    let one = Input::default().with(button::R3 | button::L1);
    let two = Input::default().with(button::R3 | button::L1 | button::R1);
    w.set_turbo(1, 1.0);
    assert!(w.person_pick(1, &one));
    assert_eq!(script(&w), None);
    w.set_turbo(1, 0.4);
    assert!(w.person_pick(1, &two));
    assert_eq!(script(&w), None);
    assert!((w.turbo_of(1) - 0.4).abs() < 1e-6);
    w.set_turbo(1, 0.8);
    w.players[1].height = 1.0;
    assert!(!w.person_pick(1, &two));
    w.players[1].height = 0.0;
    assert!(w.person_pick(1, &two));
    assert!(trick::SCREENER.contains(&script(&w).expect("the person's pick")));
    assert!((w.turbo_of(1) - 0.3).abs() < 1e-6);
}

#[test]
fn a_stronger_posting_holder_bumps_his_still_defender() {
    // `0x135644..0x135740`: on the backdown's walk, after 15 contacts,
    // face to face and stronger, his defender plays the bump reaction (off
    // balance, no walk) and the contact pushes neither.
    let (mut w, _) = backdown_world(3);
    w.players[0].posting = true;
    w.players[0].gait_channel.record = Some(0);
    w.players[0].gait_channel.rows = [704, 705];
    w.players[0].speed = 3.0;
    w.players[0].ratings.power = 0.9;
    w.players[1].ratings.power = 0.2;
    let toward = crate::contact::angle(w.players[1].position, w.players[0].position);
    w.players[1].heading = crate::restart::heading_radians(toward);
    w.contacts.count[0] = 15;
    let (a0, b0) = (w.players[1].position, w.players[0].position);
    assert!(w.backdown_bump(1, 0, [false, true]));
    assert_eq!(
        w.players[1].stand.map(|s| s.script),
        Some(stand::BUMP_REACTION)
    );
    assert!(w.down(1) && w.backdown_holds(1));
    assert_eq!(w.contacts.count[0], -1);
    assert_eq!((w.players[1].position, w.players[0].position), (a0, b0));
    // A weaker holder does not.
    let (mut w, _) = backdown_world(3);
    w.players[0].gait_channel.record = Some(0);
    w.players[0].gait_channel.rows = [704, 705];
    w.players[0].speed = 3.0;
    w.players[0].ratings.power = 0.2;
    w.players[1].ratings.power = 0.9;
    w.players[1].heading = crate::restart::heading_radians(toward);
    w.contacts.count[0] = 15;
    assert!(!w.backdown_bump(1, 0, [false, true]));
}

#[test]
fn a_holder_driving_into_his_defender_with_turbo_collides() {
    // `0x135754..0x1358fc`: 15..18 from the rim facing it, the man 2.1
    // nearer, moving with turbo: both play their collision clips, off
    // balance, and the contact pushes neither.
    let (mut w, _) = backdown_world(3);
    w.players[0].speed = 3.0;
    let (a0, b0) = (w.players[1].position, w.players[0].position);
    assert!(!w.turbo_collision(1, 0, [false, true], false));
    assert!(w.turbo_collision(1, 0, [false, true], true));
    assert_eq!(
        w.players[1].stand.map(|s| s.script),
        Some(stand::COLLISION_DEFENDER)
    );
    assert_eq!(
        w.players[0].stand.map(|s| s.script),
        Some(stand::COLLISION_CARRIER)
    );
    assert!(w.down(0) && w.down(1) && w.backdown_holds(1));
    assert_eq!((w.players[1].position, w.players[0].position), (a0, b0));
}

/// Opt-in check of the staged Level 2 shots against the original's School
/// runs (STREET_L2_RICH: the folder of `*-rich.json` captures, a sample
/// every few updates). Every captured shot is a straight make through the
/// aim, so its samples fix its arc: fitted against the ball's timer, its
/// release is one update before the timer's start (`flight_updates`), as a
/// pass's is. From that release, the actor's own angle (`FUN_00333ef8`,
/// from the shooter's distance across the floor at his last sample with
/// the ball) through the launcher's steepening and flight time must give
/// the original's updates to the aim.
#[test]
fn staged_level_two_shots_fly_the_originals_arcs() {
    let Some(folder) = std::env::var_os("STREET_L2_RICH") else {
        return;
    };
    // Least squares of y against powers of s up to `degree`.
    fn fit(s: &[f64], y: &[f64], degree: usize) -> Vec<f64> {
        let n = degree + 1;
        let mut a = vec![vec![0.0; n + 1]; n];
        for (&s, &y) in s.iter().zip(y) {
            for (i, row) in a.iter_mut().enumerate() {
                for (j, cell) in row.iter_mut().take(n).enumerate() {
                    *cell += s.powi((i + j) as i32);
                }
                row[n] += y * s.powi(i as i32);
            }
        }
        for i in 0..n {
            let pivot = a[i].clone();
            for (k, row) in a.iter_mut().enumerate() {
                if k != i {
                    let f = row[i] / pivot[i];
                    for (cell, p) in row.iter_mut().zip(&pivot) {
                        *cell -= f * p;
                    }
                }
            }
        }
        (0..n).map(|i| a[i][n] / a[i][i]).collect()
    }
    let mut checked = 0;
    let mut paths: Vec<_> = std::fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with("-rich.json"))
        .collect();
    paths.sort();
    for path in paths {
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let samples: Vec<&serde_json::Value> = doc["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["staged_clock"]["state"] == 3)
            .collect();
        let Some(first) = samples.iter().position(|s| s["ball"]["state"] == 5) else {
            continue;
        };
        let flight: Vec<_> = samples[first..]
            .iter()
            .take_while(|s| s["ball"]["state"] == 5)
            .collect();
        // A dunk's two updates of flight hold no arc to fit.
        if flight.len() < 3 {
            continue;
        }
        let f = |v: &serde_json::Value| v.as_f64().unwrap();
        let s: Vec<f64> = flight.iter().map(|x| -f(&x["ball"]["timer"])).collect();
        let axis = |k: usize| -> Vec<f64> {
            flight
                .iter()
                .map(|x| f(&x["ball"]["position"][k]))
                .collect()
        };
        let (x, y, z) = (
            fit(&s, &axis(0), 1),
            fit(&s, &axis(1), 2),
            fit(&s, &axis(2), 1),
        );
        let side = if axis(0)[0] > 0.0 { 36.75 } else { -36.75 };
        let release = -(f(&flight[0]["ball"]["flight_updates"]) + 1.0);
        let at = |c: &[f64], s: f64| c.iter().rev().fold(0.0, |acc, &k| acc * s + k);
        let from = [at(&x, release), at(&y, release), at(&z, release)].map(|v| v as f32);
        let original = (side - x[0]) / x[1] - release;
        // The shooter: the ball's last holder, where that sample has him.
        let holding = samples[..first]
            .iter()
            .rev()
            .find(|s| s["ball"]["holder"].as_u64() != Some(0))
            .unwrap();
        let holder = holding["ball"]["holder"].as_u64().unwrap();
        let root = holding["actors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| {
                u64::from_str_radix(&a["address"].as_str().unwrap()[2..], 16).unwrap() == holder
            })
            .map(|a| [f(&a["position"][0]) as f32, f(&a["position"][2]) as f32])
            .unwrap();
        let d = ((root[0] - side as f32).powi(2) + root[1] * root[1]).sqrt();
        let angle = ((20.0 - d) / 20.0).max(0.0) * f32::from_bits(0x3ec9_0fda)
            + f32::from_bits(0x3f49_0fdb);
        let goal = [side as f32, crate::shooting::NATIVE_AIM_HEIGHT, 0.0];
        let aim = [goal[0] - from[0], goal[1] - from[1], goal[2] - from[2]];
        let steep = crate::ball::steepen_entry(angle, from, aim, crate::ball::RADIUS, false);
        let ours = crate::ball::shot_flight_time(from, goal, steep) * TICK_HZ as f32;
        eprintln!(
            "{}: {original:.3} updates to the aim, ours {ours:.3} (d {d:.2})",
            path.file_name().unwrap().to_string_lossy()
        );
        assert!((f64::from(ours) - original).abs() < 0.1);
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn a_persons_press_scores_by_how_near_the_apex_it_falls() {
    // FUN_0015eaa0 (0x15eb5c..0x15ebfc): the toss's apex from ball+0x118,
    // the gravity's size an update; a press with the toss in the top
    // fifth of its rise scores 1..20 by how near the apex. The sign of the
    // gravity had made the apex fall below a rising toss, so any press
    // short of the last update or two scored 1 and locked in (a person on
    // team 0 then lost every tip-off).
    let g = crate::GRAVITY * crate::TICK_SECONDS * crate::TICK_SECONDS;
    let run = |presses: &[i32]| {
        let mut w = three_on_three(1);
        w.start_opening_toss();
        let mut inputs = vec![Input::computer(); 6];
        inputs[0] = Input::default();
        for _ in 0..toss::WAIT_UPDATES {
            w.step(&inputs);
        }
        let mut left = presses.to_vec();
        for _ in 0..400 {
            inputs[0] = Input::default();
            let vy = w.ball.velocity[1] * crate::TICK_SECONDS;
            let to_apex = (vy / g).floor() as i32;
            if vy > 0.0 && left.first().is_some_and(|&e| to_apex <= e) {
                inputs[0] = buttons(button::SHOOT);
                left.remove(0);
            }
            w.step(&inputs);
            if w.tip.winner >= 0 {
                break;
            }
        }
        (w.tip.timing[0], w.tip.winner)
    };
    // Too early (below the window) leaves nothing: the side gets 1.
    assert_eq!(run(&[30]).0, 1);
    assert_eq!(run(&[20]).0, 9);
    assert_eq!(run(&[15]).0, 14);
    assert_eq!(run(&[10]), (18, 0));
    assert_eq!(run(&[2]), (20, 0));
    // A press below the window does not use up the side's timing.
    assert_eq!(run(&[45, 10]), (18, 0));
}

#[test]
fn the_gait_clock_runs_by_the_skeletons_scale() {
    // `0x167cd0..0x167ce8`: the frame step is the motion over the record's
    // root distance times the skeleton's first scale (`FUN_0019cd48`), so
    // a bigger player's legs cycle slower over the same ground. A stand-in
    // of height / 6.5 had cycled them about a fifth too fast.
    let step = |height: f32| {
        let mut w = three_on_three(5);
        w.players[1].body = Some(crate::pose::Body {
            height,
            width: 1.0,
            head: 1.0,
        });
        let mut inputs = vec![Input::default(); 6];
        for _ in 0..30 {
            inputs[1] = stick(1.0, 0.0);
            w.step(&inputs);
        }
        assert!(w.players[1].speed > 1.0);
        w.players[1].gait_channel.frame_step
    };
    let (unit, big) = (step(1.0), step(1.3));
    assert!((unit / big - 1.3).abs() < 1e-3, "{unit} / {big}");
}

#[test]
fn a_dropped_ball_posts_its_floor_bounce_with_the_speed_it_lost() {
    // `FUN_00153278` posts slot 31 with the velocity the bounce took off
    // the ball, per update (`0x153330..0x153370`).
    let mut w = live(vec![
        Player::new(0, [-20.0, 0.0], 0.0),
        Player::new(1, [-20.0, 5.0], 0.0),
    ]);
    w.ball = Ball::loose_at([0.0, 6.0, 0.0]);
    w.ball.airborne = true;
    let mut hits = Vec::new();
    for _ in 0..TICK_HZ * 2 {
        let before = w.ball.velocity;
        w.step(&[Input::default(), Input::default()]);
        for e in &w.events {
            if let Event::BallHit { surface, speed } = *e {
                hits.push((surface, speed, before[1]));
            }
        }
    }
    let (surface, speed, falling) = hits[0];
    assert_eq!(surface, BallSurface::Floor, "{hits:?}");
    // It was falling at `falling` per second: the bounce took more than that
    // fall's update off it, and less than twice it plus gravity's update.
    let fall = -falling * crate::TICK_SECONDS;
    assert!(
        speed > fall && speed < 2.0 * fall + 0.05,
        "{speed} vs {fall}"
    );
}
