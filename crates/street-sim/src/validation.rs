//! Validation at untrusted world/snapshot boundaries.
//! All reference indexes and floating-point values are checked before a world
//! can be stepped or used for rendering. Runtime match policy is separate.
use crate::{Action, BallState, Basket, Event, Goal, Phase, World, ball::Via};

const LIMIT: f32 = 1_000_000.0;
type Result = std::result::Result<(), String>;

fn require(ok: bool, field: &str) -> Result {
    if ok {
        Ok(())
    } else {
        Err(format!("invalid world field: {field}"))
    }
}

fn floats(values: &[f32], field: &str) -> Result {
    require(
        values.iter().all(|v| v.is_finite() && v.abs() <= LIMIT),
        field,
    )
}

fn level(value: u8, minimum: u8, maximum: u8, field: &str) -> Result {
    require((minimum..=maximum).contains(&value), field)
}

fn basket(value: Basket) -> Result {
    if let Basket::Gamebreaker { level: l, .. } = value {
        level(l, 1, 2, "basket gamebreaker level")?;
    }
    Ok(())
}

/// A trick's or reaction's script clock ([`crate::trick::Run`]) over a
/// script of `steps` steps.
fn script_clock(run: &crate::trick::Run, steps: usize) -> Result {
    require(usize::from(run.pc) <= steps, "script step")?;
    require(run.frame.is_finite(), "script frame")?;
    if let Some(span) = run.span {
        require(span.rate.is_finite(), "script span rate")?;
        require(span.counter <= span.length.saturating_add(1), "script span")?;
    }
    Ok(())
}

