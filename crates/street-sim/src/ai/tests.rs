use super::*;
use crate::tests::hoops;
use crate::{BallState, Player, Rules};

fn three_on_three(seed: u32) -> World {
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

fn step_all(w: &mut World) {
    let inputs: Vec<Input> = (0..w.players.len()).map(|i| input(w, i)).collect();
    w.step(&inputs);
}

fn name(a: &Act) -> &'static str {
    match a {
        Act::CarrierPause(_) => "pause",
        Act::Shoot(_) => "shoot",
        Act::Pass(_) => "pass",
        Act::Pocket(_) => "pocket",
        Act::Cancel(_) => "cancel",
        Act::Drive(_) => "drive",
        Act::Iso(_) => "iso",
        Act::Defense(_) => "defense",
        Act::LooseBall(_) => "loose",
        Act::PrepareRebound(_) => "rebound",
        Act::FastBreak(_) => "break",
        Act::PerimPlayer(_) => "perim",
        Act::PostPlayer(_) => "post",
        Act::CutMove(_) => "cut move",
        Act::PostCut(_) => "post cut",
        Act::PostPause(_) => "post pause",
        Act::CarrierMove(_) => "carrier move",
        Act::Leaper(_) => "leap",
        Act::Follower(_) => "follow",
        Act::WaitForToss => "toss",
        Act::Pause(_) => "wait",
        Act::Goto(_) => "goto",
        Act::PerimPause(_) => "perim pause",
        Act::DriftUpCourt(_) => "drift",
        Act::Rebound(_) => "jump",
        Act::GoalTend(_) => "goal tend",
        Act::BoxOut(_) => "box out",
        Act::OpenLooseBall(_) => "open loose",
        Act::Outlet => "outlet",
        Act::WaitForHalfCourt => "wait hc",
        Act::ThrowInbound(_) => "throw in",
        Act::InboundPos(_) => "in pos",
        Act::SetPick(_) => "set pick",
        Act::Roll(_) => "roll",
        Act::RunPick(_) => "run pick",
        Act::PickOffBall(_) => "pick off ball",
        Act::KeepAway(_) => "keep away",
        Act::PostUp(_) => "post up",
        Act::Feeder(_) => "feed",
        Act::TrickPass(_) => "trick pass",
    }
}

#[test]
#[ignore = "prints a computer game for reading"]
fn trace_a_computer_game() {
    let seed = std::env::var("AI_TRACE_SEED").map_or(3, |s| s.parse().unwrap_or(3));
    let mut w = three_on_three(seed);
    for t in 0..100 * 60 {
        step_all(&mut w);
        if t % 30 == 0 {
            let acts: Vec<String> =
                w.ai.brains
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        let p = &w.players[i];
                        format!(
                            "{}:{}[{:.0},{:.0}]a{}s{}{:?}",
                            i,
                            b.queue.first().map_or("-", |n| name(&n.act)),
                            p.position[0],
                            p.position[1],
                            b.pad.angle,
                            b.pad.strength,
                            p.action.elapsed()
                        )
                    })
                    .collect();
            println!(
                "t{} ph{} ball{:?} score{:?} | {}",
                t,
                w.ai.game.phase,
                w.ball.state,
                w.score,
                acts.join(" ")
            );
        }
        for e in &w.events {
            println!("  {t}: {e:?}");
        }
    }
}

