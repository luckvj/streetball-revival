//! The staged Level 2 gamebreakers (`SCRIPTS.BIN`) as the simulation plays
//! them ([`street_sim::gamebreaker::Config`]): each script's plan, its
//! actors' playback against the disc's clips, and the hand tracks the
//! referee places the ball by. Built by every machine from its own disc
//! ([`super::Disc`]); the renderer's intros use the same plans
//! (`street-game`'s `staged`).

use street_assets::clip::Clips;
use street_assets::eagl::Skeleton;
use street_assets::timeline::{Payload, ScriptBank};
use street_sim::gamebreaker::{
    ActorPose, ActorTrack, BallCue, Config, Controls, Curve, FinishSpan, FinishTrack, Hold,
    NativePlayback, RegionMap, Rerun, SequencePlan,
};

/// Each staged script's plan from the disc's timelines: the original
/// actor tracks and scalar keys rather than baked poses.
pub fn config(bank: &ScriptBank) -> Result<Config, String> {
    let mut scripts: Vec<usize> = bank
        .gamebreakers
        .iter()
        .flatten()
        .flatten()
        .copied()
        .collect();
    scripts.sort_unstable();
    scripts.dedup();
    let mut config = Config {
        families: bank.gamebreakers,
        regions: RegionMap::from_rows(bank.gamebreaker_region_rows()),
        ..Config::default()
    };
    for script in scripts {
        let timeline = bank.scripts.get(script).ok_or("missing staged script")?;
        let mut plan = plan(timeline, script);
        add_finishers(bank, &mut plan);
        config.plans.push(plan);
    }
    config.validate()?;
    Ok(config)
}

/// Retain the actual animated hand centres so the referee can reproduce
/// passes and rim contacts without a rendered skeleton. The native state
/// word selects bone 19, bone 15, or their midpoint (00330f98).
///
/// The hands are the disc's; each player's model scale and width ratio are
/// the match's own values, which the world carries
/// (`street_sim::World::set_match_values`).
pub fn attach_hands(
    config: &mut Config,
    bank: &ScriptBank,
    clips: &Clips,
    skeleton: &Skeleton,
) -> Result<(), String> {
    for plan in &mut config.plans {
        attach_playback(plan, bank, clips)?;
    }
    let mut animations: Vec<u16> = config
        .plans
        .iter()
        .flat_map(|plan| {
            plan.actors
                .iter()
                .filter(|actor| actor.slot < 6)
                .map(|actor| actor.pose.animation)
        })
        .collect();
    animations.sort_unstable();
    animations.dedup();
    let mut hands = Vec::new();
    for animation in animations {
        let record = bank
            .animation(animation, false)
            .ok_or("missing staged animation record")?;
        let name = record
            .name
            .as_deref()
            .ok_or("unnamed staged actor animation")?;
        let clip = clips
            .get(name)
            .ok_or_else(|| format!("missing staged clip {name}"))?;
        // In place, as the staged body is drawn: the script's curves move
        // the actor along his clip's travel, so the hand the ball sits in
        // keeps only the root's rise (script 2's held ball had sat 7
        // beyond its holder, the School's capture 2).
        let mut positions = Vec::with_capacity(clip.frames());
        let mut width_deltas = Vec::with_capacity(clip.frames());
        for frame in 0..clip.frames() {
            let right = clip
                .joint_position(skeleton, 19, frame as f32, record.mirrored, true)
                .ok_or("missing right hand")?;
            let left = clip
                .joint_position(skeleton, 15, frame as f32, record.mirrored, true)
                .ok_or("missing left hand")?;
            let at = match clip.state_at_mirrored(frame, record.mirrored) & 0xc {
                8 => right,
                4 => left,
                _ => (right + left) * 0.5,
            };
            // The native body operation changes only local translations.
            // At fixed clip rotation, the world hand centre is affine in
            // width/height. Retain its unit-width displacement once per clip.
            let wide_right = clip
                .joint_position_with_body_scale(
                    skeleton,
                    19,
                    frame as f32,
                    record.mirrored,
                    true,
                    Some([1.0, 2.0, 2.0]),
                )
                .ok_or("missing width-adjusted right hand")?;
            let wide_left = clip
                .joint_position_with_body_scale(
                    skeleton,
                    15,
                    frame as f32,
                    record.mirrored,
                    true,
                    Some([1.0, 2.0, 2.0]),
                )
                .ok_or("missing width-adjusted left hand")?;
            let wide = match clip.state_at_mirrored(frame, record.mirrored) & 0xc {
                8 => wide_right,
                4 => wide_left,
                _ => (wide_right + wide_left) * 0.5,
            };
            width_deltas.push((wide - at).to_array());
            positions.push(at.to_array());
        }
        hands.push(street_sim::gamebreaker::HandTrack {
            animation,
            positions,
            width_deltas,
        });
    }
    config.hands = hands;
    config.validate()
}

