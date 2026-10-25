//! A player's animation channel on screen: the move script the simulation
//! chose, played on the animation VM's own clock
//! ([`street_assets::playback`]).
//!
//! A shot, a dunk, a lay-up, a pass and a catch are scripts in the
//! original (`crates/street-sim/src/timing.rs`): the press starts one on
//! the actor's channel (`FUN_0019c2f8`) and the VM plays its blends and
//! waits, update by update, until its callbacks let the ball go. The
//! simulation keeps each script's identity and tick on the action
//! ([`Action::Shooting`]'s `script`, [`Action::Dunking`]'s and
//! [`Action::Layup`]'s `row`, [`Action::PumpFake`]'s `script`,
//! [`Action::Passing`]'s `row`, the pass's and [`street_sim::Catch`]'s
//! `row` and `since`); this module turns that into the clip, frame and
//! blend the actor shows, so the drawn release is the update the
//! simulation's callback fires in.
//!
//! The simulation's shot, dunk, lay-up, fake and pass ticks read 1 at the
//! end of the press update, so the script's own tick is one less (the
//! press update is its tick 0, as the VM that started it there counts);
//! a shot's stall at its release gate (a person holding the button,
//! [`Action::Shooting`]'s `stall`) holds the script's tick, so the frame
//! holds with it as the original's channel does. A script whose clip the
//! disc does not carry, or whose program the executable does not hold,
//! leaves the view to its own clip.

use std::{collections::BTreeMap, sync::Arc};

use street_assets::playback::{self, Clock, Path, Program};
use street_render::animation::{Animator, Clip};
use street_sim::{Action, BallState, World, ball, rebound, timing, toss};

use crate::setup::GameClips;

/// The scripts the display can play, compiled from the executable along
/// the VM's plain path (and the pump fake's for the shots with a check).
#[derive(Clone, Debug, Default)]
pub struct Programs {
    plain: BTreeMap<u32, Program>,
    fake: BTreeMap<u32, Program>,
    /// The dunk rows' scripts along callback 28's hang ([`Path::Hang`]).
    hang: BTreeMap<u32, Program>,
    /// The rebound jumps' scripts along callback 47's branch
    /// ([`Path::Branch`]).
    branch: BTreeMap<u32, Program>,
}

impl Programs {
    /// Every shot, dunk, lay-up, pass and catch script the simulation's
    /// tables name (`timing_tables.rs`, and past the dunk table the leaps'
    /// and the board's and mid-air lay-ups' rows, [`timing::move_row`]),
    /// the tip-off jumpers' ([`toss::TIP_SCRIPTS`]) and the rebound jumps'
    /// ([`rebound::SCRIPTS`]), from the executable's bytes.
    pub fn compile(elf: &[u8]) -> street_assets::Result<Self> {
        let mut plain = BTreeMap::new();
        let mut fake = BTreeMap::new();
        for s in &timing::SHOT_SCRIPTS {
            plain.insert(s.script, playback::compile(elf, s.script, Path::Plain)?);
            if s.check > 0 {
                fake.insert(s.script, playback::compile(elf, s.script, Path::Fake)?);
            }
        }
        let mut hang = BTreeMap::new();
        for r in timing::MOVE_ROWS.iter().chain(&timing::LEAP_ROWS) {
            if let std::collections::btree_map::Entry::Vacant(e) = hang.entry(r.script) {
                e.insert(playback::compile(elf, r.script, Path::Hang)?);
            }
        }
        let rows = timing::MOVE_ROWS
            .iter()
            .chain(&timing::LEAP_ROWS)
            .chain(&timing::EXTRA_ROWS)
            .map(|r| r.script)
            .chain(timing::DIVE_SCRIPTS)
            .chain(timing::PASS_ROWS.iter().map(|r| r.script))
            .chain(timing::CATCH_ROWS.iter().map(|r| r.script))
            .chain(toss::TIP_SCRIPTS)
            .chain(rebound::SCRIPTS.iter().map(|s| s.address));
        for address in rows {
            if let std::collections::btree_map::Entry::Vacant(e) = plain.entry(address) {
                e.insert(playback::compile(elf, address, Path::Plain)?);
            }
        }
        let mut branch = BTreeMap::new();
        for s in &rebound::SCRIPTS {
            branch.insert(s.address, playback::compile(elf, s.address, Path::Branch)?);
        }
        Ok(Self {
            plain,
            fake,
            hang,
            branch,
        })
    }