/// A live half court: team 0's player 0 holds the ball `out` from the
/// rim it attacks (hoop 0, +x), a defender `gap` farther out.
fn half_court(out: f32, gap: f32) -> World {
    let rim = hoops()[0].rim;
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [rim[0] - out, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(1, [rim[0] - out - gap, 0.0], -std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        9,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    w
}

#[test]
fn the_shots_quality_is_the_originals_from_the_world() {
    // `FUN_001805e8` through the AI's view of the world: 50 Shots from 15
    // is 6.61, truncated to 6; a defender 20 farther out (beyond 18) adds
    // one; one 8 out takes one (within 10), contests (dx 4.4: 171 units,
    // 2) and one more within 14; a defender down (`FUN_0017fd08`) is not
    // counted.
    let q = |w: &World| with_ctx(w, |ctx| offence::quality(ctx, 0)).0;
    let mut w = half_court(15.0, 20.0);
    step_all(&mut w);
    w.ball.hold(0);
    w.players[1].position = [hoops()[0].rim[0] - 35.0, 0.0];
    assert_eq!(q(&w), 7);
    w.players[1].position = [hoops()[0].rim[0] - 23.0, 0.0];
    assert_eq!(q(&w), 2);
    w.players[1].action = crate::Action::Stumbling {
        elapsed: 0.0,
        duration: 1.0,
        reaction: None,
    };
    assert_eq!(q(&w), 7);
    // A full meter beyond the line: 7 whatever Threes, to 38.75.
    w.players[1].action = crate::Action::None;
    w.players[1].position = [hoops()[0].rim[0] - 70.0, 0.0];
    w.players[0].position = [hoops()[0].rim[0] - 32.0, 0.0];
    assert_eq!(q(&w), 1);
    w.set_meter(0, 50_000.0);
    assert_eq!(q(&w), 7);
}

#[test]
fn the_defenders_spot_follows_fun_00108148() {
    // Against a handler 20 out (the table's on-ball gap there is 3.5):
    // with the three-second sag (`[0x1f]`) and turbo to spare 6.8 off, 11
    // off a juke, none without turbo; the press flag (`[0x22]`) 3; the
    // pick sag (`[0x20]`) 5 less, past him, when a computer teammate of
    // his sets a pick within 10 of the defender.
    let rim = hoops()[0].rim;
    let mut w = World::new(
        hoops(),
        vec![
            Player::new(0, [rim[0] - 20.0, 0.0], std::f32::consts::FRAC_PI_2),
            Player::new(1, [rim[0] - 23.0, 0.0], -std::f32::consts::FRAC_PI_2),
            Player::new(0, [rim[0] - 26.0, 6.0], std::f32::consts::FRAC_PI_2),
        ],
        Rules::default(),
        9,
    );
    w.phase = Phase::Live;
    w.ball.hold(0);
    let gap = |w: &World, setting_pick: bool, f: &dyn Fn(&mut defence::Defense)| {
        with_ctx(w, |ctx| {
            if setting_pick {
                ctx.fronts[2] = 1;
            }
            let mut d = defence::Defense::new(ctx, 1);
            f(&mut d);
            let s = d.spot(ctx, 1, 0);
            // Signed: toward the rim (+x) positive.
            s[0] - ctx.pos(0)[0]
        })
        .0
    };
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    assert!(close(gap(&w, false, &|_| {}), 3.5));
    w.set_turbo(0, 1.0);
    assert!(close(gap(&w, false, &|d| d.sag = true), 6.8));
    assert!(close(gap(&w, false, &|_| {}), 3.5));
    w.players[0].action = crate::trick::pressed(0, 1);
    assert!(close(gap(&w, false, &|d| d.sag = true), 11.0));
    w.players[0].action = crate::Action::None;
    w.set_turbo(0, 0.1);
    assert!(close(gap(&w, false, &|d| d.sag = true), 3.5));
    assert!(close(gap(&w, false, &|d| d.press = true), 3.0));
    assert!(close(gap(&w, true, &|d| d.pick_sag = true), -1.5));
    assert!(close(gap(&w, true, &|_| {}), 3.5));
    // A posting man (state bit 11, `FUN_001816f0`: the post call's `9b
    // 0x800`) is guarded from the post: the defender 3 off, on his line
    // to the rim, closes to 1 (`0x108290..0x108380`), sag or not.
    w.players[0].posting = true;
    w.set_turbo(0, 1.0);
    assert!(close(gap(&w, false, &|_| {}), 1.0));
    assert!(close(gap(&w, false, &|d| d.sag = true), 1.0));
}

#[test]
fn the_post_mans_lob_call_is_the_post_calls_hold() {
    // "post pause" presses 0x800000 (`0x113800`), the hold the actor's
    // post call reads (actor+0x78, `0x15b544`): the sim's held trick
    // button off the ball.
    let w = front_court(4);
    let input = with_ctx(&w, |ctx| {
        let pad = Pad {
            buttons: pad::bits::POST_CALL,
            target: -1,
            ..Default::default()
        };
        pad::to_input(ctx, 1, &pad)
    })
    .0;
    assert!(input.held(crate::button::TRICK));
}

#[test]
fn the_post_man_screens_for_the_third_man_coming_at_him() {
    // `FUN_00114050`: the third man out of the middle, within 15 and
    // running at the post man, with the holder across the court's length.
    let mut w = front_court(4);
    let rim = w.hoops[0].rim;
    w.players[2].position = [rim[0] - 6.0, 8.0];
    w.players[1].position = [rim[0] - 6.0, -4.0];
    w.players[0].position = [rim[0] - 26.0, -5.0];
    let call = |w: &World| with_ctx(w, |ctx| spots::pick_off_ball_call(ctx, 2)).0;
    assert!(!call(&w), "standing");
    w.players[1].course = 0.0;
    w.players[1].speed = 10.0;
    assert!(call(&w));
    w.players[1].course = std::f32::consts::PI;
    assert!(!call(&w), "running away");
    w.players[1].course = 0.0;
    w.players[0].position[1] = 18.0;
    assert!(!call(&w), "the holder on his side's wing");
    w.players[0].position[1] = 2.0;
    assert!(with_ctx(&w, |ctx| ctx.zone(0)).0 == 2 && call(&w), "zone 2");
    w.players[0].position[1] = -5.0;
    w.lesson = Some(15);
    assert!(!call(&w), "lesson 15");
}

#[test]
fn the_post_man_sets_a_pick_once_the_window_stands_long_enough() {
    // `FUN_00114480`: the set-pick window open `fps × 2.5` frames in, the
    // holder within 35.75 of the rim and guarded within 15.
    let w = front_court(4);
    let ok = |age: u16| {
        with_ctx(&w, |ctx| {
            let pause = spots::PostPause::new(ctx);
            let b = Brain {
                age,
                ..Default::default()
            };
            pause.set_pick_call(ctx, &b)
        })
        .0
    };
    assert!(!ok(149));
    assert!(ok(150));
}

#[test]
fn a_juke_draws_the_counter_look_once_a_second() {
    // `FUN_00107710`: a defender in front of a juking handler on the
    // floor within reach rolls `skill(-13, -17, 40, 50)` plus the
    // ratings' edge, and his next look waits `fps` frames (`0x107a2c`);
    // the rewrite looked every `fps × 0.125`.
    // The handler (0) at the top, his defender (3) 4 in front of him.
    let mut w = front_court(4);
    for p in &mut w.players {
        p.ratings.steals = 0.0;
    }
    step_all(&mut w);
    // He jukes on the spot: the rewrite's trick creeps along the stick, which
    // would walk him into his defender, who now stands (`FUN_00107c40`).
    let spot = w.players[0].position;
    let mut looks = Vec::new();
    for t in 0..300 {
        quiet_playbook(&mut w);
        w.players[0].position = spot;
        w.players[0].speed = 0.0;
        w.set_turbo(3, 1.0);
        // A counter stance is cleared, so he keeps looking.
        w.players[3].action = crate::Action::None;
        w.players[0].action = crate::trick::pressed(0, 1);
        // Facing his defender.
        w.players[0].heading = std::f32::consts::FRAC_PI_2;
        step_all(&mut w);
        let pressed = w.ai.brains[3].pad.buttons & (pad::bits::STEAL | pad::bits::TWO_TURBOS)
            == pad::bits::STEAL | pad::bits::TWO_TURBOS;
        if let Some(Node {
            act: Act::Defense(d),
            ..
        }) = w.ai.brains[3].queue.first()
            && (d.counter_timer == 60 || pressed)
        {
            looks.push(t);
        }
    }
    assert!(looks.len() >= 2, "{looks:?}");
    // A press looks again at once (`[0xf] = 0`), a miss waits a second.
    assert!(looks.windows(2).all(|p| p[1] - p[0] >= 8), "{looks:?}");
    assert!(looks.windows(2).any(|p| p[1] - p[0] >= 60), "{looks:?}");
}

/// A trick (script `script`) `n` updates on its channel.
fn trick_at(script: u16, n: usize) -> crate::Action {
    let mut a = crate::trick::pressed(script, 1);
    if let crate::Action::Trick { play, .. } = &mut a {
        let steps = crate::trick::SCRIPTS[usize::from(script)].steps;
        for _ in 0..n {
            play.run.step(steps, 0.0);
        }
    }
    a
}

#[test]
fn the_counter_waits_out_the_tricks_blend() {
    // `FUN_00107710` (raw `0x10793c..0x10795c`): against a juking handler
    // whose channel 0 blends (flag 0x200000, the trick's opening `8e`)
    // the defender does not roll and looks again next update; once the
    // blend is over he rolls and waits `fps`. The first attract capture's
    // counters against tricks all came 6 or more updates in.
    let blending = trick_at(0, 2);
    let after = trick_at(0, 14);
    let run = |a: &crate::Action| match a {
        crate::Action::Trick { play, .. } => play.run.blend_counter(),
        _ => None,
    };
    assert!(run(&blending).is_some());
    assert!(run(&after).is_none());
    for (action, rolls) in [(blending, false), (after, true)] {
        let mut w = front_court(4);
        for p in &mut w.players {
            p.ratings.steals = 0.0;
        }
        step_all(&mut w);
        let spot = w.players[0].position;
        let mut timers = Vec::new();
        for _ in 0..40 {
            quiet_playbook(&mut w);
            w.players[0].position = spot;
            w.players[0].speed = 0.0;
            w.players[0].heading = std::f32::consts::FRAC_PI_2;
            w.players[0].action = action;
            w.set_turbo(3, 1.0);
            w.players[3].action = crate::Action::None;
            step_all(&mut w);
            let pressed = w.ai.brains[3].pad.buttons & pad::bits::TWO_TURBOS != 0;
            if let Some(Node {
                act: Act::Defense(d),
                ..
            }) = w.ai.brains[3].queue.first()
            {
                timers.push(if pressed { -1 } else { d.counter_timer });
            }
        }
        let looked = timers.iter().any(|&t| t == 60 || t == -1);
        assert_eq!(looked, rolls, "{timers:?}");
        if !rolls {
            assert!(timers.iter().skip(8).all(|&t| t == 0), "{timers:?}");
        }
    }
}

#[test]
fn everyone_is_busy_while_a_pass_to_himself_is_in_the_air() {
    // `FUN_00181508` (raw `0x181608..0x181634`): with the ball passed
    // (state 4) and its last holder (ball+0xfc) its target (+0x100), every
    // actor answers busy, so the playbook's checks wait out an Off the
    // Heezay; a pass to a teammate leaves them free.
    let mut w = front_court(4);
    step_all(&mut w);
    w.ball.state = BallState::Pass;
    let pass = |to: usize, via: crate::ball::Via| crate::ball::Pass {
        from: 0,
        to,
        tried: 0,
        via,
        catch: crate::ball::NO_CATCH,
        since: 0,
    };
    w.ball.pass = Some(pass(0, crate::ball::Via::Head { defender: 3 }));
    let (busy, _) = with_ctx(&w, |ctx| {
        (0..6).map(|i| offence::busy(ctx, i)).collect::<Vec<_>>()
    });
    assert_eq!(busy, vec![true; 6]);
    w.ball.pass = Some(pass(1, crate::ball::Via::Direct));
    let (busy, _) = with_ctx(&w, |ctx| {
        (0..6).map(|i| offence::busy(ctx, i)).collect::<Vec<_>>()
    });
    assert_eq!(busy, vec![false; 6]);
}

#[test]
fn the_defender_a_heezay_goes_off_looks_to_counter_in_its_windup() {
    // `FUN_00107710`: while a trick pass to himself is under way
    // (game+0x160 not -2; the ball's holder, ball+0xf4, still the passer)
    // the defender it goes off (game+0x148) rolls `skill(10, 15, 35, 50)`
    // and waits `fps`; the others do not. The first attract capture's six
    // counters with the ball in such a held windup were all its defender's.
    let mut w = front_court(4);
    step_all(&mut w);
    quiet_playbook(&mut w);
    w.players[0].heading = std::f32::consts::FRAC_PI_2;
    for j in 3..6 {
        w.set_turbo(j, 1.0);
    }
    // Every defender looks this update.
    for j in 3..6 {
        if let Some(Node {
            act: Act::Defense(d),
            ..
        }) = w.ai.brains[j].queue.first_mut()
        {
            d.counter_timer = 0;
        }
    }
    assert!(w.computer_trick_pass(0, 0, Some(3), [0.0; 3]));
    step_all(&mut w);
    assert_eq!(w.ball.state, BallState::Held(0));
    assert!(
        w.windup
            .is_some_and(|p| p.style == crate::PassStyle::Heezay)
    );
    let timer = |w: &World, j: usize| match w.ai.brains[j].queue.first() {
        Some(Node {
            act: Act::Defense(d),
            ..
        }) => d.counter_timer,
        _ => -1,
    };
    let pressed = w.ai.brains[3].pad.buttons & pad::bits::TWO_TURBOS != 0;
    assert!(pressed || timer(&w, 3) == 60, "{}", timer(&w, 3));
    for j in [4, 5] {
        assert_ne!(timer(&w, j), 60, "defender {j}");
    }
}

#[test]
fn a_released_self_pass_has_no_holder_for_a_defensive_counter() {
    // Attract captures: 73/47/111 self-pass windup samples keep the
    // passer in ball+0xf4; all 227/216/486 released-flight samples have0.
    let mut w = front_court(4);
    step_all(&mut w);
    quiet_playbook(&mut w);
    w.players[0].heading = std::f32::consts::FRAC_PI_2;
    assert!(w.computer_trick_pass(0, 0, Some(3), [0.0; 3]));
    while w.windup.is_some() {
        quiet_playbook(&mut w);
        for j in 3..6 {
            if let Some(Node {
                act: Act::Defense(d),
                ..
            }) = w.ai.brains[j].queue.first_mut()
            {
                d.counter_timer = 1000;
            }
        }
        step_all(&mut w);
    }
    assert_eq!(w.ball.state, BallState::Pass);
    assert!(
        w.ball
            .pass
            .is_some_and(|p| matches!(p.via, crate::ball::Via::Head { .. }))
    );
    w.players[0].position = [10.0, 0.0];
    w.players[0].heading = std::f32::consts::FRAC_PI_2;
    w.players[3].position = [14.0, 0.0];
    for j in 3..6 {
        w.players[j].action = crate::Action::None;
        w.set_turbo(j, 1.0);
        if let Some(Node {
            act: Act::Defense(d),
            ..
        }) = w.ai.brains[j].queue.first_mut()
        {
            d.counter_timer = 0;
        }
    }
    step_all(&mut w);
    for j in 3..6 {
        let Some(Node {
            act: Act::Defense(d),
            ..
        }) = w.ai.brains[j].queue.first()
        else {
            panic!("defender {j} lost its defense node");
        };
        assert_eq!(
            d.counter_timer, 7,
            "defender {j} still sees the released passer"
        );
        assert_eq!(w.ai.brains[j].pad.buttons & pad::bits::TWO_TURBOS, 0);
    }
}

#[test]
fn the_computer_marker_becomes_the_pad_the_ai_builds() {
    let mut w = three_on_three(4);
    assert!(input(&w, 0).is_computer());
    let mut moved = false;
    for _ in 0..240 {
        step_all(&mut w);
        moved |= w.previous_inputs.iter().any(|i| i.stick != [0, 0]);
    }
    // The AI's state lives in the world, one brain per player, and what
    // the players last did is the pad the AI built, still the computer's
    // (`FUN_00172220`), its buttons the pad's.
    assert_eq!(w.ai.brains.len(), 6);
    assert!(w.previous_inputs.iter().all(|i| i.is_computer()));
    assert!(moved, "the computer's sticks moved the players");
    assert!(w.ai.brains.iter().any(|b| !b.queue.is_empty()));
    // A round trip through bytes keeps every word.
    let back = State::decode(&w.ai.encode()).unwrap();
    assert_eq!(back, w.ai);
}

#[test]
fn without_computer_players_the_ai_neither_runs_nor_draws() {
    let mut a = three_on_three(4);
    // In play: a restart is the computer's whoever plays.
    a.phase = Phase::Live;
    let mut b = a.clone();
    let none = vec![Input::default(); 6];
    a.step(&none);
    // Player 0 alone on the computer: the AI runs and draws.
    let mut some = none.clone();
    some[0] = Input::computer();
    b.step(&some);
    assert_eq!(a.ai, State::default());
    assert_ne!(b.ai, State::default());
}

#[test]
fn the_pad_builder_adds_the_originals_trick_combo() {
    // `FUN_00182280` with a juke request for a 99 Handles handler (trick
    // level 4): 80% to go (one draw), 50% for the turbo bit (one draw),
    // then a combo of level 2..4 (two draws: the level, then the row).
    let mut w = half_court(20.0, 3.0);
    w.players[0].ratings.handles = 1.0;
    let (buttons, after) = with_ctx(&w, |ctx| {
        let mut b = Brain::default();
        offence::build(ctx, &mut b, 0, pad::bits::JUKE);
        b.pad.buttons
    });
    let mut r = w.rng.clone();
    let go = r.next_u32() % 100 < 80;
    let turbo = r.next_u32() % 100 < 50;
    let mut expect = pad::bits::JUKE;
    if turbo {
        expect |= pad::bits::TURBO;
    }
    if go {
        let level = 2 + (r.next_u32() % 3) as usize;
        let rows: Vec<usize> = (0..11)
            .filter(|&k| crate::player::TRICK_SLOT_LEVELS[k] as usize == level)
            .collect();
        expect |= crate::player::TRICK_SLOT_MASKS[rows[r.next_u32() as usize % rows.len()]];
    }
    assert_eq!(buttons, expect);
    assert_eq!(after, r, "the same draws, in order");
}

#[test]
fn a_full_meter_turns_the_shot_into_a_gamebreaker() {
    let mut w = half_court(20.0, 6.0);
    w.teams[0].meter = crate::street::METER_FULL;
    let (buttons, after) = with_ctx(&w, |ctx| {
        let mut b = Brain::default();
        offence::build(ctx, &mut b, 0, pad::bits::SHOOT);
        b.pad.buttons
    });
    assert_eq!(buttons, pad::bits::SHOOT | pad::bits::TWO_TURBOS);
    assert_eq!(after, w.rng, "no draw");
    // On the pad that is the shoot button with two turbos.
    let input = with_ctx(&w, |ctx| {
        let pad = Pad {
            buttons,
            target: -1,
            ..Default::default()
        };
        pad::to_input(ctx, 0, &pad)
    })
    .0;
    assert!(input.held(crate::button::SHOOT));
    assert!(input.turbos() >= 2);
}

#[test]
fn the_levels_follow_the_disc_tables() {
    // `0x43f080` and `0x43f0c0`.
    assert_eq!(offence::trick_level(99), 4);
    assert_eq!(offence::trick_level(61), 3);
    assert_eq!(offence::trick_level(41), 2);
    assert_eq!(offence::trick_level(0), 1);
    assert_eq!(offence::dunk_level(81), 4);
    assert_eq!(offence::dunk_level(21), 1);
    assert_eq!(offence::dunk_level(20), 0);
}

#[test]
fn a_faster_stick_waits_nine_frames_and_a_slower_one_does_not() {
    // `FUN_0013af58` with `0x4b7614` = fps × 0.15.
    let mut w = half_court(20.0, 6.0);
    w.tick = 100;
    let mut b = Brain::default();
    with_ctx(&w, |ctx| {
        b.stop(ctx, 0);
        b.steer(ctx, 0, 256, 3, 0, None);
    });
    assert_eq!(b.pad.strength, pad::SPEEDS[3]);
    with_ctx(&w, |ctx| b.steer(ctx, 0, 256, 1, 0, None));
    assert_eq!(b.pad.strength, pad::SPEEDS[1], "down at once");
    w.tick = 109;
    with_ctx(&w, |ctx| b.steer(ctx, 0, 256, 3, 0, None));
    assert_eq!(b.pad.strength, pad::SPEEDS[1], "up only after 9 frames");
    w.tick = 110;
    with_ctx(&w, |ctx| b.steer(ctx, 0, 256, 3, 0, None));
    assert_eq!(b.pad.strength, pad::SPEEDS[3]);
}

#[test]
fn turns_are_taken_fourteen_units_at_a_time() {
    // `FUN_0013b598`: a turn of 15..198 units moves the stick 14.
    let mut w = half_court(20.0, 6.0);
    w.tick = 100;
    let mut b = Brain::default();
    b.motion.history = [100; pad::HISTORY];
    with_ctx(&w, |ctx| b.steer(ctx, 0, 180, 1, 0, None));
    assert_eq!(b.pad.angle, 114);
    // Small turns go straight through.
    b.motion.history = [100; pad::HISTORY];
    with_ctx(&w, |ctx| b.steer(ctx, 0, 110, 1, 0, None));
    assert_eq!(b.pad.angle, 110);
}

#[test]
fn the_difficulty_applies_only_against_a_person() {
    let mut w = half_court(20.0, 6.0);
    w.rules.difficulty = crate::Difficulty::Hard;
    let (d, _) = with_ctx(&w, |ctx| skill::difficulty(ctx, 0));
    assert_eq!(d, 1, "computer against computer plays Medium");
    let (d, _) = with_ctx(&w, |ctx| {
        ctx.human = [false, true];
        skill::difficulty(ctx, 0)
    });
    assert_eq!(d, 2);
}

#[test]
fn an_open_handler_near_the_rim_drives_and_dunks() {
    // Nobody near: the playbook starts a play, and whichever it is
    // (a drive, or an isolation that drives once inside), the handler
    // ends at the rim with the dunk the pad builder picks.
    for seed in 0..4 {
        let mut w = half_court(9.0, 30.0);
        w.rng = Rng::new(seed);
        w.players[0].ratings.dunks = 0.9;
        w.players[0].ratings.threes = 0.2;
        let mut dunked = false;
        for _ in 0..6 * crate::TICK_HZ {
            let inputs = [Input::computer(), Input::default()];
            w.step(&inputs);
            dunked |= w
                .events
                .iter()
                .any(|e| matches!(e, crate::Event::Dunk { player: 0, .. }));
        }
        assert!(dunked, "seed {seed}");
    }
}

#[test]
fn a_defender_stands_between_his_man_and_the_basket() {
    // Team 1 defends hoop 0 (the -x one, which team 0 attacks): the
    // defender, starting behind his man, gets between him and the basket,
    // close on the ball (the on-ball gap at 25 out is about 4).
    let mut w = half_court(25.0, 12.0);
    let rim = hoops()[0].rim;
    let to_rim = |p: [f32; 2]| ((p[0] - rim[0]).powi(2) + (p[1] - rim[2]).powi(2)).sqrt();
    let mut between = false;
    for _ in 0..3 * crate::TICK_HZ {
        let inputs = [Input::default(), Input::computer()];
        w.step(&inputs);
        let (man, d) = (w.players[0].position, w.players[1].position);
        let gap = ((d[0] - man[0]).powi(2) + (d[1] - man[1]).powi(2)).sqrt();
        between |= to_rim(d) < to_rim(man) && gap < 6.0;
    }
    assert!(between);
}

/// Under the native turbo rates (the bar empties in 5 s held and refills
/// in 7.5 s) no seed's computer players stall: within 6000 updates each
/// game has had a shot or dunk and a pass from the hand.
#[test]
fn no_seed_stalls_without_a_shot_or_a_pass() {
    for seed in 0..12 {
        let mut w = three_on_three(seed);
        let (mut shots, mut passes) = (0, 0);
        for _ in 0..6000 {
            step_all(&mut w);
            for e in &w.events {
                match e {
                    crate::Event::Shot { .. } | crate::Event::Dunk { .. } => shots += 1,
                    crate::Event::Pass { .. } => passes += 1,
                    _ => {}
                }
            }
        }
        assert!(
            shots > 0 && passes > 0,
            "seed {seed}: {shots} shots, {passes} passes"
        );
    }
}

/// Three on three in team 0's front court: player 0 holds the ball 28
/// out from the rim it attacks (hoop 0, -x), his teammates nearer on the
/// wings, each defender between his man and the rim. Team 0 has just
/// brought the ball up: the game's phase is 4 (the AI's first frame runs
/// the fast break) until the step carries the ball to its holder, over
/// half court (`FUN_0013e4c8` case 4), so the AI sees the front court
/// from the second frame.
fn front_court(seed: u32) -> World {
    let rim = hoops()[0].rim;
    let at = |out: f32, z: f32| [rim[0] - out, z];
    let mut players = Vec::new();
    for (out, z) in [(28.0, 0.0), (14.0, -12.0), (14.0, 12.0)] {
        players.push(Player::new(0, at(out, z), std::f32::consts::FRAC_PI_2));
    }
    for (out, z) in [(24.0, 0.0), (11.0, -9.0), (11.0, 9.0)] {
        players.push(Player::new(1, at(out, z), -std::f32::consts::FRAC_PI_2));
    }
    let mut w = World::new(hoops(), players, Rules::default(), seed);
    w.phase = Phase::Live;
    w.possession = 0;
    w.ball.hold(0);
    w.game_phase = crate::game_phase::GamePhase {
        phase: crate::game_phase::phase::BACK_COURT,
        possession: 0,
        ..Default::default()
    };
    w
}

fn playbook(w: &World) -> &playbook::Playbook {
    &w.ai.coaches[0].playbook
}

fn front(w: &World, i: usize) -> &'static str {
    w.ai.brains[i].queue.first().map_or("-", |n| name(&n.act))
}

/// Step until the playbook runs a play (within 3 s), returning it.
fn until_play(w: &mut World) -> Option<i32> {
    for _ in 0..180 {
        step_all(w);
        let k = playbook(w).play.kind;
        if k != plays::kind::NONE {
            return Some(k);
        }
    }
    None
}

#[test]
fn everyone_waits_for_the_opening_toss() {
    // Phase 1: the tip strategy (`FUN_0012e4a0`) queues "wait for toss"
    // for all six; the team frame draws its timer every frame.
    let mut w = three_on_three(5);
    w.start_opening_toss();
    step_all(&mut w);
    assert_eq!(w.ai.game.phase, 1);
    for i in 0..6 {
        assert_eq!(front(&w, i), "toss", "player {i}");
    }
    for _ in 0..600 {
        step_all(&mut w);
        for c in &w.ai.coaches {
            assert!((0..=17).contains(&c.timer), "timer {}", c.timer);
        }
    }
}

#[test]
fn the_playbook_starts_with_its_drawn_marks() {
    // `FUN_00127318`: the shot clock mark is 1..4, the penetration and
    // post-cut marks a draw of 0..4 over the skill's frames, and the
    // pass check waits 18 frames before the first play.
    for seed in 0..8 {
        let mut w = front_court(seed);
        step_all(&mut w);
        step_all(&mut w);
        let pb = playbook(&w);
        assert_eq!(w.ai.coaches[0].strategy, team::strategy::PLAYBOOK);
        assert!((1..=4).contains(&pb.shot_mark), "{}", pb.shot_mark);
        assert!((9..9 + 40 + 5).contains(&pb.pen_mark), "{}", pb.pen_mark);
        assert!((60..130 + 5).contains(&pb.cut_mark), "{}", pb.cut_mark);
        assert_eq!(pb.play.kind, plays::kind::NONE);
        // The roles: the holder 1, the lower other slot 2, the higher 3.
        assert_eq!(w.ai.brains[0].role, 1);
        assert_eq!(w.ai.brains[1].role, 2);
        assert_eq!(w.ai.brains[2].role, 3);
    }
}

#[test]
fn the_playbook_picks_among_its_plays() {
    // `FUN_001281c8`: with a teammate the star (`FUN_0012a7d0`, the best
    // overall rating), the plays that suit him (`FUN_00128708` adding
    // weight by his ratings) come up.
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..24 {
        let mut w = front_court(seed);
        let r = &mut w.players[1].ratings;
        (r.shots, r.threes, r.dunks, r.handles) = (0.8, 0.8, 0.8, 0.8);
        if let Some(k) = until_play(&mut w) {
            assert!((0..16).contains(&k));
            seen.insert(k);
        }
    }
    assert!(seen.len() >= 4, "{seen:?}");
}

#[test]
fn the_star_penetrates_at_once_only_ahead_of_his_teammates() {
    // `FUN_001287e0` (raw `0x1287e0..0x1288bc`): the holder penetrates at
    // once when he is in the front court and both teammates are still in
    // the back court (the star always, anyone else `skill(0, 10, 10,
    // 40)`%). The loop leaves for the lane test as soon as a teammate is
    // in the front court; the port had it the other way round, so a star
    // holder in a set offence always drove. The holder (0) is the star
    // here (the first of equal ratings).
    let mut w = front_court(1);
    for j in [1, 2] {
        w.players[j].position[0] = -10.0;
    }
    assert_eq!(until_play(&mut w), Some(plays::kind::PENETRATION));
    // With both teammates in the front court he draws among his plays.
    let mut other = 0;
    for seed in 0..16 {
        let mut w = front_court(seed);
        other += usize::from(until_play(&mut w).is_some_and(|k| k != plays::kind::PENETRATION));
    }
    assert!(other >= 8, "{other} of 16 not a penetration");
}

#[test]
fn dunks_only_always_penetrates() {
    // The subclass's `FUN_00124ed8` answers yes for any holder: every play
    // is a penetration, and the handler drives.
    for seed in 0..4 {
        let mut w = front_court(seed);
        w.rules.dunks_only = true;
        assert_eq!(until_play(&mut w), Some(plays::kind::PENETRATION));
        assert_eq!(front(&w, 0), "drive");
    }
}

#[test]
fn a_forced_play_starts_its_behaviours() {
    // `[0x2c]`: each play's start, through the production step.
    use plays::kind::*;
    type Case = (i32, &'static [&'static str], &'static [&'static str]);
    let cases: [Case; 7] = [
        (PENETRATION, &["drive"], &[]),
        (PULL_UP, &["iso", "drive", "pass"], &[]),
        // The setup's carrier move turns into the patient drive in the
        // same frame when the handler already stands beyond the arc with
        // the big on his way out.
        (
            PEN_AND_KICK,
            &["carrier move", "pass", "drive"],
            &["cut move", "post cut"],
        ),
        (ROTATE, &["carrier move"], &[]),
        (POST_AND_KICK, &["carrier move"], &["post cut", "cut move"]),
        (CUTTER, &["carrier move"], &["cut move", "perim"]),
        (
            ISOLATION,
            &["carrier move", "drive", "pause", "iso"],
            &["perim", "cut move"],
        ),
    ];
    for (k, handler, others) in cases {
        let mut w = front_court(2);
        step_all(&mut w);
        w.ai.coaches[0].playbook.forced = k;
        assert_eq!(until_play(&mut w), Some(k), "play {k}");
        let h = front(&w, 0);
        assert!(handler.contains(&h), "play {k}: handler {h}");
        for j in [1, 2] {
            let f = front(&w, j);
            assert!(
                others.is_empty() || others.contains(&f),
                "play {k}: {j} {f}"
            );
        }
    }
}

