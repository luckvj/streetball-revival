//! Computer players, as the original plays them.
//!
//! In the original game a computer player fills the same pad word a
//! controller does (`FUN_0013a970`, called from the actor's pad update
//! `FUN_001587d8`), so the AI here builds a pad too and the world plays it
//! like anyone's. A client asks for the computer with [`input`], which
//! returns a marker ([`Input::computer`]); [`World::step`] replaces the
//! marker with the pad the AI builds that tick, drawing from the world's
//! generator (`FUN_003a16b8`) in the original's order where it is traced.
//! Every peer therefore computes the same pads, and the AI's state lives in
//! the [`World`] (and its snapshot).
//!
//! The structure is the original's (`docs/TRACE.md`, "Computer players"):
//!
//! - Each player has a queue of behaviours (`AIPlayer+0xc`, front first,
//!   `FUN_00133030`): "shoot", "pass", "drive", "carrier pause", "defense",
//!   "loose ball" and so on, each with its own start, done test and
//!   update. With nothing queued the player picks one by the game's phase
//!   and its role (`FUN_0017e5c8`, [`team::decide`]).
//! - Each team runs the strategy its lists hold for the game's phase and
//!   whether it has the ball (`AITeam`, `FUN_0018a708`): the tip, the
//!   loose-ball scramble, the rebounds, the fast break, the playbook, the
//!   defense, the inbounds, the end of the game.
//! - The steering is the original's: a 10-bit stick angle and one of four
//!   stick strengths (`FUN_0013af58`), smoothed, bent round players and
//!   kept off the court's edges.
//! - Skills blend four numbers by the difficulty and the score
//!   ([`skill`]).
//!
//! What is ported, approximated or missing is listed per item in the
//! modules and in `docs/TRACE.md`.

mod court;
pub(crate) use court::native_sort;
mod defence;
mod fastbreak;
mod inbound;
mod loose;
mod moves;
mod offball;
mod offence;
mod pad;
mod picks;
mod playbook;
mod plays;
mod skill;
mod spots;
mod strategy;
mod team;
pub(crate) mod words;

use crate::{Input, OpeningToss, Phase, World, rng::Rng};
use pad::{Motion, Pad};
use team::Coach;
use words::{Reader, Word, words};

/// What a person's call off the ball asks of the computer holder
/// ([`World::call_for_ball`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Call {
    /// Cross: a pass node to the caller.
    Pass,
    /// Cross with two turbos: a trick pass node with no receiver.
    TrickPass,
    /// Circle without a turbo: a shoot node.
    Shoot,
}

/// The controls for computer player `i`: the marker that tells
/// [`World::step`] to build its pad. The world is not needed; the
/// signature keeps every caller's shape.
pub fn input(_w: &World, _i: usize) -> Input {
    Input::computer()
}

words! {
    /// What the AI tracks of the game (the AIGame fields it reads).
    pub(crate) struct Game {
        /// game+0xf4 ([`crate::game_phase::phase`]): 1 the opening toss, 2
        /// a loose ball, 3 a shot or rebound, 4 the ball in the back court,
        /// 5 in the front court, 6 a restart, 9 won.
        phase: i32,
        /// The ball's state as the AI reads it (ball+0x180,
        /// [`crate::game_phase::ball`]): 0 a rebound coming off, 1 loose, 3
        /// held, 4 passed, 5 shot, 6 on the rim, 7 the referee's toss.
        ball: i32,
        /// Frames the ball has kept that state (ball+0x60).
        ball_age: i32,
        /// The holder and how long he has held it (game+0x490).
        holder: i32,
        held: i32,
        /// The holder + 1 when he caught a pass (the ball in state 4 the
        /// update before he had it), else 0: his catch animation
        /// ([`Ctx::catching`]).
        caught: i32,
        /// Frames of a loose ball or rebound (game+0x440).
        loose: i32,
        /// The side the held phases belong to (game+0x110), -1 before
        /// anyone has had the ball.
        offence: i32,
        /// game+0x43c: the side that last held the ball, or -1.
        last_team: i32,
        /// game+0x470: by player, the frames a person on the side without
        /// the ball has stood in the paint (within 10 of the rim, |z| < 7;
        /// `FUN_0013d868`), outside phase 3.
        paint: [i32; 6],
        /// game+0xfc: the other side has pocketed, a cancel to ask about.
        cancel: [bool; 2],
        pocketed: [bool; 2],
        /// game+0x4c0: in Gamebreaker Wins, the pocket check chose to
        /// cancel (`FUN_00145d28`, read by `FUN_00145e70`).
        gb_cancel: bool,
        /// `0x3cf620`: the rebound spots by player, dealt once a frame
        /// (`FUN_0012d790`, `0x4b34a0` the frame) and the long-shot flag
        /// (`0x3cf680`).
        spots: [[f32; 2]; 6],
        spots_tick: i64,
        long_shot: bool,
        /// `0x3c6f78`, a toggle every drive shares.
        toggle: bool,
        /// `0x3c9e68`, the toggle every post-up shares.
        post_toggle: bool,
        /// A trick pass to himself asked of the game this frame
        /// (`FUN_00142068`): the passer + 1, or 0; and the defender + 1 off
        /// whose head it goes (`FUN_0016e8f8`), or 0 off the board.
        trick: i32,
        trick_off: i32,
        /// The board's point a trick pass off the board was asked at
        /// (`FUN_00183318`'s draws).
        trick_point: [f32; 3],
        /// The restart's globals and ready flags ([`inbound::Restart`]).
        restart: inbound::Restart,
        /// The player + 1 a "loose ball" behaviour hands a ball left free
        /// for ten seconds this frame (`0x10e948..0x10e964`, the take
        /// `FUN_0014cb28`), or 0.
        free_take: i32,
        /// The inbounder + 1 whose "throw inbound" starts its walk this
        /// frame (its first update posts `0x3c6510`, the listeners' slot
        /// 41, `0x10b830..0x10b87c`), or 0.
        inbound_walk: i32,
        /// The screener + 1 whose "set pick" came within 5 of the defender
        /// this frame (`0x3c68f0`, the listeners' slot 72,
        /// `0x118a50..0x118ac4`), or 0.
        pick_call: i32,
    }
}

/// One queued behaviour (the node's +4 age and +8 started flag).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    pub started: bool,
    pub age: u16,
    pub act: Act,
}