    pub fn get(&self, address: u32, path: Path) -> Option<&Program> {
        match path {
            Path::Plain => self.plain.get(&address),
            Path::Fake => self.fake.get(&address),
            Path::Hang => self.hang.get(&address),
            Path::Branch => self.branch.get(&address),
        }
    }

    pub fn len(&self) -> usize {
        self.plain.len() + self.fake.len() + self.hang.len() + self.branch.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plain.is_empty()
            && self.fake.is_empty()
            && self.hang.is_empty()
            && self.branch.is_empty()
    }
}

/// What the simulation says a player's channel plays: the script, its
/// path and the update of the script's clock to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scripted {
    pub address: u32,
    pub path: Path,
    pub tick: i32,
    /// A catch script's pass: its nominal flight in updates.
    pub flight: Option<i32>,
    /// A jump's landing in the script's ticks, while the sim's jump
    /// carries the player (the tip-off's).
    pub landing: Option<i32>,
}

/// The script the simulation has on player `i`'s channel this update, if
/// any: the shot, dunk, lay-up or pump fake of his action, his pass's row
/// from the press through the follow-through, or the catch row of a pass
/// flying to him or just caught.
pub fn scripted(w: &World, i: usize) -> Option<Scripted> {
    let p = w.players.get(i)?;
    let shot = |script: u8| {
        timing::SHOT_SCRIPTS
            .get(usize::from(script))
            .map(|s| s.script)
    };
    let row = |row: u8| timing::move_row(row).map(|r| r.script);
    let plain = |address: Option<u32>, tick: i32| {
        address.map(|address| Scripted {
            address,
            path: Path::Plain,
            tick,
            flight: None,
            landing: None,
        })
    };
    match p.action {
        Action::Shooting {
            script,
            tick,
            fade,
            check,
            stall,
            ..
        } => {
            // The script's tick: the press update its 0, the gate's stall
            // taken off. A fadeaway: callback 17 at the check jumped into
            // `jumpshot_1_fadeaway`, whose script the sim then names.
            let tick =
                i32::from(tick) - 1 - i32::from(stall) - if fade { i32::from(check) } else { 0 };
            plain(shot(script), tick)
        }
        Action::PumpFake { script, tick, .. } => shot(script).map(|address| Scripted {
            address,
            path: Path::Fake,
            tick: i32::from(tick) - 1,
            flight: None,
            landing: None,
        }),
        // A dunk whose callback 28 answered: the hang's path.
        Action::Dunking {
            row: r,
            tick,
            hang: Some(true),
            ..
        } => row(r).map(|address| Scripted {
            address,
            path: Path::Hang,
            tick: i32::from(tick) - 1,
            flight: None,
            landing: None,
        }),
        Action::Dunking { row: r, tick, .. } | Action::Layup { row: r, tick, .. } => {
            plain(row(r), i32::from(tick) - 1)
        }
        Action::Dive { script, tick } => plain(
            timing::DIVE_SCRIPTS.get(usize::from(script)).copied(),
            i32::from(tick) - 1,
        ),
        Action::Passing { row, tick, .. } if row != timing::NO_PASS_ROW => plain(
            timing::PASS_ROWS.get(usize::from(row)).map(|r| r.script),
            i32::from(tick) - 1,
        ),
        // The tip-off jumper's script: its airtime wait (`8d ee 19 fe`)
        // lasts until the sim's jump lands (`FUN_0019bed8` from the
        // height and rise left).
        Action::TipJump {
            script,
            tick,
            vy,
            up,
            ..
        } => toss::TIP_SCRIPTS
            .get(usize::from(script))
            .map(|&address| Scripted {
                address,
                path: Path::Plain,
                tick: i32::from(tick) - 1,
                flight: None,
                landing: up.then(|| i32::from(tick) - 1 + i32::from(timing::airtime(vy, p.height))),
            }),
        // A rebound jump's script, the same way: its airtime wait (`8d ee
        // G fe`) lasts until the sim's jump lands. (Its wait for the
        // updates callback 36 aimed for, `8d ee F ef 00`, is one update
        // here, as every `ef` wait.) Callback 47's answer picks its path.
        Action::ReboundJump {
            script,
            tick,
            vy,
            up,
            branch,
            ..
        } => rebound::SCRIPTS.get(usize::from(script)).map(|s| Scripted {
            address: s.address,
            path: if branch { Path::Branch } else { Path::Plain },
            tick: i32::from(tick) - 1,
            flight: None,
            landing: up.then(|| i32::from(tick) - 1 + i32::from(timing::airtime(vy, p.height))),
        }),
        _ => catch(w, i),
    }
}