#[test]
fn the_shot_check_shoots_at_the_shot_clock_mark() {
    // `FUN_001290b8`: with the shot clock at or under the drawn mark the
    // handler shoots, whatever the play.
    let mut w = front_court(3);
    step_all(&mut w);
    w.shot_clock = 0.5;
    let mut shot = false;
    for _ in 0..30 {
        step_all(&mut w);
        shot |= front(&w, 0) == "shoot"
            || w.events
                .iter()
                .any(|e| matches!(e, crate::Event::Shot { player: 0, .. }));
    }
    assert!(shot);
}

#[test]
fn the_pass_check_finds_the_open_teammate() {
    // `FUN_00128b30`: a covered handler (openness 4 or less) passes to the
    // teammate who has stood open longest and is open 6 or more, in the
    // front court beyond 6 of him.
    let mut w = front_court(6);
    let rim = hoops()[0].rim;
    w.players[3].position = [rim[0] - 29.5, 0.0];
    // Player 2's defender stays with him; player 1's is far away.
    w.players[4].position = [rim[0] - 40.0, -24.0];
    // On a drive, which every check may interrupt.
    w.ai.coaches[0].playbook.forced = plays::kind::PENETRATION;
    let mut passed = false;
    for _ in 0..240 {
        step_all(&mut w);
        passed |= w
            .events
            .iter()
            .any(|e| matches!(e, crate::Event::Pass { from: 0, to: 1, .. }));
        if passed {
            break;
        }
    }
    assert!(passed);
}