/// The behaviours, by the original's type ids where they have one.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Act {
    /// 3.
    CarrierPause(offence::CarrierPause),
    /// 9.
    Shoot(offence::Shoot),
    /// 8.
    Pass(offence::Pass),
    /// 0x15 and 0x16: press Select once.
    Pocket(bool),
    Cancel(bool),
    /// 5.
    Drive(offence::Drive),
    /// 0x11.
    Iso(offence::Iso),
    /// 10.
    Defense(defence::Defense),
    /// 0xd.
    LooseBall(loose::LooseBall),
    PrepareRebound(loose::PrepareRebound),
    /// 0xc.
    FastBreak(fastbreak::FastBreak),
    PerimPlayer(spots::PerimPlayer),
    PostPlayer(spots::PostPlayer),
    CutMove(spots::CutMove),
    PostCut(spots::PostCut),
    PostPause(spots::PostPause),
    /// "carrier move" (type 0).
    CarrierMove(moves::CarrierMove),
    /// 4: the alley-oop leaper and the second leaper.
    Leaper(moves::Leaper),
    Follower(moves::Follower),
    /// "wait for toss" (`FUN_0011af98`): done when the phase is no longer
    /// the toss; its update only tells the referee the player is ready.
    WaitForToss,
    /// 0x10.
    Pause(offball::Pause),
    /// 0x18.
    Goto(offball::Goto),
    PerimPause(offball::PerimPause),
    DriftUpCourt(loose::DriftUpCourt),
    /// 0xb.
    Rebound(loose::Rebound),
    GoalTend(loose::GoalTend),
    /// 0x13.
    BoxOut(loose::BoxOut),
    /// 0xe.
    OpenLooseBall(fastbreak::OpenLooseBall),
    /// 0xf.
    Outlet,
    /// "wait for hc" (vtable `0x437898`): stand until the phase leaves 4.
    WaitForHalfCourt,
    /// "throw inbound" (`FUN_0010b658`).
    ThrowInbound(inbound::ThrowInbound),
    /// "inbound pos" (`FUN_0010ba18`).
    InboundPos(inbound::InboundPos),
    /// "set pick" (type 1, `FUN_00118508`).
    SetPick(picks::SetPick),
    /// The roll after a pick (type 2, `FUN_00117e80`).
    Roll(picks::Roll),
    /// "run pick play" (`FUN_00118f60`).
    RunPick(picks::RunPick),
    /// "pick off ball %d" (`FUN_001199e0`).
    PickOffBall(picks::PickOffBall),
    /// 0x12: "keep away" (`FUN_0010d4c8`).
    KeepAway(offence::KeepAway),
    /// 6: "post up" (`FUN_00115368`).
    PostUp(offence::PostUp),
    /// The handler who feeds a partner (`FUN_0011a580`).
    Feeder(offence::Feeder),
    /// 0x17: a trick pass (`FUN_00100c60`).
    TrickPass(offence::TrickPass),
    /// "playbook hold" (vtable `0x43ae88`): the Dive lesson's holder.
    LessonHold(offence::LessonHold),
    /// 0x10 (`FUN_0010d090`): the Trick Counter lesson's holder.
    LessonJuke(offence::LessonJuke),
}

impl Act {
    /// The original's type id (`FUN_001332e0`), 0 for the untyped ones.
    pub fn kind(&self) -> i32 {
        match self {
            Act::CarrierPause(_) => 3,
            Act::Shoot(_) => 9,
            Act::Pass(_) => 8,
            Act::Leaper(_) | Act::Follower(_) => 4,
            Act::Pocket(_) => 0x15,
            Act::Cancel(_) => 0x16,
            Act::Drive(_) => 5,
            Act::Iso(_) => 0x11,
            Act::Defense(_) => 10,
            Act::LooseBall(_) => 0xd,
            Act::FastBreak(_) => 0xc,
            Act::Pause(_) | Act::LessonJuke(_) => 0x10,
            Act::Goto(_) => 0x18,
            Act::Rebound(_) => 0xb,
            Act::BoxOut(_) => 0x13,
            Act::OpenLooseBall(_) => 0xe,
            Act::Outlet => 0xf,
            Act::SetPick(_) => 1,
            Act::Roll(_) => 2,
            Act::KeepAway(_) => 0x12,
            Act::PostUp(_) => 6,
            Act::TrickPass(_) => 0x17,
            Act::PrepareRebound(_)
            | Act::Feeder(_)
            | Act::RunPick(_)
            | Act::PickOffBall(_)
            | Act::PerimPlayer(_)
            | Act::PostPlayer(_)
            | Act::CutMove(_)
            | Act::PostCut(_)
            | Act::PostPause(_)
            | Act::LessonHold(_)
            | Act::CarrierMove(_)
            | Act::WaitForToss
            | Act::PerimPause(_)
            | Act::DriftUpCourt(_)
            | Act::GoalTend(_)
            | Act::WaitForHalfCourt
            | Act::ThrowInbound(_)
            | Act::InboundPos(_) => 0,
        }
    }

    /// The point a pass to the player is aimed by (vt+0x24, `FUN_00133380`
    /// from the pass start's `FUN_0013b900`): the target ([`Act::target`])
    /// for most, but the fast break and the roll the point near the rim
    /// on his line to it (`FUN_0010aea8`, `FUN_001180d8`), and the net cut,
    /// the circle cut and the post cut their own (`FUN_00110ae8`,
    /// `FUN_00111720`, `FUN_00112f48`). `pos` his place, `rim` the rim the
    /// offence attacks.
    pub fn pass_target(&self, pos: [f32; 2], rim: [f32; 2]) -> Option<[f32; 2]> {
        match self {
            Act::FastBreak(_) | Act::Roll(_) => Some(spots::near_rim(pos, rim)),
            Act::CutMove(c) => Some(c.pass_target(pos, rim)),
            Act::PostCut(c) => Some(c.pass_target(pos, rim)),
            _ => self.target(),
        }
    }

    /// Where the behaviour is taking the player (`FUN_0013ab38`'s vt+0x1c
    /// target), for the ones that have one.
    pub fn target(&self) -> Option<[f32; 2]> {
        match self {
            Act::CarrierMove(c) => Some(c.spot()),
            Act::CutMove(c) => Some(c.spot()),
            Act::PostCut(c) => Some(c.spot()),
            Act::FastBreak(f) => Some(f.spot()),
            Act::Goto(g) => Some(g.spot()),
            Act::ThrowInbound(t) => Some(t.spot()),
            Act::InboundPos(p) => Some(p.spot()),
            Act::RunPick(r) => r.target(),
            Act::Feeder(f) => f.target(),
            _ => None,
        }
    }

    fn tag(&self) -> u32 {
        match self {
            Act::CarrierPause(_) => 0,
            Act::Shoot(_) => 1,
            Act::Pass(_) => 2,
            Act::Pocket(_) => 3,
            Act::Cancel(_) => 4,
            Act::Drive(_) => 5,
            Act::Iso(_) => 6,
            Act::Defense(_) => 7,
            Act::LooseBall(_) => 8,
            Act::PrepareRebound(_) => 9,
            Act::FastBreak(_) => 10,
            Act::PerimPlayer(_) => 11,
            Act::PostPlayer(_) => 12,
            Act::CarrierMove(_) => 13,
            Act::WaitForToss => 14,
            Act::Pause(_) => 15,
            Act::Goto(_) => 16,
            Act::PerimPause(_) => 17,
            Act::DriftUpCourt(_) => 18,
            Act::Rebound(_) => 19,
            Act::GoalTend(_) => 20,
            Act::BoxOut(_) => 21,
            Act::OpenLooseBall(_) => 22,
            Act::Outlet => 23,
            Act::WaitForHalfCourt => 24,
            Act::CutMove(_) => 25,
            Act::PostCut(_) => 26,
            Act::PostPause(_) => 27,
            Act::Leaper(_) => 28,
            Act::Follower(_) => 29,
            Act::ThrowInbound(_) => 30,
            Act::InboundPos(_) => 31,
            Act::SetPick(_) => 32,
            Act::Roll(_) => 33,
            Act::RunPick(_) => 34,
            Act::PickOffBall(_) => 35,
            Act::KeepAway(_) => 36,
            Act::PostUp(_) => 37,
            Act::Feeder(_) => 38,
            Act::TrickPass(_) => 39,
            Act::LessonHold(_) => 40,
            Act::LessonJuke(_) => 41,
        }
    }

