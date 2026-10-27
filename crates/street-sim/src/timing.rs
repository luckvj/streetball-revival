//! When the original's animations make things happen. A shot, a pass and a
//! catch are not timed by stopwatches in the game: the press starts a move
//! script (`FUN_0019c2f8`), the animation VM (`FUN_00193440`) runs its
//! opcodes back to back inside one update until a blend (`8e`, its byte of
//! frames) or a wait (`8d`, its ticks) installs a count, and the ball
//! leaves, or the catch ends, at a game callback (`83 id`) that many
//! updates after the press. [`crate::timing_tables`] holds those counts for
//! every script a shot, dunk, lay-up, pass or catch can play; this module
//! picks the script the way the original picks it, as far as the sim
//! knows the inputs, and labels the rest.
//!
//! Traced raw 2026-10-15 (docs/TRACE.md "Shots, passes and catches on the
//! script clock"); measured against the two attract captures
//! (`ai::capture::shots_release_at_their_scripts_callback_21`,
//! `passes_leave_the_hand_when_the_animation_lets_go`).

use crate::{Action, Input, World, button, distance, math, player::NativeTricks};

pub use crate::timing_tables::{
    CATCH_ROWS, CLIP_TAIL, CatchRow, MOVE_ROWS, MoveRow, PASS_ROWS, PassRow, SHOT_SCRIPTS,
    ShotScript,
};

/// The shared set's shot slot scripts (`0x43d6c0`, `moves::SHOTS_SET`):
/// the id in [`SHOT_SCRIPTS`] for each two-turbo pair of
/// [`NativeTricks::shot_slot`] (`FUN_0016b4b8` reads `0x43d460` then
/// `FUN_0013a1f8(record, 2, +0x258, slot)`, which gives these to every
/// record but a created player's and the generic team's; theirs are the
/// player's [`crate::Shooter::slots`]).
pub const SHOT_SLOTS: [usize; 6] = [4, 7, 5, 6, 3, 1];
/// `jumpshot_1` (`0x44e51e`, the word at `0x3d7b20`): a woman's default
/// jump shot.
pub const FEMALE_SHOT: usize = 0;
/// `jumpshot_female_2hand_stepforward_set` (`0x44eb03`): a woman's runner.
pub const FEMALE_RUNNER: usize = 22;
/// `jumpshot_bird` (`0x44ec83`): record 172's default jump shot.
pub const BIRD_SHOT: usize = 23;
/// The legends' own hooks off a side fade's draw: records 149, 166 and 178
/// (`0x16ba2c..0x16bad8`, `0x16bb7c..0x16bc28`) play `jumpshot_hook_bishop`,
/// `jumpshot_hook_magic` and `jumpshot_hook_wilt`.
pub const LEGEND_HOOKS: [(u16, usize); 3] = [(149, 11), (166, 13), (178, 14)];
/// The record whose default jump shot is [`BIRD_SHOT`].
pub const BIRD: u16 = 172;
/// The three jump shots of `0x3d7b10` a male player draws among.
pub const DEFAULT_SHOTS: [usize; 3] = [18, 19, 20];
/// The three hooks of `0x3d9038` drawn when moving sideways.
pub const SIDE_HOOKS: [usize; 3] = [8, 9, 10];
/// `jumpshot_1_fadeaway` (`0x44f945`): the script every default jump shot
/// jumps into when its callback 17 finds the stick pushed away from the
/// basket.
pub const FADEAWAY: usize = 15;
/// `jumpshot_2_fadeaway` (`0x44f9ec`): a posting shooter's (bit 11)
/// facing away from the rim within 25 of it (`0x16b8ac..0x16b8f4`).
pub const POST_FADEAWAY: usize = 16;
/// `jumpshot_runner_1`: the running jump shot from out of the dunking
/// area.
pub const RUNNER: usize = 17;
/// `heaveshot_1` (`0x44f622`): the shot from the wrong half of the court
/// (`FUN_0016b4b8` at `0x16b878`: the shooter's x on the other side of
/// the centre line from the basket and more than 13 from it).
pub const HEAVE: usize = 31;
/// `jumpshot_gb_longrange` / `jumpshot_gb_shortrange`.
pub const GAMEBREAKER_LONG: usize = 34;
pub const GAMEBREAKER_SHORT: usize = 35;
/// The side fades: right-handed and left-handed shooter, left or right.
pub const SIDEFADE_RIGHT_R: usize = 27;
pub const SIDEFADE_LEFT_L: usize = 28;
pub const SIDEFADE_RIGHT_L: usize = 29;
pub const SIDEFADE_LEFT_R: usize = 30;

/// The shared set's eleven dunks (`0x43d5d0`, `moves::DUNKS_SET`) by
/// two-turbo slot, dunk ids into [`MOVE_ROWS`]' family 0.
pub const DUNK_SLOTS: [u8; 11] = [48, 21, 0, 35, 58, 20, 23, 2, 15, 37, 39];

/// The row the sim falls back on when no dunk row qualifies (the
/// original shoots a jump shot then): `dunk_1h_2f_spike`, whose slam at
/// 57 and end at 73 are the mode of the captured dunks.
pub const DUNK_FALLBACK_ROW: usize = 50;
/// The lay-up fallback: `layup_13`, the captured mode (44, 53).
pub const LAYUP_FALLBACK_ROW: usize = 85;
/// [`crate::Action::Dunking`]'s row for a move no row of [`MOVE_ROWS`]
/// scripts (the putback, the sim's own).
pub const NO_MOVE_ROW: u8 = u8::MAX;

/// The two-turbo alley-oop leaps (`0x3d70d0`, `FUN_0015a060` raw
/// `0x15b2a0..0x15b2dc`: `rand() & 1`, dunk id 24 at actor+0x1e8, the move
/// kind +0x270 1), rows 99 and 100 of [`crate::Action::Dunking`] past
/// [`MOVE_ROWS`] ([`move_row`]); their slam (callback 21) and last opcode
/// in the script's ticks (the press update its tick 0), as the rows'
/// (`tests/script_timing_disc.rs`).
pub const LEAP_ROWS: [MoveRow; 2] = [
    MoveRow {
        name: "SUPERDUNK_360_TOMMAHAWK",
        script: 0x4479b7,
        legend: -1,
        mask: 0x100000,
        family: 0,
        id: 24,
        release: 94,
        end: 109,
    },
    MoveRow {
        name: "SUPERDUNK_DINNERSERVED_360_WINDMILL",
        script: 0x447a1e,
        legend: -1,
        mask: 0x100000,
        family: 0,
        id: 24,
        release: 92,
        end: 107,
    },
];

/// The lay-ups past [`LEAP_ROWS`] that no table row scripts: first
/// `layup_3_bboard` (`0x446416`), the lay-up from behind the board
/// `FUN_0016b4b8` starts (lay-up kind 5 at +0x26c, `0x16c2a0`,
/// [`BOARD_LAYUP_ROW`]), then the ten of `0x3d8f60` `FUN_0016b1e8` draws
/// in the air ([`MIDAIR_ROWS`]). Their entries are 12 bytes: the script, a
/// word no code reads (0x14 for the tip under the rim and the dunks
/// turned into lay-ups, 0x15 the two-handed catch, 0x16 the tipped shot,
/// where the lay-up names 20..22 would fit) and the lay-up kind the start
/// writes at +0x26c (`lw v0, 8(s0)` at `0x16b2d4`), 0 in all ten: the
/// pop-up names them all "fundamental". `family` is 2, a lay-up's, though
/// the starts write +0x1e4 9 (the shoot press's, which the board's branch
/// keeps) and 5 (`0x16b2cc`). Their callback 21 and last opcode in the
/// script's ticks, as the rows' (`tests/script_timing_disc.rs`).
pub const EXTRA_ROWS: [MoveRow; 11] = [
    extra("layup_3_bboard", 0x446416, 5, 49, 62),
    extra("midair_tipin_under", 0x447586, 0, 22, 33),
    extra("dunk_to_layup_1", 0x447747, 0, 23, 36),
    extra("dunk_to_layup_2", 0x447786, 0, 14, 25),
    extra("dunk_to_layup_3", 0x4477c5, 0, 15, 22),
    extra("dunk_to_layup_4", 0x447804, 0, 14, 24),
    extra("dunk_to_layup_7", 0x4478c1, 0, 18, 25),
    extra("midair_tipin_2handcatch", 0x44761f, 0, 18, 31),
    extra("midair_tipin_shot", 0x4476b3, 0, 22, 33),
    extra("dunk_to_layup_5", 0x447843, 0, 17, 27),
    extra("dunk_to_layup_6", 0x447882, 0, 18, 25),
];

