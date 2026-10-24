//! The restart strategies' spots: where the original stands the six
//! players for a restart (game phase 6) and where its snap after a replay
//! puts them ([`crate::World::snap_formation`]).
//!
//! The possessing team's restart strategy (vtable `0x43b0e0`) chooses its
//! play in its initializer `FUN_0012bc40`: the inbounder, the row of the
//! spot tables and who takes the other two spots (`DAT_003ce9b8` and
//! `DAT_004e7408..0x4e7410`, shared with the defending team's strategy,
//! vtable `0x43b088`). `FUN_0012b8b8` reads a spot, `FUN_0012c1c0` and
//! `FUN_0012c6a8` the point each player faces, and `FUN_0018adf8` places a
//! team (`FUN_0012bb50`, `FUN_001704e0`).
//!
//! The walk to these spots after a basket ("throw inbound" and "inbound
//! pos", which `FUN_0012bc40` and `FUN_0012c578` queue) is the computer
//! players' (`ai::inbound`).

use crate::{distance, ordinary};

/// The possessing team's spots, `0x3cea20`: six rows of three spots, each
/// `{x, z, radius}` with x in the possessing team's frame (times minus
/// its side). Rows 0..2 lie in the half it defends, 3..5 in the other.
/// The third word is the distance within which "inbound pos" counts its
/// player ready (`FUN_0012bad8`, when over 2).
pub const OFFENSE: [[[f32; 3]; 3]; 6] = [
    [[47.0, 16.0, 1.0], [35.0, 8.0, 1.0], [32.0, -10.0, 15.0]],
    [[47.0, -16.0, 1.0], [35.0, -8.0, 1.0], [32.0, 10.0, 15.0]],
    [[12.0, 31.5, 1.0], [20.0, 19.5, 1.0], [13.0, 0.0, 20.0]],
    [[-47.0, 16.0, 1.0], [-38.0, 24.0, 1.0], [-18.25, 0.0, 20.0]],
    [
        [-47.0, -16.0, 1.0],
        [-38.0, -24.0, 1.0],
        [-18.25, 0.0, 20.0],
    ],
    [[-12.0, 31.5, 1.0], [-6.0, 19.5, 1.0], [-13.0, 0.0, 20.0]],
];

/// The defending team's spots, `0x3ceaf8`, by the same rows (x times the
/// defending team's side).
pub const DEFENSE: [[[f32; 3]; 3]; 6] = [
    [[12.0, 10.0, 20.0], [22.0, 12.0, 1.0], [-13.0, -10.0, 30.0]],
    [[12.0, -10.0, 20.0], [22.0, -12.0, 1.0], [-13.0, 10.0, 30.0]],
    [[2.0, 19.5, 20.0], [6.0, 12.5, 1.0], [-7.0, 0.0, 30.0]],
    [[-37.0, 8.0, 20.0], [-30.0, 16.0, 1.0], [-26.25, 0.0, 30.0]],
    [
        [-37.0, -8.0, 20.0],
        [-30.0, -16.0, 1.0],
        [-26.25, 0.0, 30.0],
    ],
    [[-24.0, 19.5, 20.0], [-18.0, 12.5, 1.0], [-23.0, 0.0, 30.0]],
];

/// A native 10-bit heading (`FUN_0018ce78`, from the spot toward the
/// point faced, the model's -z turned by it) in the simulation's
/// convention (`Player::heading`, +z at 0): half a turn apart.
pub fn heading_radians(heading: i32) -> f32 {
    let a = (heading & 0x3ff) as f32 * (std::f32::consts::TAU / 1024.0) + std::f32::consts::PI;
    if a >= std::f32::consts::PI {
        a - std::f32::consts::TAU
    } else {
        a
    }
}

/// `FUN_0018abb0`'s table at `0x3dce68`: the two slots besides each.
const OTHERS: [[usize; 2]; 3] = [[1, 2], [0, 2], [0, 1]];