#[test]
fn a_leaper_asks_for_the_alley_oop() {
    // The alley-oop leaper (`FUN_001001c0`) holds turbo and presses shoot
    // every other frame; without the ball that is the leap's press
    // (`FUN_0015a060`, [`crate::leap`]).
    let mut w = front_court(4);
    let rim = hoops()[0].rim;
    w.players[1].position = [rim[0] - 8.0, 4.0];
    step_all(&mut w);
    w.ai.brains[1].queue.clear();
    w.ai.brains[1].push(Act::Leaper(moves::Leaper::new(false)));
    let mut leapt = false;
    for _ in 0..20 {
        step_all(&mut w);
        leapt |= w.players[1].leaping();
    }
    assert!(leapt);
}

#[test]
fn rebound_spots_ring_the_rim() {
    // `FUN_0012d790`: once a shot is up, every player's spot is 3..9 from
    // the rim the offence attacks.
    let mut w = three_on_three(3);
    for _ in 0..20_000 {
        // The offence the spots are dealt for, before the update (which
        // may end the shot and turn the possession).
        let off = w.possession.min(1);
        step_all(&mut w);
        if w.ai.game.phase == 3 && w.ai.game.spots_tick == w.tick as i64 - 1 {
            let rim = w.rim_spot(crate::target_hoop(off));
            // 3.0 out along a 10-bit table direction can measure a hair
            // under 3.
            for s in w.ai.game.spots {
                let d = court::dist(s, rim);
                assert!((2.999..9.01).contains(&d), "{d}");
            }
            return;
        }
    }
    panic!("no shot");
}

#[test]
fn the_fast_break_deals_three_lanes() {
    // `FUN_00125c70`: in the back court the side with the ball runs three
    // different lanes (`0x3cddb0`).
    let mut w = three_on_three(3);
    let mut checked = 0;
    for _ in 0..20_000 {
        step_all(&mut w);
        let t = w.possession.min(1);
        if w.ai.game.phase == 4
            && w.ai.coaches[t].strategy == team::strategy::FAST_BREAK
            && w.ai.coaches[t].fast_break.lanes
        {
            let lanes: std::collections::BTreeSet<u8> = (0..6)
                .filter(|&j| w.players[j].team == t)
                .map(|j| w.ai.brains[j].lane)
                .collect();
            assert_eq!(lanes.len(), 3, "{lanes:?}");
            checked += 1;
            if checked > 30 {
                return;
            }
        }
    }
    assert!(checked > 0);
}

/// Every restart of a computer game is the original's: all six walk to
/// their spots and the inbounder passes in; the rewrite's limit
/// (`tuning::RESTART_LIMIT`) never has to put the ball in play.
#[test]
fn every_restart_of_a_computer_game_ends_with_an_inbound_pass() {
    for seed in 0..6 {
        let mut w = three_on_three(seed);
        let (mut restarts, mut longest, mut ticks) = (0, 0, 0);
        for _ in 0..20_000 {
            step_all(&mut w);
            if w.restarting() {
                ticks += 1;
                longest = longest.max(ticks);
            } else {
                ticks = 0;
            }
            for e in &w.events {
                if let crate::Event::Inbound { player } = *e {
                    restarts += 1;
                    assert_eq!(w.ball.state, crate::BallState::Pass, "seed {seed}");
                    assert_eq!(w.ball.pass.map(|p| p.from), Some(player));
                }
            }
            if matches!(w.phase, Phase::Over { .. }) {
                break;
            }
        }
        println!("seed {seed}: {restarts} restarts, longest {longest} updates");
        assert!(restarts > 3, "seed {seed}: {restarts} restarts");
        assert!(
            longest < 12 * 60,
            "seed {seed}: a restart of {longest} updates"
        );
    }
}

#[test]
#[ignore = "prints a forced pick play for reading"]
fn trace_a_pick_play() {
    let k = std::env::var("AI_PLAY").map_or(6, |s| s.parse().unwrap_or(6));
    let seed = std::env::var("AI_TRACE_SEED").map_or(2, |s| s.parse().unwrap_or(2));
    let mut w = front_court(seed);
    w.players[0].ratings.handles = 0.9;
    step_all(&mut w);
    w.ai.coaches[0].playbook.forced = k;
    for t in 0..400 {
        step_all(&mut w);
        let pb = playbook(&w);
        let acts: Vec<String> = (0..6)
            .map(|i| {
                let p = &w.players[i];
                format!(
                    "{}:{}[{:.1},{:.1}]s{}b{:x}{}",
                    i,
                    front(&w, i),
                    p.position[0],
                    p.position[1],
                    w.ai.brains[i].pad.strength,
                    w.ai.brains[i].pad.buttons,
                    match p.action {
                        crate::Action::None => "",
                        crate::Action::Stumbling { .. } => "!stumble",
                        crate::Action::Trick { .. } => "!trick",
                        _ => "!act",
                    }
                )
            })
            .collect();
        println!(
            "t{t} play{} v{:?} h{} ball{:?} | {}",
            pb.play.kind,
            pb.play.v,
            pb.play.handler,
            w.ball.state,
            acts.join(" ")
        );
        for e in &w.events {
            println!("  {t}: {e:?}");
        }
    }
}

/// Hold the playbook off: its pass wait and its shot hold keep it from
/// choosing a play or calling a shot, pass, penetration, post cut or
/// alley-oop (`FUN_001281c8` and the checks all wait on them).
fn quiet_playbook(w: &mut World) {
    let pb = &mut w.ai.coaches[0].playbook;
    pb.pass_wait = 1000;
    pb.shot_hold = 1000;
}

/// Queue a behaviour on player `i` as a strategy does (`FUN_0013aaa8`).
fn give(w: &mut World, i: usize, f: impl FnOnce(&mut Ctx) -> Act) {
    let (act, rng) = with_ctx(w, f);
    w.rng = rng;
    w.ai.brains[i].queue.clear();
    w.ai.brains[i].push(act);
}

fn front_act(w: &World, i: usize) -> Option<&Act> {
    w.ai.brains[i].queue.first().map(|n| &n.act)
}

#[test]
fn the_pick_and_roll_sets_a_screen_for_the_handler() {
    // `FUN_0011e900` → `FUN_0011eac0`: a handler with Handles over 49 runs
    // "run pick play" to a spot of his zone 27.25 from the rim; the nearer
    // teammate (+0x2c) cuts to the zone beside his and later sets the pick
    // ("set pick", `FUN_0011ef18`), the other goes to the perimeter.
    let mut set = 0;
    for seed in 0..8 {
        let mut w = front_court(seed);
        w.players[0].ratings.handles = 0.9;
        step_all(&mut w);
        w.ai.coaches[0].playbook.forced = plays::kind::PICK_AND_ROLL;
        assert_eq!(until_play(&mut w), Some(plays::kind::PICK_AND_ROLL));
        let play = playbook(&w).play;
        let screener = play.v[1];
        let third = play.v[2];
        assert!(
            (1..=2).contains(&screener) && (1..=2).contains(&third) && screener != third,
            "seed {seed}: {:?}",
            play.v
        );
        let Some(Act::RunPick(run)) = front_act(&w, 0) else {
            panic!("seed {seed}: handler {}", front(&w, 0));
        };
        let rim = w.rim_spot(crate::target_hoop(0));
        let d = court::dist(run.spot, rim);
        assert!((d - 27.25).abs() < 1e-3, "seed {seed}: spot {d} out");
        assert!(
            ["cut move", "post cut"].contains(&front(&w, screener as usize)),
            "seed {seed}: screener {}",
            front(&w, screener as usize)
        );
        for _ in 0..300 {
            step_all(&mut w);
            if front(&w, screener as usize) == "set pick" {
                set += 1;
                break;
            }
            if playbook(&w).play.kind != plays::kind::PICK_AND_ROLL {
                break;
            }
        }
    }
    assert!(set >= 3, "{set} screens set");
}