const fn extra(name: &'static str, script: u32, id: u8, release: u8, end: u8) -> MoveRow {
    MoveRow {
        name,
        script,
        legend: -1,
        mask: 0,
        family: 2,
        id,
        release,
        end,
    }
}

/// The putbacks' scripts (`FUN_00169c98`'s records, [`crate::putback`]),
/// past [`EXTRA_ROWS`], each once: dunks (family 0), with their callback
/// 21 and last opcode in the script's ticks.
pub const PUTBACK_ROWS: [MoveRow; 18] = [
    putback("putback_1h_2f_tomahawk_r", 0x4455a5, 57, 76),
    putback("putback_1h_2f_tomahawk_l", 0x4455d2, 57, 76),
    putback("putback_2h_2f_hang_l", 0x4456ac, 56, 77),
    putback("putback_2h_2f_hang_r", 0x4456d9, 56, 77),
    putback("putback_2h_2f_windmill_hang", 0x4457ed, 52, 72),
    putback("putback_1h_2f_scoop", 0x4458f6, 51, 64),
    putback("putback_1h_2f_taprim", 0x4459d5, 54, 72),
    putback("putback_1h_2f_2handcatch_r", 0x445ae1, 51, 63),
    putback("putback_1h_2f_2handcatch_l", 0x445ab4, 51, 63),
    putback("putback_1h_2f_2handcatch_hang_r", 0x445be0, 57, 68),
    putback("putback_1h_2f_2handcatch_hang_l", 0x445ba2, 57, 68),
    putback("putback_1h_2f_hang", 0x445cf4, 52, 67),
    putback("putback_2h_2f_hang_2_l", 0x445e1e, 57, 79),
    putback("putback_2h_2f_hang_2_r", 0x445e4b, 57, 79),
    putback("putback_2h_2f_hang_swing_l", 0x445f5f, 57, 119),
    putback("putback_2h_2f_hang_swing_r", 0x445f8c, 57, 119),
    putback("putback_2h_2f_hang_3_l", 0x446090, 56, 77),
    putback("putback_2h_2f_hang_3_r", 0x4460bd, 56, 77),
];

const fn putback(name: &'static str, script: u32, release: u8, end: u8) -> MoveRow {
    MoveRow {
        name,
        script,
        legend: -1,
        mask: 0,
        family: 0,
        id: 0,
        release,
        end,
    }
}

/// The staged Level 2 dunks' finishing bodies (`FUN_00334178`, started by a
/// plan's opcode-1 cue): the body runs on the actor's own channel and
/// physics, its `9e` aims flying him to the rim. Release and end as the
/// putbacks' (callback 21's tick, the body's end).
pub const FINISH_ROWS: [MoveRow; 8] = [
    putback("gbdunk_1_01_3_mo", 0x44b4c2, 98, 156),
    putback("gbdunk_1_02_10_ho", 0x44b57e, 84, 86),
    putback("gbdunk_2_04_5_mo", 0x44b62a, 64, 82),
    putback("gbdunk_2_02_8_ho", 0x44b6dc, 90, 126),
    putback("gbdunk_3_01_5_ho", 0x44b798, 98, 100),
    putback("gbdunk_3_03_6_ho", 0x44b82b, 88, 90),
    putback("gbdunk_4_01_1", 0x44b8d7, 53, 55),
    putback("gbdunk_4_02_11_mo", 0x44b988, 98, 128),
];

/// The first of [`FINISH_ROWS`] as a row.
pub const FINISH_FIRST: u8 = PUTBACK_FIRST + PUTBACK_ROWS.len() as u8;

/// The row of the finishing body at `script`, if it is one of
/// [`FINISH_ROWS`].
pub fn finish_row(script: u32) -> Option<u8> {
    FINISH_ROWS
        .iter()
        .position(|r| r.script == script)
        .map(|k| FINISH_FIRST + k as u8)
}

/// The first of [`PUTBACK_ROWS`] as a row.
pub const PUTBACK_FIRST: u8 = (MOVE_ROWS.len() + LEAP_ROWS.len() + EXTRA_ROWS.len()) as u8;

/// [`crate::Action::Layup`]'s row for `layup_3_bboard`.
pub const BOARD_LAYUP_ROW: u8 = (MOVE_ROWS.len() + LEAP_ROWS.len()) as u8;
/// The first of `FUN_0016b1e8`'s ten mid-air rows, in the table's order
/// (`rand() % 6` within 5.25 of the rim draws among the first six, else
/// `rand() % 10` among all).
pub const MIDAIR_ROWS: u8 = BOARD_LAYUP_ROW + 1;

/// A dunk's, lay-up's or leap's row: of [`MOVE_ROWS`], or past it
/// [`LEAP_ROWS`], [`EXTRA_ROWS`] and [`PUTBACK_ROWS`]; none for
/// [`NO_MOVE_ROW`].
pub fn move_row(row: u8) -> Option<&'static MoveRow> {
    let r = usize::from(row);
    MOVE_ROWS.get(r).or_else(|| {
        let r = r.checked_sub(MOVE_ROWS.len())?;
        LEAP_ROWS.get(r).or_else(|| {
            let r = r.checked_sub(LEAP_ROWS.len())?;
            EXTRA_ROWS.get(r).or_else(|| {
                let r = r.checked_sub(EXTRA_ROWS.len())?;
                PUTBACK_ROWS
                    .get(r)
                    .or_else(|| FINISH_ROWS.get(r.checked_sub(PUTBACK_ROWS.len())?))
            })
        })
    })
}

/// The dives' scripts (`FUN_0015a060` raw `0x15b3c0..0x15b494`,
/// `FUN_0015da28` `0x15df20..0x15df68`): far (`diveforball_notrans`, record
/// 71) and near (`DIVEFORBALL_SUCCESSFUL_03_8_lowcomp`, 72 from frame 7)
/// ([`crate::dive`]).
pub const DIVE_SCRIPTS: [u32; 2] = [0x4576d1, 0x45776a];
/// [`crate::Action::Passing`]'s row for a pass no row of [`PASS_ROWS`]
/// scripts (a handoff, a lob to a leaper, the trick passes).
pub const NO_PASS_ROW: u8 = u8::MAX;

/// The row of the pass table an inbound plays (the only row with flag
/// bit 16, `0x160aa4`).
pub const INBOUND_ROW: usize = 10;
/// The row a live pass plays when no row qualifies (`0x160e74`).
pub const DEFAULT_PASS_ROW: usize = 10;

/// 1024 angle units to the circle (`FUN_0018d768`: radians × 162.97466
/// + 0.5, truncated).
pub fn angle_units(radians: f32) -> i32 {
    ((radians * 162.974_66 + 0.5) as i32) & 0x3ff
}

/// `FUN_0018cd78`: the signed difference of two angles in units,
/// -512..=512.
pub fn angle_diff(a: i32, b: i32) -> i32 {
    let d = a - b;
    if d.abs() > 0x200 {
        if d < 0 { d + 0x400 } else { d - 0x400 }
    } else {
        d
    }
}

/// The original's facing word (actor+0xb0) for the sim's heading `h`.
/// The court is the original's own; the word points (-sin, -cos) where
/// the heading points (sin, cos), so the word is the units of `h + PI`
/// (half a turn, the heading's convention; wrapped into [0, 2π) first, so
/// a heading past ±π names the same word). A heading made from a word
/// ([`crate::restart::heading_radians`]) gives that word back.
pub fn facing_word(heading: f32) -> i32 {
    angle_units((heading + core::f32::consts::PI).rem_euclid(core::f32::consts::TAU))
}