/// The catch script on the receiver of a pass in the air, or still holding
/// him after the catch.
fn catch(w: &World, i: usize) -> Option<Scripted> {
    let completed = w.tick as i64 - 1;
    let (row, since, flight) = match (w.ball.state, w.ball.pass, w.catch) {
        (BallState::Pass, Some(pass), _) if pass.to == i && pass.catch != ball::NO_CATCH => {
            (pass.catch, pass.since, w.ball.flight_ticks())
        }
        (BallState::Held(h), _, Some(c)) if h == i && c.player == i && w.tick < c.until => {
            (c.row, c.since, i32::from(c.flight))
        }
        _ => return None,
    };
    let address = timing::CATCH_ROWS.get(usize::from(row))?.script;
    Some(Scripted {
        address,
        path: Path::Plain,
        tick: (completed - since as i64).clamp(0, i32::MAX as i64) as i32,
        flight: Some(flight),
        landing: None,
    })
}

/// The channel's clock answered from the loaded clips.
struct ClipClock<'a> {
    clips: &'a GameClips,
    flight: Option<i32>,
    landing: Option<i32>,
    left_hand: bool,
}

impl ClipClock<'_> {
    fn clip(&self, record: i32) -> Option<(Arc<Clip>, bool)> {
        let row = self
            .clips
            .timeline
            .animations
            .get(usize::try_from(record).ok()?)?;
        Some((self.clips.clips.get(row.name.as_deref()?)?, row.mirrored))
    }
}

impl Clock for ClipClock<'_> {
    fn frames(&self, record: i32) -> Option<i32> {
        Some(self.clip(record)?.0.frames() as i32)
    }
    fn flight(&self) -> Option<i32> {
        self.flight
    }
    fn left_hand(&self) -> bool {
        self.left_hand
    }
    fn landing(&self) -> Option<i32> {
        self.landing
    }
}

/// A blend in progress: the outgoing clip, its mirroring and the frame it
/// holds (`None` for the clip the actor was already showing), and the
/// incoming clip's weight.
#[derive(Clone)]
pub struct ShownBlend {
    pub from: Option<(Arc<Clip>, bool, f32)>,
    pub weight: f32,
}

/// The clip, frame and blend a channel shows.
#[derive(Clone)]
pub struct Shown {
    pub clip: Arc<Clip>,
    pub mirrored: bool,
    pub frame: f32,
    /// A blend in progress.
    pub blend: Option<ShownBlend>,
    /// The callbacks the script ran on this update.
    pub callbacks: Vec<u32>,
}