/// The play `FUN_0012bc40` chooses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// The row of [`OFFENSE`] and [`DEFENSE`] (`DAT_003ce9b8`).
    pub row: usize,
    /// The team slot standing at each spot (`DAT_004e7408`, `0x4e740c`,
    /// `0x4e7410`): spot 0 is the inbounder's. The defending team's slots
    /// follow the same order.
    pub order: [usize; 3],
}

/// A team's side byte (`game + 0x98 + team`, `FUN_001404c0`): the
/// direction it attacks, +1 toward +x. The bytes start at +1 for team 0
/// and -1 for team 1 (as every capture of the original has them), so
/// team 0 attacks the +x basket (hoop 0, [`crate::Hoop::native`]); the
/// original's formulas read the side only as that direction
/// (`FUN_001404e0`: the rim a side attacks is at side × 36.75).
pub fn side(team: usize) -> f32 {
    if team == 0 { 1.0 } else { -1.0 }
}

/// What `FUN_0012bc40` reads to choose the possessing team's play.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    /// The possessing team (game+0x110).
    pub team: usize,
    /// Its players' places by team slot.
    pub positions: [[f32; 2]; 3],
    /// The ball's carrier, else the ball (`ball+0xf4`, `+0x30`).
    pub ball: [f32; 2],
    /// A carrier on the team, by slot (`FUN_0018c018`).
    pub holder: Option<usize>,
    /// The play as it stands from the last restart (the globals are
    /// shared and never cleared; zero at boot).
    pub previous: Plan,
    /// The shot clock is full (`clock[0] == clock[1]`, game+0x190):
    /// every row is a candidate; otherwise rows 0 and 1 are skipped.
    pub fresh_clock: bool,
    /// People playing the side (game+0x9a+team, `FUN_0018ab00`) and which
    /// of its slots a person controls (`FUN_00156d98` not 4).
    pub people: usize,
    pub human: [bool; 3],
    /// Its players' heights in feet by slot (`FUN_001819a8`).
    pub heights: [f32; 3],
    /// A lesson's game (game+0x9c 4): the row is 2 whatever the ball.
    pub lesson: bool,
}

/// `FUN_0012bc40`'s choice. A carrier on the team is the inbounder;
/// otherwise the player nearest the ball (within 100; slot 2 if none).
/// The row is the one whose first spot lies nearest the ball in the
/// team's frame, over all six rows (`FUN_00143068` answers 1 for every
/// game class) on a full shot clock, else rows 2..5. The second spot goes
/// to the nearer of the others to the first spot when fewer than two
/// people play the side (that spot pushed past the line by the height of
/// the player who stood there in the last restart, `DAT_004e7408` not yet
/// rewritten), else to the first other slot a person controls (slot 1 if
/// none); the third to the last. In a lesson (game+0x9c 4) the row is 2
/// before the second spot is chosen (raw `0x12be00..0x12be18`).
pub fn plan(f: &Facts) -> Plan {
    let inbounder = f.holder.unwrap_or_else(|| {
        let (mut best, mut near) = (2, 100.0_f32);
        for (slot, &p) in f.positions.iter().enumerate() {
            let d = distance(p, f.ball);
            if d < near {
                near = d;
                best = slot;
            }
        }
        best
    });
    let x = -side(f.team) * f.ball[0];
    let z = f.ball[1];
    let (mut row, mut near) = (f.previous.row, 10_000.0_f32);
    for (r, spots) in OFFENSE.iter().enumerate() {
        if !f.fresh_clock && r < 2 {
            continue;
        }
        let (dx, dz) = (x - spots[0][0], z - spots[0][1]);
        let d = dx * dx + dz * dz;
        if d < near {
            near = d;
            row = r;
        }
    }
    if f.lesson {
        row = 2;
    }
    let second = if f.people < 2 {
        let first = table_spot(
            row,
            f.team,
            f.team,
            0,
            f.heights[f.previous.order[0].min(2)],
        );
        let (mut second, mut near) = (1, 100.0_f32);
        for (slot, &p) in f.positions.iter().enumerate() {
            if slot == inbounder {
                continue;
            }
            let d = distance(p, first);
            if d < near {
                near = d;
                second = slot;
            }
        }
        second
    } else {
        (0..3).find(|&k| k != inbounder && f.human[k]).unwrap_or(1)
    };
    let [a, b] = OTHERS[inbounder];
    let third = if a == second { b } else { a };
    Plan {
        row,
        order: [inbounder, second, third],
    }
}

