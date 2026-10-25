//! The original's replay manager ("Replay Manager", `FUN_003288b0`, game
//! `+0x420`), its replay director (`FUN_00324668`, game `+0x42c`) and the
//! gamebreaker task's two slowed replays.
//!
//! **The manager.** It keeps the last 300 updates (`FUN_00329248` records
//! every registered object into a 300-slot ring, `FUN_00329098` wraps it):
//! each actor's position, root height, rotation, 22 bone rotations, 21
//! packed vectors and flags (`FUN_00327450`), the ball, the camera
//! manager's replay request and the director's note (`0x324500`).
//! `FUN_00328fc0(speed, manager, frames, delay, mode)` asks for a replay of
//! the last `frames`, now or after `delay` updates (`+0x68`, the span
//! growing by the delay, `+0x6c`). The manager's update (`FUN_00328ab8`)
//! takes the request on: from recording it records one more frame, and it
//! suspends the gameplay task groups (`FUN_0033d380(0x401b2)`); then each
//! update counts `+0x64` down by the speed and moves its position `+0x38`
//! on by it (`FUN_003290d0`), showing the recorded frames on either side of
//! the position, blended by its fraction (`FUN_00329718`; an actor's
//! `FUN_003275d8` lerps its place and height and slerps its rotation and
//! every bone). Once `+0x64` is below one it asks to stop; the update
//! after, it shows the newest frame, resumes the gameplay (`FUN_0033d3a0`)
//! and posts the replay's end (`0x3c6820`, slot `+0x1dc`; the start,
//! `0x3c6810`, slot `+0x1d4`, carries the mode too). A speed change ramps
//! over a number of updates (`FUN_003299c8`, applied at the top of
//! `FUN_00328ab8`).
//!
//! **The instant replay.** The director listens to the game's posts and
//! notes the update counter (`game+0x128`) of each: a shot's release (slot
//! `+0x5c`, `+0x18`), a dunk's or alley-oop's release (slot `+0x64`,
//! `+0x1c`), a takeoff (slot `+0x24`, `+0x20`), a pass's launch (slot
//! `+0x94`, `+0x24`), an alley-oop pass's start (slot `+0x4c`, `+0x28`) and
//! a gamebreaker (slot `+0x204`, `+0x34`). A basket (slot `+0xd4`,
//! `FUN_003248d8`) then picks the replay ([`Director::basket`]):
//!
//! - a won game: always, from the last release, after 144 updates
//!   (`fps * 2.4`), with no prompt;
//! - Manual (`+0xc42f0 == 2`): the prompt is offered (`FUN_00329af0`),
//!   recording stops 54 updates later, and the trick button replays it at
//!   0.8125 (`FUN_00329c68`) until the restart's pass withdraws it (slot
//!   `+0x94` in phase 6, `FUN_00324850`);
//! - Always (0) or Automatic (1): `rand() % 100` under the chance plays
//!   it (`FUN_00324cc8`) 54 updates later (`DAT_004dbfe4`, `fps * 0.9`) at
//!   0.90625 (`0x3f680000`). The chance: a paying combo `min(2 × payout,
//!   35)`; an alley-oop finish (ball kind 3) 30, from its pass; another
//!   dunk 10, from its release; anything else 0; Always 100. A Trick
//!   Point game keeps only the combo's chance, a dunks-only game only
//!   dunks'. None while a side's meter is full, within 540 updates
//!   (`DAT_004dbfe8`, `fps * 9`) of a gamebreaker, in a lesson (game mode
//!   4), while a gamebreaker task runs (`FUN_00140ff8`, also raw
//!   `0x329cac` for the button), a gamebreaker shot is live
//!   (`ball+0x178`) or a script plays. The span reaches 90 updates
//!   (`fps * 1.5`) before the play's first note.
//!
//! While a replay plays the prompt shows again 54 updates in (60 after a
//! press); the cancel button stops it (`FUN_00130b88` → `FUN_00329d38`), and
//! only while the prompt is on offer. When the shown frame is the replay's
//! subject's moment (the release for a jump shot, the takeoff for a dunk,
//! the alley-oop's pass), the replay slows to 0.53125 over 16 updates (8
//! for the pass; the recorded object's slot `+0x1c`, `FUN_00324588`).
//! That speed change posts `0x3c6830` (`FUN_003299c8`, below 0.8125 with no
//! gamebreaker task and no script) to slot `+0x1e4`, which no listener in
//! the ELF overrides (every vtable leaves the base `0x132d48`, `jr ra`):
//! the audio has no slow-motion effect to map.
//!
//! **The gamebreaker task** (`FUN_00144df0` / `FUN_00144ef8`, created by the
//! game listener's slot `+0x204`, `FUN_001422f8`) waits 60 updates
//! (`DAT_004b7d34`), or 40 (`DAT_004b7d38`) when the ball's owner is
//! dunking, then replays the last 90 frames (`DAT_004b7d3c`) at 0.8125
//! (`0x3f500000`) and again at 0.53125 (`0x3f080000`), both in mode 5, and
//! only then posts its end (`0x3c6880`, the camera's slot `+0x20c`); the
//! values are written at `0x145574..0x1455c8`. A lesson shows them as any
//! game does: the School's Gamebreaker capture held the game's count for
//! 1.85 s and 2.81 s of the wall clock while the manager showed its 109
//! and 166 frames.
//!
//! **A staged script's replay** (mode 7). A staged Level 2 gamebreaker is
//! filmed by its own script, whose replay command (category 1, opcode 0)
//! the script manager dispatches at the end of its span, the script's last
//! frame (`FUN_003355c8`; [`crate::Event::StagedRerun`]). Its handler
//! (`FUN_00334b30`), unless the script manager is already replaying or the
//! replay manager is playing or the span reaches 301 frames
//! (`FUN_00334c20`): the command's camera on (word 8), the script manager
//! to its state 5 (`FUN_00336188`: its replay frame `+0x5c` and clock
//! `+0x60` at the span's first frame, its speed `+0x64` the command's
//! percent, word 16, 25..150 else 100), then `FUN_00328fc0(1.0, manager,
//! span, 0, 7)`, whose speed the script gates out (`FUN_003299c8` returns
//! while the script manager's state is 3..7, so no slow-motion post), and
//! `FUN_00329ac8(percent / 100, manager, 0)`, which sets the speed and its
//! target outright. So the last `span` recorded updates play at 0.4 (the
//! dunks' eight) or 0.5 (the lay-ups' and shots'), holding the game like
//! any replay. Each update the manager plays, the script manager first
//! points its cameras at `+0x5c` and moves its clock on by the percent
//! (`FUN_00336868`: `+0x60 += +0x64 / 100`, `+0x5c` its truncation;
//! [`StagedReplay`]). The replay's start and end (mode 7) leave the camera
//! (`FUN_003030c0` and `FUN_00303248` return), the HUD (`FUN_002e2930`,
//! `FUN_002e2990`) and the audio (`FUN_001a5410`, `FUN_001a54d8`: a
//! gamebreaker task runs) alone; at the end the game listener
//! (`FUN_00142648`, mode 7) hands the script back (`FUN_003362a0`: its
//! state 3 again, the command spent) and snaps no restart, and the
//! script's stop ends it in the next update, before the game's own
//! update runs again (the School's Level 2 capture: the script's state 1
//! with the game's counter unchanged, then the counter and the task's
//! end): an update of its own, which holds the gameplay too
//! ([`Replays::holds_world`]) while the manager records it again. The person's cancel press doesn't stop it:
//! `FUN_00130b88` stops a script only when its kind (the script's word 4,
//! `FUN_00337af0`) isn't 1, and every gamebreaker script is kind 1. A
//! freeze whose command's word 12 is 1 (`FUN_00336340`: groups 0x1382)
//! suspends the replay manager and the task too, so its updates stay out of
//! the ring ([`crate::gamebreaker::Hold::holds_replays`]).
//!
//! **Order.** The task list runs by priority (`FUN_0033cf98` inserts after
//! every task of equal or lower priority): "AIGame" 3, "Camera" 10, then
//! "Replay Manager" 14 and "game breaker" 14, the task created later (on
//! the gamebreaker) after the manager. The rewrite runs the director's
//! handlers on the simulation's events, then the manager, then the task,
//! all after the simulation's step. Its own: the camera runs after them
//! (in the original its per-update runs before the manager, its replay
//! start and end handlers inside the manager's update), so that the start
//! and end land in the update the manager posts them.
//!
//! **Here** is what decides and times them, the game's state: the
//! manager's ring holds each recorded update's tick and the director's
//! note ([`Slot`]) and the newest one's recorded objects, and
//! [`update`] runs the three in the original's order around the
//! gameplay's update, which a replay holds. So every machine in an online
//! match holds the same updates (`street-net`'s `Match` steps them: the
//! network's tick runs on, the world's clock stops). What a replay shows
//! is the game's: it records what it draws at each slot's tick
//! (`street-game`'s `tape`). At a replay's end the live world takes back
//! the newest slot's players and ball (the recorded objects
//! `FUN_00329718` writes back).

