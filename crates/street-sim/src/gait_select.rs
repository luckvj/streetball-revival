//! Choosing a locomotion animation the way the original does: one of the
//! 108 runtime gait records the executable builds at `4b8790` (read by
//! `street_assets::gait`), picked by `FUN_00168b30`.
//!
//! Each record names two animation rows (the right-hand row and its
//! mirrored left-hand twin), who may use it (a category mask from weight
//! and gender, a player record for the legends' own walks and runs, a
//! minimum handles rating for the fancier dribbles), its flags (family,
//! with or without the ball, defence) and its root distance and motion
//! range, which set how fast the clip plays and how far a player moves per
//! update.
//!
//! The selector draws from the shared generator (`3a16b8`) as the original
//! does: once on every call, at its start, and a second time only when no
//! record already playing answers and some record is a candidate
//! (`0x168b7c`, `0x168de0`).

/// One runtime gait record, as the selector reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaitRecord {
    /// +04 and +08: the right-hand row and its mirrored twin.
    pub rows: [u16; 2],
    /// +0c: who may use it (1 lighter men, 2 lighter women, 4 heavier).
    pub category_mask: u32,
    /// +10: a player record only it is for, or below 1 for anyone.
    pub player: i32,
    /// +14: the handles rating needed, or -1.
    pub minimum_handles: i32,
    /// +18: the root distance one animation frame covers.
    pub root_distance: f32,
    /// +1c and +20: motion per native update at the slow and fast ends.
    pub motion: [f32; 2],
    /// +24: the family and situation flags.
    pub flags: u32,
}

/// What the selector needs to know about one player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GaitProfile {
    /// `163d40`: weight below 240 is 1 for men, 2 for women; else 4.
    pub category: u32,
    /// The player's database record (`1d78c8`).
    pub identity: i32,
    /// The handles rating 0..99 (`171670` -> `1d7d00`).
    pub handles: i32,
}

impl GaitProfile {
    /// The category `163d40` derives from the database weight and gender.
    pub fn category_for(weight_lb: i32, female: bool) -> u32 {
        match (weight_lb < 240, female) {
            (true, false) => 1,
            (true, true) => 2,
            (false, _) => 4,
        }
    }
}

/// The match's gait records and each player's profile, fixed for the match.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GaitConfig {
    pub records: Vec<GaitRecord>,
    pub profiles: Vec<GaitProfile>,
    /// The ball-hand flags (`HAND_RIGHT`/`HAND_LEFT`, the clips' `_X`
    /// state bits 8 and 4 after mirroring) per frame of each held gait row.
    pub row_hands: Vec<(u16, Vec<u8>)>,
    /// The same for the standing dribble.
    pub stand_hands: Vec<u8>,
    /// Each trick script's clip hand flags per frame: its primary target,
    /// and its 0xf8 alternate (played on hand 2; the primary again when the
    /// script has none).
    pub trick_hands: Vec<(u16, [Vec<u8>; 2])>,
}

/// State bit 8: the ball in the right hand (live hand 1).
pub const HAND_RIGHT: u8 = 2;
/// State bit 4: the ball in the left hand (live hand 2).
pub const HAND_LEFT: u8 = 1;

/// A clip state word's hand flags as [`HAND_RIGHT`] | [`HAND_LEFT`].
pub fn hand_flags(state: u32) -> u8 {
    u8::from(state & 8 != 0) << 1 | u8::from(state & 4 != 0)
}

/// `14cd54`: holding the ball, the live hand (`+0x25c`) becomes 1 on the
/// right hand alone and 2 on the left alone; both, or neither, keep it.
pub fn live_hand(hand: u8, flags: u8) -> u8 {
    match flags {
        HAND_RIGHT => 1,
        HAND_LEFT => 2,
        _ => hand,
    }
}

impl GaitConfig {
    /// The hand flags of trick `script`'s clip on `side` (1 the 0xf8
    /// alternate) at `progress` (0..1 of the trick), or its last frame.
    pub fn trick_hand(&self, script: u16, side: u8, progress: f32) -> Option<u8> {
        let (_, sides) = self.trick_hands.iter().find(|(s, _)| *s == script)?;
        let frames = &sides[usize::from(side.min(1))];
        let last = frames.len().checked_sub(1)?;
        let at = (progress.clamp(0.0, 1.0) * last as f32) as usize;
        Some(frames[at.min(last)])
    }

    /// The hand flags of trick `script`'s clip on `side` at clip `frame`
    /// (its last frame past the end).
    pub fn trick_hand_at(&self, script: u16, side: u8, frame: usize) -> Option<u8> {
        let (_, sides) = self.trick_hands.iter().find(|(s, _)| *s == script)?;
        let frames = &sides[usize::from(side.min(1))];
        let last = frames.len().checked_sub(1)?;
        Some(frames[frame.min(last)])
    }

    /// The hand flags of `row` at `frame` (wrapping), if the row is known.
    pub fn row_hand(&self, row: u16, frame: usize) -> Option<u8> {
        let (_, frames) = self.row_hands.iter().find(|(r, _)| *r == row)?;
        (!frames.is_empty()).then(|| frames[frame % frames.len()])
    }
}

/// Flags the selector tests one by one (`168b30`); bit 4 is never tested
/// and bit 0x4000 only when asked for.
const TESTED: [u32; 14] = [
    0x1, 0x2, 0x100, 0x8, 0x10, 0x20, 0x40, 0x80, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x4000,
];