    fn put(&self, out: &mut Vec<u32>) {
        out.push(self.tag());
        match self {
            Act::CarrierPause(a) => a.put(out),
            Act::Shoot(a) => a.put(out),
            Act::Pass(a) => a.put(out),
            Act::Pocket(a) | Act::Cancel(a) => a.put(out),
            Act::Drive(a) => a.put(out),
            Act::Iso(a) => a.put(out),
            Act::Defense(a) => a.put(out),
            Act::LooseBall(a) => a.put(out),
            Act::PrepareRebound(a) => a.put(out),
            Act::FastBreak(a) => a.put(out),
            Act::PerimPlayer(a) => a.put(out),
            Act::PostPlayer(a) => a.put(out),
            Act::CutMove(a) => a.put(out),
            Act::PostCut(a) => a.put(out),
            Act::PostPause(a) => a.put(out),
            Act::CarrierMove(a) => a.put(out),
            Act::Leaper(a) => a.put(out),
            Act::Follower(a) => a.put(out),
            Act::Pause(a) => a.put(out),
            Act::Goto(a) => a.put(out),
            Act::PerimPause(a) => a.put(out),
            Act::DriftUpCourt(a) => a.put(out),
            Act::Rebound(a) => a.put(out),
            Act::GoalTend(a) => a.put(out),
            Act::BoxOut(a) => a.put(out),
            Act::OpenLooseBall(a) => a.put(out),
            Act::ThrowInbound(a) => a.put(out),
            Act::InboundPos(a) => a.put(out),
            Act::SetPick(a) => a.put(out),
            Act::Roll(a) => a.put(out),
            Act::RunPick(a) => a.put(out),
            Act::PickOffBall(a) => a.put(out),
            Act::KeepAway(a) => a.put(out),
            Act::PostUp(a) => a.put(out),
            Act::Feeder(a) => a.put(out),
            Act::TrickPass(a) => a.put(out),
            Act::LessonHold(a) => a.put(out),
            Act::LessonJuke(a) => a.put(out),
            Act::WaitForToss | Act::Outlet | Act::WaitForHalfCourt => {}
        }
    }

    fn get(r: &mut Reader) -> Result<Self, String> {
        Ok(match r.word()? {
            0 => Act::CarrierPause(Word::get(r)?),
            1 => Act::Shoot(Word::get(r)?),
            2 => Act::Pass(Word::get(r)?),
            3 => Act::Pocket(Word::get(r)?),
            4 => Act::Cancel(Word::get(r)?),
            5 => Act::Drive(Word::get(r)?),
            6 => Act::Iso(Word::get(r)?),
            7 => Act::Defense(Word::get(r)?),
            8 => Act::LooseBall(Word::get(r)?),
            9 => Act::PrepareRebound(Word::get(r)?),
            10 => Act::FastBreak(Word::get(r)?),
            11 => Act::PerimPlayer(Word::get(r)?),
            12 => Act::PostPlayer(Word::get(r)?),
            13 => Act::CarrierMove(Word::get(r)?),
            14 => Act::WaitForToss,
            15 => Act::Pause(Word::get(r)?),
            16 => Act::Goto(Word::get(r)?),
            17 => Act::PerimPause(Word::get(r)?),
            18 => Act::DriftUpCourt(Word::get(r)?),
            19 => Act::Rebound(Word::get(r)?),
            20 => Act::GoalTend(Word::get(r)?),
            21 => Act::BoxOut(Word::get(r)?),
            22 => Act::OpenLooseBall(Word::get(r)?),
            23 => Act::Outlet,
            24 => Act::WaitForHalfCourt,
            25 => Act::CutMove(Word::get(r)?),
            26 => Act::PostCut(Word::get(r)?),
            27 => Act::PostPause(Word::get(r)?),
            28 => Act::Leaper(Word::get(r)?),
            29 => Act::Follower(Word::get(r)?),
            30 => Act::ThrowInbound(Word::get(r)?),
            31 => Act::InboundPos(Word::get(r)?),
            32 => Act::SetPick(Word::get(r)?),
            33 => Act::Roll(Word::get(r)?),
            34 => Act::RunPick(Word::get(r)?),
            35 => Act::PickOffBall(Word::get(r)?),
            36 => Act::KeepAway(Word::get(r)?),
            37 => Act::PostUp(Word::get(r)?),
            38 => Act::Feeder(Word::get(r)?),
            39 => Act::TrickPass(Word::get(r)?),
            40 => Act::LessonHold(Word::get(r)?),
            41 => Act::LessonJuke(Word::get(r)?),
            k => return Err(format!("bad AI behaviour {k}")),
        })
    }

    /// The node's start (vt+0x44).
    fn enter(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        match self {
            Act::SetPick(a) => a.enter(ctx, b, i),
            Act::CarrierPause(a) => a.enter(ctx, i),
            Act::Pass(a) => a.enter(ctx, i),
            Act::Drive(a) => a.enter(ctx, i),
            Act::PostUp(a) => a.enter(ctx, i),
            Act::GoalTend(a) => a.enter(ctx),
            _ => {}
        }
    }

    /// The node's done test (vt+0x14). `age` is the node's (+4). Some
    /// queue behaviours behind the node, or draw, as the original's do.
    fn done(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize, age: u16) -> bool {
        let ours = ctx.offence() == ctx.team(i);
        match self {
            // `0x1009d8` is never done; the rewrite ends it when the
            // handler no longer has the ball (the original's playbook
            // replaces it, not ported).
            Act::CarrierPause(_) => !ctx.has_ball(i),
            Act::PerimPlayer(a) => a.done(),
            Act::PostPlayer(a) => a.done(),
            Act::PostCut(a) => a.done(ctx, i),
            Act::PostPause(a) => a.done(ctx, b, i, age),
            Act::CutMove(a) => a.done(ctx, b, i, age),
            Act::Shoot(a) => a.done(ctx, i),
            Act::Pass(a) => a.done(ctx, i),
            Act::Pocket(done) | Act::Cancel(done) => *done,
            Act::Drive(a) => a.done(ctx, i),
            Act::Iso(a) => a.done(ctx, i),
            // `FUN_00106458`: done once the player's side has the ball.
            Act::Defense(_) => ours,
            Act::LooseBall(a) => a.done(ctx),
            Act::PrepareRebound(a) => a.done(),
            Act::FastBreak(a) => a.done(ctx, i),
            Act::CarrierMove(a) => a.done(),
            Act::Leaper(a) => a.done(ctx, i),
            Act::Follower(a) => a.done(ctx),
            // `0x11afe8`.
            Act::WaitForToss => ctx.game.phase != 1,
            Act::Pause(a) => a.done(age),
            Act::Goto(a) => a.done(ctx, i, age),
            Act::PerimPause(a) => a.done(ctx, i, age),
            Act::DriftUpCourt(a) => a.done(),
            Act::Rebound(a) => a.done(ctx, i),
            Act::GoalTend(a) => a.done(),
            // `FUN_00117300`.
            Act::BoxOut(_) => ctx.game.ball != 5,
            Act::OpenLooseBall(a) => a.done(ctx),
            // `FUN_0010af90`.
            Act::Outlet => fastbreak::outlet_done(ctx, i),
            // `0x10b600`.
            Act::WaitForHalfCourt => ctx.game.phase != 4,
            Act::ThrowInbound(a) => a.done(ctx, i),
            Act::InboundPos(a) => a.done(ctx, i),
            Act::SetPick(a) => a.done(ctx, i, age),
            Act::Roll(a) => a.done(ctx, i, age),
            Act::RunPick(a) => a.done(ctx, i),
            Act::PickOffBall(a) => a.done(ctx, b, i, age),
            Act::KeepAway(a) => a.done(ctx, i),
            Act::PostUp(a) => a.done(age),
            Act::Feeder(a) => a.done(ctx, i),
            Act::TrickPass(a) => a.done(ctx, i),
            // `0x12b7d8`: never done.
            Act::LessonHold(_) => false,
            Act::LessonJuke(a) => a.done(age),
        }
    }