/// What the channel shows: a pose, or nothing because the script has
/// ended (the actor's ordinary animation is back).
pub enum Channel {
    Pose(Shown),
    Over,
}

/// Play `s` on the loaded clips; `None` when the executable holds no such
/// program or the disc no such clip.
pub fn shown(clips: &GameClips, s: &Scripted, left_hand: bool) -> Option<Channel> {
    let program = clips.programs.get(s.address, s.path)?;
    let clock = ClipClock {
        clips,
        flight: s.flight,
        landing: s.landing,
        left_hand,
    };
    let (pose, callbacks) = playback::pose_at(program, s.tick, &clock);
    if pose.done {
        return Some(Channel::Over);
    }
    let (clip, mirrored) = clock.clip(pose.record?)?;
    let blend = pose.blend.map(|b| ShownBlend {
        from: b.from.and_then(|(record, frame)| {
            let (clip, mirrored) = clock.clip(record?)?;
            Some((clip, mirrored, frame))
        }),
        weight: b.weight,
    });
    Some(Channel::Pose(Shown {
        clip,
        mirrored,
        frame: pose.frame,
        blend,
        callbacks,
    }))
}

/// A block jump's channel ([`street_sim::block`]): the simulation keeps
/// its script's clock itself (the arm callback 48 picked, the waits
/// callback 35 timed, the landing), so the clip, frame and blend are the
/// jump's own. `None` when the disc does not carry its clip.
pub fn block_shown(clips: &GameClips, j: &street_sim::block::Jump) -> Option<Shown> {
    let clock = ClipClock {
        clips,
        flight: None,
        landing: None,
        left_hand: false,
    };
    let (clip, mirrored) = clock.clip(i32::from(j.record()))?;
    let blend = j.blend().map(|weight| ShownBlend {
        from: j.from.and_then(|(record, frame)| {
            let (clip, mirrored) = clock.clip(i32::from(record))?;
            Some((clip, mirrored, frame))
        }),
        weight,
    });
    Some(Shown {
        clip,
        mirrored,
        frame: j.frame(),
        blend,
        callbacks: Vec::new(),
    })
}

/// A swipe's channel ([`street_sim::swipe`]): its five-update blend from
/// the clip he pressed in (held) to the swipe's record at frame 0, then
/// the wait's even steps to the script's frame, as the simulation poses
/// it.
pub fn swipe_shown(clips: &GameClips, s: &street_sim::swipe::Swipe) -> Option<Shown> {
    let clock = ClipClock {
        clips,
        flight: None,
        landing: None,
        left_hand: false,
    };
    let (clip, mirrored) = clock.clip(i32::from(s.record))?;
    let blend = s.blend().map(|weight| ShownBlend {
        from: s.from.and_then(|(record, frame)| {
            let (clip, mirrored) = clock.clip(i32::from(record))?;
            Some((clip, mirrored, frame))
        }),
        weight,
    });
    Some(Shown {
        clip,
        mirrored,
        frame: s.frame(),
        blend,
        callbacks: Vec::new(),
    })
}

/// A trick move's, a defender's reaction's or a scoop's channel
/// ([`street_sim::trick`], [`street_sim::touch::Scooping`]): the simulation
/// keeps the script's clock (its record, frame and blend), so the clip is
/// the script's own. `None` when the actor plays none, or the disc does not
/// carry the clip.
pub fn script_shown(clips: &GameClips, action: &Action) -> Option<Shown> {
    script_or_stand_shown(clips, action, None)
}