use std::collections::VecDeque;

use crate::{Action, BallState, Basket, Event, Phase, World, button};

/// The ring's length (`FUN_00329248`'s `299` wrap, `FUN_00329098`'s 300).
pub const FRAMES: usize = 300;
/// The frames a gamebreaker replays (`DAT_004b7d3c`).
pub const GAMEBREAKER_SPAN: usize = 90;
/// The gamebreaker's replay speeds, first and second (`FUN_00144ef8`).
pub const GAMEBREAKER_SPEEDS: [f32; 2] = [f32::from_bits(0x3f50_0000), f32::from_bits(0x3f08_0000)];
/// `FUN_00144ef8`'s wait before the replays: `DAT_004b7d34` (the frame
/// rate), or `DAT_004b7d38` (a third of twice it) when the ball's owner is
/// dunking (`FUN_0016e9d0`).
pub const GAMEBREAKER_DELAY: [u32; 2] = [60, 40];
/// An instant replay's speed (`FUN_00324cc8`, raw `0x324e8c`).
pub const INSTANT_SPEED: f32 = f32::from_bits(0x3f68_0000);
/// The trick button's replay (`FUN_00329c68`).
pub const REPEAT_SPEED: f32 = f32::from_bits(0x3f50_0000);
/// The subject's moment (`FUN_00324588`).
pub const MOMENT_SPEED: f32 = f32::from_bits(0x3f08_0000);
/// `FUN_003299c8` posts the slow motion below this speed.
pub const SLOW_MOTION_BELOW: f32 = 0.8125;
/// An instant replay starts this many updates after its basket:
/// `DAT_004dbfe4`, `fps * 0.9` (written at `0x32527c`).
pub const INSTANT_DELAY: u32 = 54;
/// The won game's replay waits `fps * 2.4` (raw `0x3249a4`).
pub const WON_DELAY: u32 = 144;
/// The span starts this long before the play's first note: `fps * 1.5`
/// (raw `0x324b50`, `0x324e14`).
pub const LEAD_IN: i64 = 90;
/// No instant replay within this long of a gamebreaker: `DAT_004dbfe8`,
/// `fps * 9` (written at `0x3252b8`).
pub const GAMEBREAKER_GAP: u64 = 540;
/// An offer records this much more (`+0xc42f8`), and an automatic replay
/// shows the prompt this far in (`+0xc42fc`): `fps * 0.9` (raw `0x329b8c`).
pub const OFFER_RECORD: u32 = 54;
/// The prompt's hold after a press: `DAT_004dcb14`, `fps` (`0x32a3c8`).
pub const PROMPT_HOLD: u32 = 60;

/// Replay modes (`+0x24`, carried by the start and end posts).
pub mod mode {
    /// A combo's replay.
    pub const COMBO: u32 = 1;
    /// A basket's, from its release or the whole buffer.
    pub const SHOT: u32 = 2;
    /// An alley-oop's, from its pass.
    pub const ALLEY_OOP: u32 = 3;
    /// The gamebreaker task's.
    pub const GAMEBREAKER: u32 = 5;
    /// A staged script's replay command's (`FUN_00334b30`).
    pub const STAGED: u32 = 7;
}

/// The longest span a staged script's replay command plays
/// (`FUN_00334c20`: under 301).
pub const STAGED_SPAN: u32 = 300;

/// The director's recorded note (`0x324500`): the actor it last noted and
/// whether this update was that actor's release, alley-oop pass or
/// takeoff (flag bits 1, 2 and 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Note {
    pub actor: Option<usize>,
    pub release: bool,
    pub oop_pass: bool,
    pub takeoff: bool,
}

/// The manager's state (`+0x28`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
    /// 1: recording every update.
    #[default]
    Recording,
    /// 2: a replay on screen, the gameplay suspended.
    Playing,
    /// 4: an offer's recording, which stops after [`OFFER_RECORD`].
    Frozen,
}

/// A pending change (`+0x30`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    /// 1: stop (and restore, from state 2).
    Stop,
    /// 2: play.
    Play,
    /// 4: an offer's recording.
    Freeze,
}

/// What gates a speed change and its post (`FUN_003299c8`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Gates {
    /// The script manager runs a script (its `+0x50` in 3..7).
    pub scripted: bool,
    /// A gamebreaker task runs (`FUN_00140ff8`).
    pub task_busy: bool,
}

/// One recorded update in the manager's ring: the world's tick, the
/// director's note, and whether a script played (the gate a replay reads
/// off the update it shows).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    pub tick: u64,
    pub note: Note,
    pub scripted: bool,
}

/// The recorded objects of an update: each player's place, heading and
/// height and the ball's place, which a replay's end writes back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Objects {
    pub players: Vec<([f32; 2], f32, f32)>,
    pub ball: [f32; 3],
}

impl Objects {
    /// `world`'s.
    pub fn of(world: &World) -> Self {
        Self {
            players: world
                .players
                .iter()
                .map(|p| (p.position, p.heading, p.height))
                .collect(),
            ball: world.ball.position,
        }
    }
}

/// The replay manager.
#[derive(Clone, Debug, PartialEq)]
pub struct Manager {
    slots: VecDeque<Slot>,
    /// The newest slot's recorded objects.
    newest: Option<Objects>,
    state: State,
    pending: Option<Pending>,
    /// `+0x40`, the speed; `+0x48`, its target; `+0x44`, `+0x4c` and
    /// `+0x50`, the ramp's start, length and step.
    speed: f32,
    target: f32,
    ramp_from: f32,
    ramp: u32,
    ramp_step: u32,
    /// `+0x64`: frames left to step.
    remaining: f32,
    /// `+0x38`: the position among the frames, oldest 0.
    position: f32,
    /// `+0x54`: the position last shown.
    shown_at: Option<f32>,
    /// `+0x24`: the replay's mode.
    pub mode: u32,
    /// `+0x68` and `+0x6c`: a delayed request's updates left and span.
    delay: Option<(u32, usize)>,
    /// `+0xc4300`: a replay is on offer.
    offer: bool,
    /// `+0xc42f4`: the offer's span.
    offer_frames: usize,
    /// `+0xc42f8`: frames an offer still records.
    offer_record: u32,
    /// `+0xc42fc`: updates before the prompt shows again.
    prompt_wait: u32,
    /// The prompt on screen (`FUN_002bafe0(hud, 1, team)`), for the team.
    pub prompt: Option<usize>,
    /// `+0xc42f0`: the Replays option (0 Always, 1 Automatic, 2 Manual).
    option: u8,
    /// This update recorded a slot (the game draws its frame for it).
    pub recorded_now: bool,
    /// This update posted the replay's start (`0x3c6810`).
    pub started: bool,
    /// This update restored the live game and posted the replay's end
    /// (`0x3c6820`, the listeners' slot `+0x1dc`).
    pub ended: bool,
    /// This update posted the slow motion (`0x3c6830`; no listener).
    pub slow_motion: bool,
}

impl Default for Manager {
    fn default() -> Self {
        // FUN_00328948: recording, at speed 1 (FUN_003299c8(1.0)).
        Self {
            slots: VecDeque::new(),
            newest: None,
            state: State::Recording,
            pending: None,
            speed: 1.0,
            target: 1.0,
            ramp_from: 1.0,
            ramp: 0,
            ramp_step: 0,
            remaining: 0.0,
            position: 0.0,
            shown_at: None,
            mode: 0,
            delay: None,
            offer: false,
            offer_frames: 0,
            offer_record: 0,
            prompt_wait: 0,
            prompt: None,
            option: 1,
            recorded_now: false,
            started: false,
            ended: false,
            slow_motion: false,
        }
    }
}

impl Manager {
    /// A replay holds the gameplay (state 2, its stop update included).
    pub fn busy(&self) -> bool {
        self.state == State::Playing
    }

    /// A replay is on screen (state 2).
    pub fn replaying(&self) -> bool {
        self.busy()
    }

    /// A replay is asked for, delayed or on screen: the game's end waits.
    pub fn holding(&self) -> bool {
        self.busy() || self.delay.is_some() || self.pending == Some(Pending::Play)
    }

    /// The current speed (`+0x40`).
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// The frames left to step (`+0x64`).
    pub fn remaining(&self) -> f32 {
        self.remaining
    }

    /// A replay is on offer (`+0xc4300`).
    pub fn offered(&self) -> bool {
        self.offer
    }

    /// The Replays option it reads (`+0xc42f0`).
    pub fn option(&self) -> u8 {
        self.option
    }