    /// What the done test does to the pad when it ends the node: the walk
    /// to a point lets go of the stick (`FUN_0010f318`).
    fn on_done(&self, ctx: &Ctx, b: &mut Brain, i: usize) {
        if let Act::Goto(g) = self
            && g.stops()
        {
            b.stop(ctx, i);
        }
    }

    /// The node's update (vt+0x4c).
    fn update(&mut self, ctx: &mut Ctx, b: &mut Brain, i: usize) {
        match self {
            Act::CarrierPause(a) => a.update(ctx, b, i),
            Act::Shoot(a) => a.update(ctx, b, i),
            Act::Pass(a) => a.update(ctx, b, i),
            // `FUN_00102478` / `FUN_00102510`.
            Act::Pocket(done) => {
                b.stop(ctx, i);
                b.press(pad::bits::POCKET);
                *done = true;
            }
            Act::Cancel(done) => {
                b.stop(ctx, i);
                b.press(pad::bits::CANCEL);
                *done = true;
            }
            Act::Drive(a) => a.update(ctx, b, i),
            Act::Iso(a) => a.update(ctx, b, i),
            Act::Defense(a) => a.update(ctx, b, i),
            Act::LooseBall(a) => a.update(ctx, b, i),
            Act::PrepareRebound(a) => a.update(ctx, b, i),
            Act::FastBreak(a) => a.update(ctx, b, i),
            Act::PerimPlayer(a) => a.update(ctx, b, i),
            Act::PostPlayer(a) => a.update(ctx, b, i),
            Act::CutMove(a) => a.update(ctx, b, i),
            Act::PostCut(a) => a.update(ctx, b, i),
            Act::PostPause(a) => a.update(ctx, b, i),
            Act::CarrierMove(a) => a.update(ctx, b, i),
            Act::Leaper(a) => a.update(ctx, b, i),
            Act::Follower(a) => a.update(ctx, b, i),
            Act::WaitForToss | Act::WaitForHalfCourt => {
                if matches!(self, Act::WaitForHalfCourt) {
                    b.stop(ctx, i);
                }
            }
            Act::Pause(a) => a.update(ctx, b, i),
            Act::Goto(a) => a.update(ctx, b, i),
            Act::PerimPause(a) => a.update(ctx, b, i),
            Act::DriftUpCourt(a) => a.update(ctx, b, i),
            Act::Rebound(a) => a.update(ctx, b, i),
            Act::GoalTend(a) => a.update(ctx, b, i),
            Act::BoxOut(a) => a.update(ctx, b, i),
            Act::OpenLooseBall(a) => a.update(ctx, b, i),
            Act::Outlet => fastbreak::outlet(ctx, b, i),
            Act::ThrowInbound(a) => a.update(ctx, b, i),
            Act::InboundPos(a) => a.update(ctx, b, i),
            Act::SetPick(a) => a.update(ctx, b, i),
            Act::Roll(a) => a.update(ctx, b, i),
            Act::RunPick(a) => a.update(ctx, b, i),
            Act::PickOffBall(a) => a.update(ctx, b, i),
            Act::KeepAway(a) => a.update(ctx, b, i),
            Act::PostUp(a) => a.update(ctx, b, i),
            Act::Feeder(a) => a.update(ctx, b, i),
            Act::TrickPass(a) => a.update(ctx, b, i),
            Act::LessonHold(a) => a.update(ctx, b, i),
            Act::LessonJuke(a) => a.update(ctx, b, i),
        }
    }
}

/// One computer player (AIPhysPlayer).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Brain {
    pub queue: Vec<Node>,
    pub pad: Pad,
    /// Last frame's buttons and the frame they were built.
    pub prev: u32,
    pub ran: i64,
    pub motion: Motion,
    /// AI+0x58: 1 the handler, 2 the post man, 3 the perimeter man.
    pub role: u8,
    /// AI+0x54: the fast break's lane (0..2).
    pub lane: u8,
    /// AI+0x30: inside its own update, when clearing is put off.
    pub updating: bool,
    /// The running node's age (+4), for its update; not kept.
    pub age: u16,
    /// AI+0x5c: a pick is over (set by "set pick" and "run pick play",
    /// cleared by the pick and roll's screen, `FUN_0011eac0`).
    pub pick_end: bool,
    /// AI+0x60: which way round the rim the holder comes past this
    /// screener (±1, "set pick"'s start).
    pub pick_side: i32,
    /// AI+0xb0: the player who last screened this defender, or -1 (the
    /// contact message `0x3c6910` through `FUN_00141448`/`FUN_0018b308`;
    /// never cleared).
    pub screened_by: i32,
}

impl Default for Brain {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            pad: Pad::default(),
            prev: 0,
            ran: 0,
            motion: Motion::default(),
            role: 0,
            lane: 0,
            updating: false,
            age: 0,
            pick_end: false,
            pick_side: 0,
            screened_by: -1,
        }
    }
}

impl Brain {
    /// `FUN_0013aa88`: queue a behaviour.
    pub fn push(&mut self, act: Act) {
        self.queue.push(Node {
            started: false,
            age: 0,
            act,
        });
    }

    /// `FUN_0013aa48`: drop the queue and let go of the stick (not from
    /// inside the player's own update).
    pub fn clear(&mut self, ctx: &Ctx, i: usize) {
        if !self.updating {
            self.queue.clear();
            self.stop(ctx, i);
        }
    }

    /// `FUN_0013aaa8`: replace the queue with one behaviour.
    pub fn replace(&mut self, ctx: &Ctx, i: usize, act: Act) {
        self.clear(ctx, i);
        self.push(act);
    }

    /// `FUN_001332e0`: the front behaviour's type, 0 with none.
    pub fn front_kind(&self) -> i32 {
        self.queue.first().map_or(0, |n| n.act.kind())
    }

    /// `FUN_00133148`: the front behaviour's target, if it has one.
    pub fn front_target(&self) -> Option<[f32; 2]> {
        self.queue.first().and_then(|n| n.act.target())
    }

    /// The front behaviour's point for a pass to him (vt+0x24,
    /// [`Act::pass_target`]), if it has one.
    pub fn front_pass_target(&self, pos: [f32; 2], rim: [f32; 2]) -> Option<[f32; 2]> {
        self.queue.first().and_then(|n| n.act.pass_target(pos, rim))
    }

    /// `FUN_00181c38`: hold turbo, unless the side is saving it.
    pub fn sprint(&mut self, ctx: &Ctx, i: usize) {
        if !ctx.save_turbo[ctx.team(i)] {
            self.press(pad::bits::TURBO);
        }
    }