/// [`script_shown`], or with nothing else under way the stand script on
/// the player's channel (`street_sim::stand`).
pub fn script_or_stand_shown(
    clips: &GameClips,
    action: &Action,
    stand: Option<&street_sim::stand::Standing>,
) -> Option<Shown> {
    let (shown, from) = match action {
        Action::Trick { play, .. } => (play.run.shown(), play.from),
        Action::Stumbling {
            reaction: Some(r), ..
        } => (r.run.shown(), r.from),
        Action::Scoop { play } => (play.run.shown(), play.from),
        // The counter stance and the take after it (`street_sim::counter`).
        Action::Stealing {
            stance: Some(s), ..
        } => (s.run.shown(), s.from),
        Action::None => {
            let s = stand?;
            (s.run.shown(), s.from)
        }
        _ => return None,
    };
    let clock = ClipClock {
        clips,
        flight: None,
        landing: None,
        left_hand: false,
    };
    let (clip, mirrored) = clock.clip(i32::from(shown.record?))?;
    let blend = shown.blend.map(|weight| ShownBlend {
        from: from.and_then(|(record, frame)| {
            let (clip, mirrored) = clock.clip(i32::from(record))?;
            Some((clip, mirrored, frame))
        }),
        weight,
    });
    Some(Shown {
        clip,
        mirrored,
        frame: shown.frame,
        blend,
        callbacks: Vec::new(),
    })
}

/// The simulation's own channel for player `i` while it plays a script
/// (`World::channel`, [`street_sim::held`]): a shot's, a dunk's, a
/// lay-up's, a pass's or a catch row's, with the alternates its blends
/// picked (the live hand, a left-hander, his side) and the clip the
/// simulation holds the ball on. `None` in his gait, without the disc's
/// animations in the simulation (online), or for a clip the disc does not
/// carry.
pub fn sim_shown(clips: &GameClips, w: &World, i: usize) -> Option<Shown> {
    let c = w.channel(i).filter(|c| !c.gait)?;
    let clock = ClipClock {
        clips,
        flight: None,
        landing: None,
        left_hand: false,
    };
    let (clip, mirrored) = clock.clip(i32::from(c.shown.record?))?;
    let blend = c.shown.blend.map(|weight| ShownBlend {
        from: c.from.and_then(|(record, frame)| {
            let (clip, mirrored) = clock.clip(i32::from(record))?;
            Some((clip, mirrored, frame))
        }),
        weight,
    });
    Some(Shown {
        clip,
        mirrored,
        frame: c.shown.frame,
        blend,
        callbacks: Vec::new(),
    })
}