/// `FUN_0018d768(word, from, to, &half)` (raw `0x18d768..0x18d9ec`): the
/// sector, eight of 128 units, `((u + 0x40) & 0x3ff) >> 7` of
/// `u = (word - units(atan2(from x - to x, from z - to z)) + 0x200) & 0x3ff`
/// (the half, `((u + 0x80) & 0x3ff) >> 8`, no caller here reads).
///
/// `word` is one of the original's angle words (an actor's
/// [`facing_word`], or a fixed word) and the points are on the court,
/// the original's frame. With an actor's word (he looks along (-sin,
/// -cos) of it), sector 4 is the line from `from` to `to` running the way
/// he looks, 0 the line running back at him, 2 and 6 across
/// (docs/TRACE.md "The sector in the original's frame").
pub fn sector(word: i32, from: [f32; 2], to: [f32; 2]) -> u32 {
    let a = angle_units(math::atan2(from[0] - to[0], from[1] - to[1]));
    let u = (word - a + 0x200) & 0x3ff;
    (((u + 0x40) & 0x3ff) >> 7) as u32
}

/// `FUN_0018d290(player, at, _)`: the facing word actor+0xb0 less the
/// direction from `at` to the player, `atan2(x - at x, z - at z)` in
/// units, 0..1023 (0 facing `at`, 512 facing away), and its fold to
/// 0..512.
///
/// The word is the wanted facing's [`facing_word`]
/// ([`crate::Player::wanted`]); both angles are rounded as the original
/// rounds them, which keeps the angle exact: on all 93,654
/// actor updates of the side-fade capture (docs/TRACE.md "The chooser's
/// facing angle in the original's frame").
pub fn facing_angle(facing: f32, position: [f32; 2], at: [f32; 2]) -> (i32, i32) {
    let word = facing_word(facing);
    let a = angle_units(math::atan2(position[0] - at[0], position[1] - at[1]));
    let u = (word - a) & 0x3ff;
    (u, if u <= 0x200 { u } else { 0x400 - u })
}

/// Rating 0..99 as the original reads it (`FUN_001d7d00`).
fn rating(r: f32) -> i32 {
    (r * 99.0).round().clamp(0.0, 99.0) as i32
}

/// The shot a press starts (`FUN_0016b4b8`, after its gamebreaker, dunk
/// and lay-up branches), as an id in [`SHOT_SCRIPTS`].
///
/// Drawn in the original's order from the shared generator. The player's
/// record and sex ([`crate::Shooter`]) pick his slot shots, the legends'
/// hooks, Bird's jump shot and the female set; a player without one picks
/// as the shared set's male streetballer. A gamebreaker shot takes the long
/// or short script by range where the original reads the court map at
/// `0x3d2790` (`FUN_00141298`).
pub(crate) fn shot_script(
    w: &mut World,
    i: usize,
    buttons: u16,
    long: bool,
    gamebreaker: u8,
    far: bool,
) -> usize {
    if gamebreaker > 0 {
        return if long {
            GAMEBREAKER_LONG
        } else {
            GAMEBREAKER_SHORT
        };
    }
    let shooter = w.players[i].shooter;
    if let Some(slot) = NativeTricks::shot_slot(buttons) {
        return shooter.map_or(SHOT_SLOTS[slot], |s| usize::from(s.slots[slot]));
    }
    let record = shooter.map(|s| s.record);
    let female = shooter.is_some_and(|s| s.female);
    // The default jump shot (`0x16b900..0x16b9b8` and its two copies):
    // Bird's own, else a woman's after one draw, else one of the three.
    let default = |w: &mut World| {
        if record == Some(BIRD) {
            return BIRD_SHOT;
        }
        let r = w.rng.next_u32();
        if female {
            FEMALE_SHOT
        } else {
            DEFAULT_SHOTS[(r % 3) as usize]
        }
    };
    // A side fade's draw: a legend's own hook, else 51% the fade, else one
    // of the three hooks.
    let side = |w: &mut World, fade: usize| {
        if let Some(&(_, hook)) = LEGEND_HOOKS.iter().find(|&&(r, _)| Some(r) == record) {
            return hook;
        }
        if w.rng.next_u32() % 100 < 0x33 {
            return fade;
        }
        SIDE_HOOKS[(w.rng.next_u32() % 3) as usize]
    };
    let p = &w.players[i];
    let hoop = crate::target_hoop(p.team);
    let rim = w.rim_spot(hoop);
    // The back-court heave: the shooter's x on the other side of the
    // centre line from the basket (the signs of `FUN_001404e0`'s rim
    // and of actor+0x30 differ) and more than 13 from the line. The
    // played capture: every one of the person's 16 presses from the
    // back court played `heaveshot_1`.
    if (p.position[0] >= 0.0) != (rim[0] >= 0.0) && p.position[0].abs() > 13.0 {
        return HEAVE;
    }
    let moving = p.moving();
    // Actor+0xb0: the facing he wants, which the facing (+0xa8) turns
    // toward.
    let (angle, fold) = facing_angle(p.wanted, p.position, rim);
    let hand = p.gait_channel.hand;
    if (400..=600).contains(&angle) {
        // Facing away from the basket: posting (bit 11) within 25 of the
        // rim (`FUN_0016e930`), `jumpshot_2_fadeaway`; else the default
        // three.
        if w.posting(i) && w.offence_rim_distance(i) <= 25.0 {
            return POST_FADEAWAY;
        }
        return default(w);
    }
    if moving {
        if (0xa0..0x12d).contains(&angle) {
            if hand == 1 {
                return SIDEFADE_LEFT_R;
            }
            return side(w, SIDEFADE_LEFT_L);
        }
        if (0x2a0..0x32d).contains(&angle) {
            if hand != 1 {
                return SIDEFADE_RIGHT_L;
            }
            return side(w, SIDEFADE_RIGHT_R);
        }
        // Out of the dunking area (`FUN_0016c500(actor, 1)` 1, `far`)
        // facing the rim: the runner (actor+0x1e4 7), a woman's the
        // stepping set shot (`0x16bce8..0x16bd44`).
        if fold < 0x65 && far {
            return if female { FEMALE_RUNNER } else { RUNNER };
        }
    }
    default(w)
}

/// `FUN_0019bed8(g, vy, y)`: the updates until an actor launched upward at
/// `vy` (units an update) from height `y` lands, as the jump opcode
/// (`FUN_00194c40`, raw `0x194ce4..0x194d2c`) and the airtime wait
/// (`0x1973e8..0x197434`) call it: `g` is `32.152233 / fps² × -1.2596 ×
/// 0.5` (the constants `0x42009be3` and `0xbfa13ba4`, fps 60), the roots
/// of `g t² + vy t + y` are taken, and the larger root beyond 1 is rounded
/// up (`cvt.w.s` truncates, one added when it fell short). A jump of 0.25
/// from the floor lands 45 updates later; 0.2125 in 38, 0.225 in 40.
pub fn airtime(vy: f32, y: f32) -> u8 {
    let g = f32::from_bits(0x4200_9be3) / (60.0 * 60.0) * f32::from_bits(0xbfa1_3ba4) * 0.5;
    let disc = vy * vy - 4.0 * g * y;
    let root = if disc < 1e-4 { 0.0 } else { libm::sqrtf(disc) };
    let t1 = (-vy + root) / (2.0 * g);
    let t2 = (-vy - root) / (2.0 * g);
    let ceil = |t: f32| {
        let c = (t as i32) as f32;
        if c < t { c + 1.0 } else { c }
    };
    let mut out = 1.0;
    if 1.0 < t1 {
        out = ceil(t1);
    }
    if 1.0 < t2 && t1 < t2 {
        out = ceil(t2);
    }
    (out as i32).clamp(0, 255) as u8
}