#[test]
fn a_set_pick_goes_round_the_defender_waits_and_steps_in() {
    // `FUN_00118a30`: the screener runs to the spot on the circle round the
    // rim through the holder's defender, 20 units round on the holder's
    // side; there he stands `fps × 0.25` frames (pressing `0x400000`),
    // turns to the defender for the last two, then steps into him
    // (`0x4000000`) when within 4.5, or rolls.
    let mut stance = 0;
    let mut stepped = 0;
    for seed in 0..6 {
        let mut w = front_court(seed);
        let rim = hoops()[0].rim;
        // The handler stands still at the top with his defender 3 in
        // front; the screener comes from the wing.
        w.players[3].position = [rim[0] - 24.0, 0.0];
        w.players[1].position = [rim[0] - 20.0, -9.0];
        for p in &mut w.players {
            p.ratings.dunks = 0.0;
            p.ratings.steals = 0.0;
        }
        step_all(&mut w);
        step_all(&mut w);
        quiet_playbook(&mut w);
        give(&mut w, 1, |ctx| {
            Act::SetPick(picks::SetPick::new(ctx, false))
        });
        let mut waited_at = None;
        for _ in 0..240 {
            w.set_turbo(0, 0.0);
            quiet_playbook(&mut w);
            // The defender's pick sag (`[0x20]`: he goes over the screen,
            // 5 nearer than the handler) is held off: this test is the
            // screener's, against a defender who stays.
            if let Some(Node {
                act: Act::Defense(d),
                ..
            }) = w.ai.brains[3].queue.first_mut()
            {
                d.pick_sag = false;
                d.gamble_timer = 1000;
            }
            step_all(&mut w);
            let pad = w.ai.brains[1].pad.buttons;
            if pad & pad::bits::PICK_STANCE != 0 && waited_at.is_none() {
                stance += 1;
                waited_at = Some(w.players[1].position);
                // At the spot (within 1.4, give or take the step he
                // took) or within 3 of the defender.
                let d = w.players[3].position;
                let near_def = court::dist(w.players[1].position, d) < 4.0;
                let r = court::dist(d, w.rim_spot(crate::target_hoop(0)));
                let on_circle =
                    (court::dist(w.players[1].position, w.rim_spot(crate::target_hoop(0))) - r)
                        .abs()
                        < 2.5;
                assert!(near_def || on_circle, "seed {seed}: waited off the spot");
                assert_eq!(w.ai.brains[1].pad.strength, 0, "seed {seed}: moving");
            }
            if pad & pad::bits::PICK != 0 {
                stepped += 1;
                assert!(
                    court::dist(w.players[1].position, w.players[3].position) <= 4.5,
                    "seed {seed}"
                );
            }
            if front(&w, 1) == "roll" {
                break;
            }
        }
    }
    assert!(stance >= 3, "{stance} stances");
    assert!(stepped >= 1, "{stepped} picks");
}

#[test]
fn a_set_pick_rolls_when_its_defender_goes_down() {
    // `0x118b38`: a defender down (animation flag 0x40) ends the pick; the
    // screener rolls (`FUN_00117e80(ai, -way)`): sprinting (turbo pressed),
    // he steers a quarter turn off the line to the rim, closing the turn 5
    // units a frame, until `fps × 1.5` frames or within 3 of the rim.
    let mut w = front_court(3);
    for p in &mut w.players {
        p.ratings.dunks = 0.0;
    }
    step_all(&mut w);
    step_all(&mut w);
    quiet_playbook(&mut w);
    give(&mut w, 1, |ctx| {
        Act::SetPick(picks::SetPick::new(ctx, false))
    });
    w.players[3].stumble(2.0);
    quiet_playbook(&mut w);
    step_all(&mut w);
    assert!(
        matches!(
            w.ai.brains[1].queue.get(1).map(|n| &n.act),
            Some(Act::Roll(_))
        ),
        "{:?}",
        w.ai.brains[1]
            .queue
            .iter()
            .map(|n| name(&n.act))
            .collect::<Vec<_>>()
    );
    let mut turns = Vec::new();
    let mut before = 256;
    for _ in 0..120 {
        quiet_playbook(&mut w);
        step_all(&mut w);
        match front_act(&w, 1) {
            Some(Act::Roll(r)) => {
                turns.push(r.turn);
                // Turbo while turning; straight at the rim without.
                let turbo = w.ai.brains[1].pad.buttons & pad::bits::TURBO != 0;
                assert_eq!(turbo, before != 0);
                before = r.turn;
            }
            _ => break,
        }
    }
    assert!(turns.len() > 10, "{}", turns.len());
    assert!(turns.len() <= 91, "{}", turns.len());
    assert_eq!(turns[0].abs(), 256 - 5);
    assert!(
        turns
            .windows(2)
            .all(|t| t[1].abs() == (t[0].abs() - 5).max(0))
    );
}

#[test]
fn the_pick_off_ball_frees_a_cutter_with_a_screen() {
    // `FUN_0011f628` / `FUN_0011f9c0`: once the handler stands at the top,
    // the perimeter man goes to his wing and the post man to his block;
    // then the perimeter man cuts in by the net while the post man screens
    // his defender ("pick off ball", `FUN_001199e0`).
    let mut screens = 0;
    for seed in 0..8 {
        let mut w = front_court(seed);
        step_all(&mut w);
        w.ai.coaches[0].playbook.forced = plays::kind::PICK_OFF_BALL;
        if until_play(&mut w) != Some(plays::kind::PICK_OFF_BALL) {
            continue;
        }
        for _ in 0..300 {
            step_all(&mut w);
            let play = playbook(&w).play;
            if play.kind != plays::kind::PICK_OFF_BALL {
                break;
            }
            let post = play.post as usize;
            if let Some(Act::PickOffBall(_)) = front_act(&w, post) {
                screens += 1;
                let cutter = play.perim as usize;
                assert_eq!(front(&w, cutter), "cut move", "seed {seed}");
                assert!(w.ai.game.phase == 5, "seed {seed}");
                break;
            }
        }
    }
    assert!(screens >= 2, "{screens} screens");
}

#[test]
fn a_screen_off_the_ball_ends_in_a_post_cut_or_the_perimeter() {
    // `FUN_00119a60`: the screener waits `fps × 0.2` frames at 2.5 from
    // his teammate's defender (on the teammate's line, `FUN_0011a008`),
    // steps in, and is done; he then post-cuts to the far block (one
    // draw for deep) or, with the third man headed into the paint, goes
    // to the perimeter. (Its done test counts the frames the teammate
    // and his defender move away from a screener more than 5 from the
    // defender; `FUN_00119d28`'s test is always true, so a screener who
    // starts farther than 5 off gives up in five frames: this one starts
    // 4.5 off.)
    let mut w = front_court(5);
    for p in &mut w.players {
        p.ratings.dunks = 0.0;
    }
    let rim = hoops()[0].rim;
    step_all(&mut w);
    step_all(&mut w);
    w.players[1].position = [rim[0] - 13.25, 7.5];
    w.players[2].position = [rim[0] - 20.0, 12.0];
    w.players[5].position = [rim[0] - 17.0, 10.0];
    quiet_playbook(&mut w);
    give(&mut w, 1, |ctx| {
        Act::PickOffBall(picks::PickOffBall::new(ctx, Some(2)))
    });
    let mut waited = false;
    let mut followed = None;
    for _ in 0..360 {
        quiet_playbook(&mut w);
        step_all(&mut w);
        if w.ai.brains[1].pad.buttons & pad::bits::PICK_STANCE != 0 {
            waited = true;
        }
        let f = front(&w, 1);
        if f != "pick off ball" {
            followed = Some(f);
            break;
        }
    }
    assert!(waited);
    assert!(
        matches!(followed, Some("post cut" | "perim" | "post pause")),
        "{followed:?}"
    );
}

/// Team 0's handler a person (standing, then pressing `held`), everyone
/// else the computer.
fn person_step(w: &mut World, held: u16) {
    let mut inputs: Vec<Input> = (0..w.players.len()).map(|i| input(w, i)).collect();
    inputs[0] = Input::default().with(held);
    w.step(&inputs);
}

#[test]
fn a_persons_call_sends_the_nearest_teammate_to_set_a_pick() {
    // `FUN_0012ae70`: a person's strategy call outside the paint sends the
    // nearest teammate to set a pick (`FUN_0012b0c0`: "set pick", the
    // third man to the perimeter, a second before another); calling again
    // while it is under way cancels it (everyone to the perimeter, no call
    // for `fps × 0.6` frames).
    let mut w = front_court(2);
    let rim = hoops()[0].rim;
    w.players[0].position = [rim[0] - 25.0, 0.0];
    for _ in 0..3 {
        person_step(&mut w, 0);
    }
    assert_eq!(w.ai.coaches[0].strategy, team::strategy::PLAYBOOK);
    let nearest = if court::dist(w.players[0].position, w.players[1].position)
        <= court::dist(w.players[0].position, w.players[2].position)
    {
        1
    } else {
        2
    };
    person_step(&mut w, crate::button::R3);
    assert_eq!(front(&w, nearest), "set pick");
    assert_eq!(front(&w, 3 - nearest), "perim");
    assert_eq!(playbook(&w).pick, nearest as i32);
    // The call comes after the playbook's frame (`FUN_0018ac98`).
    assert_eq!(playbook(&w).human_wait, 60);
    person_step(&mut w, 0);
    person_step(&mut w, crate::button::R3);
    assert_eq!(playbook(&w).pick_wait, 36);
    for j in [1, 2] {
        assert_eq!(front(&w, j), "perim", "{j}");
    }
}

#[test]
fn a_teammate_sets_a_pick_for_a_person_on_the_move() {
    // `FUN_0012abe0`: on a side a person plays, a teammate within 15 of the
    // moving holder, ahead of where he faces, out of the paint, with the
    // holder's defender on him, sets a pick on his own.
    // (Seed 7: the AI's draws decide when the teammate's wait ends; 7 of
    // the first twelve sets it in time since the side with the ball stops
    // at once, 0, 1, 6 and 7 before.)
    let mut w = front_court(7);
    // The wings the other way round (z negated): with front_court's, the
    // teammates' fast break (its lanes read z times the side) keeps them
    // running past the pick's moment.
    for p in &mut w.players {
        p.position[1] = -p.position[1];
    }
    let rim = hoops()[0].rim;
    w.players[0].position = [rim[0] - 26.0, 0.0];
    // The defender off the holder's line: standing on it, he stops the
    // walk (a standing opponent takes none of the push) before the side's
    // wait is over, and the teammate has cut away by then. (4 off: since
    // the stand path stops him at once, 3 leaves him on the line.)
    w.players[3].position = [rim[0] - 22.0, -4.0];
    w.players[1].position = [rim[0] - 20.0, 6.0];
    let mut set = false;
    for _ in 0..120 {
        // The person walks toward the rim.
        let mut inputs: Vec<Input> = (0..w.players.len()).map(|i| input(&w, i)).collect();
        inputs[0] = stick_toward(&w, 0, [rim[0], rim[2]]);
        w.step(&inputs);
        if (1..=2).any(|j| front(&w, j) == "set pick") {
            set = true;
            break;
        }
    }
    assert!(set);
}

