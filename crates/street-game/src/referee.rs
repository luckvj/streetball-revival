//! The referee who opens a match with the toss (`AIPhysReferee`, game
//! `+0x170`, built by `FUN_0017c600`).
//!
//! The intro's end (`FUN_00142368`) shows him (vtable `+0x3c`, `FUN_0017d1f0`)
//! and `FUN_00141fd8` places him: `FUN_0017cf28` writes his position
//! `(-0.9, 0, -3.0, 1)` (raw `0x17cf2c..0x17cf7c`), starts script
//! `0x457412` (an 8-frame blend to record 656, `stndrlx_cradle`, played
//! at one frame each 2.5 updates by opcode `0xab`) and turns him to face
//! the centre (the same quantized heading as `FUN_0018ce78`). The ball is
//! his (`FUN_0014cb28`).
//!
//! His plan (`0x1847c0`, game phase 1) queues two tasks: "referee wait"
//! (`FUN_0011ad68`, update `FUN_0011aea0`), which presses `0x200000` once
//! the six players are ready and its count is out and ends when he no
//! longer has the ball, then a "goto" (`FUN_0010f278(task, ai, (0, 0,
//! -51.5), 1, 3, 0, fps * 30)`, raw `0x1847c0..0x18488c`; update
//! `FUN_0010f3b8`): from the launch, each update before his own
//! `FUN_0013aed8` sets his stick toward `(0, -51.5)` at speed level 3
//! (`DAT_003d17d8[3]` = 64), its word `FUN_0013ac80`'s `(int)(atan2(x -
//! 0, z + 51.5) * 162.97466 + 0.5) & 0x3ff`, the
//! [`ordinary::native_heading`] of where he stands as the update begins;
//! `FUN_0013af58` keeps the stick's last word while the new one is within
//! 6 units of it (capture 3 runs on 1022 through update 262, on 1016 from
//! 263 and on 1009 from 272). The goto ends (`FUN_0010f318`) within 0.75 of the spot
//! or after 1,800 updates and `FUN_0013ac30` lets the stick go.
//!
//! His update `FUN_0017c8e0`, in its order: the pad (`+0x88` the stick's
//! word, `+0x90` its length); with the ball and no script lock
//! (channel `+0x48 & 2`) the press starts the toss script `0x44cb8f`; with
//! no lock `FUN_0017cdd0`: a stick past 16 sets `+0xb0` to its word and his
//! physical vector `+0x40` to a third of a unit along it
//! ([`input::native_velocity_step`], the cosine table `0x440ea0`) and,
//! unless record 948 already plays (`FUN_00168e78` with `0x3d9a30`),
//! starts script `0x44cb61` (`0x3d9a38`); then the channel's script and
//! root travel (`FUN_0019c658`, `FUN_0019a658`); then `+0x30 += +0x40`.
//!
//! The scripts (raw bytes):
//! - `0x44cb8f`, the toss: `96 97`, `91 0x00020002` (the lock 2), an
//!   8-frame blend to record 949 (`ref01_regcourt_toss`) from frame 0, `8d
//!   ee 06 32` (frame 6 over 50 updates), callback 44 (the launch,
//!   `FUN_0014c858`), `8c 1.5` (the rest of the clip at a frame each 1.5
//!   updates, from the launch's update to the last frame) and a jump to
//!   `0x44cb78` the next update.
//! - `0x44cb78`: `96 97`, `91 0x00820000` (no lock; 0x800000 makes the
//!   blend match the heading), an 8-frame blend to record 947
//!   (`ref01_regcourt_runoff`), `ab 1.0`.
//! - `0x44cb61`: the same with record 948 (`ref01_regcourt_runon`).
//!
//! Every script starts with `97`, which zeroes `+0x40`: the update
//! `FUN_0017cdd0` starts 948 he does not move, and from the next he moves
//! a third of a unit an update. A script's 8-frame blend holds its clip at
//! frame 0 from its start's update through the seventh after it, with no
//! root travel; the eighth completes it and the clip goes on from there.
//! So 947 shows for one update before 948 replaces it. A blend with
//! `0x800000` (`91`'s operand ORs it in) turns him (`FUN_00196698`,
//! argument 1): his facing word becomes the world root's heading less the
//! incoming clip's at frame 0 when they differ by 23 units or more (the
//! toss leaves his root at 278 on his facing 561, so 947's blend makes it
//! 276). His update ends with `FUN_00154560` (vtable `+0x64`): the facing
//! turns 48 units an update toward `+0xb0` (on it within 48) and the
//! quaternion `+0x50` is built from the cosine table with the word halved
//! as an integer, which the root travel turns by: back toward 561, then
//! from 948's start toward the stick's 1022, reached seven updates later. Capture 2 (`docs/TRACE.md`,
//! "Runtime captures") has the launch at update 120, 947's script at 135,
//! 948's at 136, the first move at 137 (`(0.0041, -0.3333)` an update, the
//! word 1022) and 948's frame 1 at 144. When somebody has the ball the
//! opening phase ends and he is hidden again (`FUN_001415c0`,
//! `FUN_0013e4c8` case 1: vtable `+0x3c` with 0).
//!
//! The rewrite's own: his clock is the simulation's toss
//! ([`street_sim::OpeningToss`]): the toss script starts 58 updates before
//! the launch the simulation times ([`street_sim::toss::WAIT_UPDATES`]);
//! the heading match of 948's blend reads the toss's last world root (the
//! original's reads the pose a blend's first update shows, which 947's
//! own match keeps at that heading); the blends are the renderer's
//! crossfades with the outgoing pose turned by the heading's change.