/// Callback 19 (`FUN_00174c10`, raw `0x174c10..0x174d24`) at a shot
/// script's release gate (`86 19`): whether the script waits another
/// update when its tick `vm` is the `gate` and `updates` have passed since
/// the press (the press update the 0th). The callback passes a computer's
/// actor and a button let go at once; a person's held button waits until
/// the actor is within 15 updates of landing (5 for the clips outside
/// `0x3d9218`): actor+0x64 less actor+0x60, the jump's airtime less the
/// updates since the jump, which go on counting through the wait. So the
/// gate opens by itself `latest` updates after the press, however long
/// the script has waited; the side fades' jump is callback 18's
/// (`FUN_00172788`), the others' their own `9f` or `a1`.
pub fn gate_waits(gate: u8, latest: u8, vm: i32, updates: i32, holding: bool) -> bool {
    gate > 0 && vm == i32::from(gate) && holding && updates < i32::from(latest)
}

/// Callback 17 (`FUN_001726c0`) at a default jump shot's check tick: with
/// the stick pushed (intensity 16 or more of 64) within 45° of straight
/// away from the basket (`FUN_0018d500` 0x180..0x280), the script jumps
/// into [`FADEAWAY`].
pub(crate) fn fades_away(w: &World, i: usize, input: &Input) -> bool {
    if !input.moves() {
        return false;
    }
    let stick = input.stick();
    let p = &w.players[i];
    let rim = w.rim_spot(crate::target_hoop(p.team));
    let away = angle_units(math::atan2(p.position[0] - rim[0], p.position[1] - rim[1]));
    let pushed = angle_units(math::atan2(stick[0], stick[1]));
    angle_diff(pushed, away).abs() <= 0x80
}

/// Where the pass filter measures a pass (`FUN_0015f930`, raw
/// `0x15fe4c..0x160104`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PassReach {
    /// The passer `trunc(fps × 0.25)` = 15 updates ahead on his velocity,
    /// where the distance band measures from (`0x1605f0`).
    pub ahead: [f32; 2],
    /// The passer the estimate's flight ahead (`0x1600bc..0x1600dc`, the
    /// float itself), where the sector is read from.
    pub from: [f32; 2],
    /// The catch spot: the receiver led the estimate's whole updates
    /// (`FUN_001636c0`, `0x160020`) when the pass leads him, else the
    /// aim's spot (`0x160030`).
    pub catch: [f32; 2],
    /// The press's flight estimate in updates, before truncation.
    pub flight: f32,
}

/// Actor+0x40 as the pass filter reads it, per second: zero for a
/// posting passer (measured where he stands, `0x15fce8`) and for one on
/// the stand path (`moving` false) unless he is in the air.
pub(crate) fn passer_velocity(w: &World, i: usize, moving: bool) -> [f32; 2] {
    let p = &w.players[i];
    if p.posting || !(moving || p.height > 0.0) {
        return [0.0; 2];
    }
    p.velocity()
}

/// The pass filter's flight estimate and the spots it measures between
/// (raw `0x15fe4c..0x160104`). The passer 15 updates ahead on `v` (per
/// second); the receiver 25 updates ahead on his velocity when the pass
/// leads him, else the aim's spot. The flight is their distance over the
/// pass speed, at least [`crate::passing::PASS_MIN_FLIGHT`]; a board pass
/// (`board`: the waypoint and the kind's ratio) times both legs, the
/// second at three quarters of the speed, and scales the whole.
pub(crate) fn pass_reach(
    w: &World,
    i: usize,
    to: usize,
    aim: &crate::passing::PassAim,
    v: [f32; 2],
    board: Option<([f32; 2], f32)>,
) -> PassReach {
    use crate::passing::{PASS_MIN_FLIGHT, PASS_SPEED};
    let p = &w.players[i];
    let r = &w.players[to];
    let fps = crate::TICK_HZ as f32;
    let v = v.map(|c| c / fps);
    let quarter = (fps * 0.25) as i32 as f32;
    let ahead = [
        p.position[0] + v[0] * quarter,
        p.position[1] + v[1] * quarter,
    ];
    let led = if aim.lead {
        let rv = if r.moving() { r.velocity() } else { [0.0; 2] }.map(|c| c / fps);
        [r.position[0] + rv[0] * 25.0, r.position[1] + rv[1] * 25.0]
    } else {
        aim.spot
    };
    let flight = match board {
        None => (distance(ahead, led) / PASS_SPEED).max(PASS_MIN_FLIGHT),
        Some((at, ratio)) => {
            (distance(ahead, at) / PASS_SPEED + distance(at, led) / (PASS_SPEED * 0.75))
                .max(PASS_MIN_FLIGHT)
                * ratio
        }
    };
    let catch = if aim.lead {
        w.lead_spot(to, flight as i32)
    } else {
        aim.spot
    };
    let from = if v == [0.0; 2] {
        p.position
    } else {
        [p.position[0] + v[0] * flight, p.position[1] + v[1] * flight]
    };
    PassReach {
        ahead,
        from,
        catch,
        flight,
    }
}

/// His velocity (actor+0x40) as the filter reads it: the update's
/// locomotion has run, and with the stick under 16 its stand path has
/// zeroed it (the computer's pass node lets go of the stick as it
/// presses: no attract pass plays a running row).
fn filter_moving(w: &World, i: usize, input: &Input) -> bool {
    w.players[i].moving() && stick_strength(w, i, input) >= crate::input::MOVES
}

/// The receiver's sector the pass filter reads (flag bits 0..7):
/// `FUN_0018d768(yaw, passer, catch spot)` (`0x1604ac`, `0x1604cc`,
/// `0x1605c8`), 4 with the receiver straight ahead, 0 behind (`0x38` the
/// rows ahead, `0x01`/`0x03`/`0x81` behind, `0x40`/`0x06` the sides). The
/// yaw is the rendered root bone's (`FUN_00154230` at `0x16010c`, the
/// facing word's sense; airborne, `+0xb0` itself at `0x160404`), read as
/// the passer's facing word; the spots are [`pass_reach`]'s.
#[cfg(test)]
pub(crate) fn pass_sector(
    w: &World,
    i: usize,
    to: usize,
    aim: &crate::passing::PassAim,
    input: &Input,
) -> u32 {
    let v = passer_velocity(w, i, filter_moving(w, i, input));
    let reach = pass_reach(w, i, to, aim, v, None);
    sector(facing_word(w.players[i].heading), reach.from, reach.catch)
}

/// What a pass press decides at once (`FUN_0015f930`): the row and when
/// the row's script lets the ball go, or no row and the release at once
/// (a handoff to a teammate within 5, `0x160ea0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassPick {
    pub row: Option<u8>,
    pub release: u8,
}

/// `FUN_0015f930`'s row filter (`0x160870..0x160e24`) and draw
/// (`0x160ebc`): every row the passer qualifies for, then one at random.
///
/// Ported: the phase-6 rows, the kick rows (L3; a computer draws them by
/// its Handles rating standing in for the record's rating 7), the dribble
/// hand, the receiver's sector from the passer's facing (the passer taken
/// a quarter second ahead), the distance band, the standing and running
/// rows (running toward the boundary within 10 updates, or with the stick
/// under 41 of 64, takes the standing rows), the airborne rows, the
/// alley-oop rows for a leaping receiver, the back-court heave with 9 or
/// more on the clock, and the handoff within 5; the sector and the
/// distance band on the press's flight estimate ([`pass_reach`]). Not
/// carried: the legends' rows (`+0x04`), the dive and self-pass rows,
/// landing within 10 updates (`0x20c`), and the running row's velocity
/// along the facing.
pub(crate) fn pass_row(
    w: &mut World,
    i: usize,
    to: usize,
    aim: &crate::passing::PassAim,
    input: &Input,
) -> PassPick {
    let Some(rows) = pass_candidates(w, i, to, aim, input) else {
        return PassPick {
            row: None,
            release: 0,
        };
    };
    if rows.is_empty() {
        return PassPick {
            row: Some(DEFAULT_PASS_ROW as u8),
            release: PASS_ROWS[DEFAULT_PASS_ROW].release,
        };
    }
    let row = rows[(w.rng.next_u32() % rows.len() as u32) as usize];
    PassPick {
        row: Some(row as u8),
        release: PASS_ROWS[row].release,
    }
}