fn stick_toward(w: &World, i: usize, at: [f32; 2]) -> Input {
    let p = w.players[i].position;
    let (dx, dz) = (at[0] - p[0], at[1] - p[1]);
    let l = crate::math::len(dx, dz).max(1e-3);
    Input::moving([dx / l * 0.5, dz / l * 0.5])
}

#[test]
fn the_side_keeps_the_ball_from_a_full_meter() {
    // `FUN_0012b1f8`: with the keep-away flag, the other side's meter full
    // and its own not, with time on the shot clock, the holder passes to
    // the best ballhandler (`FUN_0012b380`), who runs "keep away"
    // (`FUN_0010d4c8`): standing until an opponent comes within 10, then
    // off to a spot near half court.
    let mut w = front_court(3);
    w.players[2].ratings.handles = 0.95;
    // The pass stays in his hands for its windup, where a defender's
    // swipe can still take it: a sure handler keeps it.
    w.players[0].ratings.handles = 0.95;
    w.teams[1].meter = crate::street::METER_FULL;
    step_all(&mut w);
    step_all(&mut w);
    w.ai.coaches[0].playbook.keep_away = true;
    step_all(&mut w);
    assert_eq!(front(&w, 0), "pass");
    let mut kept = false;
    for _ in 0..240 {
        step_all(&mut w);
        if front(&w, 2) == "keep away" {
            kept = true;
            assert!(w.ball.state == crate::BallState::Held(2));
            assert_eq!(playbook(&w).play.kind, plays::kind::NONE);
            break;
        }
    }
    assert!(kept);
    // Keep away to a spot near half court.
    let mut went = false;
    for _ in 0..240 {
        step_all(&mut w);
        if let Some(Act::KeepAway(k)) = front_act(&w, 2)
            && k.wait == 0
        {
            went = true;
            assert!(k.spot[0].abs() <= 25.0, "{:?}", k.spot);
            assert!([6.625, 19.875].contains(&k.spot[1].abs()), "{:?}", k.spot);
            break;
        }
    }
    assert!(went);
}

#[test]
fn a_drive_started_in_the_paint_posts_up() {
    // `FUN_001096b8`: a drive started in the paint (not patiently, the
    // meter not full) hands on to "post up" (`FUN_00115368`, three
    // draws) on its first update instead of drawing its reach; the
    // post-up makes its move within its reach of the rim and goes up (or
    // passes out) on the third frame after it.
    let mut ups = 0;
    for seed in 0..20 {
        let mut w = front_court(seed);
        let rim = hoops()[0].rim;
        w.players[0].position = [rim[0] - 9.0, 3.0];
        w.players[3].position = [rim[0] - 7.0, 2.0];
        step_all(&mut w);
        step_all(&mut w);
        quiet_playbook(&mut w);
        give(&mut w, 0, |ctx| Act::Drive(offence::Drive::new(ctx, 0)));
        quiet_playbook(&mut w);
        step_all(&mut w);
        quiet_playbook(&mut w);
        step_all(&mut w);
        assert_eq!(front(&w, 0), "post up", "seed {seed}");
        let Some(Act::PostUp(p)) = front_act(&w, 0) else {
            unreachable!()
        };
        assert!((6.0..11.0).contains(&p.near), "{}", p.near);
        let mut went = false;
        for _ in 0..240 {
            quiet_playbook(&mut w);
            step_all(&mut w);
            went |= w.events.iter().any(|e| {
                matches!(
                    e,
                    crate::Event::Shot { player: 0, .. }
                        | crate::Event::Dunk { player: 0, .. }
                        | crate::Event::Pass { from: 0, .. }
                )
            });
            if went {
                break;
            }
        }
        ups += usize::from(went);
    }
    // About four in five (31 of 40 seeds; the stands' draws move
    // which ones).
    assert!(ups >= 12, "{ups} of 20 went up or passed");
}

#[test]
fn a_patient_drive_beats_its_defender_first() {
    // `FUN_00109df8`: the pen and kick's drive (`FUN_001095d8(ai, 1)`)
    // goes at the nearest standing defender and jukes him as soon as a
    // juke is on (`FUN_00182878`), then drives.
    let mut w = front_court(1);
    let rim = hoops()[0].rim;
    w.players[3].position = [rim[0] - 23.0, 0.0];
    step_all(&mut w);
    step_all(&mut w);
    quiet_playbook(&mut w);
    give(&mut w, 0, |ctx| Act::Drive(offence::Drive::patient(ctx, 0)));
    let mut juked = false;
    for _ in 0..120 {
        quiet_playbook(&mut w);
        step_all(&mut w);
        let Some(Act::Drive(d)) = front_act(&w, 0) else {
            break;
        };
        if !d.patient {
            juked = w.ai.brains[0].prev & pad::bits::JUKE != 0 || d.beat >= 0;
            break;
        }
        if d.beat >= 0 {
            assert_eq!(d.beat, 3);
        }
    }
    assert!(juked);
}

#[test]
fn the_post_up_play_feeds_the_post_and_he_posts_up() {
    // `FUN_00120ff8`: the handler feeds (`FUN_0011a580`: to the zone
    // beside the post's, then the pass once the post has stood on his
    // block `fps × 0.2` frames with a poor shot for the handler); the post
    // man posts up once he has the ball (`FUN_001212c8`), unless the play
    // has ended first (`FUN_001285d0`: a receiver who has turned handler
    // is already pausing within 12 of the rim).
    let (mut fed, mut posted) = (0, 0);
    for seed in 0..8 {
        let mut w = front_court(seed);
        for p in &mut w.players {
            p.ratings.dunks = 0.0;
        }
        step_all(&mut w);
        w.ai.coaches[0].playbook.forced = plays::kind::POST_UP;
        if until_play(&mut w) != Some(plays::kind::POST_UP) {
            continue;
        }
        w.ai.coaches[0].playbook.forced = -1;
        let play = playbook(&w).play;
        if play.invalid {
            continue;
        }
        assert_eq!(front(&w, 0), "feed", "seed {seed}");
        let post = play.post as usize;
        for _ in 0..300 {
            step_all(&mut w);
            if w.events
                .iter()
                .any(|e| matches!(e, crate::Event::Pass { from: 0, to, .. } if *to == post))
            {
                fed += 1;
                for _ in 0..90 {
                    step_all(&mut w);
                    if w.ball.state == crate::BallState::Held(post) {
                        step_all(&mut w);
                        if front(&w, post) == "post up" {
                            posted += 1;
                        }
                        break;
                    }
                }
                break;
            }
        }
    }
    assert!(fed >= 2, "{fed} feeds");
    assert!(posted >= 1, "{posted} post-ups");
}

#[test]
fn the_trick_pass_out_reads_the_defender_and_the_board() {
    // `FUN_00182d10`: with the stick at rest and a still defender 4..12
    // away, a handler with Handles 70+ goes off the defender's head (kind
    // 1) at `skill(5, 5, 25, 25)` (one draw); one under 70 never tries (no
    // draw); a lob in front of the board goes off it at `skill(0, 20, 40,
    // 60)`.
    let mut w = front_court(2);
    let rim = hoops()[0].rim;
    w.players[0].ratings.handles = 0.9;
    w.players[0].position = [rim[0] - 24.0, 0.0];
    w.players[3].position = [rim[0] - 18.0, 0.0];
    step_all(&mut w);
    w.players[0].position = [rim[0] - 24.0, 0.0];
    w.players[3].position = [rim[0] - 18.0, 0.0];
    w.players[3].speed = 0.0;
    let mut b = Brain::default();
    b.pad.strength = 0;
    let (got, after) = with_ctx(&w, |ctx| offence::trick_pass_out(ctx, &b, 0, false));
    let mut r = w.rng.clone();
    let roll = (r.next_u32() % 100) as i32;
    assert_eq!(after, r, "one draw");
    let (odds, _) = with_ctx(&w, |ctx| skill::percent(ctx, 0, 5, 5, 25, 25));
    assert_eq!(got, (roll < odds).then_some(true));
    w.players[0].ratings.handles = 0.6;
    let (got, after) = with_ctx(&w, |ctx| offence::trick_pass_out(ctx, &b, 0, false));
    assert_eq!(got, None);
    assert_eq!(after, w.rng, "no draw");
    // A lob from in front of the board.
    w.players[0].ratings.handles = 0.9;
    w.players[0].position = [rim[0] - 10.0, 1.0];
    let (got, after) = with_ctx(&w, |ctx| offence::trick_pass_out(ctx, &b, 0, true));
    let mut r = w.rng.clone();
    let roll = (r.next_u32() % 100) as i32;
    assert_eq!(after, r);
    let (odds, _) = with_ctx(&w, |ctx| skill::percent(ctx, 0, 0, 20, 40, 60));
    assert_eq!(got, (roll < odds).then_some(false));
}

#[test]
fn a_trick_pass_out_goes_off_the_defenders_head() {
    // Type 0x17 (`FUN_00100d70`): after its drawn wait the handler stops
    // and presses two turbos and pass to himself, and the game throws Off
    // the Heezay off his defender's head (`FUN_00183318`).
    let mut w = front_court(3);
    let rim = hoops()[0].rim;
    w.players[0].position = [rim[0] - 24.0, 0.0];
    w.players[3].position = [rim[0] - 21.0, 0.0];
    step_all(&mut w);
    step_all(&mut w);
    quiet_playbook(&mut w);
    give(&mut w, 0, |ctx| {
        Act::TrickPass(offence::TrickPass::new(ctx, 0, true))
    });
    let mut thrown = false;
    for _ in 0..30 {
        quiet_playbook(&mut w);
        w.players[3].position = [rim[0] - 21.0, 0.0];
        step_all(&mut w);
        if w.events.iter().any(|e| {
            matches!(
                e,
                crate::Event::Pass {
                    from: 0,
                    to: 0,
                    style: crate::passing::PassStyle::Heezay
                }
            )
        }) {
            thrown = true;
            break;
        }
    }
    assert!(thrown);
}