    fn put(&self, out: &mut Vec<u32>) {
        out.push(self.queue.len() as u32);
        for n in &self.queue {
            n.started.put(out);
            out.push(u32::from(n.age));
            n.act.put(out);
        }
        self.pad.put(out);
        self.prev.put(out);
        self.ran.put(out);
        self.motion.put(out);
        self.role.put(out);
        self.lane.put(out);
        self.updating.put(out);
        self.pick_end.put(out);
        self.pick_side.put(out);
        self.screened_by.put(out);
    }

    fn get(r: &mut Reader) -> Result<Self, String> {
        let n = r.word()? as usize;
        if n > 64 {
            return Err("AI queue too long".into());
        }
        let mut queue = Vec::with_capacity(n);
        for _ in 0..n {
            let started = bool::get(r)?;
            let age = u16::try_from(r.word()?).map_err(|_| "AI node age out of range")?;
            let act = Act::get(r)?;
            queue.push(Node { started, age, act });
        }
        Ok(Self {
            queue,
            pad: Word::get(r)?,
            prev: Word::get(r)?,
            ran: Word::get(r)?,
            motion: Word::get(r)?,
            role: Word::get(r)?,
            lane: Word::get(r)?,
            updating: Word::get(r)?,
            age: 0,
            pick_end: Word::get(r)?,
            pick_side: Word::get(r)?,
            screened_by: Word::get(r)?,
        })
    }
}

/// The AI's whole state, carried in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub(crate) game: Game,
    pub(crate) coaches: [Coach; 2],
    pub(crate) brains: Vec<Brain>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            game: Game {
                holder: -1,
                offence: -1,
                last_team: -1,
                spots_tick: -1,
                ..Default::default()
            },
            coaches: [Coach::new(0), Coach::new(1)],
            brains: Vec::new(),
        }
    }
}

impl State {
    /// The state as bytes, for the world snapshot.
    pub(crate) fn encode(&self) -> Vec<u8> {
        // A world whose AI has never run (no computer players yet) keeps
        // no AI bytes.
        if *self == Self::default() {
            return Vec::new();
        }
        let mut words = Vec::new();
        self.game.put(&mut words);
        for c in &self.coaches {
            c.put(&mut words);
        }
        words.push(self.brains.len() as u32);
        for b in &self.brains {
            b.put(&mut words);
        }
        // Each word as a zigzagged LEB128 varint: the many small counters
        // and flags take a byte each.
        let mut out = Vec::new();
        for w in words {
            let v = w as i32;
            let mut z = ((v << 1) ^ (v >> 31)) as u32;
            loop {
                let byte = (z & 0x7f) as u8;
                z >>= 7;
                if z == 0 {
                    out.push(byte);
                    break;
                }
                out.push(byte | 0x80);
            }
        }
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let mut words = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            let mut z = 0u32;
            let mut shift = 0;
            loop {
                let byte = *bytes.get(at).ok_or("AI state cut short")?;
                at += 1;
                if shift > 28 {
                    return Err("AI varint too long".into());
                }
                z |= u32::from(byte & 0x7f) << shift;
                shift += 7;
                if byte & 0x80 == 0 {
                    break;
                }
            }
            words.push(((z >> 1) as i32 ^ -((z & 1) as i32)) as u32);
        }
        let mut r = Reader::new(&words);
        let game = Word::get(&mut r)?;
        let coaches = [Word::get(&mut r)?, Word::get(&mut r)?];
        let n = r.word()? as usize;
        if n > 64 {
            return Err("too many AI players".into());
        }
        let brains = (0..n)
            .map(|_| Brain::get(&mut r))
            .collect::<Result<Vec<_>, _>>()?;
        if !r.done() {
            return Err("AI state has words left over".into());
        }
        Ok(Self {
            game,
            coaches,
            brains,
        })
    }

    /// For [`World::validate`]: no brains or one per player, and every
    /// player it names among the players.
    pub(crate) fn validate(&self, players: usize) -> Result<(), String> {
        let bad = |field: &str| Err(format!("invalid world field: AI {field}"));
        if !(self.brains.is_empty() || self.brains.len() == players) {
            return bad("player count");
        }
        let player = |v: i32| v == -1 || (0 <= v && (v as usize) < players);
        let game = &self.game;
        let plus_one = |v: i32| 0 <= v && (v as usize) <= players;
        if !player(game.holder)
            || !(-1..=1).contains(&game.offence)
            || !(-1..=1).contains(&game.last_team)
            || !plus_one(game.trick)
            || !plus_one(game.trick_off)
            || !plus_one(game.free_take)
            || !plus_one(game.inbound_walk)
            || !plus_one(game.pick_call)
        {
            return bad("game");
        }
        for c in &self.coaches {
            if !c.valid(players) {
                return bad("team");
            }
        }
        if !game.restart.valid(players) {
            return bad("restart");
        }
        for b in &self.brains {
            if !player(b.pad.target) || b.role > 3 || b.lane > 2 {
                return bad("pad");
            }
            if !player(b.screened_by) || !(-1..=1).contains(&b.pick_side) {
                return bad("pick");
            }
        }
        Ok(())
    }
}

/// What the AI reads while it decides: the world, the generator, the game
/// it tracks, and who is a person this frame.
pub(crate) struct Ctx<'a> {
    pub w: &'a World,
    rng: &'a mut Rng,
    pub game: Game,
    /// game+0x9a: a person plays on each side.
    pub human: [bool; 2],
    computer: &'a [bool],
    /// This frame's inputs (a person's buttons, for the strategy call).
    inputs: &'a [Input],
    pub tick: i64,
    /// The defense strategies' matchups (man, help, second) by side and
    /// slot.
    pub matchups: [([i32; 3], [i32; 3], [i32; 3]); 2],
    pub save_turbo: [bool; 2],
    /// team+0x94 and team+0xa0, by side.
    pub open: [[i32; 3]; 2],
    pub holder_of: [i32; 2],
    /// Every player's pad buttons (this frame's once he has run, else
    /// last frame's) and front behaviour type, as the original's AIs
    /// read each other's.
    pub pads: Vec<u32>,
    pub fronts: Vec<i32>,
    /// Every player's front behaviour's target (vt+0x1c), if it has one.
    pub targets: Vec<Option<[f32; 2]>>,
    /// Every player's AI+0x60 (the side a pick is set for) and AI+0xb0
    /// (who screened him).
    pub pick_sides: Vec<i32>,
    pub screened: Vec<i32>,
}