    /// `FUN_003299c8(speed, manager, ramp)`.
    pub fn set_speed(&mut self, speed: f32, ramp: u32, gates: Gates) {
        if gates.scripted {
            return;
        }
        if !gates.task_busy && speed < SLOW_MOTION_BELOW {
            self.slow_motion = true;
        }
        self.target = speed;
        if ramp == 0 {
            self.speed = speed;
        } else {
            self.ramp_from = self.speed;
            self.ramp = ramp;
            self.ramp_step = 0;
        }
    }

    /// `FUN_00329ac8(speed, manager, 0)`: the speed and its target set
    /// outright, ungated and posting nothing.
    pub fn set_speed_now(&mut self, speed: f32) {
        self.target = speed;
        self.speed = speed;
    }

    /// A suspended update (its task group held, `FUN_0033d380`): nothing
    /// runs, nothing is recorded or posted.
    fn suspended(&mut self) {
        self.recorded_now = false;
        self.started = false;
        self.ended = false;
        self.slow_motion = false;
    }

    /// `FUN_00328fc0(speed, manager, frames, delay, mode)`.
    pub fn request(&mut self, speed: f32, frames: usize, delay: u32, mode: u32, gates: Gates) {
        self.mode = mode;
        self.set_speed(speed, 0, gates);
        if delay == 0 {
            // At most the buffer less two; the position at the span's
            // first frame, `count - frames + 1` from the oldest.
            let count = self.slots.len();
            let (frames, remaining) = if (count as i64) - 2 < frames as i64 {
                (count.saturating_sub(2), count as f32 - 3.0)
            } else {
                (frames, frames as f32 - 1.0)
            };
            self.pending = Some(Pending::Play);
            self.remaining = remaining;
            self.position = (count + 1).saturating_sub(frames) as f32;
        } else {
            self.delay = Some((delay, frames + delay as usize - 1));
        }
        self.shown_at = None;
    }

    /// `FUN_00329af0(manager, team, frames, automatic)`: a replay on offer.
    /// Manual: the prompt now, and 54 more frames recorded; an automatic
    /// replay shows the prompt 54 updates into its playing.
    pub(crate) fn offer(&mut self, team: usize, frames: usize, automatic: bool) {
        self.offer_frames = frames;
        self.offer_record = OFFER_RECORD;
        self.prompt_wait = 0;
        if automatic {
            self.prompt_wait = OFFER_RECORD;
        } else {
            self.prompt = Some(team);
            self.pending = Some(Pending::Freeze);
        }
        self.offer = true;
    }

    /// `FUN_00329c18`: the prompt hides, the offer ends, a replay stops.
    pub(crate) fn withdraw(&mut self) {
        self.prompt = None;
        self.pending = Some(Pending::Stop);
        self.offer = false;
    }

    /// `FUN_00328a50`: the Replays option, each update; leaving Manual
    /// withdraws its offer.
    pub fn set_option(&mut self, option: u8) {
        let was = std::mem::replace(&mut self.option, option);
        if was == 2 && option != 2 {
            self.withdraw();
        }
    }

    /// `FUN_00329c68(manager, team)`: the trick button (action 0xe) plays
    /// the offer again at 0.8125, once its prompt shows.
    pub fn press_replay(
        &mut self,
        team: usize,
        director: &Director,
        other_human: bool,
        gates: Gates,
    ) {
        if !self.offer || self.prompt_wait != 0 || gates.task_busy {
            return;
        }
        if other_human && team != director.team {
            return;
        }
        self.request(REPEAT_SPEED, self.offer_frames, 0, mode::SHOT, gates);
        self.prompt_wait = PROMPT_HOLD;
        self.prompt = None;
    }

    /// `FUN_00130b88` → `FUN_00329d38`: the cancel button (action 0xf)
    /// stops a replay on offer.
    pub fn press_cancel(&mut self, team: usize, director: &Director, other_human: bool) {
        if self.state != State::Playing || (other_human && team != director.team) {
            return;
        }
        if self.offer {
            self.withdraw();
        }
    }

    /// The slots on screen: the one at the position, the next one and the
    /// fraction between them (`FUN_00329718`; at the newest, only it).
    pub fn shown(&self) -> Option<(Slot, Slot, f32)> {
        if self.state != State::Playing {
            return None;
        }
        let at = self.position.max(0.0);
        let a = (at as usize).min(self.slots.len().checked_sub(1)?);
        let b = (a + 1).min(self.slots.len() - 1);
        let t = if b == a { 0.0 } else { at - a as f32 };
        Some((self.slots[a], self.slots[b], t))
    }

    /// Updates recorded (at most [`FRAMES`]).
    pub fn recorded(&self) -> usize {
        self.slots.len()
    }

    /// The ring, oldest first.
    pub fn slots(&self) -> impl Iterator<Item = &Slot> {
        self.slots.iter()
    }

    fn record(&mut self, slot: Slot, objects: Objects) {
        if self.slots.len() == FRAMES {
            // The ring's slot is the same frame: one nearer the oldest.
            self.slots.pop_front();
            self.position -= 1.0;
            if let Some(s) = &mut self.shown_at {
                *s -= 1.0;
            }
        }
        self.slots.push_back(slot);
        self.newest = Some(objects);
        self.recorded_now = true;
    }

    /// `FUN_00329718`: shows the position when it moved, and runs the
    /// recorded objects' show slots: the director's (`FUN_00324588`)
    /// slows the replay at its subject's moment.
    fn show(&mut self, director: &Director, gates: Gates) {
        if self.shown_at == Some(self.position) || self.slots.is_empty() {
            return;
        }
        self.shown_at = Some(self.position);
        let a = (self.position.max(0.0) as usize).min(self.slots.len() - 1);
        let note = self.slots[a].note;
        if self.state == State::Playing && note.actor.is_some() && note.actor == director.subject {
            let ramp = match director.kind {
                Some(0) if note.release => Some(16),
                Some(1) if note.takeoff => Some(16),
                Some(2) if note.oop_pass => Some(8),
                _ => None,
            };
            if let Some(ramp) = ramp {
                self.set_speed(MOMENT_SPEED, ramp, gates);
            }
        }
    }

    /// One manager update (`FUN_00328ab8`). `slot` and `objects` are what a
    /// record this update holds.
    pub fn update(
        &mut self,
        slot: Slot,
        objects: &dyn Fn() -> Objects,
        director: &Director,
        gates: Gates,
    ) {
        self.started = false;
        self.ended = false;
        self.recorded_now = false;
        // The speed's ramp (raw 0x328ad0..0x328b30).
        if self.speed != self.target {
            self.ramp_step += 1;
            self.speed = if self.ramp_step == self.ramp {
                self.target
            } else {
                ((self.ramp - self.ramp_step) as f32 * self.ramp_from
                    + self.ramp_step as f32 * self.target)
                    / self.ramp as f32
            };
        }
        // A delayed request comes due (+0x68).
        if let Some((left, frames)) = self.delay {
            if left > 1 {
                self.delay = Some((left - 1, frames));
            } else {
                self.delay = None;
                self.request(self.speed, frames, 0, self.mode, gates);
            }
        }
        let mut record = true;
        let mut start = false;
        if let Some(pending) = self.pending.take() {
            match pending {
                // Raw 0x328bc4..0x328c60: the newest frame back on screen,
                // the gameplay resumed, the end posted; no record.
                Pending::Stop if self.state == State::Playing => {
                    self.position = self.slots.len() as f32 - 1.0;
                    self.show(director, gates);
                    record = false;
                    self.ended = true;
                }
                Pending::Stop | Pending::Freeze => {}
                // From recording one more frame (FUN_00329248(m, 0)); the
                // gameplay suspends (FUN_0033d380(0x401b2)).
                Pending::Play => {
                    if self.state == State::Recording {
                        self.record(slot, objects());
                    }
                    start = true;
                }
            }
            self.delay = None;
            self.state = match pending {
                Pending::Stop => State::Recording,
                Pending::Play => State::Playing,
                Pending::Freeze => State::Frozen,
            };
        }
        match self.state {
            State::Recording => {
                if record {
                    self.record(slot, objects());
                }
            }
            State::Playing => {
                if self.remaining < 1.0 {
                    self.pending = Some(Pending::Stop);
                    if self.offer {
                        self.withdraw();
                    }
                    return;
                }
                self.remaining -= self.speed;
                // FUN_003290d0: on by the speed, never past the newest.
                let newest = self.slots.len() as f32 - 1.0;
                self.position = if self.position + self.speed < newest + 1.0 {
                    self.position + self.speed
                } else {
                    newest
                };
                self.show(director, gates);
                if self.offer && self.prompt_wait > 0 {
                    self.prompt_wait -= 1;
                    if self.prompt_wait == 0 {
                        self.prompt = Some(director.team);
                    }
                }
                self.started = start;
            }
            State::Frozen => {
                if self.offer_record > 0 {
                    self.offer_record -= 1;
                    self.record(slot, objects());
                }
            }
        }
    }
}

/// The director's view of the match each update.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    pub gates: Gates,
    /// A School lesson (game mode 4).
    pub lesson: bool,
}