/// `FUN_0012b8b8`: spot `k` of `table_team`'s table at `row`, in the
/// court's frame, pushed out past the end lines (|x| > 42) or the
/// sidelines (|z| > 26.5) by a quarter of a foot for each foot over four
/// (to eight) of `height` (`FUN_001819a8`: the record's inches over 12).
pub fn table_spot(
    row: usize,
    possessing: usize,
    table_team: usize,
    k: usize,
    height: f32,
) -> [f32; 2] {
    let (spots, sign) = if table_team == possessing {
        (&OFFENSE, -side(table_team))
    } else {
        (&DEFENSE, side(table_team))
    };
    let spot = spots[row.min(5)][k.min(2)];
    let mut x = sign * spot[0];
    let mut z = spot[1];
    let push = (height.clamp(4.0, 8.0) - 4.0) * 0.25;
    if x.abs() > 42.0 {
        x += push.copysign(x);
    } else if z.abs() > 26.5 {
        z += push.copysign(z);
    }
    [x, z]
}

/// Spot `k` of `team`'s table and what it faces, with the heights that
/// push each spot read from `team`'s players in `pushers` (`DAT_004e7408`
/// as it stands when the spot is read: `FUN_0018bf58` takes the
/// strategy's own player in that slot even for the other team's spots).
/// The possessing team's first spot faces the second and the others the
/// first (`FUN_0012c1c0`); a defender faces the middle of the possessing
/// team's first spot and its own spot `k` there (`FUN_0012c6a8`).
/// `heights` are each team's players' heights in feet by slot.
pub fn spot_at(
    row: usize,
    pushers: [usize; 3],
    possessing: usize,
    team: usize,
    k: usize,
    heights: &[[f32; 3]; 2],
) -> ordinary::FormationSpot {
    let own = |k: usize| heights[team][pushers[k].min(2)];
    let position = table_spot(row, possessing, team, k, own(k));
    let target = if team == possessing {
        let j = usize::from(k == 0);
        table_spot(row, possessing, team, j, own(j))
    } else {
        let a = table_spot(row, possessing, possessing, 0, own(0));
        let b = table_spot(row, possessing, possessing, k, own(k));
        [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
    };
    let position = [position[0], 0.0, position[1], 1.0];
    let target = [target[0], 0.0, target[1], 1.0];
    let heading = ordinary::native_heading(
        [position[0], position[1], position[2]],
        [target[0], target[1], target[2]],
    );
    ordinary::FormationSpot {
        position,
        target,
        heading,
    }
}

/// Where `slot` of `team` stands and what it faces once the plan is
/// written (`FUN_0012bb50`): spot `k`, its place in the plan's order.
pub fn spot(
    plan: &Plan,
    possessing: usize,
    team: usize,
    slot: usize,
    heights: &[[f32; 3]; 2],
) -> ordinary::FormationSpot {
    let k = plan.order.iter().position(|&s| s == slot).unwrap_or(slot);
    spot_at(plan.row, plan.order, possessing, team, k, heights)
}

/// The radius within which "inbound pos" counts the player at spot `k`
/// of `team`'s table ready (`FUN_0012bad8`: the third word of the row).
pub fn radius(row: usize, possessing: usize, team: usize, k: usize) -> f32 {
    let table = if team == possessing {
        &OFFENSE
    } else {
        &DEFENSE
    };
    table[row.min(5)][k.min(2)][2]
}
