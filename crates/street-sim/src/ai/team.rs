//! The team side of the AI (`AITeam`, `FUN_00189280`, updated by
//! `FUN_0018a708`): each team runs the strategy its list holds for the
//! game's phase, the offence's list or the defence's, starting it when it
//! changes. And a player with nothing queued picks a behaviour by the
//! phase and his role (`FUN_0017e5c8`).

use super::defence::{Defense, DefenseCoach};
use super::loose::{LooseBall, Scramble};
use super::offence::CarrierPause;
use super::playbook::Playbook;
use super::strategy::{FastBreakCoach, ReboundCoach};
use super::words::words;
use super::{Act, Brain, Ctx};

/// The strategies, by the original's list entries (`AITeam+0x70` while
/// the side has the ball, `+0x7c` while it defends, indexed by the game
/// phase; built by `FUN_00189280`).
pub(super) mod strategy {
    pub const NONE: i32 = 0;
    /// Phase 1, both sides: `FUN_0012e3b8` (vtable `0x43b418`).
    pub const TIP: i32 = 1;
    /// Phase 2, both sides: `0x43b1c8` (`FUN_0012cb40`).
    pub const SCRAMBLE: i32 = 2;
    /// Phase 3 offence: `FUN_0012db58` (vtable `0x43b308`).
    pub const REBOUND: i32 = 3;
    /// Phase 3 defence: `FUN_0012de28` (vtable `0x43b2b8`).
    pub const REBOUND_D: i32 = 13;
    /// Phase 4 offence: `0x43abc8`.
    pub const FAST_BREAK: i32 = 4;
    /// Phase 5 offence: the playbook (`FUN_00126f60`, `FUN_00124028` in
    /// modes 1, 2 and 6).
    pub const PLAYBOOK: i32 = 5;
    /// Phases 4 and 5 defence: `FUN_00122e98` (one object for both).
    pub const DEFENSE: i32 = 10;
    /// Phase 6 offence: `FUN_0012bc08` (vtable `0x43b0e0`).
    pub const INBOUND: i32 = 6;
    /// Phase 6 defence: `FUN_0012c4e8` (vtable `0x43b088`).
    pub const INBOUND_D: i32 = 16;
    /// Phase 7: `FUN_00126e80` / `FUN_00126ef0`, whose start and update
    /// are empty (`0x126ee0`, `0x126f50`).
    pub const IDLE: i32 = 7;
    /// Phase 8: `0x43aad8` (`FUN_001258c8`: everyone's queue cleared).
    pub const CLEAR: i32 = 8;
    /// Phase 9: `0x43aa58` (`FUN_00125460`).
    pub const LEAVE: i32 = 9;
}

words! {
    pub(crate) struct Coach {
        /// team+0x5c: the strategy running.
        strategy: i32,
        /// team+0xa0: the side's last ball holder (an actor index; 0 or 3
        /// at the start, `FUN_00189b10`).
        holder: i32,
        /// team+0x94: two a frame each slot stands open (openness over 4),
        /// to `fps × 4` and over again (`0x18a8bc`).
        open: [i32; 3],
        /// team+0xa4: hold turbo back.
        save_turbo: bool,
        /// team+0xb4: the strategy button held last frame (`FUN_0018ac98`).
        button: bool,
        /// team+0xb8: frames to the next draw (`0x18aa4c`).
        timer: i32,
        /// team+0x88: the slots the computer drove last frame.
        computer: [bool; 3],
        defense: DefenseCoach,
        scramble: Scramble,
        rebound: ReboundCoach,
        fast_break: FastBreakCoach,
        playbook: Playbook,
    }
}

impl Coach {
    pub fn new(team: usize) -> Self {
        Self {
            holder: if team == 0 { 0 } else { 3 },
            defense: DefenseCoach::new(),
            playbook: Playbook::new(),
            ..Default::default()
        }
    }

    /// team+0x88 as the last update left it (a world set up from a
    /// capture has run none, nor has the School's game before its first
    /// lesson, [`crate::World::launch_lesson`]).
    pub(crate) fn set_computer(&mut self, computer: [bool; 3]) {
        self.computer = computer;
    }

    /// For [`super::State::validate`]: every player it names is one.
    pub fn valid(&self, players: usize) -> bool {
        let player = |v: i32| v == -1 || (0 <= v && (v as usize) < players);
        let d = &self.defense;
        d.man
            .iter()
            .chain(&d.help)
            .chain(&d.second)
            .copied()
            .all(player)
            && d.named().into_iter().all(player)
            && self.playbook.named().into_iter().all(player)
            && (-1..16).contains(&self.playbook.forced)
            && (-1..16).contains(&self.playbook.play.kind)
            && (0..=5).contains(&self.holder)
    }
}

/// `0x18a8f8`: list `[team != offence]` indexed by the phase.
fn pick(phase: i32, defending: bool) -> i32 {
    use strategy::*;
    match (phase, defending) {
        (1, _) => TIP,
        (2, _) => SCRAMBLE,
        (3, false) => REBOUND,
        (3, true) => REBOUND_D,
        (4, false) => FAST_BREAK,
        (5, false) => PLAYBOOK,
        (4 | 5, true) => DEFENSE,
        (6, false) => INBOUND,
        (6, true) => INBOUND_D,
        (7, _) => IDLE,
        (8, _) => CLEAR,
        (9, _) => LEAVE,
        _ => NONE,
    }
}