use bevy::prelude::*;
use street_render::animation::{Animator, Clip};
use street_sim::{OpeningToss, input, ordinary, restart, timing, toss};

use crate::{game::Sim, setup::GameClips};

/// Record 656, script `0x457412`'s clip while he holds the ball.
pub const CRADLE: &str = "stndrlx_cradle";
/// Record 949, the toss script's (`0x44cb8f`).
pub const TOSS: &str = "ref01_regcourt_toss";
/// Record 947, the toss script's last clip (`0x44cb78`), shown one update.
pub const RUNOFF: &str = "ref01_regcourt_runoff";
/// Record 948, the clip he leaves on (`0x44cb61`, from `FUN_0017cdd0`).
pub const RUN: &str = "ref01_regcourt_runon";
/// `FUN_0017cdd0`'s speed: a third of a unit an update.
pub const RUN_SPEED: f32 = 0.333_333_34;
/// The goto's spot (`0x1847c0`: `0xc24e0000` at z).
pub const RUN_TO: [f32; 3] = [0.0, 0.0, -51.5];
/// `FUN_0010f318`: the goto is over within 0.75 of its spot.
const GOTO_REACH: f32 = 0.75;
/// `FUN_0013af58`: a new stick word under 6 units from the last keeps it.
const STICK_HOLD: i32 = 6;
/// Script `0x457412`'s `ab 0x40200000`: a frame each 2.5 updates.
pub const CRADLE_RATE: f32 = 1.0 / 2.5;
/// `8d ee 06 32`: the toss reaches frame 6 over 50 updates.
pub const TOSS_FRAME: f32 = 6.0;
pub const TOSS_UPDATES: u16 = 50;
/// `8c 0x3fc00000`: the rest of the toss clip at a frame each 1.5 updates.
pub const AFTER_TOSS_RATE: f32 = 1.0 / 1.5;
/// Scripts `0x44cb78` and `0x44cb61`'s `ab 0x3f800000`: a frame an update.
pub const RUNOFF_RATE: f32 = 1.0;
/// Each script's blend (`8e 08`), in updates.
pub const BLEND_UPDATES: f32 = 8.0;
/// `FUN_00196698`'s tolerance: `0x41ba0b3f` (23.255491) rounded.
const HEADING_TOLERANCE: i32 = 23;

/// The clip he plays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Part {
    #[default]
    Cradle,
    Toss,
    Runoff,
    Run,
}