impl Ctx<'_> {
    /// `FUN_003a16b8`.
    pub fn rand(&mut self) -> u32 {
        self.rng.next_u32()
    }

    /// `FUN_00140558`: play is on (phases 2..5).
    pub fn live(&self) -> bool {
        (2..=5).contains(&self.game.phase)
    }

    /// `FUN_00140840`: the restart's hold (game+0x104, after a
    /// gamebreaker's basket) is over and all six are ready.
    pub fn restart_ready(&self) -> bool {
        self.w.game_phase.hold == 0 && self.game.restart.all_ready()
    }

    pub fn human_player(&self, i: usize) -> bool {
        !self.computer.get(i).copied().unwrap_or(false)
    }

    /// Player `i`'s controls this frame.
    pub fn input(&self, i: usize) -> Input {
        self.inputs.get(i).copied().unwrap_or_default()
    }

    /// game+0x3ec: every player in actor order sorted by his distance to
    /// the offence's rim ([`court::native_sort`], `0x13f7c8..0x13f838`).
    pub fn by_rim(&self) -> Vec<usize> {
        let mut all: Vec<usize> = (0..self.w.players.len()).collect();
        court::native_sort(&mut all, |j| self.hoop_dist(j));
        all
    }

    /// game+0x3ec's first: the player nearest the offence's rim.
    pub fn nearest_to_rim(&self) -> Option<usize> {
        self.by_rim().first().copied()
    }

    /// `FUN_0013f8a8(team, k)`: the side's k-th nearest the offence's rim
    /// (game+0x3ec walked for the side).
    pub fn nth_of_team_to_rim(&self, team: usize, k: usize) -> Option<usize> {
        self.by_rim()
            .into_iter()
            .filter(|&j| self.team(j) == team)
            .nth(k)
    }

    /// `FUN_0018ab78(team, k)`: the side's players by distance to the
    /// ball, nearest first (game+0x3a4: every player in actor order sorted
    /// by [`court::native_sort`], `0x13fdb8..0x13fe0c`, walked for the side).
    pub fn by_ball(&self, team: usize) -> Vec<usize> {
        let mut all: Vec<usize> = (0..self.w.players.len()).collect();
        court::native_sort(&mut all, |j| self.ball_dist(j));
        all.retain(|&j| self.team(j) == team);
        all
    }

    /// `FUN_0017f498(ai, k)`: the defense strategy's matchup for a
    /// defender: 0 his man, 1 the player to help on, 2 the second man.
    pub fn matchup(&self, i: usize, k: usize) -> Option<usize> {
        let team = self.team(i);
        let slot = self.slot(i).min(2);
        let (man, help, second) = self.matchups[team];
        let v = match k {
            0 => man[slot],
            1 => help[slot],
            _ => second[slot],
        };
        usize::try_from(v)
            .ok()
            .filter(|&j| j < self.w.players.len())
    }

    /// The side has the ball, as the team strategies read it
    /// (game+0x110 == team); nobody has it during the opening toss.
    pub fn attacking(&self, team: usize) -> bool {
        self.game.phase != 1 && self.offence() == team
    }

    /// actor+0x298: forced computer control, through the opening toss and
    /// a six-player restart (`FUN_00140880`). A pad handed to the inbounder
    /// must not clear the previous person's queued walk to his ready spot.
    pub fn frozen(&self) -> bool {
        self.w.restarting() || matches!(self.w.opening_toss, OpeningToss::Waiting(_))
    }

    /// The rebound spot dealt to player `i` (`0x3cf620 + 16 i`).
    pub fn rebound_spot(&self, i: usize) -> [f32; 2] {
        self.game.spots[i.min(5)]
    }

    /// `FUN_0012d790`: deal the six rebound spots, once a frame. Around
    /// the rim the offence attacks, six sectors of 76 units from 28 (the
    /// far side mirrored for the −x rim) take the players in the order of
    /// their distance to the rim, each the free sector nearest his angle;
    /// each spot is then drawn in its sector (`r % 76`) at 3 + `r % 35` /
    /// 10 out, or 5 + `r % 40` / 10 after a shot from beyond 16.
    pub fn deal_rebound_spots(&mut self) {
        if self.game.spots_tick == self.tick {
            return;
        }
        self.game.spots_tick = self.tick;
        let offence = self.offence();
        let rim = self.goal(offence);
        if self.game.ball_age < 3 {
            self.game.long_shot = 16.0 < self.shot_distance();
        }
        let side = self.side(offence);
        const BASE: i32 = 28;
        const STEP: i32 = (512 - BASE * 2) / 6;
        let mut sectors = [-1i32; 6];
        // Game+0x3ec's order.
        let order = self.by_rim();
        for &p in order.iter().take(6) {
            let a = self.hoop_angle(p);
            let a = if 0 <= side { a } else { 0x400 - a };
            let mut best = (0x400, 0);
            for (k, s) in sectors.iter().enumerate() {
                if *s == -1 {
                    let mid = BASE + STEP / 2 + k as i32 * STEP;
                    let d = court::turn(mid, a).abs();
                    if d < best.0 {
                        best = (d, k);
                    }
                }
            }
            sectors[best.1] = p as i32;
        }
        let (base, span) = if self.game.long_shot {
            (5.0, 40)
        } else {
            (3.0, 35)
        };
        for (k, &p) in sectors.iter().enumerate() {
            let r = self.rand();
            let mut a = BASE + k as i32 * STEP + (r % STEP as u32) as i32;
            let r = self.rand();
            if side < 0 {
                a = 0x400 - a;
            }
            let out = base + (r % span) as f32 / 10.0;
            let d = court::direction(a);
            if let Ok(p) = usize::try_from(p)
                && p < 6
            {
                self.game.spots[p] = [rim[0] + out * d[0], rim[1] + out * d[1]];
            }
        }
    }
}

/// One update's AI frame across the actors' updates ([`World::ai_begin`],
/// [`World::ai_player`]): who the computer drives, the people's actors
/// off their pads, a person on each side (game+0x9a), whether the six's
/// restart walks everyone, and the update's inputs.
pub(crate) struct Frame {
    computer: Vec<bool>,
    off: Vec<bool>,
    human: [bool; 2],
    restarting: bool,
    pub(crate) inputs: Vec<Input>,
}

impl Frame {
    /// The AI's view of `w` with the brains' words as they stand.
    fn ctx<'a>(&'a self, w: &'a World, rng: &'a mut Rng, state: &State) -> Ctx<'a> {
        Ctx {
            w,
            rng,
            game: state.game,
            human: self.human,
            computer: &self.computer,
            inputs: &self.inputs,
            tick: w.tick as i64,
            matchups: [0, 1].map(|t| {
                let d = &state.coaches[t].defense;
                (d.man, d.help, d.second)
            }),
            save_turbo: [state.coaches[0].save_turbo, state.coaches[1].save_turbo],
            open: [state.coaches[0].open, state.coaches[1].open],
            holder_of: [state.coaches[0].holder, state.coaches[1].holder],
            pads: state.brains.iter().map(|b| b.pad.buttons).collect(),
            fronts: state.brains.iter().map(Brain::front_kind).collect(),
            targets: state.brains.iter().map(Brain::front_target).collect(),
            pick_sides: state.brains.iter().map(|b| b.pick_side).collect(),
            screened: state.brains.iter().map(|b| b.screened_by).collect(),
        }
    }
}