#[test]
fn a_full_meter_shoots_as_its_thirty_seconds_run_out() {
    // `FUN_00129748`: team meter+0x18 counts down from `fps × 30`
    // (`0x4b7b14`); with under `fps × 2` left the handler shoots for the
    // gamebreaker at once. The sim's meter keeps that countdown
    // (`TeamState::meter_hold`, run by `FUN_0013eff8`'s port).
    for (left, shoots) in [(1800, false), (100, true)] {
        let mut w = front_court(3);
        w.meter_clock = crate::street::MeterClock::On;
        w.set_meter(0, 50_000.0);
        // One already pocketed: the pocket call (`FUN_0012ebf0`) stays off.
        w.teams[0].pocketed = true;
        step_all(&mut w);
        step_all(&mut w);
        assert_eq!(w.teams[0].meter_hold, crate::street::METER_HOLD - 2);
        w.teams[0].meter_hold = left;
        w.ai.coaches[0].playbook.shot_hold = 0;
        step_all(&mut w);
        assert_eq!(front(&w, 0) == "shoot", shoots, "{left}");
    }
}

#[test]
fn a_computer_handler_pockets_in_the_middle_of_a_trick() {
    // The pocket call (`FUN_0012ebf0`) replaces the handler's queue with
    // "pocket" every update it holds; its press goes in at once because the
    // holder's ball update (`FUN_0015a060`) tests the pocket press
    // (`FUN_00159eb8`) whatever move he is in. "pocket" fronts one update,
    // as in the captured original (once in a match). The rewrite used to
    // drop a press that came during a move, and the call held the button
    // down from then on, so the handler fronted "pocket" for thousands of
    // updates.
    let mut w = front_court(3);
    w.set_meter(0, 50_000.0);
    w.players[0].action = crate::trick::pressed(0, 3);
    let mut fronts = 0;
    for _ in 0..60 {
        step_all(&mut w);
        if front(&w, 0) == "pocket" {
            fronts += 1;
            assert!(
                matches!(w.players[0].action, crate::Action::Trick { .. }),
                "pressed during the trick"
            );
        }
        if w.teams[0].pocketed {
            break;
        }
    }
    assert!(w.teams[0].pocketed);
    assert_eq!(fronts, 1);
}

#[test]
fn a_person_pockets_in_the_middle_of_a_move() {
    // `FUN_00159eb8` for a person's Select (no turbo) while a trick plays.
    let mut w = front_court(3);
    w.set_meter(0, 50_000.0);
    w.players[0].action = crate::trick::pressed(0, 1);
    let mut inputs: Vec<Input> = (0..w.players.len()).map(|i| input(&w, i)).collect();
    inputs[0] = Input::default().with(crate::button::SELECT);
    w.step(&inputs);
    assert!(w.teams[0].pocketed);
}

#[test]
fn an_inbound_pass_leaves_the_hand_when_its_animation_lets_go() {
    // The restart's pass (`FUN_0015f930`, phase 6: pass row 10, a 6-update
    // blend and a 12-update wait before callback 38) stays in the
    // inbounder's hands 18 updates after the press; the press comes the
    // inbounder's drawn 6..17 updates plus one after all six are ready
    // (the captures: all 56 inbounds 6..18 from ready to the press), and
    // phase 6 ends in the release's update (`FUN_0013e4c8` in the game's
    // update after the ball's: all ready and nobody holding it). The
    // captures end it there in 45 of the 56 (the ball one update into
    // its flight), the other 11 an update later.
    for seed in [1, 5, 9] {
        let mut w = three_on_three(seed);
        assert!(w.restarting());
        let (mut ready, mut press) = (None, None);
        for t in 0..900 {
            step_all(&mut w);
            if ready.is_none() && w.restart_ready() {
                ready = Some(t);
            }
            if press.is_none() && w.windup.is_some() {
                press = Some(t);
                assert!(
                    w.events
                        .iter()
                        .any(|e| matches!(e, crate::Event::Pass { .. }))
                );
            }
            if let Some(p) = press
                && w.windup.is_none()
            {
                assert_eq!(w.ball.state, BallState::Pass, "seed {seed}");
                assert_eq!(t - p, i32::from(crate::INBOUND_RELEASE), "seed {seed}");
                assert!(!w.restarting(), "over in the release's update");
                assert_eq!(w.game_phase.phase, crate::game_phase::phase::BACK_COURT);
                let wait = p - ready.unwrap();
                assert!((7..=18).contains(&wait), "seed {seed}: {wait}");
                break;
            }
            if press.is_some() {
                assert!(matches!(w.ball.state, BallState::Held(_)));
            }
        }
        assert!(press.is_some(), "seed {seed}");
    }
}

#[test]
fn a_caught_pass_holds_its_receiver_for_the_catch() {
    // Actor+0xa4 bit 9 (`FUN_0016ea00`, the AI's `FUN_001816d0`) is the
    // receiver's catch, not the passer's throw (bit 4, `FUN_00154680`):
    // the captures show it from the release through the flight and for
    // the catch animation after it. The pass node, the carrier's
    // behaviours and the playbook's checks wait on it, so a player who
    // has just caught the ball does not act on it for the catch row's
    // hold (`timing::catch_hold`); the passer's windup is bit 4 (and the
    // pass node keeps pressing every other update through it, as the
    // captured original's does).
    // (Seed 2: on 4 and 1 the defender's steal draw takes the pass.)
    let mut w = front_court(2);
    let mut inputs: Vec<Input> = (0..w.players.len()).map(|i| input(&w, i)).collect();
    // A person passes to player 1.
    let to = w.players[1].position;
    let from = w.players[0].position;
    let d = [to[0] - from[0], to[1] - from[1]];
    let l = crate::math::len(d[0], d[1]);
    inputs[0] = Input::moving([d[0] / l, d[1] / l]).with(crate::button::PASS);
    w.step(&inputs);
    let windup = w.windup.expect("a pass in the hands");
    assert_eq!(windup.to, 1);
    inputs[0] = Input::default();
    let catching = |w: &World, i: usize| with_tracked_ctx(w, |ctx| ctx.catching(i)).0;
    let mut caught = None;
    for t in 0..120 {
        w.step(&inputs);
        if w.windup.is_some() {
            assert!(with_tracked_ctx(&w, |ctx| ctx.throwing(0)).0);
            assert!(
                !catching(&w, 1),
                "not yet: the ball is in the passer's hands"
            );
        }
        if w.ball.state == BallState::Pass {
            assert!(catching(&w, 1), "in flight to him");
        }
        if w.ball.state == BallState::Held(1) {
            caught = Some(t);
            break;
        }
    }
    assert!(caught.is_some());
    // From the catch, the catch row's hold: the flight's nominal end still
    // to come plus the row script's tail (`timing::catch_hold`); the
    // receiver stood still facing the passer, so a standing row of sector
    // 4 (`catch_stand_1`, tail 9, or `catch_stand_3r`/`3l`, tail 15).
    let catch = w.catch.expect("the catch row holds him");
    assert_eq!(catch.player, 1);
    let row = w.ball.pass.map(|p| p.catch);
    assert!(row.is_none(), "the ball is held: no pass left");
    // `until` counts from the catch update; the world is one past it.
    let hold = (catch.until - w.tick) as i32;
    assert!((8..=19).contains(&hold), "{hold}");
    let mut held = 0;
    for _ in 0..40 {
        if catching(&w, 1) {
            held += 1;
        } else {
            break;
        }
        w.step(&inputs);
        if w.ball.state != BallState::Held(1) {
            break;
        }
    }
    assert_eq!(held, hold);
}

#[test]
fn a_move_out_of_the_catch_ends_the_catch_rows_hold() {
    // The catch row's tail keeps bit 9 only while its script runs: a shot
    // started in it replaces the row, and its `9b` the state word (the
    // pass captures' computer receiver shot 4 updates into
    // `catch_stand_3r`'s 16 and lost bit 9 in the shot's first update).
    // (Seed 2: on 4 and 1 the defender's steal draw takes the pass.)
    let mut w = front_court(2);
    let mut inputs: Vec<Input> = (0..w.players.len()).map(|i| input(&w, i)).collect();
    let to = w.players[1].position;
    let from = w.players[0].position;
    let d = [to[0] - from[0], to[1] - from[1]];
    let l = crate::math::len(d[0], d[1]);
    inputs[0] = Input::moving([d[0] / l, d[1] / l]).with(crate::button::PASS);
    w.step(&inputs);
    inputs[0] = Input::default();
    let catching = |w: &World, i: usize| with_tracked_ctx(w, |ctx| ctx.catching(i)).0;
    for _ in 0..120 {
        w.step(&inputs);
        if w.ball.state == BallState::Held(1) {
            break;
        }
    }
    assert_eq!(w.ball.state, BallState::Held(1));
    w.step(&inputs);
    assert!(catching(&w, 1) && w.actor_state_bits(1) & 0x200 != 0);
    inputs[1] = Input::default().with(crate::button::SHOOT);
    w.step(&inputs);
    assert!(
        !matches!(w.players[1].action, crate::Action::None),
        "the press starts a move"
    );
    let catch = w.catch.expect("the catch row's hold");
    assert!(w.tick < catch.until, "inside the row's tail");
    assert!(!catching(&w, 1));
    assert_eq!(w.actor_state_bits(1) & 0x200, 0);
}

/// [`with_ctx`] with the world's own tracked game state (the catch, the
/// held count), as the computer's frame read it.
fn with_tracked_ctx<R>(w: &World, f: impl FnOnce(&mut Ctx) -> R) -> (R, Rng) {
    let mut rng = w.rng.clone();
    let n = w.players.len();
    let computer = vec![true; n];
    let inputs = vec![Input::computer(); n];
    let r = {
        let mut ctx = Ctx {
            w,
            rng: &mut rng,
            game: w.ai.game,
            human: [false; 2],
            computer: &computer,
            inputs: &inputs,
            tick: w.tick as i64,
            matchups: [([-1; 3], [-1; 3], [-1; 3]); 2],
            save_turbo: [false; 2],
            open: [[0; 3]; 2],
            holder_of: [0, 3],
            pads: vec![0; n],
            fronts: vec![0; n],
            targets: vec![None; n],
            pick_sides: vec![0; n],
            screened: vec![-1; n],
        };
        f(&mut ctx)
    };
    (r, rng)
}

#[test]
fn the_pass_targets_near_the_rim_stand_7_from_it_on_his_line() {
    // `FUN_0010aea8` (the fast break's and the roll's), `FUN_00112f48`
    // (the post cut's) and `FUN_00110ae8` (the net cut's), with the AI's
    // +0xb4 of 7.
    let rim = [36.0, 0.0];
    let near = |a: [f32; 2], b: [f32; 2]| court::dist(a, b) < 0.01;
    assert!(near(spots::near_rim([20.0, 0.0], rim), [29.0, 0.0]));
    assert!(near(spots::near_rim([36.0, 20.0], rim), [36.0, 7.0]));
    // Within 7 he is the point.
    assert!(near(spots::near_rim([33.0, 0.0], rim), [33.0, 0.0]));
    // The post cut: the spot on his line to the rim, else his place.
    let post = spots::PostCut {
        spot: [30.0, 1.0],
        ..Default::default()
    };
    assert!(near(post.pass_target([20.0, 0.0], rim), [29.0, 0.0]));
    assert!(near(post.pass_target([33.0, 0.0], rim), [33.0, 0.0]));
    let wide = spots::PostCut {
        spot: [20.0, 12.0],
        ..Default::default()
    };
    assert!(near(wide.pass_target([20.0, 0.0], rim), [20.0, 0.0]));
    // The net cut: near the rim on its way in; on its way out the spot,
    // or 10 from the rim toward it once within 10.
    let mut cut = spots::CutMove {
        kind: spots::CutKind::NetIn,
        spot: [20.0, 10.0],
        ..Default::default()
    };
    assert!(near(cut.pass_target([20.0, 0.0], rim), [29.0, 0.0]));
    cut.stage = true;
    assert!(near(cut.pass_target([20.0, 0.0], rim), [20.0, 10.0]));
    let n = court::dist([20.0, 10.0], rim);
    let toward = [36.0 - 16.0 / n * 10.0, 10.0 / n * 10.0];
    assert!(near(cut.pass_target([30.0, 0.0], rim), toward));
}