impl Part {
    pub fn clip(self) -> &'static str {
        match self {
            Part::Cradle => CRADLE,
            Part::Toss => TOSS,
            Part::Runoff => RUNOFF,
            Part::Run => RUN,
        }
    }

    fn looping(self) -> bool {
        !matches!(self, Part::Toss)
    }

    /// The script's `91` sets 0x800000: its blend matches the heading.
    fn matches_heading(self) -> bool {
        matches!(self, Part::Cradle | Part::Runoff | Part::Run)
    }
}

/// Where the referee is and what he plays, stepped with the simulation.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Referee {
    /// Shown (vtable `+0x3c`).
    pub on_court: bool,
    /// His root on the floor (`+0x30`).
    pub at: Vec3,
    /// The simulation's heading convention (`Player::heading`), from
    /// [`Referee::word`].
    pub heading: f32,
    /// His facing word (`+0xa8`).
    pub word: i32,
    /// The facing he turns to (`+0xb0`).
    pub wanted: i32,
    /// The goto's stick word (the steer's last, `FUN_0013af58`): none
    /// before the launch or once the goto is over.
    stick: Option<i32>,
    /// The goto ended within 0.75 of its spot (`FUN_0010f318`).
    arrived: bool,
    pub part: Part,
    /// The clip's frame (`channel +0x64`).
    pub frame: f32,
    /// Updates left of the current script's blend, which holds the frame.
    blend: u8,
    /// The toss went up.
    launched: bool,
    /// The last simulation tick stepped.
    tick: Option<u64>,
}

impl Referee {
    /// One update of the opening (`FUN_0017c8e0` and his scripts), with the
    /// clips for their root travel, length and roots' headings.
    pub fn step(
        &mut self,
        toss_state: OpeningToss,
        clip: impl Fn(Part) -> Option<std::sync::Arc<Clip>>,
    ) {
        let before = (self.part, self.frame);
        let mut moving = Vec3::ZERO;
        match toss_state {
            OpeningToss::Waiting(left) => {
                let placed = !self.on_court || self.launched;
                if placed {
                    *self = Referee::placed();
                }
                let elapsed = toss::WAIT_UPDATES.saturating_sub(left);
                let toss_from = toss::WAIT_UPDATES - TOSS_UPDATES;
                let script_from = toss_from - BLEND_UPDATES as u16;
                if elapsed < script_from {
                    if !placed && !self.blending() {
                        self.frame += CRADLE_RATE;
                    }
                } else if elapsed == script_from || (!placed && self.part != Part::Toss) {
                    // FUN_0017c8e0's press starts script 0x44cb8f (online,
                    // where a frame may step several ticks, on the first
                    // update this one sees past it).
                    self.start(Part::Toss, &clip);
                } else if !self.blending() {
                    // 8d: frame 6 on the update before the launch.
                    self.frame = TOSS_FRAME * f32::from((elapsed + 1).saturating_sub(toss_from))
                        / f32::from(TOSS_UPDATES);
                }
            }
            OpeningToss::Flight => {
                if !self.on_court {
                    return;
                }
                if !self.launched {
                    // Callback 44 on frame 6; 8c goes on in the same update.
                    self.launched = true;
                    self.set(Part::Toss, TOSS_FRAME);
                }
                self.steer();
                // FUN_0017cdd0 while no script locks him: the goto's stick.
                let mut started = false;
                if self.part != Part::Toss
                    && let Some(word) = self.stick
                {
                    self.wanted = word;
                    let [x, z] = input::native_velocity_step(RUN_SPEED, word);
                    moving = Vec3::new(x, 0.0, z);
                    if self.part != Part::Run {
                        // 0x44cb61's 97 zeroes the move it was started with.
                        self.start(Part::Run, &clip);
                        moving = Vec3::ZERO;
                        started = true;
                    }
                }
                match self.part {
                    _ if started => {}
                    Part::Toss => {
                        let last =
                            clip(Part::Toss).map_or(0.0, |c| c.frames().saturating_sub(1) as f32);
                        if self.frame >= last - 1e-3 {
                            // The script's jump to 0x44cb78 the update after.
                            self.start(Part::Runoff, &clip);
                        } else {
                            self.frame = (self.frame + AFTER_TOSS_RATE).min(last);
                        }
                    }
                    _ => {
                        if !self.blending() {
                            self.frame += RUNOFF_RATE;
                        }
                    }
                }
            }
            OpeningToss::None => {
                self.on_court = false;
                return;
            }
        }
        // FUN_0019a658: the clip's root travel, turned with him; none while
        // a blend holds the frame.
        if before.0 == self.part
            && let Some(c) = clip(self.part)
        {
            let travel = root_travel(&c, self.part.looping(), before.1, self.frame);
            let turned = actor_quat(self.word) * travel;
            self.at += Vec3::new(turned.x, 0.0, turned.z);
        }
        // +0x30 += +0x40.
        self.at += moving;
        // FUN_00154560 (vtable +0x64): the facing turns to +0xb0.
        self.word = turned_toward(self.word, self.wanted);
        self.heading = restart::heading_radians(self.word);
    }