impl World {
    /// Validate a decoded or programmatically supplied world without mutating
    /// it. Empty/two-player worlds are useful offline; servers impose their
    /// own three-per-side policy after this general validation.
    pub fn validate(&self) -> Result {
        let World {
            tick,
            hoops,
            players,
            ball,
            rules,
            score: _,
            stage_bounds: _,
            update_order,
            teams,
            possession,
            shot_clock,
            phase,
            game_phase,
            events,
            give_and_go,
            fresh_take: _,
            dribble_armed: _,
            rims,
            rim_rigs,
            windup,
            catch,
            turbo_releases,
            rng: _,
            previous_inputs,
            staged,
            gait,
            variety,
            sequence,
            combo,
            assist,
            pump_fake_watch,
            pump_fake_left: _,
            pump_fake_shot: _,
            clock,
            lesson: _,
            lesson_page: _,
            scenario,
            side_teams: _,
            opening_toss,
            tip,
            timed_dead_ball: _,
            scorer,
            celebrate_pending: _,
            libc: _,
            replays: _,
            contacts,
            ai,
            meter_clock: _,
            stats,
            poses: _,
            last_trick,
            catching,
            spot_run,
            passout,
            held_anchor,
            turbo,
            turbo_owner,
            people,
            control,
            gamebreaker_task,
            staged_start,
        } = self;
        if let Some(w) = scenario {
            require(w.is_finite() && (0.0..=1.0).contains(w), "story weight")?;
        }
        let mut seen = [false; 6];
        for &k in update_order {
            let k = usize::from(k);
            require(k < 6 && !seen[k], "update order")?;
            seen[k] = true;
        }
        floats(turbo, "turbo bars")?;
        // A swing's step runs to any size once its amplitude dies away
        // (the original's last wraps divide by it too): finite is enough.
        for rim in rims {
            require(
                [
                    rim.bend_phase,
                    rim.bend_angle,
                    rim.sway_phase,
                    rim.sway_angle,
                    rim.dangle_step,
                    rim.sway_step,
                    rim.amplitude,
                    rim.dangle_amplitude,
                    rim.kick,
                    rim.decay,
                    rim.dangle,
                ]
                .iter()
                .all(|v| v.is_finite()),
                "rim",
            )?;
        }
        for rig in rim_rigs {
            floats(rig.t.as_flattened(), "rim rig")?;
            floats(rig.r.as_flattened(), "rim rig")?;
        }
        // A world read from a snapshot holds the match's own disc values
        // and none of the disc's tables until its machine installs them
        // ([`crate::disc`]): what is checked against the tables waits.
        let plans = staged.as_ref().filter(|c| !c.plans.is_empty());
        let staged_shell = staged.is_some() && plans.is_none();
        if let Some((s, wait)) = staged_start {
            require((1..=2).contains(wait), "staged start wait")?;
            require(sequence.is_none(), "staged start beside a sequence")?;
            require(
                staged_shell || plans.and_then(|c| c.plan(s.script)).is_some(),
                "staged start without its plan",
            )?;
            require(s.frame == 0 && s.held == 0, "staged start frame")?;
            require(
                s.shooter < players.len() && s.actors.iter().flatten().all(|&p| p < players.len()),
                "staged start actor",
            )?;
        }
        if let Some(t) = gamebreaker_task {
            require(t.kind <= 1 && t.state <= 3, "gamebreaker task state")?;
            require((0..=60).contains(&t.count), "gamebreaker task wait")?;
        }
        require(
            turbo.iter().all(|t| (0.0..=1.0).contains(t)),
            "turbo bar range",
        )?;
        require(
            turbo_owner.len() == players.len() && people.len() == players.len(),
            "turbo owners and people",
        )?;
        if let Some(f) = control.frozen {
            require(f.player < players.len(), "frozen receiver")?;
        }
        if let Some(t) = &control.tables {
            require(t.places.len() == players.len(), "control tables")?;
            t.places
                .iter()
                .try_for_each(|p| floats(p, "control tables"))?;
            floats(&t.ball, "control tables")?;
        }
        require(
            *last_trick == u8::MAX || usize::from(*last_trick) < crate::trick::SCRIPTS.len(),
            "last trick script",
        )?;
        if let Some(c) = catching {
            require(c.player < players.len(), "catching player")?;
            require(
                usize::from(c.row) < crate::touch::CATCH_SCRIPTS.len(),
                "catching row",
            )?;
            script_clock(&c.run, c.steps().len())?;
            floats(&c.drift, "catching drift")?;
            if let Some((_, time)) = c.from {
                floats(&[time], "catching clip frame")?;
            }
        }
        if let Some(crate::passout::Passout {
            passer,
            kind,
            defender,
            point,
            back,
            tossed,
            tail,
        }) = passout
        {
            require(*passer < players.len(), "passout passer")?;
            floats(point, "passout point")?;
            if let Some((hand, _)) = back {
                floats(hand, "passout hand")?;
            }
            match kind {
                crate::passout::Kind::Board => {
                    require(defender.is_none(), "board passout defender")?
                }
                crate::passout::Kind::Head => require(
                    defender.is_some_and(|d| {
                        d < players.len() && players[d].team != players[*passer].team
                    }) && (back.is_none() || *tossed)
                        && (*tossed
                            || windup.is_some_and(|w| {
                                w.from == *passer && w.style == crate::passing::PassStyle::Heezay
                            }))
                        && (tail.is_none()
                            || matches!(players[*passer].action, Action::Passing { row, .. } if crate::passout::toss(row).is_some())),
                    "head passout lifecycle",
                )?,
            }
        }
        require(spot_run.count <= 2, "spot run count")?;
        floats(&spot_run.spot, "spot run spot")?;
        require(
            spot_run.waiting.is_none_or(|p| p < players.len()),
            "spot run receiver",
        )?;
        require(
            spot_run.steer.is_none_or(|(p, w, n)| {
                p < players.len() && (0..0x400).contains(&w) && (0..=64).contains(&n)
            }),
            "spot run steer",
        )?;
        if let Some(a) = held_anchor {
            require(a.player < players.len(), "held anchor player")?;
            floats(&a.local, "held anchor")?;
        }
        require(players.len() <= 64, "player count")?;
        // The ring's integers become weight-table indexes, and additions
        // must not overflow even when the decoder accepts any signed integer.
        for s in &combo.ring.slots {
            require(s.class == -1 || (0..=63).contains(&s.class), "combo class")?;
            require((-1..=63).contains(&s.extra), "combo extra")?;
            require(s.span >= 0, "combo span")?;
        }
        require(combo.side <= 1, "combo side")?;
        require(
            combo.row == -1 || (0..crate::combo::PATTERNS.len() as i32).contains(&combo.row),
            "combo row",
        )?;
        require((0..=2).contains(&combo.air), "combo pass air")?;
        if let Some((p, _)) = assist {
            require(*p < players.len(), "assister")?;
        }
        require(pump_fake_watch.len() == players.len(), "pump fake watch")?;
        require((-1..=1).contains(&clock.side), "shot clock side")?;
        require(
            stats.players.len() <= 64 && stats.on_court.len() <= 64,
            "statistics per player",
        )?;
        if let Some(config) = gait {
            require(config.records.len() <= 256, "gait record count")?;
            require(config.profiles.len() <= 64, "gait profile count")?;
            require(
                config
                    .row_hands
                    .iter()
                    .flat_map(|(_, f)| f)
                    .chain(&config.stand_hands)
                    .chain(
                        config
                            .trick_hands
                            .iter()
                            .flat_map(|(_, s)| s.iter().flatten()),
                    )
                    .all(|&h| h <= 3),
                "gait hand flags",
            )?;
            require(
                config.records.iter().all(|r| {
                    r.root_distance.is_finite()
                        && r.motion.iter().all(|m| m.is_finite())
                        && r.rows.iter().all(|&row| row < 1144)
                }),
                "gait record values",
            )?;
            for p in players {
                require(
                    matches!(p.gait_channel.hand, 1 | 2) && p.gait_channel.trick_side <= 1,
                    "live ball hand",
                )?;
                if let Some(index) = p.gait_channel.record
                    && !config.records.is_empty()
                {
                    require(
                        usize::from(index) < config.records.len(),
                        "gait record index",
                    )?;
                }
            }
        }
        if let Some(config) = staged {
            config.validate().map_err(|_| "staged plans")?;
        }
        require(
            variety.used.iter().all(|&v| v <= 15),
            "staged variant flags",
        )?;
        if let Some(s) = sequence {
            require(s.script < 70, "staged script")?;
            require(s.level == 2, "staged level")?;
            require(s.team < 2, "staged team")?;
            require(s.shooter < players.len() && s.shooter < 6, "staged shooter")?;
            require(players[s.shooter].team == s.team, "staged shooter team")?;
            require(
                s.actors
                    .iter()
                    .flatten()
                    .all(|&p| p < players.len() && p < 6),
                "staged actor",
            )?;
            let mut bound = [false; 6];
            for &actor in s.actors.iter().flatten() {
                require(!bound[actor], "duplicate staged actor")?;
                bound[actor] = true;
            }
            for clock in s.native_clocks.iter().flatten() {
                require(clock.valid(), "native clock scalar domains")?;
            }
            let plan = plans.and_then(|c| c.plan(s.script));
            require(
                staged_shell || plan.is_some(),
                "staged sequence without its plan",
            )?;
            if let Some(plan) = plan {
                require(s.frame < plan.frame_count, "staged frame")?;
                require(
                    s.held == 0
                        || plan
                            .controls
                            .hold
                            .is_some_and(|h| h.frame == s.frame && s.held <= h.duration),
                    "staged freeze",
                )?;
                let processed_tick = s
                    .native_clocks
                    .iter()
                    .flatten()
                    .map(|clock| clock.last_tick)
                    .max()
                    .or_else(|| s.frame.checked_sub(1));
                for (role, clock) in s.native_clocks.iter().enumerate() {
                    let processed = processed_tick.and_then(|tick| {
                        plan.actors
                            .iter()
                            .rfind(|actor| usize::from(actor.slot) == role && actor.frame <= tick)
                    });
                    require(
                        processed.is_none_or(|actor| actor.playback.is_none() || clock.is_some()),
                        "native clock missing processed role",
                    )?;
                    let Some(clock) = clock else {
                        continue;
                    };
                    require(clock.valid(), "native clock scalar domains")?;
                    require(
                        Some(clock.last_tick) == processed_tick,
                        "native clocks disagree on manager tick",
                    )?;
                    let actor = plan
                        .actors
                        .get(usize::from(clock.track))
                        .ok_or("native clock track outside plan")?;
                    let current = plan
                        .actors
                        .get(usize::from(clock.current_track))
                        .ok_or("native current clock track outside plan")?;
                    require(
                        usize::from(actor.slot) == role && usize::from(current.slot) == role,
                        "native clock role",
                    )?;
                    require(
                        actor.playback.is_some() && current.playback.is_some(),
                        "native clock metadata",
                    )?;
                    let playback = actor.playback.unwrap();
                    let current_playback = current.playback.unwrap();
                    require(
                        clock.blend_updates == 0 || playback.blend_ticks() != 0 && clock.active,
                        "native clock blend policy",
                    )?;
                    require(
                        clock.current_track == clock.track || current.frame < actor.frame,
                        "native clock current source precedes incoming track",
                    )?;
                    let current_phase = |bits: u32| {
                        let value = f32::from_bits(bits);
                        if current_playback.looping() {
                            value < f32::from(current_playback.frames)
                        } else {
                            value <= f32::from(current_playback.frames - 1)
                        }
                    };
                    require(
                        current_phase(clock.frame_bits) && current_phase(clock.sampled_bits),
                        "native clock current clip phase",
                    )?;
                    let target = if playback.looping() {
                        0.0
                    } else {
                        ((u32::from(playback.frames - 1) * u32::from(playback.flags[10])) >> 8)
                            as f32
                    };
                    require(
                        clock.target_bits == target.to_bits()
                            && f32::from_bits(clock.end_bits) <= f32::from(playback.frames - 1),
                        "native clock incoming clip phase",
                    )?;
                    let latest = plan
                        .actors
                        .iter()
                        .enumerate()
                        .rfind(|(_, candidate)| {
                            usize::from(candidate.slot) == role
                                && candidate.frame <= clock.last_tick
                        })
                        .map(|(index, _)| index);
                    require(
                        latest == Some(usize::from(clock.track)),
                        "native clock latest source track",
                    )?;
                    let duration = if playback.initial || playback.looping() {
                        actor.duration
                    } else {
                        actor.duration.saturating_sub(6)
                    };
                    require(
                        if clock.blend_updates != 0 {
                            clock.counter == u32::from(clock.blend_updates)
                        } else if playback.looping() {
                            clock.counter == u32::MAX && clock.active
                        } else {
                            clock.counter <= duration.saturating_add(u32::from(!clock.active))
                        },
                        "native clock duration counter",
                    )?;
                    require(
                        clock.last_tick <= s.frame && actor.frame <= clock.last_tick,
                        "native clock manager tick",
                    )?;
                    require(
                        clock.last_tick == s.frame || clock.last_tick.saturating_add(1) == s.frame,
                        "native clock update order",
                    )?;
                    require(
                        (clock.blend_updates != 0) == (clock.current_track != clock.track),
                        "native clock blend identity",
                    )?;
                }
                require(
                    !s.resume_pending
                        || plan
                            .controls
                            .hold
                            .is_some_and(|hold| hold.frame == s.frame && hold.duration == s.held)
                            && s.native_clocks.iter().any(Option::is_some),
                    "native resume lifecycle",
                )?;
            }
        }
        require(*tick <= u64::MAX - 1024, "tick capacity")?;
        require(
            turbo_releases.len() == players.len() && previous_inputs.len() == players.len(),
            "per-player list lengths",
        )?;
        ai.validate(players.len())?;
        for k in 0..6 {
            let with = contacts.with[k];
            require(
                (with == -1 || (0 <= with && (with as usize) < players.len()))
                    && crate::contact::SCREEN_REST <= contacts.count[k],
                "contact counts",
            )?;
        }
        let player = |index: usize| require(index < players.len(), "player reference");
        let team = |index: usize| require(index < teams.len(), "team reference");
        let hoop = |index: usize| require(index < hoops.len(), "hoop reference");
        team(*possession)?;
        floats(&[*shot_clock], "shot clock")?;
        for h in hoops {
            floats(&h.rim, "hoop position")?;
            floats(&[h.rim_radius], "rim radius")?;
            require(h.rim_radius > 0.0, "positive rim radius")?;
        }
        for p in players {
            let crate::Player {
                team: t,
                ratings,
                position,
                heading,
                speed,
                course,
                wanted,
                guard: _,
                gait: _,
                action,
                action_buttons: _,
                native_tricks,
                native_dunks,
                height,
                turbo,
                gait_channel,
                pass_kind: _,
                received_flight,
                weight,
                body,
                rise,
                barred: _,
                fumble: _,
                fumble_timer: _,
                cooldown: _,
                celebrate: _,
                stand,
                stand_count: _,
                boxing: _,
                boxed: _,
                posting: _,
                restart_walk: _,
                partner,
                boxer,
                face_wait,
                look,
                left_handed: _,
                dribble: _,
                channel_start,
                flight,
                on_rim: _,
                aim_point,
            } = p;
            team(*t)?;
            floats(&[*rise], "player rise")?;
            floats(&flight.velocity, "player flight")?;
            floats(aim_point, "player aim point")?;
            floats(&flight.moved, "player flight")?;
            floats(&flight.point, "player flight")?;
            floats(&[flight.slide], "player flight")?;
            if let Some(s) = channel_start
                && let Some((_, time)) = s.from
            {
                floats(&[time], "channel start frame")?;
            }
            floats(&[*course, *wanted], "player course")?;
            require(*face_wait >= 0, "face time")?;
            require(
                partner.is_none_or(|k| usize::from(k) < players.len())
                    && boxer.is_none_or(|k| usize::from(k) < players.len()),
                "box-out partners",
            )?;
            require(
                usize::from(look.situation) < crate::look::SITUATIONS.len()
                    && usize::from(look.target) < crate::look::TARGETS.len()
                    && look.timer >= -1,
                "head look",
            )?;
            if let Some(s) = stand {
                let script = crate::stand::STAND_SCRIPTS.get(usize::from(s.script));
                require(script.is_some(), "stand script")?;
                script_clock(&s.run, script.map_or(0, |c| c.steps.len()))?;
            }
            require(*weight > 0, "player weight")?;
            if let Some(b) = body {
                floats(&[b.height, b.width, b.head], "body scales")?;
                require(
                    b.height > 0.0 && b.width > 0.0 && b.head > 0.0,
                    "positive body scales",
                )?;
            }
            require(
                native_dunks.is_none_or(|mask| mask < 1 << 11),
                "native dunk slots",
            )?;
            if let Some(tricks) = native_tricks {
                for (slot, identity) in tricks.slots.iter().enumerate() {
                    require(
                        usize::from(identity.slot) == slot,
                        "native trick slot order",
                    )?;
                    require(
                        identity.level == crate::player::TRICK_SLOT_LEVELS[slot],
                        "native trick slot level",
                    )?;
                    require(identity.script < 75, "native trick script")?;
                    require(
                        identity
                            .equipped_row
                            .is_none_or(|row| (54..=92).contains(&row)),
                        "native trick equipped row",
                    )?;
                }
            }
            let crate::Ratings {
                shots,
                threes,
                dunks,
                rebounds,
                blocks,
                steals,
                handles,
                speed: skill_speed,
                power,
                overall,
                reach,
            } = ratings;
            let skills = [
                *shots,
                *threes,
                *dunks,
                *rebounds,
                *blocks,
                *steals,
                *handles,
                *skill_speed,
                *power,
                *overall,
            ];
            require(
                skills
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                "ratings",
            )?;
            floats(&[*reach], "reach")?;
            require(*reach > 0.0, "positive reach")?;
            floats(position, "player position")?;
            floats(
                &[*heading, *speed, *height, *turbo],
                "player movement/turbo",
            )?;
            require((0.0..=1.0).contains(turbo), "turbo range")?;
            floats(
                &[gait_channel.phase, gait_channel.frame_step],
                "player gait channel",
            )?;
            require(gait_channel.phase >= 0.0, "player gait channel phase")?;
            require(
                gait_channel.frame_step > 0.0,
                "player gait channel frame step",
            )?;
            require(
                gait_channel.blend_counter >= 0,
                "player gait channel blend counter",
            )?;
            require(
                (-1..=i32::from(u16::MAX)).contains(received_flight),
                "player received flight",
            )?;
            floats(&[action.elapsed()], "action elapsed")?;
            require(action.elapsed() >= 0.0, "action elapsed range")?;
            if let Action::Stealing {
                counter, stance, ..
            } = *action
            {
                require(counter == stance.is_some(), "counter stance")?;
            }
            match *action {
                // A swipe's last update past its script's leaves the clock
                // at its span; the next step ends it ([`crate::swipe`]).
                Action::Stealing { swipe: Some(s), .. } => {
                    require(
                        usize::from(s.script) < crate::swipe::SCRIPTS.len() && s.tick <= s.span(),
                        "swipe script and tick",
                    )?;
                    floats(&[s.from.map_or(0.0, |(_, t)| t)], "swipe clip frame")?;
                }
                Action::Stealing {
                    stance: Some(s), ..
                } => {
                    require(
                        usize::from(s.script) < crate::counter::SCRIPTS.len(),
                        "counter stance script",
                    )?;
                    script_clock(&s.run, s.steps().len())?;
                    require((0..0x400).contains(&s.word), "counter stance facing")?;
                    floats(
                        &[s.from.map_or(0.0, |(_, t)| t)],
                        "counter stance clip frame",
                    )?;
                }
                Action::Jumping { block: Some(j), .. } => {
                    require(
                        usize::from(j.script) < crate::block::SCRIPTS.len(),
                        "block jump script",
                    )?;
                    floats(
                        &[j.vy, j.drift[0], j.drift[1], j.from.map_or(0.0, |(_, t)| t)],
                        "block jump",
                    )?;
                }
                Action::None | Action::Stealing { .. } | Action::Jumping { .. } => {}
                Action::Dive { script, .. } => {
                    require(
                        usize::from(script) < crate::timing::DIVE_SCRIPTS.len(),
                        "dive script",
                    )?;
                }
                Action::Scoop { play } => {
                    script_clock(&play.run, crate::touch::SCOOP_STEPS.len())?;
                    floats(&[play.from.map_or(0.0, |(_, t)| t)], "scoop clip frame")?;
                }
                Action::TipJump {
                    script, vy, drift, ..
                } => {
                    require(
                        usize::from(script) < crate::toss::TIP_SCRIPTS.len(),
                        "tip jump script",
                    )?;
                    floats(&[vy, drift[0], drift[1]], "tip jump")?;
                }
                Action::ReboundJump {
                    script,
                    vy,
                    drift,
                    from,
                    word,
                    ..
                } => {
                    require(
                        usize::from(script) < crate::rebound::SCRIPTS.len(),
                        "rebound jump script",
                    )?;
                    require((0..0x400).contains(&word), "rebound jump facing")?;
                    floats(&[vy, drift[0], drift[1]], "rebound jump")?;
                    if let Some((_, time)) = from {
                        floats(&[time], "rebound jump clip")?;
                    }
                }
                Action::PumpFake { script, .. } => {
                    require(
                        usize::from(script) < crate::timing::SHOT_SCRIPTS.len(),
                        "pump fake script",
                    )?;
                }
                Action::Passing { row, .. } => {
                    require(
                        row == crate::timing::NO_PASS_ROW
                            || usize::from(row) < crate::timing::PASS_ROWS.len(),
                        "passing row",
                    )?;
                }
                Action::Shooting {
                    hoop: h,
                    gamebreaker,
                    script,
                    ..
                } => {
                    hoop(h)?;
                    level(gamebreaker, 0, 2, "shooting gamebreaker level")?;
                    require(
                        usize::from(script) < crate::timing::SHOT_SCRIPTS.len(),
                        "shooting script",
                    )?;
                }
                Action::Dunking {
                    hoop: h,
                    from,
                    to,
                    row,
                    ..
                }
                | Action::Layup {
                    hoop: h,
                    from,
                    to,
                    row,
                    ..
                } => {
                    hoop(h)?;
                    floats(&from, "action origin")?;
                    floats(&to, "action target")?;
                    require(
                        row == crate::timing::NO_MOVE_ROW || crate::timing::move_row(row).is_some(),
                        "move row",
                    )?;
                }
                Action::Trick { id, play } => {
                    require(
                        usize::from(id.script) < crate::trick::SCRIPTS.len(),
                        "trick script",
                    )?;
                    level(id.level, 0, 4, "trick value class")?;
                    if id.slot != crate::trick::DRAWN {
                        let configured = native_tricks
                            .unwrap_or_else(crate::NativeTricks::ordinary)
                            .slots
                            .get(usize::from(id.slot))
                            .copied();
                        require(configured == Some(id), "latched trick slot identity")?;
                    }
                    script_clock(
                        &play.run,
                        crate::trick::SCRIPTS[usize::from(id.script)].steps.len(),
                    )?;
                    floats(&play.drift, "trick drift")?;
                    require((0..0x400).contains(&play.word), "trick facing")?;
                    require(
                        play.faked.is_none_or(|d| usize::from(d) < players.len()),
                        "faked defender",
                    )?;
                    if let Some((_, time)) = play.from {
                        floats(&[time], "trick clip frame")?;
                    }
                }
                Action::Stumbling {
                    duration, reaction, ..
                } => {
                    floats(&[duration], "stumble duration")?;
                    require(duration >= 0.0, "stumble duration range")?;
                    if let Some(r) = reaction {
                        require(
                            usize::from(r.script)
                                < crate::trick::REACTIONS.len() + crate::leap::CALL_FOR_BALL.len(),
                            "reaction script",
                        )?;
                        script_clock(&r.run, r.steps().len())?;
                        floats(&r.drift, "reaction drift")?;
                        floats(&[r.vy], "reaction rise")?;
                        if let Some((_, time)) = r.from {
                            floats(&[time], "reaction clip frame")?;
                        }
                    }
                }
            }
        }
        let crate::Ball {
            radius,
            position,
            velocity,
            state,
            shot,
            pass,
            flight_left,
            flight_time,
            flight_start,
            spin_axis,
            spin,
            airborne: _,
            through: _,
            bank: _,
            armed: _,
            ceiling,
            dunked: _,
            rebound,
            touched,
            moved,
            touch,
            long: _,
            shooter,
            rolled: _,
            hits: _,
        } = ball;
        floats(moved, "ball move")?;
        for p in touch.iter().chain([shooter]).flatten() {
            player(usize::from(*p))?;
        }
        floats(&[*ceiling], "court ceiling")?;
        require(!*rebound || *state == BallState::Loose, "rebound flag")?;
        require(!*touched || *state == BallState::Shot, "touched flag")?;
        require(crate::ball::valid_radius(*radius), "native ball radius")?;
        floats(position, "ball position")?;
        floats(velocity, "ball velocity")?;
        floats(
            &[*flight_left, *flight_time, *flight_start],
            "ball flight time",
        )?;
        floats(spin_axis, "ball spin axis")?;
        floats(&[*spin], "ball spin")?;
        require(*spin >= 0.0, "ball spin range")?;
        if let BallState::Held(p) = state {
            player(*p)?;
        }
        if let Some(s) = shot {
            player(s.shooter)?;
            hoop(s.hoop)?;
            level(s.gamebreaker, 0, 2, "shot gamebreaker level")?;
            require(
                s.slam.is_none_or(|b| {
                    matches!(
                        b,
                        crate::Basket::Dunk { .. } | crate::Basket::AlleyOop { .. }
                    ) && s.gamebreaker == 0
                        && !s.layup
                }),
                "shot slam",
            )?;
        }
        if let Some(p) = pass {
            player(p.from)?;
            player(p.to)?;
            require(p.since <= *tick, "pass release tick")?;
            match p.via {
                Via::Direct | Via::Floor => {}
                Via::Head { defender } => player(defender)?,
                Via::Glass { hoop: h } => hoop(h)?,
            }
        }
        require(
            match state {
                BallState::Shot => shot.is_some() && pass.is_none(),
                BallState::Pass => pass.is_some() && shot.is_none(),
                BallState::Held(_) | BallState::Loose => shot.is_none() && pass.is_none(),
            },
            "ball state/flight payload",
        )?;
        let crate::Rules {
            goal,
            win_by: _,
            shot_clock: limit,
            scoring: _,
            dunks_only: _,
            tricks: _,
            gamebreakers: _,
            gamebreakers_score: _,
            spot: _,
            spot_trick_points: _,
            unlimited_turbo: _,
            no_counters: _,
            easy_2_pointers: _,
            hard_2_pointers: _,
            difficulty: _,
        } = rules;
        if let Some(limit) = limit {
            floats(&[*limit], "rules shot clock")?;
            require(*limit > 0.0, "rules shot clock range")?;
        }
        match goal {
            Goal::Gamebreaker(l) => level(*l, 1, 2, "goal gamebreaker level")?,
            Goal::Points(n) | Goal::TrickPoints(n) => require(*n > 0, "goal target")?,
        }
        for t in teams {
            let crate::TeamState {
                trick_points: _,
                combo,
                meter,
                meter_fraction,
                meter_fill,
                meter_scale,
                meter_sequence_fill,
                meter_hold,
                meter_gamebreaker: _,
                meter_cpu_limit: _,
                human: _,
                pocketed: _,
                recent_moves,
                paid: _,
                pump_fake: _,
                possession_seen,
            } = t;
            floats(
                &[
                    *meter_fraction,
                    *meter_fill,
                    *meter_scale,
                    *meter_sequence_fill,
                ],
                "team meter scalars",
            )?;
            require((0.0..1.0).contains(meter_fraction), "meter fraction range")?;
            require(
                *meter_fill >= 0.0 && *meter_scale >= 0.0 && *meter_sequence_fill >= 0.0,
                "meter fill range",
            )?;
            require(
                (0..=crate::street::METER_HOLD).contains(meter_hold),
                "meter hold range",
            )?;
            require(*meter <= crate::street::METER_FULL, "meter capacity")?;
            require(
                (1..=crate::street::MAX_MULTIPLIER).contains(&combo.multiplier),
                "combo multiplier",
            )?;
            require(
                recent_moves
                    .iter()
                    .all(|k| (-3..crate::trick::SCRIPTS.len() as i32).contains(k)),
                "recent trick key",
            )?;
            require((-1..=1).contains(possession_seen), "meter possession")?;
        }
        match *phase {
            Phase::Live => {}
            Phase::DeadBall { left, team: t } => {
                floats(&[left], "dead ball time")?;
                team(t)?;
            }
            Phase::Over { winner } => team(winner)?,
        }
        require(game_phase.valid(), "game phase")?;
        require(
            tip.timing.iter().all(|t| *t == -1 || (1..=20).contains(t))
                && matches!(tip.winner, -1 | 0 | 3),
            "tip-off",
        )?;
        if let crate::OpeningToss::Waiting(left) = *opening_toss {
            require(
                (1..=crate::toss::WAIT_UPDATES).contains(&left),
                "opening toss wait",
            )?;
        }
        if let Some((a, b)) = give_and_go {
            player(*a)?;
            player(*b)?;
        }
        if let Some(crate::passing::Windup {
            from,
            to,
            style: _,
            left,
            row,
            aim:
                crate::PassAim {
                    spot,
                    lead: _,
                    reach: _,
                },
        }) = windup
        {
            player(*from)?;
            player(*to)?;
            floats(spot, "pass aim spot")?;
            require(
                usize::from(*row) < crate::timing::PASS_ROWS.len(),
                "pass windup row",
            )?;
            // A passout to oneself holds the ball to its toss (callback 53,
            // [`crate::passout`]); every other row to its release.
            let release = crate::passout::toss(*row)
                .filter(|_| from == to)
                .map_or(crate::timing::PASS_ROWS[usize::from(*row)].release, |t| {
                    t.tick
                });
            require((1..=release).contains(left), "pass windup")?;
        }
        if let Some(crate::Catch {
            player: p,
            until,
            row,
            flight: _,
            since,
        }) = catch
        {
            player(*p)?;
            require(*until <= *tick + 128, "catch hold")?;
            require(
                usize::from(*row) < crate::timing::CATCH_ROWS.len(),
                "catch row",
            )?;
            require(*since <= *tick, "catch start")?;
        }
        if let Some(s) = scorer {
            player(*s)?;
        }
        require(events.len() <= 256, "event count")?;
        for event in events {
            match *event {
                Event::Shot {
                    player: p, hoop: h, ..
                }
                | Event::Dunk { player: p, hoop: h } => {
                    player(p)?;
                    hoop(h)?;
                }
                Event::Score {
                    team: t,
                    player: p,
                    basket: b,
                    ..
                } => {
                    team(t)?;
                    player(p)?;
                    basket(b)?;
                }
                Event::GamebreakerFrame { script, frame, .. } => {
                    require(script < 70 && frame < 10_000, "staged frame event")?;
                    // Against the plans once installed ([`crate::disc`]).
                    if let Some(config) = staged.as_ref().filter(|c| !c.plans.is_empty()) {
                        require(
                            config.plan(script).is_some_and(|p| frame < p.frame_count),
                            "staged event plan",
                        )?;
                    }
                }
                Event::StagedRerun {
                    script,
                    from,
                    frames,
                    speed,
                    camera,
                    ..
                } => {
                    require(
                        script < 70
                            && frames > 0
                            && frames <= crate::replay::STAGED_SPAN
                            && from < 10_000
                            && (25..=150).contains(&speed)
                            && camera <= 9,
                        "staged replay event",
                    )?;
                    if let Some(config) = staged.as_ref().filter(|c| !c.plans.is_empty()) {
                        require(
                            config
                                .plan(script)
                                .and_then(|p| p.controls.rerun)
                                .is_some_and(|r| r.from == from && r.to - r.from == frames),
                            "staged replay event plan",
                        )?;
                    }
                }
                Event::RimOut | Event::PassoutBounce { .. } => {}
                Event::BallHit { speed, .. } => {
                    if !speed.is_finite() || speed < 0.0 {
                        return Err("invalid ball contact speed".into());
                    }
                }
                Event::Block {
                    player: p,
                    shooter: q,
                }
                | Event::Goaltend {
                    player: p,
                    shooter: q,
                }
                | Event::Pass { from: p, to: q, .. }
                | Event::Launched { from: p, to: q, .. }
                | Event::Steal {
                    player: p, from: q, ..
                }
                | Event::Counter { player: p, from: q }
                | Event::Pick {
                    screener: p,
                    defender: q,
                } => {
                    player(p)?;
                    player(q)?;
                }
                Event::Catch { player: p }
                | Event::Pickup { player: p }
                | Event::Dribble { holder: p, .. }
                | Event::SlamShake { player: p }
                | Event::Take { player: p }
                | Event::RimBump { player: p }
                | Event::Rebound { player: p }
                | Event::Inbound { player: p }
                | Event::Fumble { player: p }
                | Event::Deflection { player: p }
                | Event::Layup { player: p }
                | Event::PumpFake { player: p }
                | Event::PumpFakeBite { player: p }
                | Event::NoTurbo { player: p }
                | Event::TakeOff { player: p, .. }
                | Event::RimHold { player: p, .. }
                | Event::Dive { player: p }
                | Event::Reach { player: p, .. }
                | Event::Jump { player: p, .. } => player(p)?,
                Event::Slam { player: p, hoop: h } => {
                    player(p)?;
                    hoop(h)?;
                }
                Event::ShotClockViolation { team: t }
                | Event::Combo { team: t, .. }
                | Event::GamebreakerReady { team: t }
                | Event::MeterDropped { team: t }
                | Event::Pocket { team: t }
                | Event::Cancel { team: t } => team(t)?,
                Event::GameOver { winner } => team(winner)?,
                Event::ReplayRoll { .. } => {}
                Event::InboundWalk { player: p } => player(p)?,
                Event::TrickStart {
                    player: p,
                    level: l,
                    script,
                } => {
                    player(p)?;
                    level(l, 1, 5, "event trick start level")?;
                    require(
                        usize::from(script) < crate::trick::SCRIPTS.len(),
                        "event trick start script",
                    )?;
                }
                Event::Trick {
                    player: p,
                    level: l,
                    script,
                    fakeout,
                    ..
                } => {
                    player(p)?;
                    level(l, 1, 5, "event trick level")?;
                    require(
                        usize::from(script) < crate::trick::SCRIPTS.len(),
                        "event trick script",
                    )?;
                    if let Some(p) = fakeout {
                        player(p)?;
                    }
                }
                Event::Gamebreaker {
                    player: p,
                    level: l,
                } => {
                    player(p)?;
                    level(l, 1, 2, "event gamebreaker level")?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Hoop, Pass, Player, Rules, Shot};

    fn world() -> World {
        World::new(
            [
                Hoop {
                    rim: [37.0, 11.75, 0.0],
                    rim_radius: 0.75,
                },
                Hoop {
                    rim: [-37.0, 11.75, 0.0],
                    rim_radius: 0.75,
                },
            ],
            vec![
                Player::new(0, [-10.0, 0.0], 0.0),
                Player::new(1, [10.0, 0.0], 0.0),
            ],
            Rules::default(),
            1,
        )
    }

    #[test]
    fn head_passout_identity_round_trips_and_rejects_board_only_state() {
        let mut w = world();
        w.passout = Some(crate::passout::Passout {
            passer: 0,
            kind: crate::passout::Kind::Head,
            defender: Some(1),
            point: [1.0, 6.0, 2.0],
            back: None,
            tossed: true,
            tail: None,
        });
        assert_eq!(w.validate(), Ok(()));
        let restored = World::from_snapshot(&w.to_snapshot()).unwrap();
        assert_eq!(restored.passout, w.passout);
        assert_eq!(restored.checksum(), w.checksum());
        for change in [
            |p: &mut crate::passout::Passout| p.defender = None,
            |p: &mut crate::passout::Passout| p.defender = Some(0),
            |p: &mut crate::passout::Passout| p.defender = Some(2),
            |p: &mut crate::passout::Passout| p.tossed = false,
            |p: &mut crate::passout::Passout| p.tail = Some(1),
        ] {
            let mut bad = w.clone();
            change(bad.passout.as_mut().unwrap());
            assert!(bad.validate().is_err());
            assert!(World::from_snapshot(&bad.to_snapshot()).is_err());
        }
    }

    #[test]
    fn nested_references_are_rejected_during_snapshot_decode() {
        let bad: [fn(&mut World); 18] = [
            |w| w.ball.state = BallState::Held(2),
            |w| w.players[0].team = 2,
            |w| w.possession = 2,
            |w| w.phase = Phase::DeadBall { left: 1.0, team: 2 },
            |w| w.phase = Phase::Over { winner: 2 },
            |w| w.give_and_go = Some((0, 2)),
            |w| {
                w.windup = Some(crate::Windup {
                    from: 0,
                    to: 2,
                    style: crate::PassStyle::Normal,
                    left: 5,
                    row: 10,
                    aim: crate::PassAim {
                        spot: [0.0; 2],
                        lead: true,
                        reach: f32::MAX,
                    },
                })
            },
            |w| {
                w.players[0].action = Action::Shooting {
                    hoop: 2,
                    elapsed: 0.0,
                    gamebreaker: 0,
                    fade: false,
                    tick: 0,
                    script: 0,
                    release: 30,
                    check: 15,
                    end: 40,
                    gate: 26,
                    latest: 47,
                    stall: 0,
                }
            },
            // A shot script past the table (`World::check_pump_fake` reads
            // its row).
            |w| {
                w.players[0].action = Action::Shooting {
                    hoop: 0,
                    elapsed: 0.0,
                    gamebreaker: 0,
                    fade: false,
                    tick: 0,
                    script: crate::timing::SHOT_SCRIPTS.len() as u8,
                    release: 30,
                    check: 15,
                    end: 40,
                    gate: 26,
                    latest: 47,
                    stall: 0,
                }
            },
            |w| {
                w.ball.state = BallState::Pass;
                w.ball.pass = Some(Pass {
                    from: 0,
                    to: 1,
                    tried: 0,
                    via: Via::Head { defender: 2 },
                    catch: crate::ball::NO_CATCH,
                    since: 0,
                });
            },
            |w| {
                w.events.push(Event::Trick {
                    player: 0,
                    level: 1,
                    script: 0,
                    fakeout: Some(2),
                    points: 500,
                })
            },
            |w| {
                w.ball.state = BallState::Shot;
                w.ball.shot = Some(Shot {
                    shooter: 2,
                    hoop: 0,
                    made: false,
                    points: 1,
                    gamebreaker: 0,
                    layup: false,
                    tried: 0,
                    slam: None,
                });
            },
            |w| {
                w.ball.state = BallState::Shot;
                w.ball.shot = Some(Shot {
                    shooter: 0,
                    hoop: 2,
                    made: false,
                    points: 1,
                    gamebreaker: 0,
                    layup: false,
                    tried: 0,
                    slam: None,
                });
            },
            |w| {
                w.ball.state = BallState::Pass;
                w.ball.pass = Some(Pass {
                    from: 2,
                    to: 1,
                    tried: 0,
                    via: Via::Direct,
                    catch: crate::ball::NO_CATCH,
                    since: 0,
                });
            },
            |w| {
                w.ball.state = BallState::Pass;
                w.ball.pass = Some(Pass {
                    from: 0,
                    to: 2,
                    tried: 0,
                    via: Via::Direct,
                    catch: crate::ball::NO_CATCH,
                    since: 0,
                });
            },
            |w| {
                w.ball.state = BallState::Pass;
                w.ball.pass = Some(Pass {
                    from: 0,
                    to: 1,
                    tried: 0,
                    via: Via::Glass { hoop: 2 },
                    catch: crate::ball::NO_CATCH,
                    since: 0,
                });
            },
            |w| {
                w.events.push(Event::Score {
                    team: 2,
                    player: 0,
                    points: 1,
                    basket: Basket::Layup,
                })
            },
            |w| {
                w.events.push(Event::Pick {
                    screener: 0,
                    defender: 2,
                })
            },
        ];
        for modify in bad {
            let mut w = world();
            modify(&mut w);
            assert!(w.validate().is_err());
            assert!(World::from_snapshot(&w.to_snapshot()).is_err());
        }
    }

    #[test]
    fn nan_infinity_and_extreme_finite_values_cannot_enter_a_world() {
        let fields: [fn(&mut World, f32); 20] = [
            |w, v| w.hoops[1].rim[2] = v,
            |w, v| w.hoops[0].rim_radius = v,
            |w, v| w.players[1].position[0] = v,
            |w, v| w.players[0].ratings.reach = v,
            |w, v| w.players[0].ratings.shots = v,
            |w, v| w.players[0].heading = v,
            |w, v| w.ball.velocity[2] = v,
            |w, v| w.ball.flight_left = v,
            |w, v| w.ball.spin = v,
            |w, v| w.ball.spin_axis[1] = v,
            |w, v| w.shot_clock = v,
            |w, v| w.rules.shot_clock = Some(v),
            |w, v| w.teams[1].meter_fraction = v,
            |w, v| {
                w.players[0].action = Action::Stumbling {
                    elapsed: 0.0,
                    duration: v,
                    reaction: None,
                }
            },
            |w, v| w.players[0].height = v,
            |w, v| w.players[0].speed = v,
            |w, v| w.players[0].turbo = v,
            |w, v| w.turbo[3] = v,
            |w, v| w.teams[1].meter_sequence_fill = v,
            |w, v| {
                w.players[0].action = Action::Layup {
                    hoop: 0,
                    elapsed: 0.0,
                    from: [v, 0.0],
                    to: [0.0, 0.0],
                    tick: 0,
                    release: 44,
                    end: 53,
                    row: crate::timing::LAYUP_FALLBACK_ROW as u8,
                }
            },
        ];
        for set in fields {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
                let mut w = world();
                set(&mut w, value);
                assert!(w.validate().is_err());
                assert!(World::from_snapshot(&w.to_snapshot()).is_err());
            }
        }
    }

    #[test]
    fn staged_state_rejects_bad_bindings_clocks_masks_and_nested_nan() {
        use crate::gamebreaker::{Config, Controls, Hold, Sequence, SequencePlan};
        let mut valid = world();
        let mut config = Config::default();
        config.plans.push(SequencePlan {
            script: 1,
            kind: 1,
            frame_count: 10,
            actors: vec![],
            curves: vec![],
            ball_cues: vec![],
            finishes: vec![],
            controls: Controls {
                hold: Some(Hold {
                    frame: 2,
                    duration: 5,
                    camera: 0,
                    holds_replays: true,
                }),
                ..Default::default()
            },
        });
        valid.staged = Some(config.into());
        valid.sequence = Some(Sequence {
            script: 1,
            shooter: 0,
            team: 0,
            level: 2,
            frame: 2,
            held: 3,
            actors: [Some(0), Some(1), None, None, None, None, None],
            native_clocks: [None; 7],
            resume_pending: false,
            finishing: 0,
        });
        valid.validate().unwrap();
        let back = World::from_snapshot(&valid.to_snapshot()).unwrap();
        assert_eq!(back.with_tables_of(&valid), valid);
        // A machine reading a bad world's snapshot refuses it: at the read,
        // or once its own tables are under it (crate::disc).
        let refused = |w: &World| {
            World::from_snapshot(&w.to_snapshot()).is_err()
                || World::from_snapshot(&w.to_snapshot())
                    .is_ok_and(|back| back.with_tables_of(w).validate().is_err())
        };
        let bad: [fn(&mut World); 12] = [
            |w| w.sequence.as_mut().unwrap().script = 70,
            |w| w.sequence.as_mut().unwrap().level = 1,
            |w| w.sequence.as_mut().unwrap().team = 2,
            |w| w.sequence.as_mut().unwrap().team = 1,
            |w| w.sequence.as_mut().unwrap().shooter = 2,
            |w| w.sequence.as_mut().unwrap().frame = 10,
            |w| w.sequence.as_mut().unwrap().frame = 3,
            |w| w.sequence.as_mut().unwrap().held = 6,
            |w| w.sequence.as_mut().unwrap().actors[1] = Some(0),
            |w| w.sequence.as_mut().unwrap().actors[1] = Some(2),
            |w| w.variety.used[12] = 16,
            |w| w.staged = None,
        ];
        for change in bad {
            let mut w = valid.clone();
            change(&mut w);
            assert!(w.validate().is_err());
            assert!(refused(&w));
        }
        let mut w = valid;
        std::sync::Arc::make_mut(w.staged.as_mut().unwrap()).plans[0]
            .curves
            .push(crate::gamebreaker::Curve {
                slot: 0,
                field: 0,
                start_frame: 0,
                end_frame: 2,
                start: f32::NAN,
                end: 0.0,
                start_tangent: 0.0,
                end_tangent: 0.0,
                interpolation: 1,
            });
        assert!(w.validate().is_err());
        assert!(refused(&w));
    }

    #[test]
    fn valid_play_remains_valid_through_snapshots() {
        let mut w = world();
        for _ in 0..5000 {
            let input = (0..w.players.len())
                .map(|i| crate::ai::input(&w, i))
                .collect::<Vec<_>>();
            w.step(&input);
            w.validate().unwrap();
            assert_eq!(World::from_snapshot(&w.to_snapshot()).unwrap(), w);
        }
    }
}