/// The replay director (`FUN_00324668`, vtable `0x49b518`): the game
/// listener that notes the play and picks the instant replay. Built with
/// `+0x10`, `+0x14` and `+0x34` at -1 (`None`), the rest 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Director {
    /// `+0x10`: the replay's kind, which moment slows it (0 the release,
    /// 1 the takeoff, 2 the alley-oop's pass).
    pub kind: Option<u8>,
    /// `+0x14`: the actor last noted.
    pub subject: Option<usize>,
    /// `+0x18`: the last release.
    pub release: u64,
    /// `+0x1c`: the last dunk or alley-oop release.
    pub slam_release: u64,
    /// `+0x20`: the last takeoff.
    pub takeoff: u64,
    /// `+0x24`: the last pass's launch.
    pub pass: u64,
    /// `+0x28`: the last alley-oop pass's start.
    pub oop_pass: u64,
    /// `+0x34`: the last gamebreaker.
    pub gamebreaker: Option<u64>,
    /// `+0x40`: the last basket's team.
    pub team: usize,
    /// The rewrite's own: last update's heights (takeoffs) and ball
    /// (releases).
    pub heights: Vec<f32>,
    pub shot: bool,
}

impl Director {
    /// `0x324500`, the director's recorded object: this update's note.
    pub fn note(&self, now: u64) -> Note {
        Note {
            actor: self.subject,
            release: self.release == now,
            oop_pass: self.oop_pass == now,
            takeoff: self.takeoff == now,
        }
    }

    /// The game's posts of one update, in order.
    pub fn observe(&mut self, w: &World, events: &[Event], tape: &mut Manager, cx: Context) {
        let now = w.tick;
        // Slot +0x24 (the actor VM's jump opcodes): a player leaves the floor.
        for (i, p) in w.players.iter().enumerate() {
            if self.heights.get(i).copied().unwrap_or(0.0) == 0.0 && p.height > 0.0 {
                self.takeoff = now;
                self.subject = Some(i);
            }
        }
        // Slot +0x5c (FUN_0014aaf8): a jump shot or lay-up leaves the hand.
        let shot = matches!(w.ball.state, BallState::Shot);
        if shot
            && !self.shot
            && let Some(s) = w.ball.shot
        {
            self.release = now;
            self.subject = Some(s.shooter);
        }
        for (k, e) in events.iter().enumerate() {
            match *e {
                // Slots +0x64 then +0x5c: a dunk's or alley-oop's release.
                Event::Slam { player, .. } => {
                    self.slam_release = now;
                    self.release = now;
                    self.subject = Some(player);
                }
                // Slot +0x4c (FUN_0014bff8, FUN_003247c0): a pass to an
                // alley-oop's flier (or to himself); slot +0x94
                // (FUN_0014c0c0): any pass's launch.
                Event::Pass { from, to, .. } => {
                    let flier = w.players.get(to).is_some_and(|p| p.leaping());
                    if flier || from == to {
                        self.oop_pass = now;
                        self.subject = Some(to);
                    }
                    self.pass = now;
                }
                // Slot +0x94 in phase 6 (FUN_00324850): the restart's pass
                // withdraws a Manual offer.
                Event::Inbound { .. } => {
                    if tape.option == 2 {
                        tape.withdraw();
                    }
                }
                // Slot +0x204.
                Event::Gamebreaker { .. } => self.gamebreaker = Some(now),
                // Slot +0xd4 (FUN_001515a8's post).
                Event::Score {
                    team,
                    player,
                    points,
                    basket,
                } => {
                    // The simulation's draw for this basket, if it made one.
                    let roll = events[k + 1..]
                        .iter()
                        .take_while(|e| !matches!(e, Event::Score { .. }))
                        .find_map(|e| match *e {
                            Event::ReplayRoll { draw } => Some(draw),
                            _ => None,
                        });
                    self.basket(w, team, player, points, basket, roll, tape, cx)
                }
                _ => {}
            }
        }
        self.heights = w.players.iter().map(|p| p.height).collect();
        self.shot = shot;
    }

    /// `FUN_003248d8`: a basket. `roll` is the shared generator's draw the
    /// simulation made for it ([`Event::ReplayRoll`]).
    #[allow(clippy::too_many_arguments)]
    fn basket(
        &mut self,
        w: &World,
        team: usize,
        player: usize,
        points: u32,
        basket: Basket,
        roll: Option<u32>,
        tape: &mut Manager,
        cx: Context,
    ) {
        self.team = team;
        if points == 0 {
            return;
        }
        // The ball's kind (+0x184): 2 a jump shot, 5 a lay-up, 4 a dunk,
        // 3 an alley-oop's finish, 4 when its move has 0x800.
        let kind = match basket {
            Basket::Layup => 5,
            Basket::Dunk { .. } | Basket::AlleyOop { advanced: true } => 4,
            Basket::AlleyOop { advanced: false } => 3,
            Basket::Jumper { .. } | Basket::Gamebreaker { .. } => 2,
        };
        let slam = self.release == self.slam_release;
        // A gamebreaker shot is live (ball+0x178).
        let gamebreaker_ball = matches!(basket, Basket::Gamebreaker { .. });
        self.kind = Some(match (slam, kind) {
            (false, _) => 0,
            (true, 3) => 2,
            (true, _) => 1,
        });
        self.subject = Some(player);
        let now = w.tick as i64;
        // The won game (the game's slot +0xbc, FUN_0012e800).
        if matches!(w.phase, Phase::Over { .. }) {
            let from = self.release as i64;
            self.play(
                w,
                tape,
                cx,
                from,
                mode::SHOT,
                WON_DELAY,
                gamebreaker_ball,
                true,
            );
            return;
        }
        // The combo (game+0xd0): its payout word (+0x150), whatever side it
        // was made for, and `FUN_00131d18`'s start: the matched row's oldest
        // walked event's tick (+0x14c) while +0x148 holds a row, else -1.
        let combo = &w.combo;
        let payout = (combo.total != 0)
            .then_some((combo.total, if combo.row == -1 { -1 } else { combo.since }));
        let (from, replay_mode, mut chance) = if let Some((total, since)) = payout {
            (i64::from(since), mode::COMBO, (2 * total).min(35))
        } else if slam && kind == 3 {
            (self.pass as i64, mode::ALLEY_OOP, 30)
        } else if slam {
            (self.release as i64, mode::SHOT, 10)
        } else {
            (0, mode::SHOT, 0)
        };
        // The game's rules (config +0xe8): 1, a Trick Point game, keeps
        // only a combo's chance; 2, dunks only, only kinds 3 and 4.
        if matches!(w.rules.goal, crate::Goal::TrickPoints(_)) && payout.is_none() {
            chance = 0;
        }
        if w.rules.dunks_only && !matches!(kind, 3 | 4) {
            chance = 0;
        }
        let meters = w.teams.iter().any(|t| t.meter_full());
        if tape.option == 2 && !gamebreaker_ball && !meters {
            if !(cx.gates.scripted || cx.lesson) {
                let frames = (now - from + LEAD_IN).clamp(0, tape.recorded() as i64) as usize;
                tape.offer(team, frames, false);
            }
            return;
        }
        if tape.option == 0 {
            chance = 100;
        }
        // `rand() % 100 < chance` (FUN_003a16b8, unsigned remainder).
        if roll.is_some_and(|draw| ((draw % 100) as i32) < chance) {
            self.play(
                w,
                tape,
                cx,
                from,
                replay_mode,
                INSTANT_DELAY,
                gamebreaker_ball,
                false,
            );
        }
    }

    /// `FUN_00324cc8(director, from, mode, delay, forced)`.
    #[allow(clippy::too_many_arguments)]
    fn play(
        &mut self,
        w: &World,
        tape: &mut Manager,
        cx: Context,
        from: i64,
        replay_mode: u32,
        delay: u32,
        gamebreaker_ball: bool,
        forced: bool,
    ) {
        let now = w.tick;
        let mut refused = cx.lesson || cx.gates.task_busy || gamebreaker_ball || cx.gates.scripted;
        if !forced {
            refused |= w.teams.iter().any(|t| t.meter_full());
            refused |= self
                .gamebreaker
                .is_some_and(|g| now.saturating_sub(g) < GAMEBREAKER_GAP);
        }
        if refused {
            self.kind = None;
            return;
        }
        let frames = (now as i64 - from + LEAD_IN).clamp(0, tape.recorded() as i64) as usize;
        if !matches!(w.phase, Phase::Over { .. }) {
            tape.offer(self.team, frames, true);
        }
        tape.request(INSTANT_SPEED, frames, delay, replay_mode, cx.gates);
    }
}

/// The gamebreaker task's state (`+0x30`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Case 1: updates left before the first replay.
    Wait(u32),
    /// Case 2: the first replay plays.
    First,
    /// Case 3: the second replay plays.
    Second,
}