/// Resolve the original staged actor VM fields against actual loaded clips.
/// Setup installs these on the referee plans; intros use the same resolver.
pub fn attach_playback(
    plan: &mut SequencePlan,
    bank: &ScriptBank,
    clips: &Clips,
) -> Result<(), String> {
    let timeline = bank
        .scripts
        .get(plan.script)
        .ok_or("missing staged playback script")?;
    let actors: Vec<_> = timeline
        .records
        .iter()
        .filter_map(|record| match &record.payload {
            Payload::Actor(actor) => Some(actor),
            _ => None,
        })
        .collect();
    if actors.len() != plan.actors.len() {
        return Err("staged actor playback record count differs".into());
    }
    let mut started = [false; 7];
    for (track, actor) in plan.actors.iter_mut().zip(actors) {
        let record = bank
            .animation(actor.animation, false)
            .ok_or("missing staged playback animation")?;
        let name = record
            .name
            .as_deref()
            .ok_or("unnamed staged playback animation")?;
        let clip = clips
            .get(name)
            .ok_or_else(|| format!("missing staged playback clip {name}"))?;
        let frames = u16::try_from(clip.frames()).map_err(|_| "staged playback clip too long")?;
        let started = started
            .get_mut(usize::from(actor.slot))
            .ok_or("invalid staged playback slot")?;
        let playback = NativePlayback {
            frames,
            speed_word: actor.words[0],
            end_fraction: actor.animation_end,
            flags: actor.flags,
            initial: !*started,
            loop_distances: [
                bank.staged_loop_distance(actor.animation, false),
                bank.staged_loop_distance(actor.animation, true),
            ],
        };
        *started = true;
        if !(2..=2048).contains(&frames)
            || playback.speed_word == 0
            || playback.speed_word > i16::MAX as u16
            || playback.end_fraction > i16::MAX as u16
            || playback.flags[1] == 0
            || playback.flags[9] != 0
            || !playback.step().is_finite()
            || playback.step() <= 0.0
        {
            return Err(format!("unsupported staged playback for {name}"));
        }
        track.playback = Some(playback);
    }
    Ok(())
}

/// One script's plan: its actor tracks, scalar curves and ball commands.
pub fn plan(timeline: &street_assets::timeline::Timeline, script: usize) -> SequencePlan {
    let actors = timeline
        .records
        .iter()
        .filter_map(|record| {
            let Payload::Actor(actor) = &record.payload else {
                return None;
            };
            Some(ActorTrack {
                frame: record.frame,
                duration: record.duration,
                slot: actor.slot,
                pose: ActorPose {
                    position: actor.position,
                    heading: f32::from(actor.heading).to_radians(),
                    animation: actor.animation,
                    animation_frame: 0.0,
                },
                animation_speed: f32::from(actor.words[0]) / 256.0,
                playback: None,
                keep_position: actor.flags[0] != 0,
                keep_heading: actor.flags[4] != 0,
            })
        })
        .collect();
    let curves = timeline
        .actor_curves()
        .iter()
        .map(|curve| {
            let factor = if curve.field == 3 {
                std::f32::consts::PI / 180.0
            } else {
                1.0
            };
            Curve {
                slot: curve.slot as u8,
                field: curve.field,
                start_frame: curve.start_frame,
                end_frame: curve.end_frame,
                start: curve.start * factor,
                end: curve.end * factor,
                start_tangent: curve.start_tangent * factor,
                end_tangent: curve.end_tangent * factor,
                interpolation: curve.interpolation,
            }
        })
        .collect();
    let ball_cues = timeline
        .records
        .iter()
        .filter_map(|record| {
            let Payload::Command(command) = &record.payload else {
                return None;
            };
            if command.dispatch != 0 {
                return None;
            }
            Some(BallCue {
                frame: record.frame,
                duration: record.duration,
                data: command.bytes.as_slice().try_into().ok()?,
            })
        })
        .collect();
    SequencePlan {
        script,
        kind: timeline.kind,
        frame_count: timeline.frames,
        actors,
        curves,
        ball_cues,
        controls: controls(timeline),
        finishes: Vec::new(),
    }
}