/// What the AI reads of the AIGame (`FUN_0013d868`'s counters): the
/// game's phase, the side the held phases belong to and the loose-ball
/// count (game+0xf4, +0x110, +0x440) as the game's update left them after
/// the last update ([`World::game_phase`]), and the ball as the original's
/// state (ball+0x180, [`World::native_ball_state`]).
fn track(game: &mut Game, w: &World, computer: &[bool]) {
    game.phase = w.game_phase.phase;
    game.offence = w.game_phase.possession;
    game.loose = w.game_phase.loose;
    let ball = w.native_ball_state();
    if ball == game.ball {
        game.ball_age += 1;
    } else {
        game.ball_age = 0;
    }
    let passed = game.ball == 4;
    game.ball = ball;
    let holder = w.handler().map_or(-1, |h| h as i32);
    if holder >= 0 && holder != game.holder {
        game.caught = if passed { holder + 1 } else { 0 };
    }
    if holder >= 0 && holder == game.holder {
        game.held += 1;
    } else {
        game.held = 0;
    }
    game.holder = holder;
    if let Some(h) = w.handler() {
        game.last_team = w.players[h].team.min(1) as i32;
    }
    // game+0x470 (`0x13d968`): a person of the side without the ball in
    // the paint; the side with it starts over.
    if game.phase != 3 {
        for (i, p) in w.players.iter().enumerate().take(6) {
            let team = p.team.min(1) as i32;
            if team == w.possession.min(1) as i32 {
                game.paint[i] = 0;
            } else if !computer.get(i).copied().unwrap_or(false) {
                let rim = w.rim_spot(crate::target_hoop(w.possession.min(1)));
                let d = crate::distance(p.position, rim);
                if d < 10.0 && p.position[1].abs() < 7.0 {
                    game.paint[i] += 1;
                }
            }
        }
    }
    // game+0xfc: set when the other side pockets.
    for team in 0..2 {
        let them = w.teams[1 - team].pocketed;
        if them && !game.pocketed[1 - team] {
            game.cancel[team] = true;
        }
    }
    game.pocketed = [w.teams[0].pocketed, w.teams[1].pocketed];
}

/// `FUN_0013a970`: one computer player's frame. The pad's buttons start
/// empty (its stick keeps last frame's setting); with nothing queued the
/// player picks a behaviour; then the front behaviour runs, a finished
/// one giving way to the next in the same frame.
fn run_player(ctx: &mut Ctx, b: &mut Brain, i: usize) {
    b.pad.buttons = 0;
    b.pad.target = -1;
    if b.ran != ctx.tick - 1 {
        b.prev = 0;
    }
    b.ran = ctx.tick;
    if b.queue.is_empty() {
        team::decide(ctx, b, i);
    }
    b.updating = true;
    // `FUN_00133030`: retire a finished front node and start the next.
    // A done test may queue behaviours behind the node (the cuts' do), so
    // the node is held out of the queue while it runs and put back first.
    if b.queue.first().is_some_and(|n| n.started) {
        let mut node = b.queue.remove(0);
        if node.act.done(ctx, b, i, node.age) {
            node.act.on_done(ctx, b, i);
        } else {
            b.queue.insert(0, node);
        }
    }
    if !b.queue.is_empty() {
        let mut node = b.queue.remove(0);
        if !node.started {
            node.started = true;
            node.age = 0;
            node.act.enter(ctx, b, i);
        }
        if node.act.done(ctx, b, i, node.age) {
            node.act.on_done(ctx, b, i);
        } else {
            node.age = node.age.saturating_add(1);
            b.age = node.age;
            node.act.update(ctx, b, i);
            b.age = 0;
        }
        b.queue.insert(0, node);
    }
    b.updating = false;
    b.prev = b.pad.buttons;
}

impl World {
    /// The AI's frame up to the players' (`FUN_0013d868`'s counters, then
    /// the teams' strategies, the AITeams ahead of the AIPhysPlayers in the
    /// process registry), with the hand-over a team's restart start asked
    /// for. Each computer player's own frame then runs in his actor's update
    /// ([`World::ai_player`]). During a six-player restart the computer
    /// drives everyone, people too (`FUN_001406b8` sets every actor+0x298
    /// through `FUN_00140880`; `FUN_001587d8` then reads the AI's pad),
    /// while the strategies still see who is a person. A person's actor
    /// `off` ([`World::pad_off`]: a pass's receiver, or outside the court's
    /// rectangle) reads his AI's frame too. `None` when nobody is the
    /// computer's: the AI then neither runs nor draws.
    pub(crate) fn ai_begin(&mut self, inputs: &[Input], off: &[bool]) -> Option<Frame> {
        let n = self.players.len();
        let computer: Vec<bool> = (0..n)
            .map(|i| inputs.get(i).is_some_and(Input::is_computer))
            .collect();
        let off: Vec<bool> = (0..n)
            .map(|i| off.get(i).copied().unwrap_or(false))
            .collect();
        let restarting = self.restarting();
        if !restarting && !computer.iter().any(|&c| c) && !off.iter().any(|&o| o) {
            return None;
        }
        let mut human = [false; 2];
        for (i, p) in self.players.iter().enumerate() {
            if !computer[i] {
                human[p.team.min(1)] = true;
            }
        }
        let frame = Frame {
            computer,
            off,
            human,
            restarting,
            inputs: (0..n)
                .map(|i| inputs.get(i).copied().unwrap_or_default())
                .collect(),
        };
        let mut state = std::mem::take(&mut self.ai);
        if state.brains.len() != n {
            state.brains = vec![Brain::default(); n];
        }
        track(&mut state.game, self, &frame.computer);
        let mut rng = self.rng.clone();
        {
            let mut ctx = frame.ctx(self, &mut rng, &state);
            for team in 0..2 {
                team::update(&mut ctx, &mut state.coaches, &mut state.brains, team);
            }
            state.game = ctx.game;
        }
        self.rng = rng;
        // The hand-over the possessing team's restart start asked for
        // (`FUN_0014cb28` in `FUN_0012bc40`).
        let hand = std::mem::take(&mut state.game.restart.hand);
        self.ai = state;
        if let Some(to) = usize::try_from(hand - 1).ok().filter(|&to| to < n) {
            self.ball.drop_loose();
            self.take_ball(to);
        }
        Some(frame)
    }

    /// `FUN_001587d8`'s call of `FUN_0013a970`: player `i`'s AI frame at the
    /// start of his actor's update, reading the others as their updates
    /// left them (this update's for those updated before him), and his pad
    /// as the computer builds it; `None` when he is a person's. What his
    /// frame asked of the game follows at once: a trick pass
    /// (`FUN_00183318` → `FUN_00142068`, thrown off his defender's head or
    /// off the board) and a ball his "loose ball" behaviour found free for
    /// ten seconds.
    /// The call's node for holder `h` from `caller` ([`World::call_for_ball`]):
    /// the queue cleared and the node built (its draws) and pushed now.
    pub(crate) fn push_call(&mut self, frame: &Frame, h: usize, caller: usize, call: Call) {
        if h >= self.ai.brains.len() {
            return;
        }
        let mut state = std::mem::take(&mut self.ai);
        let mut rng = self.rng.clone();
        {
            let mut ctx = frame.ctx(self, &mut rng, &state);
            let b = &mut state.brains[h];
            b.clear(&ctx, h);
            let act = match call {
                Call::Pass => Act::Pass(offence::Pass::new(&mut ctx, h, Some(caller))),
                Call::TrickPass => Act::TrickPass(offence::TrickPass::new_out(&mut ctx)),
                Call::Shoot => Act::Shoot(offence::Shoot::new(&mut ctx, h)),
            };
            b.push(act);
            state.game = ctx.game;
        }
        self.rng = rng;
        self.ai = state;
    }