/// Actor+0x90, `i`'s stick strength this update: a computer's is its
/// pad's (its input points the stick at the receiver, which the sim reads
/// for a pad without the AI's target; the pass node lets go of it,
/// `FUN_0013ac30`, as it presses).
pub(crate) fn stick_strength(w: &World, i: usize, input: &Input) -> i32 {
    match w.ai.brains.get(i) {
        Some(b) if input.is_computer() => b.pad.strength.min(64),
        _ => input.strength(),
    }
}

/// The rows of [`PASS_ROWS`] [`pass_row`]'s filter lets through, before
/// its draw; None for a handoff.
pub(crate) fn pass_candidates(
    w: &World,
    i: usize,
    to: usize,
    aim: &crate::passing::PassAim,
    input: &Input,
) -> Option<Vec<usize>> {
    let verdicts = pass_verdicts(w, i, to, aim, input)?;
    Some(
        (0..PASS_ROWS.len())
            .filter(|&k| verdicts[k].is_none())
            .collect(),
    )
}

/// [`pass_candidates`] row by row: the first test that keeps each row
/// out, or None for a row the filter lets through.
pub(crate) fn pass_verdicts(
    w: &World,
    i: usize,
    to: usize,
    aim: &crate::passing::PassAim,
    input: &Input,
) -> Option<Vec<Option<&'static str>>> {
    let p = &w.players[i];
    let r = &w.players[to];
    let computer = input.is_computer();
    let kick = input.held(button::L3);
    let oop = r.leaping();
    let phase6 = w.restarting();
    let airborne = p.height > 0.0;
    let intensity = stick_strength(w, i, input);
    let moving = filter_moving(w, i, input);
    let v = p.velocity();
    let ahead = [p.position[0] + v[0] / 6.0, p.position[1] + v[1] / 6.0];
    let heading_out = ahead[0].abs() >= 32.0 || ahead[1].abs() >= 16.5;
    // `0x208`: a moving passer on the ground takes the standing rows when
    // he is running out of bounds, or barely moving the stick.
    let slow = p.speed * crate::TICK_SECONDS <= f32::from_bits(0x3ba3_d70a) || intensity < 41;
    let standing_rows = !moving || airborne || (heading_out != slow);
    let landing_soon = false;
    let handoff =
        distance(p.position, r.position) <= 5.0 && !matches!(r.action, Action::Dunking { .. });
    if handoff {
        return None;
    }
    let own_half = (p.position[0] < 0.0) == (w.rim_spot(crate::target_hoop(p.team))[0] < 0.0);
    let heave_allowed = !own_half && w.shot_clock as i32 >= 9;
    let reach = pass_reach(w, i, to, aim, passer_velocity(w, i, moving), None);
    let dist = distance(reach.ahead, reach.catch);
    let sector = sector(facing_word(p.heading), reach.from, reach.catch);
    let hand = p.gait_channel.hand;
    let rating = rating(p.ratings.handles);
    let verdict = |row: &PassRow| {
        let f = row.flags;
        if !kick {
            if computer {
                if rating < i32::from(row.min_rating) {
                    return Some("rating");
                }
            } else if row.min_rating >= 0 {
                return Some("rating");
            }
        } else if computer || row.min_rating == -1 {
            return Some("kick");
        }
        if f & (0x100000 | 0x200000 | 0x400000) != 0 {
            return Some("special");
        }
        if f & 0x80000 != 0 && !heave_allowed {
            return Some("heave");
        }
        if phase6 != (f & 0x10000 != 0) {
            return Some("phase6");
        }
        if !kick && ((f & 0x800 != 0 && hand == 1) || (f & 0x1000 != 0 && hand == 2)) {
            return Some("hand");
        }
        if f != u32::MAX && f & (1 << sector) == 0 {
            return Some("sector");
        }
        if !kick
            && ((row.min_dist > 0.0 && dist < row.min_dist)
                || (row.max_dist > 0.0 && row.max_dist < dist))
        {
            return Some("distance");
        }
        if f & 0x400 == 0 {
            if airborne && !landing_soon {
                return Some("airborne");
            }
        } else if !airborne || landing_soon {
            return Some("airborne");
        }
        if !kick {
            if f & 0x200 != 0 && !standing_rows {
                return Some("standing");
            }
            if f & 0x100 != 0 && (!moving || standing_rows || landing_soon) {
                return Some("running");
            }
        }
        if oop != (f & 0x8000 != 0) {
            return Some("oop");
        }
        if row.legend > 0 {
            return Some("legend");
        }
        None
    };
    Some(PASS_ROWS.iter().map(verdict).collect())
}

/// `FUN_001611d8`'s draw at the release: the catch row the receiver
/// plays from the release through the flight, as an index into
/// [`CATCH_ROWS`]. Ported: the sector of the catch spot from the
/// receiver's facing, standing against running, airborne (row 0), the
/// shortest flight, which the filter compares with the passer's +0x1b0
/// (`0x162028`: `lw 0x1b0($s5)`, `$s5` the passer), the flight of the
/// last pass he was thrown (`passer_flight`), and the first pass after the
/// actors' reset (`DAT_003d65b8`, `0x162248`), which admits only rows with
/// flag `0x20000` (`catch_stand_3r`/`3l`). The sector is measured in the
/// original's frame from the passer's spot ([`catch_bit`]). Not carried:
/// the spot's closeness test (`0x3d70b4`).
/// Board passes take only the 0x40000 rows and measure from the waypoint.
///
/// With no row let through (`0x162380`: the count −1) there is no draw
/// and no row: +0x1bc is cleared and the pass goes on at once
/// (`0x1623a4..0x162400`), so `None`. (The list holds row 0 alone only
/// through the spot's closeness test, not carried.)
pub(crate) fn catch_row(
    w: &mut World,
    from: usize,
    to: usize,
    spot: [f32; 2],
    passer_flight: i32,
    first_pass: bool,
) -> Option<usize> {
    if w.players[to].height > 0.0 {
        return Some(0);
    }
    let rows = catch_candidates(w, from, to, spot, passer_flight, first_pass);
    if rows.is_empty() {
        return None;
    }
    Some(rows[(w.rng.next_u32() % rows.len() as u32) as usize])
}

/// The rows of [`CATCH_ROWS`] [`catch_row`]'s filter lets through for a
/// receiver on the ground, before its draw; `from` is the passer.
pub(crate) fn catch_candidates(
    w: &World,
    from: usize,
    to: usize,
    spot: [f32; 2],
    passer_flight: i32,
    first_pass: bool,
) -> Vec<usize> {
    let moving = w.players[to].moving();
    let bit = catch_bit(w, from, to, spot);
    let mut rows: Vec<usize> = Vec::new();
    for (k, row) in CATCH_ROWS.iter().enumerate() {
        let f = row.flags;
        if f != u32::MAX && f & bit == 0 {
            continue;
        }
        if row.min_flight >= 1 && i32::from(row.min_flight) > passer_flight {
            continue;
        }
        if first_pass && f & 0x20000 == 0 {
            continue;
        }
        if f & 0x800 != 0 && !moving {
            continue;
        }
        if f & 0x4000 != 0 && moving {
            continue;
        }
        if f & 0x2000 != 0 && !moving {
            continue;
        }
        let board = w.passout.is_some_and(|po| po.passer == from && !po.tossed);
        // A posting receiver takes only the 0x8000 rows, anyone else none
        // of them (`0x162204..0x162240`).
        if (f & 0x8000 != 0) != w.posting(to) || board != (f & 0x40000 != 0) {
            continue;
        }
        rows.push(k);
    }
    rows
}