/// The gamebreaker task (game `+0x434`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamebreakerTask {
    pub step: Option<Step>,
    /// This update the task posted its end (the camera's slot `+0x20c`).
    pub ended: bool,
}

impl GamebreakerTask {
    /// `FUN_00140ff8`: a task is running (its `+0x2c`).
    pub fn busy(&self) -> bool {
        self.step.is_some()
    }

    /// One task update after the replay manager's: `stepped` when the
    /// game ran this update. The replays hold the game's updates, so its
    /// end lands on the update the simulation's task empties the meter
    /// ([`crate::street::GamebreakerTask`]), the third game update after its
    /// wait, in a lesson too (the School's Gamebreaker capture: 63 game
    /// updates after the press, the replays 1.85 s and 2.81 s of the wall
    /// clock between them).
    pub fn update(
        &mut self,
        w: &World,
        events: &[Event],
        stepped: bool,
        tape: &mut Manager,
        gates: Gates,
    ) {
        self.ended = false;
        for e in events {
            // Slot +0x204 (FUN_001422f8): one task at a time, its kind the
            // message's; a staged Level 2's (kind 0) is the simulation's
            // (`crate::street::GamebreakerTask`): it asks for no replays,
            // its script filming it. Its script starts two updates after
            // the press ([`World::staged_start`]).
            if let Event::Gamebreaker { level, .. } = *e
                && !(level == 2 && (w.sequence.is_some() || w.staged_start.is_some()))
                && self.step.is_none()
            {
                let dunking = match w.ball.state {
                    BallState::Held(o) => w
                        .players
                        .get(o)
                        .is_some_and(|p| matches!(p.action, Action::Dunking { .. })),
                    _ => false,
                };
                // Case 0 sets the wait; case 1 counts from the next update.
                self.step = Some(Step::Wait(GAMEBREAKER_DELAY[usize::from(dunking)]));
                return;
            }
        }
        // Cases 2 and 3 wait while the manager's +0x28 is 2 or 3.
        let gates = Gates {
            task_busy: true,
            ..gates
        };
        match self.step {
            Some(Step::Wait(left)) if stepped => {
                if left > 1 {
                    self.step = Some(Step::Wait(left - 1));
                } else {
                    tape.request(
                        GAMEBREAKER_SPEEDS[0],
                        GAMEBREAKER_SPAN,
                        0,
                        mode::GAMEBREAKER,
                        gates,
                    );
                    self.step = Some(Step::First);
                }
            }
            Some(Step::First) if !tape.busy() => {
                tape.request(
                    GAMEBREAKER_SPEEDS[1],
                    GAMEBREAKER_SPAN,
                    0,
                    mode::GAMEBREAKER,
                    gates,
                );
                self.step = Some(Step::Second);
            }
            Some(Step::Second) if !tape.busy() => {
                self.step = None;
                self.ended = true;
            }
            _ => {}
        }
    }
}

/// What a fresh press of a person's pad asks of the replay manager: the
/// trick button replays an offer (`AIACTION_START_REPLAY_PRESS`, 0xe), the
/// pass button cancels one (`AIACTION_CANCEL_REPLAY_PRESS`, 0xf), as
/// layout A names them (`FUN_0012f2c8`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    /// The pad's side.
    pub team: usize,
    /// The buttons pressed this update (not held from the last).
    pub buttons: u16,
    /// A person plays the other side too.
    pub other_human: bool,
}

/// The script manager replaying a staged script (its state 5,
/// `FUN_00336188`) while the replay manager plays the span (mode 7): what
/// its cameras show. The game draws the recorded updates; the camera is the
/// script's own `camera` at its track's frame `shown`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StagedReplay {
    pub script: usize,
    /// Played for the side attacking +x, its x mirrored.
    pub mirrored: bool,
    /// The command's camera (its word 8, 0..9).
    pub camera: u8,
    /// `+0x64`: the command's speed in percent.
    pub percent: u8,
    /// The clock the cameras were set at this update (`+0x60` as
    /// `FUN_00336868` read it, or the span's first frame at the command):
    /// their records and keys the truncation's, the keys' values at the
    /// clock itself (`FUN_00336d18`).
    pub shown: f32,
    /// `+0x5c`, the truncation of `+0x60`, the clock.
    pub frame: u32,
    pub clock: f32,
    /// The replay is over (`FUN_003362a0`): the script manager ends its
    /// script in the next update.
    pub ending: bool,
}

impl StagedReplay {
    /// `FUN_00336188`: at the command, the cameras at the span's first
    /// frame, the clock there.
    pub fn new(script: usize, mirrored: bool, from: u32, percent: u8, camera: u8) -> Self {
        Self {
            script,
            mirrored,
            camera,
            percent,
            shown: from as f32,
            frame: from,
            clock: from as f32,
            ending: false,
        }
    }

    /// `FUN_00336868`, each update the manager is playing (`+0x28` 2 or
    /// 3): the cameras at `+0x5c`, then the clock on by the percent.
    pub fn step(&mut self) {
        self.shown = self.clock;
        self.clock = crate::math::ee_add(self.clock, f32::from(self.percent) / 100.0);
        // cvt.w.s: toward zero.
        self.frame = self.clock as u32;
    }
}

/// The replay manager, director and gamebreaker task of one match, and the
/// script manager's replay of a staged script.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Replays {
    pub manager: Manager,
    pub director: Director,
    pub task: GamebreakerTask,
    pub staged: Option<StagedReplay>,
}

impl Replays {
    /// The gameplay holds this update (its caller doesn't step the world):
    /// a replay plays, or the script manager ends a staged script after
    /// its replay ([`StagedReplay::ending`]).
    pub fn holds_world(&self) -> bool {
        self.manager.busy() || self.staged.is_some()
    }

    /// [`update`] on these.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        world: &mut World,
        events: &mut Vec<Event>,
        stepped: bool,
        presses: &[Press],
        option: u8,
        lesson: bool,
    ) {
        update(
            &mut self.manager,
            &mut self.director,
            &mut self.task,
            &mut self.staged,
            world,
            events,
            stepped,
            presses,
            option,
            lesson,
        );
    }
}

/// The live world's staged script is frozen on a command that holds the
/// replay manager too (`FUN_00336340`'s 0x1382): its freeze's updates,
/// counted in the sequence's `held`, until the update its actors resume.
fn replays_held(world: &World) -> bool {
    let Some(s) = world.sequence else {
        return false;
    };
    s.held > 0
        && !s.resume_pending
        && world
            .staged
            .as_ref()
            .and_then(|c| c.plan(s.script))
            .and_then(|p| p.controls.hold)
            .is_some_and(|h| h.holds_replays && h.frame == s.frame)
}

/// One update of the replay director, manager and gamebreaker task, after
/// the gameplay's (see the module notes for the order): the Replays option
/// and the buttons first (the original's "action tap" task runs before the
/// game), then the update's posts to the director (`stepped`: the gameplay
/// ran; `events` its events), the script manager's part (its replay's
/// cameras, a staged script's replay command), the manager, the replay's
/// end on the live `world` (its recorded objects back, a pending restart
/// snapped to its spots, whose events replace `events`), and the task.
/// While a replay plays ([`Manager::busy`]) the gameplay holds: the caller
/// doesn't step it.
#[allow(clippy::too_many_arguments)]
pub fn update(
    manager: &mut Manager,
    director: &mut Director,
    task: &mut GamebreakerTask,
    staged: &mut Option<StagedReplay>,
    world: &mut World,
    events: &mut Vec<Event>,
    stepped: bool,
    presses: &[Press],
    option: u8,
    lesson: bool,
) {
    // A script plays: the script manager replaying one, the live update's,
    // or while a replay plays the update it shows.
    let scripted = staged.is_some()
        || match manager.shown() {
            Some((a, _, _)) => a.scripted,
            None => world.sequence.is_some(),
        };
    // A gamebreaker task runs (`FUN_00140ff8`): the shot's here, a staged
    // script's in the simulation.
    let gates = Gates {
        scripted,
        task_busy: task.busy() || world.gamebreaker_task.is_some(),
    };
    let cx = Context { gates, lesson };
    manager.set_option(option);
    for p in presses {
        if p.buttons & button::TRICK != 0 {
            manager.press_replay(p.team, director, p.other_human, gates);
        }
        if p.buttons & button::PASS != 0 && !gates.scripted {
            manager.press_cancel(p.team, director, p.other_human);
        }
    }
    if stepped {
        director.observe(world, events, manager, cx);
    }
    // The script manager (priority 4, before the replay manager's 14): its
    // replay's cameras while the manager plays (`FUN_00336868`), or a
    // staged script's replay command this update (`FUN_00334b30`).
    match staged {
        // FUN_003362a0's state 3, then the stop at the same frame.
        Some(s) if s.ending => *staged = None,
        Some(s) if manager.busy() => s.step(),
        Some(_) => {}
        None => {
            let command = events.iter().find_map(|e| match *e {
                Event::StagedRerun {
                    script,
                    mirrored,
                    from,
                    frames,
                    speed,
                    camera,
                } => Some((script, mirrored, from, frames, speed, camera)),
                _ => None,
            });
            if let Some((script, mirrored, from, frames, speed, camera)) = command
                && !manager.busy()
                && frames <= STAGED_SPAN
            {
                let percent = if (25..=150).contains(&speed) {
                    speed
                } else {
                    100
                };
                *staged = Some(StagedReplay::new(
                    script,
                    mirrored,
                    from,
                    percent,
                    camera.min(9),
                ));
                // The script's state 5 gates the request's own speed out.
                let held = Gates {
                    scripted: true,
                    ..gates
                };
                manager.request(1.0, frames as usize, 0, mode::STAGED, held);
                manager.set_speed_now(f32::from(percent) / 100.0);
            }
        }
    }
    // A freeze holding the replay manager and the task (0x1382).
    if replays_held(world) {
        manager.suspended();
        task.ended = false;
        return;
    }
    let now = world.tick;
    let slot = Slot {
        tick: now,
        note: director.note(now),
        scripted: world.sequence.is_some(),
    };
    let live = &*world;
    manager.update(slot, &|| Objects::of(live), director, gates);
    if manager.ended {
        // The replay's end (slot +0x1dc): the newest recorded objects back
        // (`FUN_00329718`), the director's kind clears (0x324cb8), and the
        // game listener (FUN_00142648) snaps a pending restart to its spots
        // (FUN_001407c8) for modes but 6 and 7 when no gamebreaker task runs.
        if let Some(newest) = &manager.newest {
            restore(world, newest);
        }
        director.kind = None;
        if let Phase::DeadBall { team, .. } = world.phase
            && !matches!(manager.mode, 6 | 7)
            && !task.busy()
        {
            world.events.clear();
            world.snap_formation(team);
            *events = world.events.clone();
        }
        // Mode 7 (FUN_00142648): the script manager back to its script
        // (`FUN_003362a0`), whose stop ends it the next update.
        if manager.mode == mode::STAGED
            && let Some(s) = staged
        {
            s.ending = true;
        }
    }
    task.update(world, events, stepped, manager, gates);
}