/// `FUN_0018a708` for one team.
pub(super) fn update(ctx: &mut Ctx, coaches: &mut [Coach; 2], brains: &mut [Brain], team: usize) {
    let squad = ctx.squad(team);
    let coach = &mut coaches[team];
    // `0x18a744..0x18a7d8`: a player the computer takes over drops what
    // he was doing (`FUN_0013aa48`), unless forced computer control is
    // already on for the opening toss or a restart (actor+0x298).
    for (k, &j) in squad.iter().enumerate().take(3) {
        let now = !ctx.human_player(j);
        if now != coach.computer[k] {
            coach.computer[k] = now;
            if now && !ctx.frozen() {
                brains[j].clear(ctx, j);
            }
        }
    }
    // `0x18a7f0`: the side's ball holder.
    if let Some(h) = ctx.w.handler().filter(|&h| ctx.team(h) == team) {
        coach.holder = h as i32;
    }
    // `0x18a85c..0x18a8f0`: the open counters, on offence; on defence
    // they clear.
    if ctx.attacking(team) {
        for (k, &j) in squad.iter().enumerate().take(3) {
            if super::offence::openness(ctx, j) > 4 {
                if coach.open[k] < 240 {
                    coach.open[k] += 2;
                } else {
                    coach.open[k] = 0;
                }
            }
        }
    } else {
        coach.open = [0; 3];
    }
    ctx.open[team] = coach.open;
    ctx.holder_of[team] = coach.holder;
    let s = pick(ctx.game.phase, !ctx.attacking(team));
    if coach.strategy != s {
        coach.strategy = s;
        super::strategy::start(ctx, coach, brains, team);
    }
    if s == strategy::NONE {
        coach.save_turbo = false;
    } else {
        super::strategy::update(ctx, coach, brains, team);
        // `FUN_0018ac98`: a person on the side asks for the strategy
        // (vt+0x44) as the button goes down.
        if strategy_button(ctx, team) {
            if !coach.button {
                super::strategy::call(ctx, coach, brains, team);
            }
            coach.button = true;
        } else {
            coach.button = false;
        }
        // `0x18a9a0..0x18aa40`: stop sprinting below the strategy's low
        // mark (vt+0x34) and start again at its high mark (vt+0x3c).
        let (low, high) = super::strategy::turbo_marks(ctx, coach, team);
        let turbo = team_turbo(ctx, team);
        if !coach.save_turbo {
            if turbo < low {
                coach.save_turbo = true;
            }
        } else if turbo >= high {
            coach.save_turbo = false;
        }
    }
    ctx.save_turbo[team] = coach.save_turbo;
    // `0x18aa4c`: the team's timer, drawn again from `r % (fps × 0.15) +
    // fps × 0.15` when it runs out.
    if coach.timer < 1 {
        let r = ctx.rand();
        coach.timer = (r % 9) as i32 + 9;
    } else {
        coach.timer -= 1;
    }
}

/// The turbo the save test reads: `**(game + 0xb0 + 4 × {3, 0}[team])`,
/// the side's computer bar ([`crate::turbo::COMPUTER_SLOT`]).
fn team_turbo(ctx: &Ctx, team: usize) -> f32 {
    ctx.w.turbo[crate::turbo::COMPUTER_SLOT[team.min(1)]]
}

/// `FUN_0018ac98`: the first person on the side holds the strategy
/// button (0x2000000, R3), or, while the ball is in a shot, a dunk, a
/// lay-up or an alley-oop leap, the kick button (0x800, L3).
fn strategy_button(ctx: &Ctx, team: usize) -> bool {
    let Some(j) = ctx.squad(team).into_iter().find(|&j| ctx.human_player(j)) else {
        return false;
    };
    let input = ctx.input(j);
    if input.held(crate::button::R3) {
        return true;
    }
    let shooting = ctx
        .w
        .handler()
        .is_some_and(|h| ctx.shooting(h) || ctx.laying_up(h) || ctx.leaping(h) || ctx.dunking(h));
    shooting && input.held(crate::button::L3)
}

/// `FUN_0017e5c8`: a behaviour for a player with none, by the phase.
pub(super) fn decide(ctx: &mut Ctx, b: &mut Brain, i: usize) {
    let team = ctx.team(i);
    let defending = !ctx.attacking(team);
    match ctx.game.phase {
        3 => {
            if ctx.game.ball == 0 {
                b.push(Act::LooseBall(LooseBall::new(1)));
            } else {
                // "prepare rebound" at the player's spot in `0x3cf620`.
                let spot = ctx.rebound_spot(i);
                let r = super::loose::PrepareRebound::new(ctx, spot, 2);
                b.push(Act::PrepareRebound(r));
            }
        }
        4 if defending => b.push(Act::Defense(Defense::new(ctx, i))),
        4 => b.push(Act::FastBreak(super::fastbreak::FastBreak::new())),
        5 if defending => b.push(Act::Defense(Defense::new(ctx, i))),
        5 => match b.role {
            1 => b.push(Act::CarrierPause(CarrierPause::new(0.0, 9.0, true))),
            2 => b.push(Act::PostPlayer(super::spots::PostPlayer::default())),
            3 => b.push(Act::PerimPlayer(super::spots::PerimPlayer::new(false))),
            _ => {}
        },
        _ => b.stop(ctx, i),
    }
}