#[test]
fn the_fast_breaks_pass_wait_runs_through_a_pass_in_the_air() {
    // `FUN_00125c70` runs its checks whoever holds the ball, and
    // `FUN_00126288` counts its wait down before it looks for the holder
    // (`0x125ecc..0x125f60`): a pass in the air does not hold it.
    let mut w = three_on_three(3);
    let mut checked = 0;
    for _ in 0..40_000 {
        let t = w.possession.min(1);
        let on = |w: &World| {
            w.ai.game.phase == 4
                && w.ai.coaches[t].strategy == team::strategy::FAST_BREAK
                && w.ai.coaches[t].fast_break.lanes
                && w.ball.state == BallState::Pass
                && w.possession.min(1) == t
        };
        let before = (on(&w), w.ai.coaches[t].fast_break.wait);
        step_all(&mut w);
        if before.0 && on(&w) && before.1 > 1 {
            assert_eq!(w.ai.coaches[t].fast_break.wait, before.1 - 1);
            checked += 1;
            if checked > 20 {
                return;
            }
        }
    }
    assert!(checked > 0);
}

/// The pocket and cancel checks are the game object's (vt+0xc4, vt+0xcc):
/// the points game's margin 2..4 against its target, the trick-point
/// game's 25,000..100,000, none in No Gamebreakers, and Gamebreaker Wins
/// only to a Level 2, choosing between pocket and cancel by the other
/// side's meter (`FUN_00145d28`, `FUN_00145e70`).
#[test]
fn each_game_class_asks_its_own_pocket_and_cancel() {
    use crate::{Goal, Mode};
    let pocket = |w: &World| with_ctx(w, |ctx| playbook::pocket_check(ctx, 0)).0;
    // The points game: 10 to 10 at 21 pockets; 18 to 10 is within reach.
    let mut w = front_court(3);
    w.set_meter(0, 50_000.0);
    w.score = [10, 10];
    assert!(pocket(&w));
    w.score = [18, 10];
    assert!(!pocket(&w));
    // No Gamebreakers never pockets.
    let mut w = front_court(3);
    w.rules = Rules::mode(Mode::NoGamebreakers);
    w.teams[0].meter = 50_000;
    assert!(!pocket(&w));
    // The trick-point game to a million: far off pockets; within 25,000
    // (the least margin) of it does not.
    let mut w = front_court(3);
    w.rules = Rules::mode(Mode::TrickPointChallenge);
    w.rules.goal = Goal::TrickPoints(1_000_000);
    w.set_meter(0, 50_000.0);
    assert!(pocket(&w));
    w.stats.add_team(0, crate::stats::TRICK_POINTS, 980_000);
    assert!(!pocket(&w));
    // Gamebreaker Wins to a Level 1: never.
    let mut w = front_court(3);
    w.rules = Rules::mode(Mode::GamebreakerWins);
    w.set_meter(0, 50_000.0);
    assert!(!pocket(&w));
    // To a Level 2: a pocket with no cancel possible.
    w.rules.goal = Goal::Gamebreaker(2);
    assert!(pocket(&w));
    // The other side pocketed with an empty meter: pocket (100%).
    w.teams[1].pocketed = true;
    assert!(pocket(&w));
    // Its meter at half: no pocket (0%), and the cancel the check chose
    // goes once.
    w.teams[1].meter = 25_000;
    let (asked, _) = with_ctx(&w, |ctx| {
        let pocketed = playbook::pocket_check(ctx, 0);
        let first = playbook::cancel_check(ctx, 0);
        let again = playbook::cancel_check(ctx, 0);
        (pocketed, first, again)
    });
    assert_eq!(asked, (false, true, false));
}

/// The type the points game's pocket and cancel checks test is the side's
/// database team (`FUN_001ecdb8` of game+0x184's +0x40): Team Whitewater
/// (0x2c) never pockets and always cancels once the other side pockets
/// (`FUN_0012ebf0`, `FUN_0012ed28`).
#[test]
fn team_whitewater_never_pockets_and_always_cancels() {
    let pocket = |w: &World| with_ctx(w, |ctx| playbook::pocket_check(ctx, 0));
    let mut w = front_court(3);
    w.set_meter(0, 50_000.0);
    w.score = [10, 10];
    assert!(pocket(&w).0);
    w.side_teams = [crate::WHITEWATER, 36];
    let (pocketed, rng) = pocket(&w);
    assert!(!pocketed);
    assert_eq!(rng, w.rng, "no draw for Whitewater's pocket");
    // The other side has pocketed: the cancel is asked once, and the
    // draw's 8% does not matter to Whitewater.
    w.teams[1].pocketed = true;
    let mut always = 0;
    for seed in 0..40 {
        let mut x = w.clone();
        x.rng = crate::rng::Rng::new(seed);
        let ((first, again), rng) = with_ctx(&x, |ctx| {
            ctx.game.cancel[0] = true;
            (
                playbook::cancel_check(ctx, 0),
                playbook::cancel_check(ctx, 0),
            )
        });
        assert_ne!(rng, x.rng, "the draw is made");
        assert!(!again);
        always += usize::from(first);
    }
    assert_eq!(always, 40);
}

/// The trick-point game's pass check (`FUN_00183c78`): with more than 4
/// on the shot clock the handler passes rather than shoot until his side
/// has won it (vt+0x7c, `FUN_0018c698`).
#[test]
fn the_trick_point_game_passes_until_its_side_has_won() {
    use crate::Mode;
    let mut w = front_court(3);
    w.rules = Rules::mode(Mode::TrickPointChallenge);
    assert!(!w.basket_would_win(0, 0, 0));
    w.stats.add_team(0, crate::stats::TRICK_POINTS, 100_001);
    assert!(w.basket_would_win(0, 0, 0));
}

#[test]
fn the_free_balls_take_hands_it_over_after_the_ais_frame() {
    // `0x10e948..0x10e964`: the take the "loose ball" failsafe asks for
    // (`FUN_0014cb28`) is the world's, right after that player's frame (in
    // his actor's update).
    let mut w = three_on_three(1);
    w.ball.drop_loose();
    w.ball.airborne = false;
    w.ball.position = [0.0, 0.5, 0.0];
    w.ai.game.free_take = 5;
    let inputs: Vec<Input> = (0..6).map(|_| Input::computer()).collect();
    let _ = w.computer_inputs(&inputs, &[false; 6]);
    assert_eq!(w.ball.state, BallState::Held(4));
    assert_eq!(w.ai.game.free_take, 0);
}

#[test]
fn a_defender_updated_after_the_shooter_sees_the_shot_let_go_in_its_own_update() {
    // `FUN_001587d8` runs each computer actor's AI frame (`FUN_0013a970`)
    // at the start of his update, in the match's order: a defender
    // updated after the shooter finds the jump shot let go in the
    // release's own update and goes up at it then ("a late jump left
    // over", `FUN_00106a88`, taken once the holder is gone) or not at all;
    // one updated before finds it the next update. (The block jumps want
    // the disc's animations.)
    let poses = crate::pose::disc_bank();
    if poses.get().is_none() {
        return;
    }
    let mut same = [0u32; 2];
    let mut next = [0u32; 2];
    for seed in 1..=6 {
        let mut w = three_on_three(seed);
        w.poses = poses.clone();
        w.gait = capture::disc_gait();
        w.update_order = [2, 5, 3, 0, 4, 1];
        w.start_opening_toss();
        let order = w.actor_order();
        let at = |j: usize| order.iter().position(|&k| k == j);
        let mut released: Option<(u64, usize)> = None;
        for _ in 0..20_000 {
            let shooter = w
                .handler()
                .filter(|&h| matches!(w.players[h].action, crate::Action::Shooting { .. }));
            let up: Vec<bool> = w
                .players
                .iter()
                .map(|p| matches!(p.action, crate::Action::Jumping { block: Some(_), .. }))
                .collect();
            step_all(&mut w);
            if let Some(h) = shooter
                && w.ball.state == BallState::Shot
            {
                released = Some((w.tick, h));
            }
            for (i, p) in w.players.iter().enumerate() {
                if !matches!(p.action, crate::Action::Jumping { block: Some(_), .. }) || up[i] {
                    continue;
                }
                if let Some((t, h)) = released {
                    let before = usize::from(at(i) < at(h));
                    if w.tick == t {
                        same[before] += 1;
                    } else if w.tick == t + 1 {
                        next[before] += 1;
                    }
                }
            }
            if matches!(w.phase, crate::Phase::Over { .. }) {
                break;
            }
        }
    }
    // [after the shooter, before him].
    assert_eq!(next[0], 0, "{same:?} {next:?}");
    assert!(same[0] > 0 && next[1] > 0, "{same:?} {next:?}");
}

#[test]
fn a_computers_board_pass_to_a_teammate_keeps_its_special_route() {
    // FUN_00183318 posts the board point and two-turbo pass for a named
    // teammate too; it must reach callback 38 as a board pass, not plain.
    let mut w = front_court(3);
    step_all(&mut w);
    step_all(&mut w);
    w.players[0].position = [25.0, 12.0];
    w.players[0].ratings.handles = 1.0;
    w.players[2].position = [24.0, -10.0];
    for p in &mut w.players[3..] {
        p.position = [-20.0, 20.0];
    }
    quiet_playbook(&mut w);
    give(&mut w, 0, |ctx| {
        Act::TrickPass(offence::TrickPass::new(ctx, 2, false))
    });
    for _ in 0..30 {
        quiet_playbook(&mut w);
        step_all(&mut w);
        if let Some(style) = w.events.iter().find_map(|e| match e {
            crate::Event::Pass {
                from: 0,
                to: 2,
                style,
            } => Some(*style),
            _ => None,
        }) {
            assert_eq!(style, crate::PassStyle::Glass);
            let windup = w.windup.expect("callback 38 still holds the ball");
            assert_eq!((windup.from, windup.to), (0, 2));
            assert!((91..=92).contains(&windup.row));
            assert!(w.passout.is_some_and(|po| po.passer == 0 && !po.tossed));
            assert_eq!(w.validate(), Ok(()));
            return;
        }
    }
    panic!("the queued computer board pass never threw");
}