    /// `FUN_0017cf28`: at his spot facing the centre, cradling the ball.
    pub fn placed() -> Self {
        let at = Vec3::from_array(toss::REFEREE);
        let word = ordinary::native_heading(toss::REFEREE, [0.0; 3]);
        Self {
            on_court: true,
            at,
            heading: restart::heading_radians(word),
            word,
            wanted: word,
            stick: None,
            arrived: false,
            part: Part::Cradle,
            frame: 0.0,
            // Script 0x457412's 8-frame blend.
            blend: BLEND_UPDATES as u8 - 1,
            launched: false,
            tick: None,
        }
    }

    /// The goto's update before his own (`FUN_0010f318`, `FUN_0010f3b8`):
    /// over within 0.75 of the spot, else `FUN_0013aed8`'s word from where
    /// he stands, which `FUN_0013af58` takes only 6 units or more from the
    /// stick's last.
    fn steer(&mut self) {
        if self.arrived {
            return;
        }
        let to = Vec3::from_array(RUN_TO);
        if Vec2::new(self.at.x - to.x, self.at.z - to.z).length() < GOTO_REACH {
            self.arrived = true;
            self.stick = None;
            return;
        }
        let word = ordinary::native_heading(self.at.to_array(), RUN_TO);
        let kept = self
            .stick
            .is_some_and(|last| timing::angle_diff(word, last).abs() < STICK_HOLD);
        if !kept {
            self.stick = Some(word);
        }
    }

    fn set(&mut self, part: Part, frame: f32) {
        self.part = part;
        self.frame = frame;
    }

    /// A script's start: its clip from frame 0, held through the blend,
    /// and with 0x800000 the blend's heading match (`FUN_00196698`) from
    /// the pose shown (the clip playing, at its frame, turned by his facing).
    fn start(&mut self, part: Part, clip: &impl Fn(Part) -> Option<std::sync::Arc<Clip>>) {
        if part.matches_heading()
            && let Some(world) = clip(self.part)
                .and_then(|c| c.root_rotation(self.frame, self.part.looping()))
                .map(|q| actor_quat(self.word) * q)
            && let Some(incoming) = clip(part).and_then(|c| c.root_rotation(0.0, part.looping()))
        {
            self.word = matched_heading(self.word, world, incoming);
            self.heading = restart::heading_radians(self.word);
        }
        self.set(part, 0.0);
        self.blend = BLEND_UPDATES as u8 - 1;
    }

    /// Counts the blend down; true while it still holds the frame.
    fn blending(&mut self) -> bool {
        if self.blend > 0 {
            self.blend -= 1;
            return true;
        }
        false
    }
}

/// His turn (`+0x50`) from his facing word as `FUN_00154560` and
/// `FUN_00196698` build it: `(0, sin(w / 2), 0, cos(w / 2))` from the
/// cosine table `0x440ea0` with the word halved as an integer, so an odd
/// word turns him a unit short.
pub fn actor_quat(word: i32) -> Quat {
    let half = (word & 0x3ff) / 2;
    Quat::from_xyzw(
        0.0,
        input::native_sin_1024(half),
        0.0,
        input::native_cos_1024(half),
    )
}