/// Staged opcode 1 starts a finishing animation VM, whose callback 21
/// releases the actual shot. This is not the command's duration or the
/// preceding alley-oop's arrival. Fixed VM waits also replace the generic
/// animation clock while the finishing script runs.
fn add_finishers(bank: &ScriptBank, plan: &mut SequencePlan) {
    let triggers: Vec<_> = plan
        .ball_cues
        .iter()
        .copied()
        .filter(|cue| cue.opcode() == 1)
        .collect();
    for cue in triggers {
        let Ok(slot) = u8::try_from(cue.source()) else {
            continue;
        };
        let Some(pose) = plan
            .pose_at(cue.frame as f32)
            .get(usize::from(slot))
            .copied()
            .flatten()
        else {
            continue;
        };
        let Some(finisher) = bank
            .finishers
            .iter()
            .find(|f| f.animation == pose.animation)
        else {
            continue;
        };
        let Some(release_tick) = finisher.release_tick else {
            continue;
        };
        plan.finishes.push(FinishTrack {
            slot,
            start_frame: cue.frame,
            animation: pose.animation,
            release_bits: finisher.release_bits,
            row: street_sim::timing::finish_row(finisher.address),
            spans: finisher
                .spans
                .iter()
                .map(|span| FinishSpan {
                    start_tick: span.start_tick,
                    duration: span.duration,
                    from_frame: span.from_frame as f32,
                    to_frame: span.to_frame as f32,
                })
                .collect(),
        });
        let frame = cue.frame.saturating_add(release_tick);
        if frame >= plan.length() {
            continue;
        }
        let mut data = cue.data;
        data[..2].copy_from_slice(&2u16.to_le_bytes());
        data[4..6].copy_from_slice(&(-1i16).to_le_bytes());
        plan.ball_cues.push(BallCue {
            frame,
            duration: 1,
            data,
        });
    }
    plan.ball_cues.sort_by_key(|cue| cue.frame);
}

/// The script's control commands (category 1, dispatch 3) and replay
/// commands (dispatch 1): where it stops, its freeze and its slowed replay.
/// Each command's word 8 is the camera id it turns on (0..9, else 0); a
/// freeze's word 12 is 1 when it holds the replay manager too
/// (`FUN_00336340`: groups 0x1382, else 0x382); a replay's word 16 its
/// speed in percent (25..150, else 100); a replay of 301 frames or more is
/// not played (`FUN_00334c20`).
pub fn controls(timeline: &street_assets::timeline::Timeline) -> Controls {
    let mut out = Controls::default();
    let word = |bytes: &[u8], at: usize| {
        bytes
            .get(at..at + 4)
            .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    let camera = |bytes: &[u8]| {
        u8::try_from(word(bytes, 8))
            .ok()
            .filter(|&c| c <= 9)
            .unwrap_or(0)
    };
    for record in &timeline.records {
        let Payload::Command(command) = &record.payload else {
            continue;
        };
        match (command.dispatch, command.opcode) {
            (3, 0) => {
                out.stop = Some(out.stop.map_or(record.frame, |s| s.min(record.frame)));
            }
            (1, 2) if out.hold.is_none() => {
                out.hold = Some(Hold {
                    frame: record.frame,
                    duration: record.duration.clamp(1, 600),
                    camera: camera(&command.bytes),
                    holds_replays: word(&command.bytes, 12) == 1,
                });
            }
            (1, 0) if out.rerun.is_none() && record.duration < 301 => {
                let speed = word(&command.bytes, 16);
                out.rerun = Some(Rerun {
                    from: record.frame,
                    to: (record.frame + record.duration).min(timeline.frames),
                    speed: if (25..=150).contains(&speed) {
                        speed as u8
                    } else {
                        100
                    },
                    camera: camera(&command.bytes),
                });
            }
            _ => (),
        }
    }
    out.rerun = out.rerun.filter(|r| r.from < r.to);
    out
}