/// The live world back with the newest recorded objects: each player's
/// place, height and heading, and the ball's place (`FUN_00329718` on the
/// newest frame; the rest of the game was held).
fn restore(world: &mut World, newest: &Objects) {
    for (p, &(position, heading, height)) in world.players.iter_mut().zip(&newest.players) {
        p.position = position;
        p.heading = heading;
        p.height = height;
    }
    world.ball.position = newest.ball;
}

/// Bytes of [`Replays`] in a fixed order, for the match's snapshots and
/// checksums (`street-net`'s `Match`): little-endian words, floats by their
/// bits.
struct Out(Vec<u8>);

impl Out {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    fn u32(&mut self, v: u32) {
        self.0.extend(v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend(v.to_le_bytes());
    }
    fn f(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    fn index(&mut self, v: Option<usize>) {
        self.u8(v.map_or(u8::MAX, |v| v.min(254) as u8));
    }
}

struct In<'a> {
    b: &'a [u8],
}

impl In<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.b.len() < n {
            return Err("replays: truncated".into());
        }
        let (a, rest) = self.b.split_at(n);
        self.b = rest;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("replays: bad flag".into()),
        }
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8")))
    }
    /// A finite float within a bound the manager's values keep.
    fn f(&mut self) -> Result<f32, String> {
        let v = f32::from_bits(self.u32()?);
        if v.is_finite() && v.abs() <= 1.0e6 {
            Ok(v)
        } else {
            Err("replays: bad number".into())
        }
    }
    fn finite(&mut self) -> Result<f32, String> {
        let v = f32::from_bits(self.u32()?);
        if v.is_finite() {
            Ok(v)
        } else {
            Err("replays: bad number".into())
        }
    }
    fn index(&mut self) -> Result<Option<usize>, String> {
        Ok(match self.u8()? {
            u8::MAX => None,
            v => Some(usize::from(v)),
        })
    }
}