/// `FUN_00154560`: the facing word 48 units nearer the wanted one the
/// shorter way round, or on it within 48.
pub fn turned_toward(word: i32, wanted: i32) -> i32 {
    let diff = wanted - word;
    if diff == 0 {
        return word;
    }
    if diff.abs() < 0x31 {
        return wanted & 0x3ff;
    }
    let step = if diff < -0x200 || (1..=0x200).contains(&diff) {
        0x30
    } else {
        -0x30
    };
    (word + step) & 0x3ff
}

/// `FUN_00196698`'s and `FUN_00154230`'s heading of a root quaternion: the
/// model root's axis `(0, 1, 0)` turned by it, `(int)(atan2(x, z) *
/// 162.97466 + 0.5)`, 1024 added when not positive, then `(+ 0x200) & 0x3ff`.
pub fn root_word(q: Quat) -> i32 {
    let v = q * Vec3::Y;
    let a = (street_sim::math::atan2(v.x, v.z) * 162.974_66 + 0.5) as i32;
    let a = if a > 0 { a } else { a + 0x400 };
    (a + 0x200) & 0x3ff
}

/// `FUN_00196698(channel, 1)`: the facing word that puts the incoming
/// clip's root (`incoming`, its frame 0) on the world root's heading
/// (`world`), unless they are within the tolerance.
pub fn matched_heading(word: i32, world: Quat, incoming: Quat) -> i32 {
    let new = (root_word(world) - root_word(incoming)) & 0x3ff;
    let delta = (word - new) & 0x3ff;
    if !(HEADING_TOLERANCE..=1024 - HEADING_TOLERANCE).contains(&delta) {
        word
    } else {
        new
    }
}

/// The root's floor travel from frame `a` to `b` of `clip`
/// (`FUN_0019a658`: the sampled root less the last one, `+0x90`): a looped
/// clip's wrap takes it back by the lap's travel, as capture 2's run does.
fn root_travel(clip: &Clip, looping: bool, a: f32, b: f32) -> Vec3 {
    let at = |f: f32| clip.root(f, looping, false).unwrap_or(Vec3::ZERO);
    at(b) - at(a)
}

/// Marks the referee's model.
#[derive(Component)]
pub struct RefereeView;

/// His update after the simulation's.
pub fn step(sim: Res<Sim>, clips: Res<GameClips>, mut referee: ResMut<Referee>) {
    let w = &sim.world;
    if referee.tick == Some(w.tick) {
        return;
    }
    referee.tick = Some(w.tick);
    referee.step(w.opening_toss, |part| clips.clips.get(part.clip()));
}