    pub(crate) fn ai_player(&mut self, frame: &Frame, i: usize) -> Option<Input> {
        if !(frame.computer[i] || frame.restarting || frame.off[i]) {
            return None;
        }
        let n = self.players.len();
        let mut state = std::mem::take(&mut self.ai);
        let mut rng = self.rng.clone();
        let mut out = {
            let mut ctx = frame.ctx(self, &mut rng, &state);
            run_player(&mut ctx, &mut state.brains[i], i);
            let out = pad::to_input(&ctx, i, &state.brains[i].pad);
            state.game = ctx.game;
            out
        };
        if frame.computer[i] {
            // The pad stays the computer's: what asks whose controller
            // drives the actor (`FUN_00172220`, the controller 4) still
            // finds the computer, as the shot's release gate and the pass
            // rows do. A person the restart walks keeps his own.
            out.buttons |= crate::button::COMPUTER;
        }
        self.rng = rng;
        let trick = std::mem::take(&mut state.game.trick);
        let off = std::mem::take(&mut state.game.trick_off);
        let point = std::mem::take(&mut state.game.trick_point);
        let defender = usize::try_from(off - 1).ok().filter(|&d| d < n);
        let free = std::mem::take(&mut state.game.free_take);
        let walk = std::mem::take(&mut state.game.inbound_walk);
        let pick = std::mem::take(&mut state.game.pick_call);
        self.ai = state;
        if let Some(p) = usize::try_from(walk - 1).ok().filter(|&p| p < n) {
            self.events.push(crate::Event::InboundWalk { player: p });
        }
        if let Some(p) = usize::try_from(pick - 1).ok().filter(|&p| p < n) {
            self.events.push(crate::Event::PickCall { screener: p });
        }
        if let Some(to) = usize::try_from(free - 1).ok().filter(|&to| to < n)
            && !matches!(self.ball.state, crate::BallState::Held(_))
        {
            self.take_ball(to);
        }
        if let Some(t) = usize::try_from(trick - 1).ok().filter(|&t| t < n) {
            let receiver = usize::try_from(self.ai.brains[t].pad.target)
                .ok()
                .filter(|&to| to < n)
                .unwrap_or(t);
            self.computer_trick_pass(t, receiver, defender, point);
        }
        Some(out)
    }

    /// The whole AI frame at once ([`World::ai_begin`], then every
    /// player's from the highest index down, the actors' order): `inputs`
    /// with the computer's pads in place.
    #[cfg(test)]
    pub(crate) fn computer_inputs(&mut self, inputs: &[Input], off: &[bool]) -> Option<Vec<Input>> {
        let frame = self.ai_begin(inputs, off)?;
        let mut out = frame.inputs.clone();
        for i in (0..out.len()).rev() {
            if let Some(input) = self.ai_player(&frame, i) {
                out[i] = input;
            }
        }
        Some(out)
    }

    /// A restart is under way with the original's six (game phase 6).
    pub fn restarting(&self) -> bool {
        matches!(self.phase, Phase::DeadBall { .. })
            && self.players.len() == 6
            && !self.timed_dead_ball
    }

    /// The restart's play: the row of the spot tables and the team slot at
    /// each spot ([`crate::restart::Plan`]).
    pub fn restart_plan(&self) -> crate::restart::Plan {
        self.ai.game.restart.plan()
    }

    /// `FUN_00140840`: all six have marked themselves ready for the inbound
    /// (game+0x404..0x418) and the restart's hold after a gamebreaker's
    /// basket (game+0x104, [`crate::game_phase::GAMEBREAKER_HOLD`]) is over.
    pub fn restart_ready(&self) -> bool {
        self.game_phase.hold == 0 && self.ai.game.restart.all_ready()
    }

    /// `FUN_001406b8`'s part of a restart that is the AI's: every ready
    /// flag down.
    pub(crate) fn lower_ready_flags(&mut self) {
        self.ai.game.restart.ready = [false; 6];
    }

    /// `FUN_0018adf8` for both teams (`FUN_001407c8`): each team's restart
    /// strategy becomes its strategy and starts (the possessing team's
    /// `FUN_0012bc40` choosing its play from where everyone stands now,
    /// the defence's `FUN_0012c520`). Returns the play chosen.
    pub(crate) fn start_restart_strategies(&mut self, possessing: usize) -> crate::restart::Plan {
        let n = self.players.len();
        let mut state = std::mem::take(&mut self.ai);
        if state.brains.len() != n {
            state.brains = vec![Brain::default(); n];
        }
        // Who is a person: the slots the computer did not drive last
        // frame (team+0x88).
        let mut computer = vec![false; n];
        for team in 0..2 {
            let squad: Vec<usize> = (0..n)
                .filter(|&i| self.players[i].team.min(1) == team)
                .collect();
            for (k, &j) in squad.iter().enumerate().take(3) {
                computer[j] = state.coaches[team].computer[k];
            }
        }
        let mut human = [false; 2];
        for (i, p) in self.players.iter().enumerate() {
            if !computer[i] {
                human[p.team.min(1)] = true;
            }
        }
        let mut rng = self.rng.clone();
        {
            let mut ctx = Ctx {
                w: self,
                rng: &mut rng,
                game: state.game,
                human,
                computer: &computer,
                inputs: &[],
                tick: self.tick as i64,
                matchups: [([-1; 3], [-1; 3], [-1; 3]); 2],
                save_turbo: [state.coaches[0].save_turbo, state.coaches[1].save_turbo],
                open: [state.coaches[0].open, state.coaches[1].open],
                holder_of: [state.coaches[0].holder, state.coaches[1].holder],
                pads: state.brains.iter().map(|b| b.pad.buttons).collect(),
                fronts: state.brains.iter().map(Brain::front_kind).collect(),
                targets: state.brains.iter().map(Brain::front_target).collect(),
                pick_sides: state.brains.iter().map(|b| b.pick_side).collect(),
                screened: state.brains.iter().map(|b| b.screened_by).collect(),
            };
            for team in [possessing, 1 - possessing] {
                if team == possessing {
                    state.coaches[team].strategy = team::strategy::INBOUND;
                    inbound::start_offence(&mut ctx, &mut state.brains, team);
                } else {
                    state.coaches[team].strategy = team::strategy::INBOUND_D;
                    inbound::start_defence(&mut ctx, &mut state.brains, team);
                }
            }
            state.game = ctx.game;
        }
        self.rng = rng;
        // The snap hands the ball over itself.
        state.game.restart.hand = 0;
        let plan = state.game.restart.plan();
        self.ai = state;
        plan
    }
}

/// `FUN_0017f4d0(ai, 0)` for player `i` outside the AI's own frame (the
/// person's pass target asks it of each teammate,
/// [`World::lined_up`]): his lane to the rim the offence attacks is open
/// ([`offence::path_open`]).
pub(crate) fn lane_open(w: &World, i: usize) -> bool {
    let mut rng = w.rng.clone();
    let n = w.players.len();
    let computer = vec![true; n];
    let inputs = vec![Input::default(); n];
    let ctx = Ctx {
        w,
        rng: &mut rng,
        game: Game::default(),
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
    offence::path_open(&ctx, i)
}

#[cfg(test)]
/// A context for calling the AI's pieces directly, as the step does
/// (the tests' and the capture comparisons').
pub(crate) fn with_ctx<R>(w: &World, f: impl FnOnce(&mut Ctx) -> R) -> (R, Rng) {
    let mut rng = w.rng.clone();
    let n = w.players.len();
    let computer = vec![true; n];
    let inputs = vec![Input::computer(); n];
    let mut game = Game::default();
    track(&mut game, w, &computer);
    let r = {
        let mut ctx = Ctx {
            w,
            rng: &mut rng,
            game,
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

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod capture;