/// The flag bit the catch filter reads (`0x161f20..0x161fa8`): the
/// sector of the ball's way in, `FUN_0018d768(word, spot, passer)` with the
/// receiver's facing word, so 4 is a ball coming at his face and 0 one
/// from behind (`catch_stand_1` 0x10); sector 0 is split into its
/// neighbours 1 and 7 (bits 2 and 0x80) by the sign of the word less the
/// direction from the passer to the receiver (`FUN_0016f508`, the pair
/// table of `FUN_0013fed0`). The ball comes from the passer's spot
/// (`0x161f2c`) unless the pass has a waypoint (game+0x150 when game+0x160
/// is not -2), carried by [`crate::passout`]. The word is the release's
/// (sp+0x604): the receiver's facing, as [`World::release_point`] turned
/// it at a lead that meets the court's edge (`0x1619f4`, `0x161e4c`); a lead
/// within the court keeps `FUN_001636c0`'s, his facing too.
pub(crate) fn catch_bit(w: &World, from: usize, to: usize, spot: [f32; 2]) -> u32 {
    let r = &w.players[to];
    let source = w
        .passout
        .filter(|po| po.passer == from && !po.tossed)
        .map_or(w.players[from].position, |po| [po.point[0], po.point[2]]);
    let word = facing_word(r.heading);
    let mut bit = 1u32 << sector(word, spot, source);
    if bit == 1 {
        let toward = angle_units(math::atan2(
            r.position[0] - source[0],
            r.position[1] - source[1],
        ));
        bit = if angle_diff(word, toward) < 0 {
            2
        } else {
            0x80
        };
    }
    bit
}

/// Updates a catch keeps its receiver catching (bit 9) once the ball is
/// in his hands: the flight's nominal end still to come plus the row
/// script's tail. A running receiver's tail is the captures' 9 (74 of 77
/// running catches): the `catch_run_*` scripts' own `9c` would clear it
/// at the flight's end, so whatever keeps it is not identified.
pub fn catch_hold(row: usize, flight_left: i32) -> i32 {
    let tail = if CATCH_ROWS[row].flags & 0x2000 != 0 {
        CLIP_TAIL
    } else {
        CATCH_ROWS[row].tail
    };
    flight_left.max(0) + i32::from(tail)
}

/// The ordinary players' dunk unlocks (`FUN_00139850`, raw
/// `0x139850..0x139ad4`, the table `FUN_00139d38(table, 0, +0x254, id)`
/// reads for a record that is neither a created player nor a legend): of
/// the 60 dunk ids listed at `0x43d5d0`, the eight at `0x43d61c` are 1
/// (a press without a turbo held may draw them), the eight at `0x43d5fc`
/// 2 (with one held, the held word's 0x2000), the eleven slot dunks
/// ([`DUNK_SLOTS`]) `1 << level` of their combo (`0x43d3b0`: 4, 8 or 16,
/// which neither press asks for), the other 33 0.
pub const PLAIN_DUNKS: [u8; 8] = [3, 5, 10, 13, 22, 38, 32, 34];
pub const TURBO_DUNKS: [u8; 8] = [4, 6, 9, 12, 11, 41, 42, 16];

/// The unlock mask of dunk `id` for an ordinary player.
pub fn dunk_unlock(id: u8) -> u32 {
    if PLAIN_DUNKS.contains(&id) {
        1
    } else if TURBO_DUNKS.contains(&id) {
        2
    } else if let Some(k) = DUNK_SLOTS.iter().position(|&d| d == id) {
        1 << crate::player::TRICK_SLOT_LEVELS[k]
    } else {
        0
    }
}

/// The dunk ids whose flag word (`0x43f5f0`, `FUN_00171de8`) has 0x800,
/// an advanced dunk: the five `bbpass_1` rows (ids 30 and 48..51), the
/// pass off the board to oneself with its inline finish.
pub const ADVANCED_DUNKS: [u8; 5] = [30, 48, 49, 50, 51];

/// What the shoot press with a dunk on draws (`FUN_001693b8(actor, 0)`):
/// a dunk or lay-up row of [`MOVE_ROWS`], or a jump shot of
/// [`SHOT_SCRIPTS`] when no row qualifies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressMove {
    /// The row; `advanced` for the four-turbo slot's `bbpass_1` from 8.5
    /// or more out, which the sim does not play: a drawn row stands in,
    /// on the advanced dunk's odds.
    Row {
        row: u8,
        advanced: bool,
    },
    Shot(usize),
}

/// The jump shot the Block lessons' holder always takes (`0x44ed73`,
/// `FUN_0016b4b8`'s `0x16b558`).
pub const BLOCK_LESSON_SHOT: usize = 4;

/// The three hooks of `0x3d82f0` the empty draw plays three times in four.
pub const FALLBACK_HOOKS: [usize; 3] = [8, 9, 10];

/// `FUN_001693b8(actor, 0)` (raw `0x1693b8..0x169c94`), the shoot press's
/// move when a dunk is on:
///
/// - the face reaction's draws (`FUN_00159190(actor, r & 1 ? 1 : 2, fps ×
///   2)` draws once more for its clip);
/// - a two-turbo combo held (`0x43d3b0`, the trick slots' masks): the
///   slot's dunk ([`DUNK_SLOTS`]), its first row of family 0; an advanced
///   one ([`ADVANCED_DUNKS`], the four-turbo slot's 48) only from 8.5 or
///   more out (`0x169628..0x16965c`), else the draw below;
/// - the Dunks-rating gates ([`MoveGates::draw`]);
/// - every row of both families the filter lets through ([`press_candidates`]:
///   a dunk row only if its id is unlocked for the held word's turbo bit,
///   [`dunk_unlock`]), one drawn (`rand() % count`);
/// - none: a jump shot (`0x1699ac..0x169bb4`): a legend's own hook
///   ([`LEGEND_HOOKS`]) or Bird's shot, else `rand() % 100` under 75 a
///   hook of [`FALLBACK_HOOKS`] (`rand() % 3`), else a woman's
///   [`FEMALE_SHOT`] after one draw, or one of the default three.
///
/// Not carried: a created player's or a legend's own unlocks
/// (`FUN_00139d38`'s record branches) and rows.
pub(crate) fn press_move(w: &mut World, i: usize, buttons: u16) -> PressMove {
    w.rng.next_u32();
    w.face(i, 2 * crate::TICK_HZ as i32);
    let p = &w.players[i];
    let rim = w.rim_spot(crate::target_hoop(p.team));
    let d = distance(p.position, rim);
    let mut advanced = false;
    if let Some(slot) = NativeTricks::slot(buttons) {
        let id = DUNK_SLOTS[slot];
        if let Some(k) = MOVE_ROWS.iter().position(|r| r.family == 0 && r.id == id) {
            if !ADVANCED_DUNKS.contains(&id) {
                return PressMove::Row {
                    row: k as u8,
                    advanced: false,
                };
            }
            advanced = d >= 8.5;
        }
    }
    let turbo = buttons & crate::button::TURBOS != 0;
    let mut gates = MoveGates::draw(w, rating(w.players[i].ratings.dunks));
    // The Dunk and Advanced Dunk lessons: no easy rows, and no need for
    // them (`0x169744..0x16979c`).
    if matches!(w.lesson, Some(3 | 0x15)) {
        gates.need_easy = false;
        gates.no_easy = true;
    }
    let rows = press_candidates(w, i, gates, turbo);
    if rows.is_empty() {
        let shooter = w.players[i].shooter;
        let record = shooter.map(|s| s.record);
        if let Some(&(_, hook)) = LEGEND_HOOKS.iter().find(|&&(r, _)| Some(r) == record) {
            return PressMove::Shot(hook);
        }
        if record == Some(BIRD) {
            return PressMove::Shot(BIRD_SHOT);
        }
        let hook = w.rng.next_u32() % 100 < 0x4b;
        let k = (w.rng.next_u32() % 3) as usize;
        return PressMove::Shot(if hook {
            FALLBACK_HOOKS[k]
        } else if shooter.is_some_and(|s| s.female) {
            FEMALE_SHOT
        } else {
            DEFAULT_SHOTS[k]
        });
    }
    let row = rows[(w.rng.next_u32() % rows.len() as u32) as usize] as u8;
    PressMove::Row { row, advanced }
}

/// The Dunks-rating gates `FUN_001693b8` sets before its filter (raw
/// `0x1696c0..0x169744`): under 21, a rating of 0 or under `rand() % 100`
/// keeps only the easy rows (0x80000); 21..59 drops the hard ones
/// (0x400000) with no draw; 60 and over draws, under 51 of 100 dropping
/// the easy rows, else the hard ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MoveGates {
    pub need_easy: bool,
    pub no_easy: bool,
    pub no_hard: bool,
}