/// Draws him: shown or hidden, at his root, playing his clip at its frame
/// (in place: [`Referee::at`] carries the travel), the outgoing pose of a
/// blend turned by his heading's change.
pub fn sync(
    referee: Res<Referee>,
    clips: Res<GameClips>,
    mut views: Query<(&mut Transform, &mut Animator, &mut Visibility), With<RefereeView>>,
) {
    for (mut transform, mut animator, mut visibility) in &mut views {
        visibility.set_if_neq(if referee.on_court {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if !referee.on_court {
            continue;
        }
        transform.translation = referee.at;
        transform.rotation = actor_quat(referee.word);
        let part = referee.part;
        if let Some(clip) = clips.clips.get(part.clip()) {
            animator.play_with_mirror_and_heading(
                &clip,
                BLEND_UPDATES / 60.0,
                part.looping(),
                true,
                false,
                Some(referee.heading),
            );
            animator.current.frame = referee.frame;
            animator.hold();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_referee_plays_the_discs_clips_through_the_opening() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let clips =
            street_render::animation::Clips::load(&mut iso, &[CRADLE, TOSS, RUNOFF, RUN]).unwrap();
        for part in [Part::Cradle, Part::Toss, Part::Runoff, Part::Run] {
            assert!(clips.get(part.clip()).is_some(), "{}", part.clip());
        }
        let toss_frames = clips.get(TOSS).unwrap().frames();
        assert!(toss_frames as f32 > TOSS_FRAME);
        // A match's opening through the simulation: placed, cradling, the
        // toss at frame 6 on the update before the launch, then the rest
        // and the run.
        let hoop = |x: f32| street_sim::Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        let players = (0..6)
            .map(|k| street_sim::Player::new(k / 3, [0.0, k as f32], 0.0))
            .collect();
        let mut w = street_sim::World::new(
            [hoop(37.0), hoop(-37.0)],
            players,
            street_sim::Rules::default(),
            1,
        );
        w.start_opening_toss();
        let mut r = Referee::default();
        let clip = |part: Part| clips.get(part.clip());
        let mut parts = Vec::new();
        for _ in 0..toss::WAIT_UPDATES {
            w.step(&[street_sim::Input::default(); 6]);
            r.step(w.opening_toss, clip);
            parts.push(r.part);
        }
        assert_eq!(w.opening_toss, OpeningToss::Flight);
        // 8c goes on from the launch's update.
        assert_eq!(
            (r.part, r.frame),
            (Part::Toss, TOSS_FRAME + AFTER_TOSS_RATE)
        );
        let cradled = parts.iter().filter(|&&p| p == Part::Cradle).count();
        // The first update after the intro is the first counted (elapsed
        // 1); the toss script starts its 8-update blend 58 before the launch.
        assert_eq!(
            cradled,
            usize::from(toss::WAIT_UPDATES - TOSS_UPDATES - BLEND_UPDATES as u16 - 1)
        );
        // The rest of the toss at a frame each 1.5 updates to its last
        // frame, then 947 for one update and the run.
        let rest = ((toss_frames - 1) as f32 - TOSS_FRAME) / AFTER_TOSS_RATE;
        let mut steps = 1;
        while r.part == Part::Toss {
            r.step(OpeningToss::Flight, clip);
            steps += 1;
        }
        assert_eq!(steps as f32, rest.round() + 1.0);
        assert_eq!(r.part, Part::Runoff);
        r.step(OpeningToss::Flight, clip);
        assert_eq!(r.part, Part::Run);
        let running = r.at;
        for _ in 0..48 {
            r.step(OpeningToss::Flight, clip);
        }
        // Two laps of 948: its root travel comes back, the goto's third of a
        // unit an update toward (0, -51.5) does not.
        let run = r.at - running;
        assert!((run.length() - 48.0 * RUN_SPEED).abs() < 0.05, "{run}");
        assert!(run.z < -15.0 && run.x.abs() < 0.5, "{run}");
    }

    #[test]
    fn the_referee_cradles_tosses_at_frame_6_and_runs_off_like_script_0x44cb8f() {
        let mut r = Referee::default();
        let no_clips = |_: Part| None;
        // The intro's end: placed at (-0.9, 0, -3.0) facing the centre.
        r.step(OpeningToss::Waiting(toss::WAIT_UPDATES - 1), no_clips);
        assert!(r.on_court);
        assert_eq!(r.at, Vec3::new(-0.9, 0.0, -3.0));
        let facing = Vec3::new(r.heading.sin(), 0.0, r.heading.cos());
        let centre = -r.at.normalize();
        assert!(facing.dot(centre) > 0.99, "{facing} {centre}");
        assert_eq!((r.part, r.frame), (Part::Cradle, 0.0));
        // Script 0x457412's blend holds frame 0 from its update through the
        // seventh after it; the eighth gains a frame each 2.5 updates.
        let wait = toss::WAIT_UPDATES;
        for left in (wait - 8..wait - 1).rev() {
            r.step(OpeningToss::Waiting(left), no_clips);
        }
        assert_eq!(r.frame, 0.0);
        r.step(OpeningToss::Waiting(wait - 9), no_clips);
        assert_eq!(r.frame, CRADLE_RATE);
        // The toss script starts 58 updates before the launch...
        let script_left = TOSS_UPDATES + BLEND_UPDATES as u16;
        for left in (script_left + 1..wait - 9).rev() {
            r.step(OpeningToss::Waiting(left), no_clips);
        }
        assert_eq!(r.part, Part::Cradle);
        r.step(OpeningToss::Waiting(script_left), no_clips);
        assert_eq!((r.part, r.frame), (Part::Toss, 0.0));
        // ...holds frame 0 through its blend, then reaches frame 6 over 50,
        // on the update before the launch.
        for left in (TOSS_UPDATES + 1..script_left).rev() {
            r.step(OpeningToss::Waiting(left), no_clips);
        }
        assert_eq!(r.frame, 0.0);
        r.step(OpeningToss::Waiting(TOSS_UPDATES), no_clips);
        assert_eq!(r.frame, TOSS_FRAME / f32::from(TOSS_UPDATES));
        r.step(OpeningToss::Waiting(25), no_clips);
        assert_eq!(r.frame, 3.12);
        // The simulation launches where Waiting(0) would be.
        for left in (1..25).rev() {
            r.step(OpeningToss::Waiting(left), no_clips);
        }
        assert_eq!(r.frame, TOSS_FRAME);
        let placed = r.word;
        // Without the clip's length the toss is over at once: script
        // 0x44cb78 on the launch, then FUN_0017cdd0's 0x44cb61, still the
        // update after that, and the run toward (0, -51.5).
        r.step(OpeningToss::Flight, no_clips);
        assert_eq!((r.part, r.frame), (Part::Runoff, 0.0));
        let at = r.at;
        r.step(OpeningToss::Flight, no_clips);
        assert_eq!((r.part, r.frame), (Part::Run, 0.0));
        assert_eq!(r.at, at);
        r.step(OpeningToss::Flight, no_clips);
        // The word 1022: (0.0041, -0.3333) an update, as capture 2 has it.
        let word = ordinary::native_heading(at.to_array(), RUN_TO);
        assert_eq!(word, 1022);
        let step = r.at - at;
        assert!(
            (step - Vec3::new(0.00409, 0.0, -0.33331)).length() < 1e-4,
            "{step}"
        );
        // Without clips nothing matches the heading; his facing turns 48
        // units an update from 561 toward the stick's 1022.
        assert_eq!((placed, r.wanted, r.word), (561, 1022, 657));
        for _ in 0..6 {
            r.step(OpeningToss::Flight, no_clips);
        }
        assert_eq!(r.frame, 0.0);
        r.step(OpeningToss::Flight, no_clips);
        assert_eq!(r.frame, RUNOFF_RATE);
        // Somebody has the ball: gone.
        r.step(OpeningToss::None, no_clips);
        assert!(!r.on_court);
    }

    #[test]
    fn a_blend_with_0x800000_turns_him_past_the_tolerance_like_fun_00196698() {
        // A root whose axis (0, 1, 0) lies along +z, turned by w units:
        // its heading word is w + 0x200.
        let turn = |w: i32| {
            Quat::from_rotation_y(w as f32 * (std::f32::consts::TAU / 1024.0))
                * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
        };
        assert_eq!(root_word(turn(100)), 612);
        // The world root on word 100, the incoming root turned 300 units:
        // the facing becomes 100 - 300.
        assert_eq!(
            matched_heading(100, turn(100), turn(300)),
            (100 - 300) & 0x3ff
        );
        // Within 22 units it keeps the facing; 23 and 1001 turn him.
        assert_eq!(matched_heading(100, turn(122), turn(0)), 100);
        assert_eq!(matched_heading(100, turn(123), turn(0)), 123);
        assert_eq!(matched_heading(100, turn(100 + 1001), turn(0)), 77);
        assert_eq!(matched_heading(100, turn(100 + 1002), turn(0)), 100);
    }

    #[test]
    fn his_facing_turns_48_units_an_update_the_short_way_like_fun_00154560() {
        assert_eq!(turned_toward(561, 561), 561);
        assert_eq!(turned_toward(276, 561), 324);
        assert_eq!(turned_toward(324, 1022), 276);
        assert_eq!(turned_toward(35, 1022), 1011);
        assert_eq!(turned_toward(1011, 1022), 1022);
        assert_eq!(turned_toward(1000, 100), 1048 & 0x3ff);
        assert_eq!(turned_toward(100, 148), 148);
        // An odd word turns the model a unit short (561 shows as 560).
        let want = Quat::from_rotation_y(560.0 * std::f32::consts::TAU / 1024.0);
        assert!(actor_quat(561).angle_between(want) < 1e-4);
    }

    /// A capture's opening, update by update from the launch until the
    /// goto is over (his update goes on while he is hidden): the rest of
    /// the toss's root travel, 947's update, 948's start, the goto's move,
    /// its stick's re-aims and 948's root travel. Skipped unless
    /// `STREET_AI_CAPTURE` names a capture with the referee and
    /// `STREET_ISO` is set.
    #[test]
    fn the_referee_runs_off_like_the_captured_original() {
        let (Some(capture), Some(iso)) = (
            std::env::var_os("STREET_AI_CAPTURE"),
            std::env::var_os("STREET_ISO"),
        ) else {
            return;
        };
        let samples = captured_referee(&capture);
        let Some(launch) = samples.iter().find(|s| s.ball_state == 7).map(|s| s.frame) else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(iso).unwrap();
        let clips =
            street_render::animation::Clips::load(&mut iso, &[CRADLE, TOSS, RUNOFF, RUN]).unwrap();
        let clip = |part: Part| clips.get(part.clip());
        // From the update before the launch (frame 6 there), placed where
        // the capture has him.
        let before = samples.iter().find(|s| s.frame == launch - 1).unwrap();
        let mut r = Referee::placed();
        r.set(Part::Toss, TOSS_FRAME);
        r.blend = 0;
        r.at = Vec3::from_array(before.position);
        let (mut worst, mut compared, mut moved_at) = (0.0f32, 0, None);
        let mut sticks = Vec::new();
        let mut frame = launch;
        // Until his channel lets go of 948 as the goto ends.
        for s in samples
            .iter()
            .filter(|s| s.frame >= launch)
            .take_while(|s| s.record != -1)
        {
            while frame <= s.frame {
                r.step(OpeningToss::Flight, clip);
                if let Some(word) = r.stick
                    && sticks.last().is_none_or(|&(_, last)| last != word)
                {
                    sticks.push((frame - launch, word));
                }
                frame += 1;
            }
            let error = r.at.distance(Vec3::from_array(s.position));
            worst = worst.max(error);
            compared += 1;
            if moved_at.is_none() && r.part == Part::Run && r.frame >= 1.0 {
                moved_at = Some((s.frame - launch, s.record));
            }
        }
        eprintln!(
            "referee: {compared} updates from the launch, worst {worst:.6}, sticks {sticks:?}"
        );
        assert!(compared > 120, "{compared}");
        // 948's frame 1 eight updates after its script, 16 after the launch.
        assert_eq!(moved_at, Some((24, 948)));
        // 1022 from the launch, then FUN_0013af58's re-aims.
        assert_eq!(sticks, [(0, 1022), (143, 1016), (152, 1009)]);
        assert!(worst < 1e-3, "worst {worst}");
        // The goto is over on the next update: he stays.
        let at = r.at;
        r.step(OpeningToss::Flight, clip);
        assert!(r.arrived && r.stick.is_none());
        assert!(r.at.distance(at) < 0.05, "{}", r.at.distance(at));
    }

    struct Sample {
        frame: i64,
        ball_state: i64,
        position: [f32; 3],
        record: i64,
    }

    fn captured_referee(path: &std::ffi::OsStr) -> Vec<Sample> {
        use std::io::BufRead;
        let file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
        let mut out: Vec<Sample> = Vec::new();
        for line in file.lines() {
            let line = line.unwrap();
            if !line.starts_with("{\"frame\"") {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(&line).unwrap();
            let Some(p) = v["referee"]["position"].as_array() else {
                if !out.is_empty() {
                    break;
                }
                continue;
            };
            if v["torn"].as_bool() == Some(true) {
                continue;
            }
            let frame = v["frame"].as_i64().unwrap();
            if out.last().is_some_and(|s| s.frame >= frame) {
                break;
            }
            out.push(Sample {
                frame,
                ball_state: v["ball_state"].as_i64().unwrap_or(-1),
                position: std::array::from_fn(|i| p[i].as_f64().unwrap() as f32),
                record: v["referee"]["animation"].as_i64().unwrap_or(-1),
            });
            if out.len() > 400 {
                break;
            }
        }
        out
    }
}
