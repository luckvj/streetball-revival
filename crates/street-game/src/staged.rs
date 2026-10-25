//! Disc timelines shared by the renderer and the deterministic referee.
//!
//! The referee's plans are built by `street-net`'s `disc` (every machine in
//! a match builds them from its own disc, the game for a match played
//! alone too); the intros use the same.

pub use street_net::disc::staged::{attach_hands, attach_playback, config, controls, plan};

#[cfg(test)]
mod tests {
    use super::*;
    use street_assets::timeline::{Payload, ScriptBank};
    use street_sim::gamebreaker::{
        ActorPose, ActorTrack, Config, Controls, Hold, Rerun, SequencePlan,
    };

    /// A staged run in a School capture (`tools/pine_played_capture.py`
    /// schema 3, `%TEMP%/street-played-school-<n>.jsonl`): each update's
    /// six actors' places (x, height, z) from the press on, the script
    /// manager's state (+0x50) by update, and the presser.
    struct StagedRun {
        capture: String,
        press: i64,
        shooter: usize,
        frames: Vec<(i64, i64, [[f32; 3]; 6])>,
    }

    fn staged_runs() -> Vec<StagedRun> {
        let Some(list) = std::env::var_os("STREET_PLAYED_CAPTURE") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for path in std::env::split_paths(&list) {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let mut pointers: Vec<u64> = Vec::new();
            let mut rows: Vec<(i64, i64, [[f32; 3]; 6], Option<usize>)> = Vec::new();
            for line in text.lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                if v["event"] == "pointers" {
                    pointers = v["actors"].as_array().map_or(Vec::new(), |a| {
                        a.iter().filter_map(serde_json::Value::as_u64).collect()
                    });
                    continue;
                }
                if v.get("frame").is_none() || v["repeat"].as_bool() == Some(true) {
                    continue;
                }
                let Some(state) = v["script_words"][8].as_i64() else {
                    continue;
                };
                let places = std::array::from_fn(|k| {
                    let p = &v["actors"][k]["position"];
                    [0, 1, 2].map(|q| p[q].as_f64().unwrap_or(0.0) as f32)
                });
                let holder = v["ball"]["holder"]
                    .as_u64()
                    .and_then(|h| pointers.iter().position(|&a| a == h));
                rows.push((v["frame"].as_i64().unwrap(), state, places, holder));
            }
            for k in 1..rows.len() {
                if !(3..=7).contains(&rows[k].1) || (3..=7).contains(&rows[k - 1].1) {
                    continue;
                }
                let Some(shooter) = rows[k - 1].3 else {
                    continue;
                };
                let end = (k..rows.len())
                    .find(|&j| !(3..=7).contains(&rows[j].1))
                    .unwrap_or(rows.len());
                out.push(StagedRun {
                    capture: path.file_name().unwrap().to_string_lossy().into_owned(),
                    press: rows[k - 1].0,
                    shooter,
                    frames: rows[k - 1..end].iter().map(|r| (r.0, r.1, r.2)).collect(),
                });
            }
        }
        out
    }

    #[test]
    fn the_schools_level_2_run_plays_a_disc_plan_from_two_updates_after_the_press() {
        // The original running a staged Level 2 (the School's Level 2
        // lesson; the earlier staged evidence froze the disc's plans, not
        // a run): the family by the presser's spot (`FUN_00141298`), one
        // of its variants (`FUN_001851f0`), the roles bound by the plan's
        // kind, then the plan's frames one an update. The capture's actors
        // stand on the plan's frame 0 two updates after the press (the
        // task's first update starts the script, `FUN_00144ef8` state 0;
        // the script manager poses it in the next) and follow its track;
        // the freeze (`Hold`) suspends the game's counter, so its updates
        // are not in the capture's numbering.
        let Ok(iso) = std::env::var("STREET_ISO") else {
            return;
        };
        let runs = staged_runs();
        if runs.is_empty() {
            return;
        }
        let mut iso = street_assets::iso::Iso::open(iso).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let config = config(&bank).unwrap();
        let mut rows = Vec::new();
        for run in &runs {
            let (_, _, at_press) = run.frames[0];
            let players: Vec<street_sim::Player> = (0..6)
                .map(|k| street_sim::Player::new(k / 3, [at_press[k][0], at_press[k][2]], 0.0))
                .collect();
            let family = config
                .regions
                .family(players[run.shooter].position, players[run.shooter].team);
            // Every plan by its fit: the mean floor distance of the bound
            // actors over the run at the plan's frame `f - press - offset`.
            let mut best: Option<(f32, usize, i64, f32, [Option<usize>; 7])> = None;
            let mirrored = street_sim::restart::side(players[run.shooter].team) > 0.0;
            for plan in &config.plans {
                let actors = street_sim::gamebreaker::bind(
                    &players,
                    run.shooter,
                    plan.kind,
                    plan.initial_positions(),
                );
                for offset in 0..4i64 {
                    let (mut sum, mut n, mut worst) = (0.0f32, 0u32, 0.0f32);
                    for &(f, _, places) in &run.frames[1..] {
                        let frame = f - run.press - offset;
                        if frame < 0 || frame as u32 >= plan.length() {
                            continue;
                        }
                        let pose = plan.spatial_pose_at(frame as f32);
                        for (slot, actor) in actors.iter().enumerate() {
                            let (Some(k), Some(p)) = (actor, pose[slot]) else {
                                continue;
                            };
                            let x = if mirrored {
                                -p.position[0]
                            } else {
                                p.position[0]
                            };
                            let d = ((x - places[*k][0]).powi(2)
                                + (p.position[2] - places[*k][2]).powi(2))
                            .sqrt();
                            sum += d;
                            worst = worst.max(d);
                            n += 1;
                        }
                    }
                    let mean = sum / n.max(1) as f32;
                    if n > 0 && best.is_none_or(|b| mean < b.0) {
                        best = Some((mean, plan.script, offset, worst, actors));
                    }
                }
            }
            let (mean, script, offset, worst, actors) = best.unwrap();
            let in_family = family.is_some_and(|f| {
                config.families[usize::from(f)]
                    .iter()
                    .flatten()
                    .any(|&s| s == script)
            });
            let plan = config.plan(script).unwrap();
            rows.push(serde_json::json!({
                "capture": run.capture, "press": run.press, "shooter": run.shooter,
                "family": family, "script": script, "in_family": in_family,
                "offset": offset, "mean": mean, "worst": worst,
                "actors": actors, "frames": run.frames.len(), "plan_length": plan.length(),
                "stop": plan.controls.stop,
                "hold": plan.controls.hold.map(|h| [h.frame, h.duration]),
            }));
        }
        if let Some(dir) = std::env::var_os("STREET_PLAYED_CAPTURE_REPORT") {
            let path = std::path::PathBuf::from(dir).join("staged_runs.json");
            std::fs::write(path, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        }
        for row in &rows {
            assert!(row["in_family"] == true && row["offset"] == 2, "{rows:?}");
            assert!(row["mean"].as_f64().unwrap() < 0.25, "{rows:?}");
        }
    }

    #[test]
    fn production_staged_reference_positions_match_original_capture() {
        let Ok(path) = std::env::var("STREET_STAGED_PATH_CAPTURE") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(std::env::var("STREET_ISO").unwrap()).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let clips = street_render::animation::Clips::load_all(&mut iso).unwrap();
        let mut config = config(&bank).unwrap();
        // Use the same native playback/position policy installed by the
        // referee, not the unresolved authored-plan fallback.
        for plan in &mut config.plans {
            attach_playback(plan, &bank, &clips).unwrap();
        }
        let text = std::fs::read_to_string(path).unwrap();
        let mut rows = Vec::new();
        let mut worst = 0.0f32;
        for line in text.lines() {
            let v: Vec<u32> = line
                .split_whitespace()
                .map(|x| x.parse().unwrap())
                .collect();
            assert_eq!(v.len(), 9);
            let plan = config.plan(v[2] as usize).unwrap();
            let mut actors = [None; 7];
            actors[v[3] as usize] = Some(0);
            let sequence = street_sim::gamebreaker::Sequence {
                script: v[2] as usize,
                shooter: 0,
                team: v[5] as usize,
                level: 2,
                frame: v[4],
                held: 0,
                actors,
                native_clocks: [None; 7],
                resume_pending: false,
                finishing: 0,
            };
            let mut players = vec![street_sim::Player::new(v[5] as usize, [0.0; 2], 0.0)];
            let _ = plan.apply(&sequence, &mut players).count();
            let model = [
                players[0].position[0],
                players[0].height,
                players[0].position[1],
            ];
            let native = [v[6], v[7], v[8]].map(f32::from_bits);
            let error = std::array::from_fn::<_, 3, _>(|k| (model[k] - native[k]).abs());
            worst = error.into_iter().fold(worst, f32::max);
            rows.push(
                serde_json::json!({"sample":v[0],"actor_id":v[1],"script":v[2],
                "slot":v[3],"frame":v[4],"team":v[5],"native":native,"model":model,"error":error}),
            );
        }
        let report =
            serde_json::json!({"observations":rows.len(),"max_component_error":worst,"cases":rows});
        std::fs::write(
            std::env::var("STREET_STAGED_PATH_REPORT").unwrap(),
            serde_json::to_string_pretty(&report).unwrap(),
        )
        .unwrap();
        eprintln!(
            "original staged reference placements: observations={} max_component_error={worst}",
            rows.len()
        );
        assert!(!rows.is_empty(), "no admitted original positions");
        assert!(
            worst <= 1e-4,
            "original staged reference placements differ; every residual retained"
        );
    }

    #[test]
    fn production_database_height_scale_matches_guarded_own_skeleton_capture() {
        use std::fmt::Write;

        let Ok(capture) = std::env::var("STREET_STAGED_HEIGHT_CAPTURE") else {
            return;
        };
        let source = std::fs::read_to_string(capture).unwrap();
        let mut iso = street_assets::iso::Iso::open(std::env::var("STREET_ISO").unwrap()).unwrap();
        let players = street_assets::db::players(&mut iso).unwrap();
        let mut cases = String::new();
        let mut observations = 0;
        let mut exact = 0;
        let mut nearest_baseline = 0;
        for line in source.lines() {
            let v: Vec<u32> = line
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(v.len(), 4);
            let player = &players[v[2] as usize];
            let model = player.height_scale().to_bits();
            let baseline = (f32::from(player.model_height_in) * 1.2 / 78.0).to_bits();
            exact += usize::from(model == v[3]);
            nearest_baseline += usize::from(baseline == v[3]);
            if observations != 0 {
                cases.push(',');
            }
            write!(cases,"{{\"sample\":{},\"actor_id\":{},\"player_record\":{},\"model_height\":{},\"native_scale_bits\":{},\"model_scale_bits\":{model},\"nearest_baseline_bits\":{baseline}}}",v[0],v[1],v[2],player.model_height_in,v[3]).unwrap();
            observations += 1;
        }
        assert!(observations > 0);
        let report = format!(
            "{{\"observations\":{observations},\"exact_scales\":{exact},\"nearest_baseline_exact_scales\":{nearest_baseline},\"cases\":[{cases}]}}\n"
        );
        std::fs::write(
            std::env::var("STREET_STAGED_HEIGHT_REPORT").unwrap(),
            report,
        )
        .unwrap();
        eprintln!(
            "original own skeleton scale: observations={observations} exact={exact} nearest_baseline={nearest_baseline}"
        );
        assert_eq!(exact, observations, "original own skeleton scale differs");
    }

    #[test]
    fn production_native_staged_motion_matches_original_capture() {
        use std::fmt::Write;
        use street_sim::gamebreaker::NativeMotion;

        let Ok(capture) = std::env::var("STREET_STAGED_MOTION_CAPTURE") else {
            return;
        };
        let source = std::fs::read_to_string(capture).unwrap();
        let mut iso = street_assets::iso::Iso::open(std::env::var("STREET_ISO").unwrap()).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let clips = street_render::animation::Clips::load_all(&mut iso).unwrap();
        let native_table =
            std::fs::read_to_string(std::env::var("STREET_STAGED_MOTION_TABLE").unwrap()).unwrap();
        let captured_rows: Vec<_> = native_table
            .lines()
            .map(|line| {
                let v: Vec<u32> = line
                    .split_whitespace()
                    .map(|s| s.parse().unwrap())
                    .collect();
                assert_eq!(v.len(), 3);
                (v[0] as u16, v[1] as u16, v[2])
            })
            .collect();
        assert_eq!(captured_rows.len(), 108);
        assert_eq!(bank.staged_loop_rows(), captured_rows);
        let plans: Vec<_> = bank
            .scripts
            .iter()
            .enumerate()
            .map(|(index, timeline)| {
                let mut resolved = plan(timeline, index);
                attach_playback(&mut resolved, &bank, &clips).unwrap();
                resolved
            })
            .collect();
        let mut report_cases = String::new();
        let mut observations = 0;
        let mut exact = 0;
        let mut candidate_exact = 0;
        let mut blocked = 0;
        for line in source.lines() {
            let v: Vec<u32> = line
                .split_whitespace()
                .map(|s| s.parse().unwrap())
                .collect();
            assert_eq!(v.len(), 16);
            let actor = &plans[v[2] as usize].actors[v[3] as usize];
            let playback = actor.playback.unwrap();
            assert!(playback.looping());
            assert!(
                playback.loop_distances.contains(&Some(v[5])),
                "original row root distance differs"
            );
            let motion = NativeMotion {
                position: [
                    f32::from_bits(v[6]),
                    f32::from_bits(v[7]),
                    f32::from_bits(v[8]),
                ],
                previous: [
                    f32::from_bits(v[9]),
                    f32::from_bits(v[10]),
                    f32::from_bits(v[11]),
                ],
                height: f32::from_bits(v[12]),
                scale: f32::from_bits(v[13]),
                last_tick: v[14],
                blending: false,
            };
            let candidate = motion.step(f32::from_bits(v[5])).unwrap();
            // On the first unblended actor update, the preceding manager
            // still saw the blend flag. AB restores the source-configured
            // rate; motion matching first applies on the next manager tick.
            let was_blending = playback.blend_ticks() != 0 && v[4] == playback.blend_ticks();
            let model = if was_blending {
                playback.step()
            } else {
                candidate
            };
            candidate_exact += usize::from(candidate.to_bits() == v[15]);
            exact += usize::from(model.to_bits() == v[15]);
            blocked += usize::from(was_blending);
            if observations != 0 {
                report_cases.push(',');
            }
            write!(report_cases, "{{\"sample\":{},\"actor_id\":{},\"script\":{},\"actor_track\":{},\"elapsed\":{},\"native_step_bits\":{},\"candidate_step_bits\":{},\"model_step_bits\":{},\"manager_blend_blocked\":{was_blending}}}",
                v[0], v[1], v[2], v[3], v[4], v[15], candidate.to_bits(), model.to_bits()).unwrap();
            observations += 1;
        }
        assert!(observations > 0);
        let report = format!(
            "{{\"observations\":{observations},\"exact_steps\":{exact},\"candidate_exact_steps\":{candidate_exact},\"manager_blend_blocked\":{blocked},\"cases\":[{report_cases}]}}\n"
        );
        std::fs::write(
            std::env::var("STREET_STAGED_MOTION_REPORT").unwrap(),
            report,
        )
        .unwrap();
        eprintln!(
            "original staged motion: observations={observations} exact_steps={exact} candidate_exact_steps={candidate_exact} manager_blend_blocked={blocked}"
        );
        assert_eq!(exact, observations, "staged motion rate differs");
    }

    /// The adapter fixes source/VM/capture admission before calculating any
    /// residual. Every admitted observation is evaluated by the production
    /// resolver and clock, and retained even when its result differs.
    #[test]
    fn production_native_staged_clock_matches_original_capture() {
        use std::fmt::Write;

        let Ok(capture) = std::env::var("STREET_STAGED_CLOCK_CAPTURE") else {
            return;
        };
        let source = std::fs::read_to_string(capture).unwrap();
        let mut iso = street_assets::iso::Iso::open(std::env::var("STREET_ISO").unwrap()).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let clips = street_render::animation::Clips::load_all(&mut iso).unwrap();
        let players = street_assets::db::players(&mut iso).unwrap();
        let plans: Vec<_> = bank
            .scripts
            .iter()
            .enumerate()
            .map(|(index, timeline)| {
                let mut resolved = plan(timeline, index);
                attach_playback(&mut resolved, &bank, &clips).unwrap();
                resolved
            })
            .collect();
        let mut cases = String::new();
        let mut observations = 0;
        let mut exact_frames = 0;
        let mut exact_steps = 0;
        let mut max_frame_ulps = 0;
        let mut max_step_ulps = 0;
        let mut stateful_exact_frames = 0;
        let mut stateful_exact_steps = 0;
        let mut stateful_exact_counters = 0;
        let mut cache = std::collections::HashMap::new();
        for line in source.lines() {
            let values: Vec<u32> = line
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(values.len(), 10, "invalid guarded observation");
            let [
                sample,
                actor_id,
                script,
                actor_track,
                elapsed,
                native_frame,
                native_step,
                player_record,
                native_row,
                native_counter,
            ] = values[..].try_into().unwrap();
            assert!(elapsed <= 10_000, "outside the admitted clock bound");
            let actor = &plans[script as usize].actors[actor_track as usize];
            let playback = actor.playback.unwrap();
            let phase = playback.phase_at(elapsed as f32, actor.duration);
            assert!(!phase.blending, "previous-row blending was not admitted");
            let model_frame = phase.channel_frame.to_bits();
            let model_step = playback.step().to_bits();
            let role = usize::from(actor.slot);
            let mirrored = native_row != u32::from(actor.pose.animation);
            let key = (script, role, player_record, mirrored);
            let clocks = cache.entry(key).or_insert_with(|| {
                let plan = &plans[script as usize];
                let mut sequence = street_sim::gamebreaker::Sequence {
                    script: script as usize,
                    shooter: role.min(5),
                    team: usize::from(!mirrored),
                    level: 2,
                    frame: 0,
                    held: 0,
                    actors: [Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), None],
                    native_clocks: [None; 7],
                    resume_pending: false,
                    finishing: 0,
                };
                let mut scales = [1.0; 6];
                if role < 6 {
                    scales[role] = players[player_record as usize].height_scale();
                }
                let mut clocks = Vec::new();
                for tick in 0..plan.frame_count {
                    sequence.frame = tick;
                    // This actor-only event comes from the original hold
                    // lifecycle, never from a fitted clock residual.
                    if plan.controls.hold.is_some_and(|hold| hold.frame == tick) {
                        sequence.resume_native_actors(plan);
                    }
                    sequence.update_native_clocks(plan, &scales);
                    clocks.push(sequence.native_clocks[role]);
                }
                clocks
            });
            let stateful = clocks[(actor.frame + elapsed) as usize].unwrap();
            stateful_exact_frames += usize::from(stateful.frame_bits == native_frame);
            stateful_exact_steps += usize::from(stateful.step_bits == native_step);
            stateful_exact_counters += usize::from(stateful.counter == native_counter);
            let frame_ulps = model_frame.abs_diff(native_frame);
            let step_ulps = model_step.abs_diff(native_step);
            exact_frames += usize::from(frame_ulps == 0);
            exact_steps += usize::from(step_ulps == 0);
            max_frame_ulps = max_frame_ulps.max(frame_ulps);
            max_step_ulps = max_step_ulps.max(step_ulps);
            if observations != 0 {
                cases.push(',');
            }
            write!(
                cases,
                "{{\"sample\":{sample},\"actor_id\":{actor_id},\"script\":{script},\"actor_track\":{actor_track},\"elapsed\":{elapsed},\"native_frame_bits\":{native_frame},\"model_frame_bits\":{model_frame},\"native_step_bits\":{native_step},\"model_step_bits\":{model_step},\"frame_ulps\":{frame_ulps},\"step_ulps\":{step_ulps},\"stateful_frame_bits\":{},\"stateful_step_bits\":{},\"native_counter\":{native_counter},\"stateful_counter\":{}}}", stateful.frame_bits, stateful.step_bits, stateful.counter
            )
            .unwrap();
            observations += 1;
        }
        assert!(
            observations > 0,
            "capture contains no admitted observations"
        );
        let report = format!(
            "{{\"observations\":{observations},\"exact_frames\":{exact_frames},\"exact_steps\":{exact_steps},\"stateful_exact_frames\":{stateful_exact_frames},\"stateful_exact_steps\":{stateful_exact_steps},\"stateful_exact_counters\":{stateful_exact_counters},\"max_frame_ulps\":{max_frame_ulps},\"max_step_ulps\":{max_step_ulps},\"cases\":[{cases}]}}\n"
        );
        std::fs::write(std::env::var("STREET_STAGED_CLOCK_REPORT").unwrap(), report).unwrap();
        eprintln!(
            "original staged clocks: observations={observations} exact_frames={exact_frames} exact_steps={exact_steps} max_frame_ulps={max_frame_ulps} max_step_ulps={max_step_ulps}"
        );
        eprintln!(
            "stateful source clocks: exact_frames={stateful_exact_frames} exact_steps={stateful_exact_steps} exact_counters={stateful_exact_counters}"
        );
        assert_eq!(
            stateful_exact_steps, observations,
            "stateful configured step differs"
        );
        assert_eq!(
            stateful_exact_frames, observations,
            "stateful channel clock differs"
        );
        assert_eq!(
            stateful_exact_counters, observations,
            "stateful channel counter differs"
        );
    }

    #[test]
    fn all_194_disc_actor_records_retain_native_playback_and_fit_six_player_setup() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let clips = street_render::animation::Clips::load_all(&mut iso).unwrap();
        let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
        let mut bank_records = 0;
        for (index, timeline) in bank.scripts.iter().enumerate() {
            let mut resolved = plan(timeline, index);
            attach_playback(&mut resolved, &bank, &clips).unwrap();
            bank_records += resolved.actors.len();
        }
        assert_eq!(
            bank_records, 571,
            "intros use the same resolved playback inputs"
        );
        let players = street_assets::db::players(&mut iso).unwrap();
        let scales: Vec<_> = players
            .iter()
            .take(6)
            .map(|player| player.height_scale())
            .collect();
        let bodies: Vec<_> = players
            .iter()
            .take(6)
            .map(|player| player.native_body_scale())
            .collect();
        let mut config = config(&bank).unwrap();
        attach_hands(&mut config, &bank, &clips, &skeleton).unwrap();
        // The match's own values: each player's model scale and width.
        config.player_width_ratios = bodies
            .iter()
            .copied()
            .map(street_render::animation::native_body_width_ratio)
            .collect();
        config.player_scales = scales;
        config.validate().unwrap();
        let mut records = 0;
        let mut loops = 0;
        let mut adjusted_finite = 0;
        let mut seek_without_sample = 0;
        let mut edge_checks = 0;
        for plan in &config.plans {
            let sources: Vec<_> = bank.scripts[plan.script]
                .records
                .iter()
                .filter_map(|record| match &record.payload {
                    Payload::Actor(actor) => Some(actor),
                    _ => None,
                })
                .collect();
            assert_eq!(sources.len(), plan.actors.len());
            let mut started = [false; 7];
            for (actor, source) in plan.actors.iter().zip(sources) {
                let playback = actor
                    .playback
                    .expect("every original actor needs resolved VM inputs");
                assert_eq!(playback.flags, source.flags);
                assert_eq!(playback.speed_word, source.words[0]);
                assert_eq!(playback.end_fraction, source.animation_end);
                assert_eq!(
                    playback.loop_distances,
                    [
                        bank.staged_loop_distance(source.animation, false),
                        bank.staged_loop_distance(source.animation, true)
                    ]
                );
                assert_eq!(playback.initial, !started[usize::from(actor.slot)]);
                started[usize::from(actor.slot)] = true;
                let name = bank
                    .animation(actor.pose.animation, false)
                    .unwrap()
                    .name
                    .as_deref()
                    .unwrap();
                assert_eq!(
                    usize::from(playback.frames),
                    clips.get(name).unwrap().frames()
                );
                records += 1;
                loops += usize::from(playback.looping());
                if !playback.initial && !playback.looping() {
                    assert!(playback.step() > actor.animation_speed);
                    adjusted_finite += 1;
                }
                for elapsed in [
                    0,
                    1,
                    5,
                    6,
                    7,
                    actor.duration - 1,
                    actor.duration,
                    actor.duration + 1,
                    10_000,
                ] {
                    let phase = playback.phase_at(elapsed as f32, actor.duration);
                    let limit = if playback.looping() {
                        f32::from(playback.frames)
                    } else {
                        f32::from(playback.frames - 1)
                    };
                    assert!(
                        phase.channel_frame >= 0.0 && phase.channel_frame <= limit,
                        "script{} {} tick{elapsed}: {phase:?}",
                        plan.script,
                        name
                    );
                    assert!(
                        phase.display_frame >= 0.0 && phase.display_frame <= limit,
                        "script{} {} tick{elapsed}: {phase:?}",
                        plan.script,
                        name
                    );
                    seek_without_sample += usize::from(phase.channel_frame != phase.display_frame);
                    edge_checks += 1;
                }
            }
        }
        assert_eq!(records, 194);
        assert_eq!(loops, 54);
        assert_eq!(adjusted_finite, 91);
        assert!(seek_without_sample > 0);
        assert_eq!(config.hands.len(), 121);
        let encoded = config.encode();
        assert_eq!(&encoded[..4], b"GBC\x09");
        assert!(encoded.len() <= 128 * 1024);
        assert!(encoded.len() <= 192 * 1024);
        assert_eq!(Config::decode(&encoded).unwrap(), config);
        eprintln!(
            "staged GBC9 bank_records={bank_records} records={records} looping={loops} adjusted_finite={adjusted_finite} edge_checks={edge_checks} channel_vs_sample_differences={seek_without_sample} hand_tracks={} compressed_bytes={} inflated_bytes={}",
            config.hands.len(),
            encoded.len(),
            u32::from_le_bytes(encoded[4..8].try_into().unwrap())
        );
    }

    #[test]
    fn disc_native_clocks_freeze_before_the_hold_tick_and_resume_before_the_manager() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let clips = street_render::animation::Clips::load_all(&mut iso).unwrap();
        let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
        let mut config = config(&bank).unwrap();
        attach_hands(&mut config, &bank, &clips, &skeleton).unwrap();
        config.player_scales = vec![1.0; 6];
        config.player_width_ratios = vec![1.0; 6];
        let plan = config.plan(10).unwrap();
        assert_eq!(bank.scripts[10].name, "GamLR3");
        let hold = plan.controls.hold.unwrap();
        assert_eq!((hold.frame, hold.duration), (81, 60));
        let positions = plan.initial_positions();
        let players = (0..6)
            .map(|i| street_sim::Player::new(i / 3, positions[i].unwrap(), 0.0))
            .collect();
        let hoops = [
            street_sim::Hoop {
                rim: [40.0, 10.0, 0.0],
                rim_radius: 1.0,
            },
            street_sim::Hoop {
                rim: [-40.0, 10.0, 0.0],
                rim_radius: 1.0,
            },
        ];
        let mut world = street_sim::World::new(hoops, players, street_sim::Rules::default(), 2002);
        world.phase = street_sim::Phase::Live;
        world.ball.hold(0);
        world.staged = Some(std::sync::Arc::new(config));
        world.sequence = Some(street_sim::gamebreaker::Sequence {
            script: 10,
            shooter: 0,
            team: 0,
            level: 2,
            frame: 0,
            held: 0,
            actors: [Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), None],
            native_clocks: [None; 7],
            resume_pending: false,
            finishing: 0,
        });
        for _ in 0..hold.frame {
            world.step(&[]);
        }
        let before = world.sequence.unwrap();
        assert_eq!(before.frame, 81);
        assert_eq!(before.pose_frame(), 80);
        assert_eq!(before.native_clocks[0].unwrap().counter, 20);
        let frozen_positions: Vec<_> = world.players.iter().map(|p| p.position).collect();
        for _ in 0..hold.duration {
            world.step(&[]);
            let sequence = world.sequence.unwrap();
            assert_eq!(sequence.frame, 81);
            assert_eq!(sequence.native_clocks, before.native_clocks);
            assert_eq!(
                world.players.iter().map(|p| p.position).collect::<Vec<_>>(),
                frozen_positions
            );
        }
        let bytes = world.to_snapshot();
        assert!(bytes.len() <= 192 * 1024);
        eprintln!(
            "staged stateful held world: snapshot_bytes={} sequence_bytes={}",
            bytes.len(),
            world.sequence.unwrap().encode().len()
        );
        // The plans are the disc's, installed by the machine reading the
        // snapshot (`street_sim::disc`).
        let mut restored = street_sim::World::from_snapshot(&bytes)
            .unwrap()
            .with_tables_of(&world);
        assert_eq!(restored, world);
        world.step(&[]);
        restored.step(&[]);
        assert_eq!(restored, world);
        let resumed = world.sequence.unwrap();
        assert!(resumed.resume_pending);
        assert_eq!(resumed.frame, 81);
        assert_eq!(resumed.pose_frame(), 80);
        assert_eq!(resumed.native_clocks[0].unwrap().counter, 21);
        assert_eq!(
            street_sim::World::from_snapshot(&world.to_snapshot())
                .unwrap()
                .with_tables_of(&world),
            world
        );
        world.step(&[]);
        restored.step(&[]);
        assert_eq!(restored, world);
        let forward = world.sequence.unwrap();
        assert!(!forward.resume_pending);
        assert_eq!(forward.frame, 82);
        assert_eq!(forward.pose_frame(), 81);
        assert_eq!(forward.native_clocks[0].unwrap().counter, 22);
        for _ in 0..80 {
            world.step(&[]);
            restored.step(&[]);
            assert_eq!(restored, world);
        }
        let mut invalid = world.clone();
        if let Some(clock) = invalid
            .sequence
            .as_mut()
            .and_then(|s| s.native_clocks[0].as_mut())
        {
            clock.frame_bits = f32::NAN.to_bits();
        }
        assert!(invalid.validate().is_err());
        let mut invalid_counter = world.clone();
        invalid_counter.sequence.as_mut().unwrap().native_clocks[0]
            .as_mut()
            .unwrap()
            .counter = u32::MAX;
        assert!(invalid_counter.validate().is_err());
        // Its counter is checked against the plan's clip, once the
        // machine's tables are under the snapshot (`street_sim::disc`).
        assert!(
            street_sim::World::from_snapshot(&invalid_counter.to_snapshot())
                .unwrap()
                .with_tables_of(&invalid_counter)
                .validate()
                .is_err()
        );

        // Exercise every shipped actor replacement through the mutable VM,
        // including AB's -1 counter sentinel when a loop leaves its blend.
        // Geometry was checked above; omit its large immutable tables from
        // this per-tick validation so the check measures clock transitions.
        let mut runtime_config = world.staged.as_ref().unwrap().as_ref().clone();
        runtime_config.hands.clear();
        let runtime_config = std::sync::Arc::new(runtime_config);
        world.staged = Some(runtime_config.clone());
        let mut tracks = std::collections::HashSet::new();
        let mut later_loops = std::collections::HashSet::new();
        let reject = |invalid: street_sim::World, reason: &str| {
            assert_eq!(
                invalid.validate().unwrap_err(),
                format!("invalid world field: {reason}")
            );
            // Refused at the read, or once the machine's tables are under
            // it (`street_sim::disc`).
            assert!(
                street_sim::World::from_snapshot(&invalid.to_snapshot())
                    .is_ok_and(|back| back.with_tables_of(&invalid).validate().is_err())
                    || street_sim::World::from_snapshot(&invalid.to_snapshot()).is_err()
            );
        };
        let mut checked_missing = false;
        let mut checked_current_phase = false;
        let mut checked_future_blend = false;
        let mut checked_direct_blend = false;
        for plan in &runtime_config.plans {
            let mut sequence = street_sim::gamebreaker::Sequence {
                script: plan.script,
                shooter: 0,
                team: 0,
                level: 2,
                frame: 0,
                held: 0,
                actors: [Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), None],
                native_clocks: [None; 7],
                resume_pending: false,
                finishing: 0,
            };
            for tick in 0..plan.frame_count {
                sequence.frame = tick;
                if plan.controls.hold.is_some_and(|hold| hold.frame == tick) {
                    sequence.resume_native_actors(plan);
                }
                sequence.update_native_clocks(plan, &runtime_config.player_scales);
                assert_eq!(
                    street_sim::gamebreaker::Sequence::decode(&sequence.encode()).unwrap(),
                    sequence
                );
                for clock in sequence.native_clocks.iter().flatten() {
                    tracks.insert((plan.script, clock.track));
                    let playback = plan.actors[usize::from(clock.track)].playback.unwrap();
                    if !playback.initial && playback.looping() && clock.blend_updates == 0 {
                        assert_eq!(clock.counter, u32::MAX);
                        later_loops.insert((plan.script, clock.track));
                    }
                }
                world.sequence = Some(sequence);
                world
                    .validate()
                    .unwrap_or_else(|error| panic!("script {} tick {tick}: {error}", plan.script));
                if tick > 0 && !checked_missing {
                    let mut invalid = world.clone();
                    invalid.sequence.as_mut().unwrap().native_clocks = [None; 7];
                    reject(invalid, "native clock missing processed role");
                    let mut invalid = world.clone();
                    invalid.sequence.as_mut().unwrap().native_clocks[0] = None;
                    reject(invalid, "native clock missing processed role");
                    checked_missing = true;
                }
                for (role, clock) in sequence.native_clocks.iter().enumerate() {
                    let Some(clock) = clock else { continue };
                    let actor = &plan.actors[usize::from(clock.track)];
                    let playback = actor.playback.unwrap();
                    if !checked_current_phase {
                        let current = plan.actors[usize::from(clock.current_track)]
                            .playback
                            .unwrap();
                        for sampled in [false, true] {
                            let mut invalid = world.clone();
                            let bad = invalid.sequence.as_mut().unwrap().native_clocks[role]
                                .as_mut()
                                .unwrap();
                            let beyond = f32::from(current.frames).to_bits();
                            if sampled {
                                bad.sampled_bits = beyond;
                            } else {
                                bad.frame_bits = beyond;
                            }
                            reject(invalid, "native clock current clip phase");
                        }
                        for end in [false, true] {
                            let mut invalid = world.clone();
                            let bad = invalid.sequence.as_mut().unwrap().native_clocks[role]
                                .as_mut()
                                .unwrap();
                            let beyond = f32::from(playback.frames).to_bits();
                            if end {
                                bad.end_bits = beyond;
                            } else {
                                bad.target_bits = beyond;
                            }
                            reject(invalid, "native clock incoming clip phase");
                        }
                        checked_current_phase = true;
                    }
                    if clock.blend_updates != 0
                        && !checked_future_blend
                        && let Some((future, _)) =
                            plan.actors.iter().enumerate().find(|(_, next)| {
                                usize::from(next.slot) == role && next.frame > actor.frame
                            })
                    {
                        let mut invalid = world.clone();
                        invalid.sequence.as_mut().unwrap().native_clocks[role]
                            .as_mut()
                            .unwrap()
                            .current_track = future as u16;
                        reject(
                            invalid,
                            "native clock current source precedes incoming track",
                        );
                        let mut invalid = world.clone();
                        invalid.sequence.as_mut().unwrap().native_clocks[role]
                            .as_mut()
                            .unwrap()
                            .active = false;
                        reject(invalid, "native clock blend policy");
                        checked_future_blend = true;
                    }
                    if playback.blend_ticks() == 0
                        && !checked_direct_blend
                        && let Some((previous, _)) =
                            plan.actors.iter().enumerate().rfind(|(_, prior)| {
                                usize::from(prior.slot) == role && prior.frame < actor.frame
                            })
                    {
                        let mut invalid = world.clone();
                        let bad = invalid.sequence.as_mut().unwrap().native_clocks[role]
                            .as_mut()
                            .unwrap();
                        bad.blend_updates = 1;
                        bad.counter = 1;
                        bad.current_track = previous as u16;
                        reject(invalid, "native clock blend policy");
                        checked_direct_blend = true;
                    }
                }
            }
        }
        assert_eq!(tracks.len(), 194);
        assert_eq!(later_loops.len(), 15);
        assert!(
            checked_missing
                && checked_current_phase
                && checked_future_blend
                && checked_direct_blend
        );
    }

    #[test]
    fn disc_plans_fit_network_and_match_original_actor_paths() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let config = config(&bank).unwrap();
        let bytes = config.encode();
        assert!(bytes.len() < 30_000);
        assert_eq!(Config::decode(&bytes).unwrap(), config);
        assert_eq!(config.plans.len(), 16);
        // GamDN1b: freezes at 67 for 60 frames on camera 1, stops at 192,
        // then replays 40..192 at 40% on camera 2.
        let dn1b = config.plan(1).unwrap().controls;
        assert_eq!(dn1b.stop, Some(192));
        assert_eq!(
            dn1b.hold,
            Some(Hold {
                frame: 67,
                duration: 60,
                camera: 1,
                holds_replays: true,
            })
        );
        assert_eq!(
            dn1b.rerun,
            Some(Rerun {
                from: 40,
                to: 192,
                speed: 40,
                camera: 2
            })
        );
        // Every gamebreaker replays at its own end, and has a camera for
        // its freeze and its replay.
        for plan in &config.plans {
            let c = plan.controls;
            let timeline = &bank.scripts[plan.script];
            assert_eq!(c.rerun.map(|r| r.to), c.stop, "{}", timeline.name);
            for id in [c.hold.unwrap().camera, c.rerun.unwrap().camera] {
                assert!(timeline.camera(timeline.frames as f32, false, id).is_some());
            }
        }
        for plan in &config.plans {
            let timeline = &bank.scripts[plan.script];
            for frame in 0..plan.frame_count {
                let source = timeline.sample(frame as f32, false);
                let poses = plan.pose_at(frame as f32);
                for (slot, pose) in poses.iter().enumerate() {
                    match (&source.actors[slot], pose) {
                        (Some(source), Some(pose)) => {
                            // Opcode 3 temporarily gives the player its
                            // own vertical physics rather than the authored
                            // position keys (00334088). Its path is checked
                            // separately below for all nine disc records.
                            if plan.ball_cues.iter().any(|cue| {
                                cue.opcode() == 3
                                    && cue.source() == slot as i16
                                    && frame >= cue.frame
                                    && frame <= cue.frame.saturating_add(cue.duration)
                            }) {
                                continue;
                            }
                            for axis in 0..3 {
                                assert!(
                                    (source.actor.position[axis] - pose.position[axis]).abs()
                                        < 0.001,
                                    "{} frame {frame} slot {slot}",
                                    timeline.name
                                );
                            }
                        }
                        (None, None) => (),
                        _ => panic!("{} frame {frame} actor availability differs", timeline.name),
                    }
                }
            }
        }
    }

    #[test]
    fn all_nine_disc_actor_launches_rise_and_land_without_moving_the_ball() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let mut count = 0;
        for (index, timeline) in bank.scripts.iter().enumerate() {
            let plan = plan(timeline, index);
            for cue in plan.ball_cues.iter().filter(|cue| cue.opcode() == 3) {
                count += 1;
                let slot = cue.source() as usize;
                let start = plan.pose_at(cue.frame as f32)[slot].unwrap();
                let mid = plan.pose_at(cue.frame as f32 + cue.duration as f32 * 0.5)[slot].unwrap();
                let end = plan.pose_at((cue.frame + cue.duration) as f32)[slot].unwrap();
                // Actor keys can elevate the origin (InLegend03). At
                // half the duration, y = y0/2 + g*T^2/8, with the native
                // 40.5 ft/s^2 acceleration and a ground-level endpoint.
                assert!(start.position[1] >= 0.0, "{}", timeline.name);
                let peak =
                    start.position[1] * 0.5 + 40.5 * (cue.duration as f32 / 60.0).powi(2) / 8.0;
                assert!((mid.position[1] - peak).abs() < 0.0001, "{}", timeline.name);
                assert!(end.position[1] < 0.0001, "{}", timeline.name);
                for axis in [0, 2] {
                    assert_eq!(mid.position[axis], start.position[axis]);
                    assert_eq!(end.position[axis], start.position[axis]);
                }
                let mut without = plan.clone();
                without.ball_cues.retain(|c| c.opcode() != 3);
                for frame in cue.frame..=cue.frame + cue.duration {
                    assert_eq!(plan.ball_at(frame), without.ball_at(frame));
                }
            }
        }
        assert_eq!(count, 9);
    }

    #[test]
    fn authoritative_staged_hands_use_each_database_players_native_body_geometry() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let clips = street_render::animation::Clips::load_all(&mut iso).unwrap();
        let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
        let players = street_assets::db::players(&mut iso).unwrap();
        let mut config = config(&bank).unwrap();
        attach_hands(&mut config, &bank, &clips, &skeleton).unwrap();
        config.player_scales = vec![1.0; 6];
        config.player_width_ratios = vec![1.0; 6];
        // Verify the complete encoded geometry is what the authoritative
        // consumer receives, including the same full-precision float bits.
        config = Config::decode(&config.encode()).unwrap();
        let tracks = config.hands.clone();
        let mut max_error = 0.0f32;
        let mut max_width_change = 0.0f32;
        let mut comparisons = 0;
        let mut widened_players = 0;
        for (index, player) in players.iter().enumerate() {
            let body = player.native_body_scale();
            let ratio = street_render::animation::native_body_width_ratio(body);
            widened_players += usize::from(ratio > 1.0);
            // Alternate normal and tiny-player outer scale while retaining
            // the original native body width/height ratio.
            let scale = player.height_scale() * if index % 2 == 0 { 1.0 } else { 0.5 };
            config.player_scales = vec![scale];
            config.player_width_ratios = vec![ratio];
            for track in &tracks {
                let record = bank.animation(track.animation, false).unwrap();
                let clip = clips.get(record.name.as_deref().unwrap()).unwrap();
                let count = clip.frames() as u32;
                let plan = SequencePlan {
                    script: 0,
                    kind: 1,
                    frame_count: count + 1,
                    actors: vec![ActorTrack {
                        frame: 0,
                        duration: count + 1,
                        slot: 0,
                        pose: ActorPose {
                            position: [2.0, 3.0, -1.0],
                            heading: 0.6,
                            animation: track.animation,
                            animation_frame: 0.0,
                        },
                        animation_speed: 1.0,
                        playback: None,
                        keep_position: false,
                        keep_heading: false,
                    }],
                    curves: Vec::new(),
                    ball_cues: Vec::new(),
                    controls: Controls::default(),
                    finishes: Vec::new(),
                };
                for frame in 0..count {
                    let sample = |body| {
                        let right = clip
                            .joint_position_with_body_scale(
                                &skeleton,
                                19,
                                frame as f32,
                                record.mirrored,
                                true,
                                body,
                            )
                            .unwrap();
                        let left = clip
                            .joint_position_with_body_scale(
                                &skeleton,
                                15,
                                frame as f32,
                                record.mirrored,
                                true,
                                body,
                            )
                            .unwrap();
                        match clip.state_at_mirrored(frame as usize, record.mirrored) & 0xc {
                            8 => right,
                            4 => left,
                            _ => (right + left) * 0.5,
                        }
                    };
                    let local = sample(body) * scale;
                    if ratio > 1.0 {
                        max_width_change =
                            max_width_change.max((local - sample(None) * scale).length());
                    }
                    for team in 0..2 {
                        let sequence = street_sim::gamebreaker::Sequence {
                            script: 0,
                            shooter: 0,
                            team,
                            level: 2,
                            frame,
                            held: 0,
                            actors: [Some(0), None, None, None, None, None, None],
                            native_clocks: [None; 7],
                            resume_pending: false,
                            finishing: 0,
                        };
                        let mut expected = local.to_array();
                        let mut origin = [2.0, 3.0, -1.0];
                        let mut heading = 0.6f32;
                        // Team 0 attacks +x: its scripts play mirrored.
                        if team == 0 {
                            expected[0] = -expected[0];
                            origin[0] = -origin[0];
                            heading = -heading;
                        }
                        let (sin, cos) = heading.sin_cos();
                        expected = [
                            origin[0] + expected[0] * cos + expected[2] * sin,
                            origin[1] + expected[1],
                            origin[2] - expected[0] * sin + expected[2] * cos,
                        ];
                        let actual = config
                            .hand_position(&plan, &sequence, 0, frame as f32)
                            .unwrap();
                        let error = actual
                            .into_iter()
                            .zip(expected)
                            .map(|(a, b)| (a - b).abs())
                            .fold(0.0, f32::max);
                        max_error = max_error.max(error);
                        assert!(
                            error < 1e-5,
                            "player{index} animation{} frame{frame} team{team}: error{error}",
                            track.animation
                        );
                        comparisons += 1;
                    }
                }
            }
        }
        assert!(widened_players > 0);
        assert!(
            max_width_change > 0.01,
            "must distinguish width changes from uniform height"
        );
        eprintln!(
            "staged native geometry: {comparisons} comparisons, {} database players, {widened_players} widened, maximum error {max_error}, maximum width change {max_width_change}",
            players.len()
        );
    }

    #[test]
    fn animated_ball_attachments_fit_the_match_setup() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let clips = street_render::animation::Clips::load_all(&mut iso).unwrap();
        let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
        let mut config = config(&bank).unwrap();
        attach_hands(&mut config, &bank, &clips, &skeleton).unwrap();
        config.player_scales = vec![1.0; 6];
        config.player_width_ratios = vec![1.0; 6];
        let mut mirrored_single_hand_frames = 0;
        for track in &config.hands {
            let record = bank.animation(track.animation, false).unwrap();
            if !record.mirrored {
                continue;
            }
            let clip = clips.get(record.name.as_deref().unwrap()).unwrap();
            for (frame, &actual) in track.positions.iter().enumerate() {
                // Native loader swaps 8/4 on a mirrored record before hand
                // selection. The pose is already mirrored by the sampler.
                let hand = match clip.state_at(frame) & 0xc {
                    8 => 15,
                    4 => 19,
                    _ => continue,
                };
                let expected = clip
                    .joint_position(&skeleton, hand, frame as f32, true, true)
                    .unwrap()
                    .to_array();
                assert_eq!(
                    actual, expected,
                    "animation {} frame {frame}",
                    track.animation
                );
                mirrored_single_hand_frames += 1;
            }
        }
        assert!(mirrored_single_hand_frames > 0);
        let encoded = config.encode();
        eprintln!(
            "staged bytes={} frames={}",
            encoded.len(),
            config
                .hands
                .iter()
                .map(|h| h.positions.len())
                .sum::<usize>()
        );
        assert!(
            encoded.len() < 128 * 1024,
            "{} bytes for {} attachment tracks",
            encoded.len(),
            config.hands.len()
        );
        assert_eq!(Config::decode(&encoded).unwrap(), config);
        assert!(!config.hands.is_empty());
        let court = street_assets::court::load(&mut iso, "RUCKER").unwrap();
        let hoops = crate::setup::hoops(&court).unwrap();
        // The disc's animations, as a match installs them: a dunker's
        // finishing body flies on his posed hands.
        let disc = street_net::disc::Disc::read(&mut iso).unwrap();
        let mut failures = Vec::new();
        for plan in &config.plans {
            for team in 0..2 {
                let shooter = team * 3;
                let poses = plan.initial_positions();
                let players: Vec<_> = (0..6)
                    .map(|player| {
                        let slot = if team == 0 { player } else { (player + 3) % 6 };
                        let mut position = poses[slot].unwrap_or([0.0, player as f32 * 4.0]);
                        if team == 0 {
                            position[0] = -position[0];
                        }
                        street_sim::Player::new(player / 3, position, 0.0)
                    })
                    .collect();
                let actors = street_sim::gamebreaker::bind(&players, shooter, plan.kind, poses);
                let mut world =
                    street_sim::World::new(hoops, players, street_sim::Rules::default(), 2002);
                world.set_match_values(vec![Default::default(); 6], vec![1.0; 6], vec![1.0; 6]);
                disc.install(&mut world).unwrap();
                world.ball.hold(shooter);
                world.phase = street_sim::Phase::Live;
                world.possession = team;
                world.staged = Some(std::sync::Arc::new(config.clone()));
                world.sequence = Some(street_sim::gamebreaker::Sequence {
                    script: plan.script,
                    shooter,
                    team,
                    level: 2,
                    frame: 0,
                    held: 0,
                    actors,
                    native_clocks: [None; 7],
                    resume_pending: false,
                    finishing: 0,
                });
                if plan.script == config.plans[0].script && team == 0 {
                    // The match's setup carries the match's own values, not
                    // the disc's plans and hands, which every machine
                    // installs from its own disc (`street_net::disc`); with
                    // them back the world is the referee's.
                    assert_eq!(
                        street_sim::World::from_snapshot(&world.to_snapshot())
                            .unwrap()
                            .with_tables_of(&world),
                        world
                    );
                    let message = street_net::protocol::Message::Welcome {
                        setup_version: 0,
                        seats: vec![0],
                        setup: street_net::protocol::MatchSetup {
                            court: "RUCKER".into(),
                            teams: [0, 1],
                            roster: (0..6).collect(),
                            seed: 2002,
                            seat_teams: [0, 1, 0, 1, 0, 1],
                            world: world.clone(),
                            extra: Vec::new(),
                            roster_data: Vec::new(),
                        },
                    };
                    let grams = message.datagrams(2002);
                    let mut assembly = street_net::protocol::Reassembly::default();
                    let mut complete = None;
                    for gram in grams.iter().rev() {
                        let street_net::protocol::Message::Part {
                            id,
                            index,
                            count,
                            bytes,
                        } = street_net::protocol::Message::decode(gram).unwrap()
                        else {
                            panic!("setup must span multiple datagrams");
                        };
                        complete = assembly
                            .add(
                                "127.0.0.1:9".parse().unwrap(),
                                std::time::Duration::ZERO,
                                id,
                                index,
                                count,
                                bytes,
                            )
                            .or(complete);
                    }
                    let street_net::protocol::Message::Welcome { setup, .. } =
                        street_net::protocol::Message::decode(&complete.unwrap()).unwrap()
                    else {
                        panic!("the welcome comes back whole");
                    };
                    let shell = setup.world.staged.as_deref().unwrap();
                    assert!(shell.plans.is_empty() && shell.hands.is_empty());
                    assert_eq!(setup.world.with_tables_of(&world), world);
                    eprintln!(
                        "full staged match setup: {} bytes in {} datagrams",
                        message.encode().len(),
                        grams.len()
                    );
                }
                // The basket counts on the ball's arrival; a carried dunk
                // crosses the rim within the script's own frames.
                let mut baskets = 0;
                for _ in 0..plan.frame_count + 600 {
                    world.step(&[]);
                    baskets += world
                        .events
                        .iter()
                        .filter(|event| {
                            matches!(
                                event,
                                street_sim::Event::Score {
                                    basket: street_sim::Basket::Gamebreaker { level: 2, .. },
                                    ..
                                }
                            )
                        })
                        .count();
                    if world.sequence.is_none()
                        && !matches!(
                            world.ball.state,
                            street_sim::BallState::Shot | street_sim::BallState::Pass
                        )
                    {
                        break;
                    }
                }
                if baskets != 1 {
                    failures.push(format!(
                        "{} side{team}: {baskets} baskets; {:?} at {:?}",
                        bank.scripts[plan.script].name, world.ball.state, world.ball.position
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn every_staged_dunker_flies_on_his_finishing_body() {
        // The dunk plans (0..7) hand their dunker to his finishing body
        // (`FUN_00334178`): its `9e` aim flies him to the rim and he lands
        // on his own. The School's run of script 1 (GamLR2's family 9,
        // `level2-batch/case09`) peaks at 7.86 above the floor at frame 137
        // and lands at 177; script 2 (`case10`) leaves at 4.03, its
        // staged clip's root lift, and peaks about 9.6 at 129.
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let disc = street_net::disc::Disc::read(&mut iso).unwrap();
        let bank = ScriptBank::read(&mut iso).unwrap();
        let config = config(&bank).unwrap();
        for script in 0..16 {
            let plan = config.plan(script).unwrap();
            // The presser holds it: the first pass's or shot's thrower.
            let first = plan
                .ball_cues
                .iter()
                .filter(|c| c.opcode() == 0 || c.opcode() == 2)
                .find_map(|c| usize::try_from(c.source()).ok())
                .unwrap();
            let positions = plan.initial_positions();
            let players = (0..6)
                .map(|i| street_sim::Player::new(1, positions[i].unwrap_or([0.0, 0.0]), 0.0))
                .collect();
            let mut world = street_sim::World::new(
                street_sim::Hoop::native(),
                players,
                street_sim::Rules::default(),
                2002,
            );
            world.set_match_values(vec![Default::default(); 6], vec![1.0; 6], vec![1.0; 6]);
            disc.install(&mut world).unwrap();
            world.phase = street_sim::Phase::Live;
            world.ball.hold(first);
            world.sequence = Some(street_sim::gamebreaker::Sequence {
                script,
                shooter: first,
                team: 1,
                level: 2,
                frame: 0,
                held: 0,
                actors: [Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), None],
                native_clocks: [None; 7],
                resume_pending: false,
                finishing: 0,
            });
            let dunker = plan.finishes.first().map_or(0, |f| usize::from(f.slot));
            let mut last = world.sequence.unwrap();
            let (mut peak, mut landed, mut scores) = ((0.0f32, 0), None, 0);
            let mut shot = false;
            let mut takeoff = 0.0f32;
            let mut balls = Vec::new();
            let mut updates = 0;
            // A shot may outlast its script and score as the Level 2's.
            while let Some(s) = world
                .sequence
                .or_else(|| (world.ball.state == street_sim::BallState::Shot).then_some(last))
            {
                last = s;
                updates += 1;
                assert!(updates < 1000);
                world.step(&[]);
                // The passes reach their catchers: the bounce passes off
                // the floor, the legs to and from the backboard past the
                // rim (the board-disable record).
                shot |= world.ball.state == street_sim::BallState::Shot;
                assert!(
                    shot || world.ball.state != street_sim::BallState::Loose,
                    "script {script}: loose at frame {} {:?}",
                    s.frame,
                    world.ball.position
                );
                let h = world.players[dunker].height;
                if s.held == 0 && [71u32, 123, 133].contains(&s.frame) {
                    balls.push((s.frame, world.ball.position));
                }
                if s.frame == 98 {
                    takeoff = h;
                }
                if world
                    .sequence
                    .is_some_and(|q| q.finishing & (1 << dunker) != 0)
                {
                    if h > peak.0 {
                        peak = (h, s.frame);
                    }
                    if peak.0 > 0.0 && h <= 0.0 && landed.is_none() {
                        landed = Some(s.frame);
                    }
                }
                scores += world
                    .events
                    .iter()
                    .filter(|e| {
                        matches!(
                            e,
                            street_sim::Event::Score {
                                basket: street_sim::Basket::Gamebreaker { level: 2, .. },
                                ..
                            }
                        )
                    })
                    .count();
            }
            assert!(
                script >= 8 || peak.0 > 1.0,
                "script {script}: the dunker never left the floor"
            );
            assert_eq!(scores, 1, "script {script}");
            if script == 2 {
                // The School's run (`case10`): his body starts at 98 with
                // the staged clip's root lift (4.03) and peaks about 9.6
                // at 129.
                assert!((takeoff - 4.03).abs() < 0.05, "{takeoff}");
                // The ball in the drawn (in-place) hand, then off the
                // board and in the dunker's: the capture's 71, 123, 133.
                for (frame, want, within) in [
                    (71, [-23.57, 7.28, 0.35], 0.5),
                    (123, [-28.31, 18.36, 0.45], 0.2),
                    (133, [-29.25, 18.46, -0.01], 0.2),
                ] {
                    let at = balls.iter().find(|b| b.0 == frame).map(|b| b.1).unwrap();
                    assert!(
                        (0..3).all(|k| (at[k] - want[k]).abs() < within),
                        "frame {frame}: {at:?}"
                    );
                }
                assert!(
                    (peak.0 - 9.6).abs() < 0.1 && (128..=131).contains(&peak.1),
                    "{peak:?}"
                );
            }
            if script == 1 {
                assert!(
                    (peak.0 - 7.86).abs() < 0.1 && (136..=138).contains(&peak.1),
                    "{peak:?}"
                );
                assert!(
                    landed.is_some_and(|f| (176..=178).contains(&f)),
                    "{landed:?}"
                );
            }
        }
    }
}