impl MoveGates {
    pub(crate) fn draw(w: &mut World, dunks: i32) -> Self {
        let mut g = Self::default();
        if dunks < 21 {
            if dunks == 0 || dunks < (w.rng.next_u32() % 100) as i32 {
                g.need_easy = true;
            }
        } else if dunks < 60 {
            g.no_hard = true;
        } else if w.rng.next_u32() % 100 < 0x33 {
            g.no_easy = true;
        } else {
            g.no_hard = true;
        }
        g
    }
}

/// The rows of [`MOVE_ROWS`] `FUN_001693b8(actor, 0)`'s filter lets
/// through under `gates` with a turbo held or not (`0x1697d8..0x169988`),
/// before its draw: no alley-oop finish (0x100000), a dunk only if
/// unlocked ([`dunk_unlock`]: 2 with a turbo, else 1), then the approach
/// sector, the hand, the gates, the distance band and the legends' rows.
pub(crate) fn press_candidates(w: &World, i: usize, gates: MoveGates, turbo: bool) -> Vec<usize> {
    let p = &w.players[i];
    let rim = w.rim_spot(crate::target_hoop(p.team));
    let sector = approach_sector(w, i);
    let d = distance(p.position, rim);
    let hand = p.gait_channel.hand;
    let unlock = if turbo { 2 } else { 1 };
    let MoveGates {
        need_easy,
        no_easy,
        no_hard,
    } = gates;
    let mut rows: Vec<usize> = Vec::new();
    for (k, row) in MOVE_ROWS.iter().enumerate() {
        let m = row.mask;
        if m & 0x100000 != 0 {
            continue;
        }
        if row.family == 0 && dunk_unlock(row.id) & unlock == 0 {
            continue;
        }
        if m & (1 << sector) == 0 {
            continue;
        }
        if (m & 0x40000 != 0 && hand == 1) || (m & 0x20000 != 0 && hand == 2) {
            continue;
        }
        if need_easy && m & 0x80000 == 0 {
            continue;
        }
        if no_easy && m & 0x80000 != 0 {
            continue;
        }
        if no_hard && m & 0x400000 != 0 {
            continue;
        }
        let band = if d >= 12.5 {
            0xc00
        } else if d > 8.0 {
            0xa00
        } else {
            0x900
        };
        if m & band == 0 {
            continue;
        }
        if row.legend > 0 {
            continue;
        }
        rows.push(k);
    }
    rows
}