/// `FUN_00168b30`: the record to play for `requested` flags. A record the
/// player is already playing (`current_row` is one of its rows) is kept.
/// Otherwise every eligible record is a candidate, except that once a
/// record made for this very player turns up, later generic ones join only
/// if the first draw (`draws[0] % 100 < 50`) failed and they need a handles
/// rating; the second draw picks among the candidates. No candidate is
/// record 0, as in the original.
pub fn select(
    records: &[GaitRecord],
    current_row: Option<u16>,
    requested: u32,
    profile: GaitProfile,
    draws: [u32; 2],
) -> usize {
    let mut d = draws.into_iter();
    select_drawing(records, current_row, requested, profile, || {
        d.next().unwrap_or(0)
    })
}

/// [`select`] taking its draws from `draw` as the original takes them
/// from its generator: the first always, the second only for the pick.
pub fn select_drawing(
    records: &[GaitRecord],
    current_row: Option<u16>,
    requested: u32,
    profile: GaitProfile,
    mut draw: impl FnMut() -> u32,
) -> usize {
    let gate = draw() % 100 < 50;
    let mut own = false;
    let mut candidates = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if record.category_mask & profile.category == 0 || record.minimum_handles > profile.handles
        {
            continue;
        }
        if TESTED
            .iter()
            .any(|&bit| requested & bit != 0 && record.flags & bit == 0)
        {
            continue;
        }
        if current_row.is_some_and(|row| row == record.rows[0] || row == record.rows[1]) {
            return index;
        }
        if record.player < 1 {
            if !own || (!gate && record.minimum_handles != -1) {
                candidates.push(index);
            }
        } else if record.player == profile.identity {
            own = true;
            candidates.push(index);
        }
    }
    if candidates.is_empty() {
        0
    } else {
        candidates[draw() as usize % candidates.len()]
    }
}

/// The flags a moving player asks for: its family (`GaitFamily` bits)
/// and whether it holds the ball (2) or not (1).
pub fn locomotion_flags(family: u32, held: bool) -> u32 {
    family | if held { 2 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(rows: u16, category: u32, player: i32, handles: i32, flags: u32) -> GaitRecord {
        GaitRecord {
            rows: [rows, rows],
            category_mask: category,
            player,
            minimum_handles: handles,
            root_distance: 0.5,
            motion: [0.3, 0.36],
            flags,
        }
    }

    #[test]
    fn the_live_hand_follows_exclusive_hand_flags() {
        // `14cd54`: right alone is hand 1, left alone hand 2; both or
        // neither keep it.
        assert_eq!(hand_flags(8), HAND_RIGHT);
        assert_eq!(hand_flags(4), HAND_LEFT);
        assert_eq!(hand_flags(12), HAND_RIGHT | HAND_LEFT);
        assert_eq!(live_hand(2, HAND_RIGHT), 1);
        assert_eq!(live_hand(1, HAND_LEFT), 2);
        assert_eq!(live_hand(1, HAND_RIGHT | HAND_LEFT), 1);
        assert_eq!(live_hand(2, 0), 2);
    }

    #[test]
    fn the_selector_follows_fun_00168b30() {
        let records = [
            record(1, 0xffff_ffff, 158, -1, 0x1001), // a legend's own run
            record(2, 1, -1, -1, 0x1001),            // a generic run
            record(3, 1, -1, 60, 0x1002 | 0x4),      // a dribble needing 60
            record(4, 1, -1, -1, 0x1002),            // a generic held run
            record(5, 2, -1, -1, 0x1001),            // women only
            record(6, 0xffff_ffff, -1, -1, 0x1001 | 0x4000),
        ];
        let man = GaitProfile {
            category: 1,
            identity: 20,
            handles: 50,
        };
        // An ordinary man running without the ball: the generic runs
        // (0x4000 is only tested when asked for); never the women's or a
        // legend's.
        let mut seen: Vec<usize> = (0..40)
            .map(|k| select(&records, None, 0x1001, man, [k, k]))
            .collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen, [1, 5]);
        // A row already playing is kept.
        assert_eq!(select(&records, Some(6), 0x1001, man, [0, 0]), 5);
        // The handles gate: 50 cannot take the 60 dribble.
        assert_eq!(select(&records, None, 0x1002, man, [0, 0]), 3);
        let skilled = GaitProfile { handles: 70, ..man };
        let mut held: Vec<usize> = (0..40)
            .map(|k| select(&records, None, 0x1002, skilled, [k, k]))
            .collect();
        held.sort_unstable();
        held.dedup();
        assert_eq!(held, [2, 3]);
        // A legend always gets their own run: later generic records need a
        // failed gate and a handles requirement to join.
        let legend = GaitProfile {
            category: 1,
            identity: 158,
            handles: 50,
        };
        for k in 0..40 {
            assert_eq!(select(&records, None, 0x1001, legend, [k, k]), 0);
        }
        // Nothing eligible: record 0.
        assert_eq!(select(&records, None, 0x2000, man, [0, 0]), 0);
        assert_eq!(GaitProfile::category_for(230, false), 1);
        assert_eq!(GaitProfile::category_for(180, true), 2);
        assert_eq!(GaitProfile::category_for(250, false), 4);
    }
}