/// Put the channel's pose on the animator: the clip at its frame, held
/// (the simulation's updates move it), with the script's blend as the
/// renderer's crossfade at the script's weight.
pub fn apply(animator: &mut Animator, shown: &Shown, heading: f32) {
    let same = Arc::ptr_eq(&animator.current.clip, &shown.clip)
        && animator.current.mirrored == shown.mirrored;
    match &shown.blend {
        Some(blend) if blend.weight < 1.0 => {
            if !same || !animator.fading() {
                // The blend's start: what the animator shows goes out.
                animator.play_with_mirror_and_heading_restart(
                    &shown.clip,
                    f32::MAX,
                    false,
                    true,
                    shown.mirrored,
                    Some(heading),
                    true,
                );
                if let Some((clip, mirrored, frame)) = &blend.from
                    && let Some(previous) = animator.previous_mut()
                {
                    previous.clip = clip.clone();
                    previous.mirrored = *mirrored;
                    previous.frame = *frame;
                    previous.looping = false;
                    previous.in_place = true;
                }
            }
            animator.set_fade_progress(blend.weight);
        }
        _ => {
            if !same {
                animator.play_with_mirror_and_heading_restart(
                    &shown.clip,
                    1e-3,
                    false,
                    true,
                    shown.mirrored,
                    Some(heading),
                    true,
                );
            }
            animator.clear_fade();
        }
    }
    animator.current.looping = false;
    animator.current.in_place = true;
    animator.current.frame = shown.frame;
    animator.hold();
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_assets::playback::{FixedClock, callback_tick};
    use street_sim::{Event, Hoop, Input, Phase, Player, Rules, button};

    fn disc() -> Option<(crate::setup::Disc, GameClips)> {
        let path = std::env::var("STREET_ISO").ok()?;
        let mut disc =
            crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = crate::setup::load_clips(&mut disc.iso).unwrap();
        Some((disc, clips))
    }

    #[test]
    fn the_programs_run_their_callbacks_on_the_walked_updates() {
        // The channel's clock against the walker's (`timing_tables.rs`,
        // measured on the captures): every shot's, dunk's, lay-up's and
        // pass's release lands on the table's update.
        let Some((_, clips)) = disc() else {
            return;
        };
        let clock = FixedClock {
            frames: Some(200),
            ..Default::default()
        };
        let programs = &clips.programs;
        for s in &timing::SHOT_SCRIPTS {
            let program = programs.get(s.script, Path::Plain).unwrap();
            assert_eq!(
                callback_tick(program, 21, &clock),
                Some(i32::from(s.release)),
                "{}",
                s.name
            );
            if s.check > 0 {
                assert!(
                    programs.get(s.script, Path::Fake).is_some(),
                    "{} fake",
                    s.name
                );
            }
        }
        // The default jump shots' pump fake (the `85` of callback 20 fallen
        // through) never lets the ball go.
        for id in timing::DEFAULT_SHOTS {
            let s = &timing::SHOT_SCRIPTS[id];
            let fake = programs.get(s.script, Path::Fake).unwrap();
            assert_eq!(callback_tick(fake, 21, &clock), None, "{} fake", s.name);
        }
        // Every row [`timing::move_row`] names: the table's, the leaps',
        // the board's lay-up and the mid-air ones.
        let rows = timing::MOVE_ROWS
            .iter()
            .chain(&timing::LEAP_ROWS)
            .chain(&timing::EXTRA_ROWS);
        for r in rows.filter(|r| r.release > 0) {
            let program = programs.get(r.script, Path::Plain).unwrap();
            assert_eq!(
                callback_tick(program, 21, &clock),
                Some(i32::from(r.release)),
                "{}",
                r.name
            );
        }
        for r in timing::PASS_ROWS.iter().filter(|r| r.release > 0) {
            let program = programs.get(r.script, Path::Plain).unwrap();
            assert_eq!(
                callback_tick(program, 38, &clock),
                Some(i32::from(r.release)),
                "{:#x}",
                r.script
            );
        }
        assert!(programs.len() > 36 + 99 + 93 + 39 - 60);
    }

    #[test]
    fn every_scripted_clip_is_on_the_disc() {
        // Each program's blend targets name animation records whose clips
        // the disc carries, so the channel always has something to show.
        let Some((_, clips)) = disc() else {
            return;
        };
        let clock = ClipClock {
            clips: &clips,
            flight: None,
            landing: None,
            left_hand: false,
        };
        let mut missing = Vec::new();
        let mut targets = 0;
        for program in clips.programs.plain.values() {
            for op in &program.ops {
                if let playback::Op::Blend { targets: t, .. } = op {
                    for target in t {
                        if target.record == playback::CURRENT_RECORD {
                            continue;
                        }
                        targets += 1;
                        if clock.clip(target.record).is_none() {
                            missing.push((program.address, target.record));
                        }
                    }
                }
            }
        }
        assert!(targets > 300, "{targets}");
        assert!(missing.is_empty(), "{missing:?}");
    }

    fn live(players: Vec<Player>) -> World {
        let mut w = World::new(Hoop::native(), players, Rules::default(), 7);
        w.phase = Phase::Live;
        w.ball.hold(0);
        w
    }

    /// Step `w` with `inputs` until `stop` says so (within 240 updates),
    /// returning how many updates it took.
    fn step_until(w: &mut World, inputs: &[Input], stop: impl Fn(&World) -> bool) -> u32 {
        for n in 1..=240 {
            w.step(inputs);
            if stop(w) {
                return n;
            }
        }
        panic!("never happened");
    }

    #[test]
    fn the_lay_ups_past_the_dunk_table_play_their_scripts() {
        // Through World::step: from behind the board within 3.5 of the
        // middle the shoot press starts `layup_3_bboard` (`FUN_0016b4b8`,
        // a row past the leaps), whose script the channel plays (it was not
        // compiled before, so the display fell back on the gait).
        let Some((_, clips)) = disc() else {
            return;
        };
        let mut w = live(vec![Player::new(
            0,
            [40.0, 1.0],
            std::f32::consts::FRAC_PI_2,
        )]);
        w.players[0].gait_channel.hand = 1;
        w.step(&[Input::default().with(button::SHOOT)]);
        let Action::Layup { row, .. } = w.players[0].action else {
            panic!("the board's lay-up: {:?}", w.players[0].action);
        };
        assert_eq!(row, timing::BOARD_LAYUP_ROW);
        let s = scripted(&w, 0).unwrap();
        assert_eq!(s.address, timing::EXTRA_ROWS[0].script);
        assert!(matches!(shown(&clips, &s, false), Some(Channel::Pose(_))));
    }

    #[test]
    fn the_channel_shows_the_sims_scripts_and_releases_on_their_updates() {
        // Through World::step: the script the sim names plays from the
        // press, and its release callback runs on the update the sim lets
        // the ball go in.
        let Some((_, clips)) = disc() else {
            return;
        };
        let clock = |flight| ClipClock {
            clips: &clips,
            flight,
            landing: None,
            left_hand: false,
        };
        let release_of = |s: &Scripted, callback: u32| {
            callback_tick(
                clips.programs.get(s.address, s.path).unwrap(),
                callback,
                &clock(s.flight),
            )
            .unwrap()
        };
        let callbacks_shown =
            |w: &World, i: usize| match shown(&clips, &scripted(w, i).unwrap(), false) {
                Some(Channel::Pose(p)) => p.callbacks,
                _ => panic!("no pose"),
            };

        // A jump shot from the top of the key, the button held through it
        // (20 in front of the rim team 0 attacks, at +x, facing it).
        let rim = Hoop::native()[0].rim;
        let mut w = live(vec![Player::new(
            0,
            [rim[0] - 20.0, 0.0],
            std::f32::consts::FRAC_PI_2,
        )]);
        let held = [Input::default().with(button::SHOOT)];
        w.step(&held);
        let s = scripted(&w, 0).expect("the shot script from the press");
        let Action::Shooting { script, .. } = w.players[0].action else {
            panic!("a shot");
        };
        assert_eq!(s.address, timing::SHOT_SCRIPTS[usize::from(script)].script);
        // The press update is the script's tick 0, inside its opening
        // blend.
        assert_eq!(s.tick, 0);
        let Some(Channel::Pose(p)) = shown(&clips, &s, false) else {
            panic!("a pose");
        };
        assert!(p.blend.is_some_and(|b| b.from.is_none() && b.weight < 1.0));
        // The button let go at 20 (before the release gate), as a person
        // who wants the ball gone at the script's own moment.
        let let_go = [Input::default()];
        let mut n = 0;
        let n = loop {
            n += 1;
            w.step(if n < 20 { &held } else { &let_go });
            if w.events
                .iter()
                .any(|e| matches!(e, Event::Shot { player: 0, .. }))
            {
                break n;
            }
            assert!(n < 240, "never released");
        };
        let s = scripted(&w, 0).unwrap();
        assert_eq!(s.tick, release_of(&s, 21));
        assert_eq!(callbacks_shown(&w, 0), [21]);
        assert_eq!(
            n,
            u32::from(timing::SHOT_SCRIPTS[usize::from(script)].release)
        );

        // A dunk: the button where a dunk is on (standing within 7.5 of the
        // rim, in front of it: `World::dunk_on`), with a two-turbo combo,
        // whose slot's dunk `FUN_001693b8` plays (one turbo draws among the
        // dunk and lay-up rows).
        let mut w = live(vec![Player::new(
            0,
            [rim[0] - 7.0, 0.0],
            -std::f32::consts::FRAC_PI_2,
        )]);
        let turbo = [Input::default().with(button::SHOOT | button::R1 | button::L2)];
        w.step(&turbo);
        let Action::Dunking { row, release, .. } = w.players[0].action else {
            panic!("a dunk: {:?}", w.players[0].action);
        };
        let s = scripted(&w, 0).unwrap();
        assert_eq!(s.address, timing::MOVE_ROWS[usize::from(row)].script);
        assert_eq!(s.tick, 0);
        let n = step_until(&mut w, &turbo, |w| {
            w.ball.state != street_sim::BallState::Held(0)
        });
        assert_eq!(n, u32::from(release));
        let s = scripted(&w, 0).unwrap();
        assert_eq!(s.tick, release_of(&s, 21));
        assert_eq!(callbacks_shown(&w, 0), [21]);

        // A pass: the row's script from the press, its callback 38 on the
        // release update; the receiver's catch row from that update, on
        // through the flight and the hold after the catch.
        let mut w = live(vec![
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(0, [0.0, 14.0], std::f32::consts::PI),
        ]);
        let press = [Input::default().with(button::PASS), Input::default()];
        w.step(&press);
        let windup = w.windup.expect("the pass in the hands");
        let s = scripted(&w, 0).unwrap();
        assert_eq!(s.address, timing::PASS_ROWS[usize::from(windup.row)].script);
        assert_eq!(s.tick, 0, "the press update is the script's first");
        let idle = [Input::default(), Input::default()];
        let n = step_until(&mut w, &idle, |w| {
            w.ball.state == street_sim::BallState::Pass
        });
        assert_eq!(
            n,
            u32::from(timing::PASS_ROWS[usize::from(windup.row)].release)
        );
        let s = scripted(&w, 0).unwrap();
        assert_eq!(s.tick, release_of(&s, 38));
        assert_eq!(callbacks_shown(&w, 0), [38]);
        // The follow-through: the same script, on past the release.
        w.step(&idle);
        let after = scripted(&w, 0).unwrap();
        assert_eq!((after.address, after.tick), (s.address, s.tick + 1));
        // The receiver: the catch row from the release update.
        let pass = w.ball.pass.unwrap();
        let catch = scripted(&w, 1).expect("the catch script on the receiver");
        assert_eq!(
            catch.address,
            timing::CATCH_ROWS[usize::from(pass.catch)].script
        );
        assert_eq!(catch.tick, 1);
        assert_eq!(catch.flight, Some(w.ball.flight_ticks()));
        let flown = step_until(&mut w, &idle, |w| {
            w.ball.state == street_sim::BallState::Held(1)
        });
        let c = w.catch.expect("the hold");
        assert_eq!((c.row, c.since), (pass.catch, pass.since));
        let held = scripted(&w, 1).unwrap();
        assert_eq!(held.address, catch.address);
        assert_eq!(held.tick as u32, 1 + flown);
        assert_eq!(held.flight, Some(i32::from(c.flight)));
        assert!(matches!(
            shown(&clips, &held, false),
            Some(Channel::Pose(_))
        ));
        // The hold runs the script out within a few updates of the sim's
        // hold (the row's tail is the captures' measure, the clip's
        // play-out the script's).
        let mut over_at = None;
        for n in 0..40u64 {
            w.step(&idle);
            match scripted(&w, 1) {
                Some(s) if matches!(shown(&clips, &s, false), Some(Channel::Over)) => {
                    over_at = Some(n);
                    break;
                }
                None => {
                    over_at = Some(n);
                    break;
                }
                _ => {}
            }
        }
        assert!(over_at.is_some());
    }
}