/// The approach sector seen from the rim the dunk and lay-up filter reads
/// (mask bits 0..7): `FUN_0018d768(word, rim, shooter)` (`0x1694e4`) with
/// the rim's fixed word, 0x100 for the rim at +x (x >= 0) and 0x300 at
/// -x (`0x1693e0..0x169430`), so 4 is straight out from the rim onto the
/// court and 0 behind the board.
pub(crate) fn approach_sector(w: &World, i: usize) -> u32 {
    let p = &w.players[i];
    let rim = w.rim_spot(crate::target_hoop(p.team));
    let word = if rim[0] >= 0.0 { 0x100 } else { 0x300 };
    sector(word, rim, p.position)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FUN_0018d768` on the original's own court: the word less the
    /// direction from `to` to `from`, +0x200, +0x40, >> 7.
    fn raw_sector(word: i32, from: [f32; 2], to: [f32; 2]) -> u32 {
        let a = angle_units(math::atan2(from[0] - to[0], from[1] - to[1]));
        let u = (word - a + 0x200) & 0x3ff;
        (((u + 0x40) & 0x3ff) >> 7) as u32
    }

    #[test]
    fn the_sector_is_fun_0018d768s_on_the_originals_court() {
        // Any word and any line between two points read the original's
        // sector (the lines kept off the sector boundaries).
        let from = [3.0, -2.0];
        for word in (0..1024).step_by(37) {
            for k in 0..16 {
                let a = (k as f32 * 22.5 + 7.0).to_radians();
                let to = [from[0] + 10.0 * a.sin(), from[1] + 10.0 * a.cos()];
                assert_eq!(
                    sector(word, from, to),
                    raw_sector(word, from, to),
                    "word {word}, line {k}"
                );
            }
        }
        // Captured cases (the original's frame; the rows' masks hold
        // them). The side-fade capture's update 3730: the computer's
        // `pass_fwd_2` (0x38) with the receiver straight ahead of the
        // passer's yaw (+0xa8 322).
        let passer = [-15.994_855, -13.155_948];
        let receiver = [-39.350_407, -3.183_166];
        assert_eq!(sector(322, passer, receiver), 4);
        // The first pad capture's 14166: `pass_behind_1` (0x81 and 0x03)
        // to a receiver behind him (yaw 772).
        let passer = [34.405_212, 7.606_945];
        let receiver = [20.947_496, -12.808_018];
        assert_eq!(sector(772, passer, receiver), 1);
        // The side-fade capture's 395 and 2369: `catch_stand_1` and
        // `catch_stand_3r` (both 0x10), the ball from the passer coming at
        // the receiver's face (his facing word 937 and 855).
        let spot = [-13.346_424, 14.209_323];
        let from = [-1.047_825, -2.008_913];
        assert_eq!(sector(937, spot, from), 4);
        let spot = [36.979_973, -6.929_669];
        let from = [45.758_297, -14.272_648];
        assert_eq!(sector(855, spot, from), 4);
        // The first pad capture's 1654: a drive at the rim at -x (word
        // 0x300) from straight out on the court.
        let shooter = [-30.361_216, -0.960_023];
        assert_eq!(sector(0x300, [-36.75, 0.0], shooter), 4);
        // The facing word of a heading reads the same sector: the
        // passer's yaw 322 as the sim's heading.
        let h = 322.0 * std::f32::consts::TAU / 1024.0 - std::f32::consts::PI;
        assert_eq!(facing_word(h), 322);
        assert_eq!(
            sector(
                facing_word(h),
                [-15.994_855, -13.155_948],
                [-39.350_407, -3.183_166]
            ),
            4
        );
    }

    #[test]
    fn the_filters_read_the_sectors_on_the_sims_court() {
        use crate::{Hoop, Player, Rules};
        let world = |players: Vec<Player>| World::new(Hoop::native(), players, Rules::default(), 1);
        // The pass: a standing passer facing +z (heading 0, the word 512)
        // reads a receiver ahead as 4, behind as 0, across as 2 (+x) and 6
        // (-x).
        for (at, want) in [
            ([0.0, 10.0], 4),
            ([0.0, -10.0], 0),
            ([10.0, 0.0], 2),
            ([-10.0, 0.0], 6),
        ] {
            let w = world(vec![
                Player::new(0, [0.0, 0.0], 0.0),
                Player::new(0, at, 0.0),
            ]);
            let aim = w.pass_aim(0, 1);
            assert_eq!(
                pass_sector(&w, 0, 1, &aim, &Input::default()),
                want,
                "receiver at {at:?}"
            );
        }
        // The catch: a receiver facing +z with the ball coming from a
        // passer ahead of him reads 4 (bit 0x10, `catch_stand_1`); from
        // behind, sector 0 goes to bit 2 on the +x side of the line from
        // the passer, 0x80 on the -x side (as sectors 1 and 7 read).
        for (passer, want) in [
            ([0.0, 20.0], 0x10),
            ([2.0, -20.0], 2),
            ([-2.0, -20.0], 0x80),
            ([20.0, -20.0], 2),
            ([-20.0, -20.0], 0x80),
        ] {
            let w = world(vec![
                Player::new(0, passer, 0.0),
                Player::new(0, [0.0, 0.0], 0.0),
            ]);
            assert_eq!(
                catch_bit(&w, 0, 1, [0.0, 0.0]),
                want,
                "passer at {passer:?}"
            );
        }
        // The drive: straight out from either rim onto the court is 4,
        // behind the board 0 (team 0 attacks the rim at +x, word 0x100).
        for (team, at, want) in [
            (0, [30.0, 0.0], 4),
            (0, [40.0, 0.0], 0),
            (1, [-30.0, 0.0], 4),
            (1, [-40.0, 0.0], 0),
            (0, [36.75, -8.0], 6),
            (0, [36.75, 8.0], 2),
            (1, [-36.75, -8.0], 2),
            (1, [-36.75, 8.0], 6),
        ] {
            let w = world(vec![Player::new(team, at, 0.0)]);
            assert_eq!(approach_sector(&w, 0), want, "team {team} at {at:?}");
        }
        // No dunk or lay-up row with a partial mask takes an approach from
        // behind the board (sectors 0, 1, 7): 37 rows, all within 2..6.
        assert!(
            MOVE_ROWS
                .iter()
                .filter(|r| r.mask & 0xff != 0xff && r.mask & 0x10_0000 == 0)
                .all(|r| r.mask & 0x83 == 0)
        );
    }

    #[test]
    fn the_facing_angle_is_the_originals() {
        use std::f32::consts::{FRAC_PI_2, PI};
        // Team 0 attacks +x: the rim at (36.75, 0), the shooter 16.75 out
        // toward the centre. The original measures the rim's direction at
        // 769 (its truncation of -256), so facing the rim reads 1023 and
        // facing away 511, as on the captured presses.
        let rim = [36.75, 0.0];
        let me = [20.0, 0.0];
        assert_eq!(facing_angle(FRAC_PI_2, me, rim), (1023, 1));
        assert_eq!(facing_angle(-FRAC_PI_2, me, rim), (511, 511));
        // Running toward -z (the original's word 0) is the left band
        // (160..300), toward +z the right band (672..812).
        assert_eq!(facing_angle(PI, me, rim), (255, 255));
        assert_eq!(facing_angle(0.0, me, rim), (767, 257));
        // A heading past ±π names the same word.
        assert_eq!(facing_angle(-PI, me, rim), (255, 255));
        assert_eq!(facing_angle(-3.0 * FRAC_PI_2, me, rim), (1023, 1));
    }

    #[test]
    fn angle_differences_wrap_the_short_way() {
        assert_eq!(angle_diff(10, 1000), 34);
        assert_eq!(angle_diff(1000, 10), -34);
        assert_eq!(angle_diff(300, 100), 200);
        assert_eq!(angle_units(0.0), 0);
        assert_eq!(angle_units(std::f32::consts::PI), 512);
    }

    #[test]
    fn the_tables_carry_the_traced_moments() {
        // The inbound row: 6 blend + 12 wait before callback 38; the
        // default jump shots release at 34/34/33 with the pump-fake and
        // fade check at 14/13/12; `dunk_4` (id 20) slams at 55;
        // `catch_stand_3r` holds 15 after the flight.
        assert_eq!(PASS_ROWS[INBOUND_ROW].release, 18);
        assert_eq!(PASS_ROWS[INBOUND_ROW].flags & 0x10000, 0x10000);
        let shots: Vec<(u8, u8)> = DEFAULT_SHOTS
            .iter()
            .map(|&s| (SHOT_SCRIPTS[s].release, SHOT_SCRIPTS[s].check))
            .collect();
        assert_eq!(shots, [(34, 14), (34, 13), (33, 12)]);
        assert_eq!(SHOT_SCRIPTS[FADEAWAY].release, 19);
        assert_eq!(SHOT_SCRIPTS[RUNNER].release, 24);
        // The heave: callback 20 alone at 8 (no fade check, no gate), the
        // ball gone at 19.
        let heave = &SHOT_SCRIPTS[HEAVE];
        assert_eq!(
            (
                heave.name,
                heave.release,
                heave.check,
                heave.gate,
                heave.fades
            ),
            ("heaveshot_1", 19, 8, 0, false)
        );
        // The release gates of the default three: the stepback's at 31
        // opens by itself at 45 (its jump at 15 lands at 60, 15 before);
        // the played captures' long holds released it at 47.
        let gates: Vec<(u8, u8, bool)> = DEFAULT_SHOTS
            .iter()
            .map(|&s| {
                (
                    SHOT_SCRIPTS[s].gate,
                    SHOT_SCRIPTS[s].latest,
                    SHOT_SCRIPTS[s].fades,
                )
            })
            .collect();
        assert_eq!(gates, [(30, 44, true), (32, 46, true), (31, 45, true)]);
        assert_eq!(
            (SHOT_SCRIPTS[FADEAWAY].gate, SHOT_SCRIPTS[FADEAWAY].latest),
            (15, 30)
        );
    }

    #[test]
    fn the_side_fades_open_their_gate_by_callback_18s_landing() {
        // Callback 18 (`83 12 1`, `FUN_00172788`) jumps the side fades
        // 15 / fps = 0.25 up, at 15 (`right_r`, and `left_l` through it)
        // or 10 (`right_l`, `left_r`); the landing is 45 updates on. The
        // clips at their gates, 320 and 321, are in `0x3d9218` (the gate
        // opens 15 before the landing); 322 and 323 are not (5 before).
        let land = i32::from(airtime(0.25, 0.0));
        assert_eq!(land, 45);
        for (s, jump, cap) in [
            (SIDEFADE_RIGHT_R, 15, 15),
            (SIDEFADE_LEFT_L, 15, 15),
            (SIDEFADE_RIGHT_L, 10, 5),
            (SIDEFADE_LEFT_R, 10, 5),
        ] {
            let script = &SHOT_SCRIPTS[s];
            assert_eq!(
                i32::from(script.latest),
                jump + land - cap,
                "{}",
                script.name
            );
        }
        // Held past the gate, `right_l` (gate 28, callback 21 four ticks
        // on) waits until the button is up, or until update 50: the
        // ball leaves at 54 however long the button stays down. The
        // played captures' hold of 30 released it at 34.
        let s = &SHOT_SCRIPTS[SIDEFADE_RIGHT_L];
        let release = |up: i32| {
            let mut stall = 0;
            for u in 0..200 {
                if gate_waits(s.gate, s.latest, u - stall, u, u < up) {
                    stall += 1;
                }
                if u - stall == i32::from(s.release) {
                    return u;
                }
            }
            panic!("no release for a hold of {up}");
        };
        assert_eq!(release(30), 34);
        assert_eq!(release(10), 32);
        assert_eq!(release(49), 53);
        assert_eq!(release(50), 54);
        assert_eq!(release(500), 54);
        // A computer's actor never waits.
        assert!(!gate_waits(s.gate, s.latest, 28, 28, false));
        assert!(gate_waits(s.gate, s.latest, 28, 49, true));
        assert!(!gate_waits(s.gate, s.latest, 28, 50, true));
    }

    #[test]
    fn the_airtime_is_fun_0019bed8s() {
        // The jump opcodes' velocities: 0.25 (the default shots), 0.2125
        // (`jumpshot_5`, `jumpshot_bird`), 0.225 (`jumpshot_7`, the
        // gamebreaker shots), 0.15 (the female set).
        assert_eq!(airtime(0.25, 0.0), 45);
        assert_eq!(airtime(0.2125, 0.0), 38);
        assert_eq!(airtime(0.225, 0.0), 40);
        assert_eq!(airtime(0.15, 0.0), 27);
        // No jump: the larger root is not beyond 1 and the result is 1.
        assert_eq!(airtime(0.0, 0.0), 1);
        let dunk_4 = MOVE_ROWS
            .iter()
            .find(|r| r.family == 0 && r.id == 20)
            .unwrap();
        assert_eq!((dunk_4.name, dunk_4.release), ("dunk_4", 55));
        assert_eq!(CATCH_ROWS[6].name, "catch_stand_3r");
        assert_eq!(catch_hold(6, 2), 17);
        assert_eq!(catch_hold(0, 3), 3 + i32::from(CLIP_TAIL));
        assert_eq!(catch_hold(1, 0), i32::from(CLIP_TAIL));
        // The fallbacks are rows, so the display has a script to play.
        let spike = &MOVE_ROWS[DUNK_FALLBACK_ROW];
        assert_eq!(
            (spike.name, spike.family, spike.release, spike.end),
            ("dunk_1h_2f_spike", 0, 57, 73)
        );
        let layup = &MOVE_ROWS[LAYUP_FALLBACK_ROW];
        assert_eq!(
            (layup.name, layup.family, layup.release, layup.end),
            ("layup_13", 2, 44, 53)
        );
    }
}