impl Replays {
    /// The three as bytes ([`Replays::decode`] reads them back).
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Out(Vec::with_capacity(64 + 10 * self.manager.slots.len()));
        let m = &self.manager;
        w.u32(m.slots.len() as u32);
        for s in &m.slots {
            w.u64(s.tick);
            w.index(s.note.actor);
            w.u8(u8::from(s.note.release)
                | u8::from(s.note.oop_pass) << 1
                | u8::from(s.note.takeoff) << 2);
            w.bool(s.scripted);
        }
        match &m.newest {
            None => w.u8(0),
            Some(o) => {
                w.u8(1);
                w.u8(o.players.len() as u8);
                for &(p, heading, height) in &o.players {
                    w.f(p[0]);
                    w.f(p[1]);
                    w.f(heading);
                    w.f(height);
                }
                o.ball.iter().for_each(|&v| w.f(v));
            }
        }
        w.u8(match m.state {
            State::Recording => 1,
            State::Playing => 2,
            State::Frozen => 4,
        });
        w.u8(match m.pending {
            None => 0,
            Some(Pending::Stop) => 1,
            Some(Pending::Play) => 2,
            Some(Pending::Freeze) => 4,
        });
        for v in [m.speed, m.target, m.ramp_from] {
            w.f(v);
        }
        w.u32(m.ramp);
        w.u32(m.ramp_step);
        w.f(m.remaining);
        w.f(m.position);
        match m.shown_at {
            None => w.u8(0),
            Some(v) => {
                w.u8(1);
                w.f(v);
            }
        }
        w.u32(m.mode);
        match m.delay {
            None => w.u8(0),
            Some((left, frames)) => {
                w.u8(1);
                w.u32(left);
                w.u32(frames as u32);
            }
        }
        w.bool(m.offer);
        w.u32(m.offer_frames as u32);
        w.u32(m.offer_record);
        w.u32(m.prompt_wait);
        w.index(m.prompt);
        w.u8(m.option);
        for v in [m.recorded_now, m.started, m.ended, m.slow_motion] {
            w.bool(v);
        }
        let d = &self.director;
        w.u8(d.kind.unwrap_or(u8::MAX));
        w.index(d.subject);
        for v in [d.release, d.slam_release, d.takeoff, d.pass, d.oop_pass] {
            w.u64(v);
        }
        match d.gamebreaker {
            None => w.u8(0),
            Some(t) => {
                w.u8(1);
                w.u64(t);
            }
        }
        w.u8(d.team as u8);
        w.u8(d.heights.len() as u8);
        d.heights.iter().for_each(|&h| w.f(h));
        w.bool(d.shot);
        match self.task.step {
            None => w.u8(0),
            Some(Step::Wait(left)) => {
                w.u8(1);
                w.u32(left);
            }
            Some(Step::First) => w.u8(2),
            Some(Step::Second) => w.u8(3),
        }
        w.bool(self.task.ended);
        match &self.staged {
            None => w.u8(0),
            Some(s) => {
                w.u8(1);
                w.u32(s.script as u32);
                w.bool(s.mirrored);
                w.u8(s.camera);
                w.u8(s.percent);
                w.f(s.shown);
                w.u32(s.frame);
                w.f(s.clock);
                w.bool(s.ending);
            }
        }
        w.0
    }

    /// The three from [`Replays::encode`]'s bytes, checked to be ones the
    /// manager can hold (an untrusted snapshot's).
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut r = In { b: bytes };
        let bad = |what: &str| Err(format!("replays: bad {what}"));
        let n = r.u32()? as usize;
        if n > FRAMES {
            return bad("ring");
        }
        let mut slots = VecDeque::with_capacity(n);
        for _ in 0..n {
            let tick = r.u64()?;
            let actor = r.index()?;
            let flags = r.u8()?;
            if flags > 7 {
                return bad("note");
            }
            slots.push_back(Slot {
                tick,
                note: Note {
                    actor,
                    release: flags & 1 != 0,
                    oop_pass: flags & 2 != 0,
                    takeoff: flags & 4 != 0,
                },
                scripted: r.bool()?,
            });
        }
        let newest = match r.u8()? {
            0 => None,
            1 => {
                let n = usize::from(r.u8()?);
                let mut players = Vec::with_capacity(n);
                for _ in 0..n {
                    players.push(([r.f()?, r.f()?], r.f()?, r.f()?));
                }
                Some(Objects {
                    players,
                    ball: [r.f()?, r.f()?, r.f()?],
                })
            }
            _ => return bad("objects"),
        };
        let state = match r.u8()? {
            1 => State::Recording,
            2 => State::Playing,
            4 => State::Frozen,
            _ => return bad("state"),
        };
        let pending = match r.u8()? {
            0 => None,
            1 => Some(Pending::Stop),
            2 => Some(Pending::Play),
            4 => Some(Pending::Freeze),
            _ => return bad("pending"),
        };
        let (speed, target, ramp_from) = (r.f()?, r.f()?, r.f()?);
        let (ramp, ramp_step) = (r.u32()?, r.u32()?);
        // The position moves one nearer the oldest at each wrap of the
        // ring, recording or not: any finite value.
        let (remaining, position) = (r.f()?, r.finite()?);
        let shown_at = match r.u8()? {
            0 => None,
            1 => Some(r.finite()?),
            _ => return bad("shown"),
        };
        let mode = r.u32()?;
        let delay = match r.u8()? {
            0 => None,
            1 => Some((r.u32()?, r.u32()? as usize)),
            _ => return bad("delay"),
        };
        let offer = r.bool()?;
        let offer_frames = r.u32()? as usize;
        let offer_record = r.u32()?;
        let prompt_wait = r.u32()?;
        let prompt = r.index()?;
        let option = r.u8()?;
        let (recorded_now, started, ended, slow_motion) =
            (r.bool()?, r.bool()?, r.bool()?, r.bool()?);
        if ramp > 1024
            || ramp_step > ramp
            || offer_frames > FRAMES
            || offer_record > OFFER_RECORD
            || prompt_wait > PROMPT_HOLD.max(OFFER_RECORD)
            || prompt.is_some_and(|t| t > 1)
            || option > 2
            || delay.is_some_and(|(left, frames)| left > 1024 || frames > 4096)
            || !(0.0..=4.0).contains(&speed)
            || !(0.0..=4.0).contains(&target)
            || !(0.0..=4.0).contains(&ramp_from)
        {
            return bad("manager");
        }
        let kind = match r.u8()? {
            u8::MAX => None,
            k if k <= 2 => Some(k),
            _ => return bad("director kind"),
        };
        let subject = r.index()?;
        let [release, slam_release, takeoff, pass, oop_pass] =
            [r.u64()?, r.u64()?, r.u64()?, r.u64()?, r.u64()?];
        let gamebreaker = match r.u8()? {
            0 => None,
            1 => Some(r.u64()?),
            _ => return bad("director gamebreaker"),
        };
        let team = usize::from(r.u8()?);
        let n = usize::from(r.u8()?);
        let heights = (0..n).map(|_| r.f()).collect::<Result<Vec<_>, _>>()?;
        let shot = r.bool()?;
        if team > 1 {
            return bad("director team");
        }
        let step = match r.u8()? {
            0 => None,
            1 => match r.u32()? {
                left @ 0..=60 => Some(Step::Wait(left)),
                _ => return bad("task wait"),
            },
            2 => Some(Step::First),
            3 => Some(Step::Second),
            _ => return bad("task"),
        };
        let task = GamebreakerTask {
            step,
            ended: r.bool()?,
        };
        let staged = match r.u8()? {
            0 => None,
            1 => {
                let s = StagedReplay {
                    script: r.u32()? as usize,
                    mirrored: r.bool()?,
                    camera: r.u8()?,
                    percent: r.u8()?,
                    shown: r.f()?,
                    frame: r.u32()?,
                    clock: r.f()?,
                    ending: r.bool()?,
                };
                if s.script > 255
                    || s.camera > 9
                    || !(25..=150).contains(&s.percent)
                    || s.clock < 0.0
                    || !(0.0..=s.clock).contains(&s.shown)
                {
                    return bad("staged replay");
                }
                Some(s)
            }
            _ => return bad("staged replay"),
        };
        if !r.b.is_empty() {
            return bad("length");
        }
        Ok(Self {
            manager: Manager {
                slots,
                newest,
                state,
                pending,
                speed,
                target,
                ramp_from,
                ramp,
                ramp_step,
                remaining,
                position,
                shown_at,
                mode,
                delay,
                offer,
                offer_frames,
                offer_record,
                prompt_wait,
                prompt,
                option,
                recorded_now,
                started,
                ended,
                slow_motion,
            },
            director: Director {
                kind,
                subject,
                release,
                slam_release,
                takeoff,
                pass,
                oop_pass,
                gamebreaker,
                team,
                heights,
                shot,
            },
            task,
            staged,
        })
    }

    /// The players any of the three name are the match's `players`.
    pub fn validate(&self, players: usize) -> Result<(), String> {
        let named = self
            .manager
            .slots
            .iter()
            .filter_map(|s| s.note.actor)
            .chain(self.director.subject)
            .chain(
                self.manager
                    .newest
                    .as_ref()
                    .map(|o| o.players.len().saturating_sub(1)),
            )
            .chain((!self.director.heights.is_empty()).then(|| self.director.heights.len() - 1));
        for k in named {
            if k >= players {
                return Err(format!("replays: player {k} of {players}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(tick: u64) -> Slot {
        Slot {
            tick,
            ..Slot::default()
        }
    }

    fn objects() -> Objects {
        Objects::default()
    }

    /// An update that must record nothing.
    fn none() -> Objects {
        unreachable!("nothing recorded")
    }

    fn world() -> World {
        World::new(
            [crate::Hoop {
                rim: [-37.0, 11.75, 0.0],
                rim_radius: 0.75,
            }; 2],
            Vec::new(),
            crate::Rules::default(),
            1,
        )
    }

    fn shows(tape: &mut Manager, director: &Director, slot: Slot) -> u64 {
        tape.update(slot, &objects, director, Gates::default());
        tape.shown().map_or(u64::MAX, |(a, _, _)| a.tick)
    }

    #[test]
    fn a_replay_steps_the_last_frames_at_its_speed_like_fun_00328ab8() {
        let mut tape = Manager::default();
        let d = Director::default();
        for t in 0..400 {
            assert_eq!(shows(&mut tape, &d, frame(t)), u64::MAX);
        }
        assert_eq!(tape.recorded(), FRAMES);
        tape.request(
            GAMEBREAKER_SPEEDS[0],
            GAMEBREAKER_SPAN,
            0,
            mode::GAMEBREAKER,
            Gates::default(),
        );
        // The update it starts records one more (tick 400) and steps once.
        let first = shows(&mut tape, &d, frame(400));
        assert!(tape.started && tape.busy());
        // The span's first frame is 89 before the newest; the first step is
        // 0.8125 past it.
        assert_eq!(tape.remaining, 89.0 - 0.8125);
        assert_eq!(first, 400 - 89);
        let (a, b, t) = tape.shown().unwrap();
        assert_eq!((a.tick, b.tick, t), (311, 312, 0.8125));
        let mut steps = 1;
        let mut ticks = vec![first];
        loop {
            let at = {
                tape.update(frame(u64::MAX), &none, &d, Gates::default());
                tape.shown().map_or(u64::MAX, |(a, _, _)| a.tick)
            };
            if tape.pending.is_some() {
                break;
            }
            ticks.push(at);
            steps += 1;
        }
        // floor(88 / 0.8125) + 1 = 109 frames shown, then one update asking
        // to stop, then the newest frame back.
        assert_eq!(steps, 109);
        assert!(tape.busy() && !tape.ended);
        assert!(ticks.windows(2).all(|w| w[1] >= w[0]));
        tape.update(frame(u64::MAX), &none, &d, Gates::default());
        assert!(tape.ended && !tape.busy());
        assert_eq!(tape.slots().last().unwrap().tick, 400);
        // Recording again.
        shows(&mut tape, &d, frame(401));
        assert!(!tape.ended && tape.recorded() == FRAMES);
        // The second, slower one: floor(88 / 0.53125) + 1 = 166 frames.
        tape.request(
            GAMEBREAKER_SPEEDS[1],
            GAMEBREAKER_SPAN,
            0,
            mode::GAMEBREAKER,
            Gates::default(),
        );
        let mut shown = 0;
        loop {
            shows(&mut tape, &d, frame(402));
            if tape.pending.is_some() || !tape.busy() {
                break;
            }
            shown += 1;
        }
        assert_eq!(shown, 166);
    }

    #[test]
    fn a_delayed_request_waits_and_grows_its_span_like_fun_00328fc0() {
        let mut tape = Manager::default();
        let d = Director::default();
        let mut t = 0;
        for _ in 0..200 {
            t += 1;
            shows(&mut tape, &d, frame(t));
        }
        // At 0.90625, 100 frames, 54 updates later: the span grows by 53.
        tape.request(
            INSTANT_SPEED,
            100,
            INSTANT_DELAY,
            mode::SHOT,
            Gates::default(),
        );
        for _ in 0..INSTANT_DELAY - 1 {
            t += 1;
            shows(&mut tape, &d, frame(t));
            assert!(!tape.busy());
        }
        t += 1;
        let first = shows(&mut tape, &d, frame(t));
        assert!(tape.busy() && tape.started);
        // The newest frame is the update it started; the span 152 back.
        assert_eq!(t, 254);
        assert_eq!(tape.remaining, 152.0 - INSTANT_SPEED);
        assert_eq!(first, 254 - 152);
        // A speed change ramps: 0.53125 over 16 updates, the post with it.
        tape.set_speed(MOMENT_SPEED, 16, Gates::default());
        assert!(tape.slow_motion);
        tape.update(frame(u64::MAX), &none, &d, Gates::default());
        let step = (INSTANT_SPEED - MOMENT_SPEED) / 16.0;
        assert!((tape.speed() - (INSTANT_SPEED - step)).abs() < 1e-6);
        for _ in 0..15 {
            tape.update(frame(u64::MAX), &none, &d, Gates::default());
        }
        assert_eq!(tape.speed(), MOMENT_SPEED);
        // Not under a gamebreaker task; not at all under a script.
        tape.slow_motion = false;
        tape.set_speed(
            0.5,
            0,
            Gates {
                task_busy: true,
                scripted: false,
            },
        );
        assert!(!tape.slow_motion && tape.speed() == 0.5);
        tape.set_speed(
            0.25,
            0,
            Gates {
                task_busy: false,
                scripted: true,
            },
        );
        assert_eq!(tape.speed(), 0.5);
    }

    #[test]
    fn the_gamebreaker_task_waits_then_plays_both_replays_before_its_end() {
        let mut tape = Manager::default();
        let d = Director::default();
        let mut task = GamebreakerTask::default();
        let w = world();
        for t in 0..200 {
            shows(&mut tape, &d, frame(t));
        }
        let go = [Event::Gamebreaker {
            player: 0,
            level: 1,
        }];
        let g = Gates::default();
        task.update(&w, &go, true, &mut tape, g);
        assert!(task.busy());
        let mut updates = 0;
        let mut replay_updates = 0;
        let mut ends = 0;
        let mut t = 200;
        while task.busy() {
            let stepped = !tape.busy();
            t += 1;
            tape.update(frame(t), &objects, &d, g);
            if tape.busy() || tape.ended {
                replay_updates += 1;
            }
            ends += usize::from(tape.ended);
            task.update(&w, &[], stepped, &mut tape, g);
            updates += 1;
            assert!(updates < 1000);
        }
        assert!(task.ended);
        assert_eq!(ends, 2);
        // 60 updates' wait, then the two replays: 109 + 166 frames, an
        // update each asking to stop and one restoring, and the live update
        // the second one starts after.
        assert_eq!(replay_updates, (109 + 2) + (166 + 2));
        assert_eq!(updates, 60 + 1 + (109 + 2) + (166 + 2) - 1);
        // A lesson the same (the School's capture: its end 63 game updates
        // after the press, the task's first update the one after it, the
        // two replays 1.85 s and 2.81 s of the wall clock between).
    }

    #[test]
    fn the_subjects_moment_slows_the_replay_like_fun_00324588() {
        let mut tape = Manager::default();
        let mut d = Director::default();
        for t in 0..120 {
            let mut f = frame(t);
            // The subject (2) released at tick 60.
            f.note = Note {
                actor: Some(2),
                release: t == 60,
                ..Note::default()
            };
            tape.update(f, &objects, &d, Gates::default());
        }
        d.kind = Some(0);
        d.subject = Some(2);
        tape.request(INSTANT_SPEED, 100, 0, mode::SHOT, Gates::default());
        let mut at = 0;
        while tape.target == INSTANT_SPEED {
            tape.update(frame(999), &objects, &d, Gates::default());
            at = tape.shown().unwrap().0.tick;
            assert!(tape.busy());
        }
        // The frame of the release, then a ramp down to 0.53125.
        assert_eq!(at, 60);
        assert!(tape.slow_motion);
        let mut was = tape.speed();
        for _ in 0..40 {
            tape.update(frame(999), &objects, &d, Gates::default());
            assert!(tape.speed() <= was);
            was = tape.speed();
        }
        assert_eq!(tape.speed(), MOMENT_SPEED);
        // A takeoff only slows a dunk's (kind 1), a pass an alley-oop's.
        let other = Director {
            kind: Some(1),
            subject: Some(2),
            ..Director::default()
        };
        let mut tape2 = Manager::default();
        for t in 0..50 {
            let mut f = frame(t);
            f.note = Note {
                actor: Some(2),
                release: true,
                ..Note::default()
            };
            tape2.update(f, &objects, &other, Gates::default());
        }
        tape2.request(INSTANT_SPEED, 40, 0, mode::SHOT, Gates::default());
        for _ in 0..30 {
            tape2.update(frame(999), &objects, &other, Gates::default());
        }
        assert_eq!(tape2.speed(), INSTANT_SPEED);
    }

    #[test]
    fn a_staged_scripts_replay_command_replays_its_span_like_fun_00334b30() {
        let mut r = Replays::default();
        let mut w = world();
        let mut events = Vec::new();
        for t in 0..320 {
            w.tick = t;
            r.update(&mut w, &mut events, true, &[], 1, false);
        }
        assert_eq!(r.manager.recorded(), FRAMES);
        // The script's last frame: GamDN1b's command, 40..192 at 40% on
        // camera 2.
        w.tick = 320;
        events = vec![Event::StagedRerun {
            script: 1,
            mirrored: true,
            from: 40,
            frames: 152,
            speed: 40,
            camera: 2,
        }];
        r.update(&mut w, &mut events, true, &[], 1, false);
        let m = &r.manager;
        assert!(m.busy() && m.started && m.mode == mode::STAGED);
        // FUN_00329ac8's speed, and no slow-motion post (the script gates
        // FUN_003299c8 out).
        assert_eq!(m.speed(), 0.4);
        assert!(!m.slow_motion);
        // The span's first frame, 151 before the newest, on by 0.4.
        let (a, _, t) = m.shown().unwrap();
        assert_eq!(a.tick, 320 - 151);
        assert!((t - 0.4).abs() < 1e-4);
        assert_eq!(
            r.staged,
            Some(StagedReplay {
                script: 1,
                mirrored: true,
                camera: 2,
                percent: 40,
                shown: 40.0,
                frame: 40,
                clock: 40.0,
                ending: false,
            })
        );
        // Each update the cameras follow the script's clock; neither press
        // stops it.
        let press = Press {
            team: 0,
            buttons: button::PASS | button::TRICK,
            other_human: false,
        };
        let mut updates = 1;
        let mut cameras = vec![40.0];
        while !r.manager.ended {
            r.update(&mut w, &mut Vec::new(), false, &[press], 1, false);
            if let Some(s) = r.staged {
                cameras.push(s.shown);
            }
            updates += 1;
            assert!(updates < 1000);
        }
        // 376 frames shown (151 at 0.4), an update asking to stop and the
        // end; the cameras from 40 on at 0.4 a frame.
        assert_eq!(updates, 376 + 2);
        // The clock's add chops (`add.s`).
        let on = |clock: f32| crate::math::ee_add(clock, 0.4);
        assert_eq!(cameras[..4], [40.0, 40.0, on(40.0), on(on(40.0))]);
        // The stop's update too: the cameras in frame 190 as it shows the
        // newest.
        assert_eq!(*cameras.last().unwrap() as u32, 190);
        // Mode 7's end hands the script back, whose stop ends it the next
        // update: an update of its own, the gameplay still held, the
        // manager recording again.
        assert!(r.staged.is_some_and(|s| s.ending) && !r.manager.busy());
        assert!(r.holds_world());
        let recorded = r.manager.recorded();
        r.update(&mut w, &mut Vec::new(), false, &[], 1, false);
        assert!(r.staged.is_none() && !r.holds_world());
        assert_eq!(r.manager.slots().last().unwrap().tick, 320);
        assert_eq!(r.manager.recorded(), recorded);
    }

    #[test]
    fn the_three_go_through_their_bytes_whole() {
        let mut r = Replays::default();
        let d = Director::default();
        for t in 0..320 {
            let mut f = frame(t);
            f.note.actor = Some((t % 6) as usize);
            f.note.release = t % 7 == 0;
            f.scripted = t % 11 == 0;
            r.manager.update(f, &objects, &d, Gates::default());
        }
        r.manager.request(
            INSTANT_SPEED,
            100,
            INSTANT_DELAY,
            mode::SHOT,
            Gates::default(),
        );
        r.manager.offer(1, 80, true);
        r.director.kind = Some(1);
        r.director.subject = Some(4);
        r.director.gamebreaker = Some(77);
        r.director.heights = vec![0.0, 1.5, 0.0, 0.0, 2.0, 0.0];
        r.task.step = Some(Step::Wait(12));
        r.staged = Some(StagedReplay::new(9, true, 40, 50, 3));
        r.staged.as_mut().unwrap().step();
        let back = Replays::decode(&r.encode()).unwrap();
        assert_eq!(back, r);
        assert!(back.validate(6).is_ok());
        assert!(back.validate(4).is_err());
        let mut bytes = r.encode();
        bytes.pop();
        assert!(Replays::decode(&bytes).is_err());
        let mut bytes = r.encode();
        bytes.push(0);
        assert!(Replays::decode(&bytes).is_err());
    }
}
