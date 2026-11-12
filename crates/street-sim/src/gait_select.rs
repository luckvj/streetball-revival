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

/// The rows callback 52 (`FUN_00177270`, argument 1) calls moving: those
/// with flag 1 in the row classes (`0x43f0e8`, 144 entries).
pub const MOVING_ROWS: [u16; 133] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90,
    91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
    112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130,
    131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 142, 143, 144, 145, 146, 147, 148, 149, 150,
    151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169,
    170, 171, 172, 173, 957,
];

/// A gait record's script (the table at `0x3e7370`, script and name, keyed
/// here by its blend's primary row): the blend's start frame from a still
/// clip and whether `91` sets 0x800000 before it (the facing matched to
/// the incoming clip, `FUN_00196698`); whether the script asks callback 52
/// (`FUN_00177270`) first, which from a moving row blends at `-19`
/// instead ([`share_start`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordScript {
    pub row: u16,
    pub still: u8,
    pub heading: bool,
    pub share: bool,
}

/// How a record's blend starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RecordStart {
    /// From this frame, the facing matched or not.
    Frame { frame: f32, heading: bool },
    /// At the outgoing clip's share of its step ([`share_start`]).
    Share,
    /// `-9` (`FOOT_SYNC`): the incoming clip's frame whose step matches
    /// the clip channel 0 shows ([`crate::held::foot_sync`]), the facing
    /// matched.
    Sync,
}

/// The records whose script tests callback 15 (`FUN_00177208`, `84 0f`)
/// between its still and moving paths: the dribble runs and walks, whose
/// blend then starts at `-9` ([`RecordStart::Sync`]).
pub const SYNC_ROWS: [u16; 48] = [
    79, 80, 82, 84, 86, 88, 90, 92, 94, 96, 98, 100, 102, 104, 106, 108, 111, 113, 114, 116, 118,
    120, 121, 123, 125, 126, 129, 131, 132, 134, 136, 138, 140, 142, 144, 146, 148, 152, 154, 156,
    158, 160, 162, 164, 166, 168, 170, 172,
];

/// Callback 15's list (`0x3d9290`, 158 records, `FUN_00168e78`): the
/// trick moves' clips (the crossovers, the freestyles, the juke's
/// dropstep, the backdown's spin-out); channel 0 showing one of these
/// (and no blend) takes the `-9` start.
pub const SYNC_FROM: [u16; 158] = [
    179, 178, 181, 180, 182, 183, 184, 185, 187, 186, 189, 188, 191, 190, 195, 194, 197, 196, 192,
    193, 683, 682, 684, 685, 687, 686, 691, 690, 693, 692, 695, 694, 697, 696, 699, 698, 701, 700,
    703, 702, 689, 688, 734, 735, 201, 200, 203, 202, 205, 204, 207, 206, 209, 208, 210, 211, 213,
    212, 214, 215, 216, 217, 218, 219, 221, 220, 222, 223, 224, 225, 226, 227, 229, 228, 230, 231,
    233, 232, 235, 234, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250,
    251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267, 268, 269,
    270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288,
    289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 730, 731,
    708, 709, 710, 711, 563, 564,
];

/// How the record whose primary row is `row` starts from the row the
/// channel plays (`playing`; `still` the record channel 0 shows when no
/// blend runs, as callback 15 reads it); None for a row no script names.
pub fn record_start(row: u16, playing: Option<u16>, still: Option<u16>) -> Option<RecordStart> {
    let script = RECORD_SCRIPTS.iter().find(|s| s.row == row)?;
    if script.share && playing.is_some_and(|r| MOVING_ROWS.contains(&r)) {
        return Some(RecordStart::Share);
    }
    if SYNC_ROWS.contains(&row) && still.is_some_and(|r| SYNC_FROM.contains(&r)) {
        return Some(RecordStart::Sync);
    }
    Some(RecordStart::Frame {
        frame: f32::from(script.still),
        heading: script.heading,
    })
}

/// A blend's start `-19` (`0x197fd0..0x198088`): the outgoing clip looked
/// ahead by the blend's length in updates at its rate, wrapped once past
/// its last frame, as a share of its length, on the incoming clip, the
/// share truncated. The outgoing clip is the channel's frame (`+0x64`) of
/// its clip, at the rate the new record's clip plays (`+0x68`, set before
/// the script runs), or, while a blend runs, that blend's start frame
/// (`+0x60`) of its target at 0.8 a frame. Matches the attract capture's 16
/// such blends.
pub fn share_start(frame: f32, rate: f32, length: u8, from_frames: usize, to_frames: usize) -> f32 {
    let n = from_frames.max(1) as f32;
    let mut f = frame + f32::from(length) * rate;
    if n - 1.0 < f {
        f -= n - 1.0;
    }
    ((f / n * to_frames as f32) as i32) as f32
}

/// The blend length of every gait record's script.
pub const RECORD_BLEND: u8 = 6;

/// The rate `-19` reads while a blend runs (`0x197fec`).
pub const SHARE_BLEND_RATE: f32 = 0.8;

#[rustfmt::skip]
pub const RECORD_SCRIPTS: [RecordScript; 111] = [
    RecordScript { row: 0, still: 4, heading: true, share: true }, // run_1
    RecordScript { row: 1, still: 4, heading: true, share: true }, // run_3
    RecordScript { row: 2, still: 4, heading: true, share: true }, // run_4
    RecordScript { row: 957, still: 4, heading: true, share: true }, // run_5
    RecordScript { row: 3, still: 5, heading: true, share: true }, // run_7
    RecordScript { row: 4, still: 5, heading: true, share: true }, // run_bishop
    RecordScript { row: 5, still: 4, heading: true, share: true }, // run_thegirl
    RecordScript { row: 7, still: 4, heading: true, share: true }, // run_thekid
    RecordScript { row: 8, still: 6, heading: true, share: true }, // run_hookup
    RecordScript { row: 9, still: 6, heading: true, share: true }, // run_penthouse
    RecordScript { row: 10, still: 6, heading: true, share: true }, // run_drj
    RecordScript { row: 11, still: 5, heading: true, share: true }, // run_bird
    RecordScript { row: 12, still: 5, heading: true, share: true }, // run_magic
    RecordScript { row: 13, still: 4, heading: true, share: true }, // run_pistol
    RecordScript { row: 14, still: 5, heading: true, share: true }, // run_dawkins
    RecordScript { row: 15, still: 5, heading: true, share: true }, // run_female_2
    RecordScript { row: 16, still: 4, heading: true, share: true }, // run_female_3
    RecordScript { row: 17, still: 4, heading: true, share: true }, // run_wilt
    RecordScript { row: 21, still: 5, heading: true, share: true }, // walk_1
    RecordScript { row: 22, still: 5, heading: true, share: true }, // walk_cocky_1
    RecordScript { row: 23, still: 8, heading: true, share: true }, // walk_cocky_2
    RecordScript { row: 28, still: 9, heading: true, share: true }, // walk_cocky_3
    RecordScript { row: 29, still: 7, heading: true, share: true }, // walk_bishop
    RecordScript { row: 30, still: 7, heading: true, share: true }, // walk_thegirl
    RecordScript { row: 24, still: 7, heading: true, share: true }, // walk_thekid
    RecordScript { row: 26, still: 7, heading: true, share: true }, // walk_hookup
    RecordScript { row: 27, still: 7, heading: true, share: true }, // walk_penthouse
    RecordScript { row: 32, still: 6, heading: true, share: true }, // walk_drj
    RecordScript { row: 33, still: 7, heading: true, share: true }, // walk_bird
    RecordScript { row: 34, still: 7, heading: true, share: true }, // walk_magic
    RecordScript { row: 35, still: 7, heading: true, share: true }, // walk_pistol
    RecordScript { row: 36, still: 7, heading: true, share: true }, // walk_dawkins
    RecordScript { row: 37, still: 7, heading: true, share: true }, // walk_female_2
    RecordScript { row: 38, still: 7, heading: true, share: true }, // walk_female_3
    RecordScript { row: 39, still: 6, heading: true, share: true }, // walk_wilt
    RecordScript { row: 18, still: 0, heading: false, share: true }, // jogbackwards_1
    RecordScript { row: 19, still: 0, heading: false, share: true }, // jogbackwards_2
    RecordScript { row: 20, still: 0, heading: false, share: true }, // jogbackwards_3
    RecordScript { row: 40, still: 0, heading: false, share: false }, // shuffle_right_1
    RecordScript { row: 41, still: 0, heading: false, share: false }, // shuffle_left_1
    RecordScript { row: 42, still: 0, heading: false, share: false }, // shuffle_forward_1
    RecordScript { row: 44, still: 0, heading: false, share: false }, // shuffle_back_1
    RecordScript { row: 713, still: 0, heading: true, share: false }, // into_backdown
    RecordScript { row: 714, still: 5, heading: true, share: false }, // from_backdown_back
    RecordScript { row: 708, still: 0, heading: true, share: false }, // backdown_spinout
    RecordScript { row: 710, still: 0, heading: false, share: false }, // backdown_stepout
    RecordScript { row: 48, still: 12, heading: true, share: false }, // stop_to_dribble
    RecordScript { row: 176, still: 0, heading: true, share: false }, // dribble_to_stop_1
    RecordScript { row: 174, still: 0, heading: true, share: false }, // dribble_to_stop_2
    RecordScript { row: 47, still: 11, heading: true, share: false }, // dribble_to_stop_3
    RecordScript { row: 706, still: 0, heading: true, share: false }, // dribble_stand_side_1
    RecordScript { row: 75, still: 0, heading: true, share: false }, // dribble_stand_1
    RecordScript { row: 77, still: 0, heading: true, share: false }, // dribble_stand_2
    RecordScript { row: 667, still: 0, heading: true, share: false }, // dribble_stand_flair_1
    RecordScript { row: 669, still: 12, heading: true, share: false }, // dribble_stand_flair_2
    RecordScript { row: 670, still: 0, heading: true, share: false }, // dribble_stand_flair_3
    RecordScript { row: 673, still: 0, heading: true, share: false }, // dribble_stand_flair_4
    RecordScript { row: 674, still: 0, heading: true, share: false }, // dribble_stand_flair_5
    RecordScript { row: 680, still: 0, heading: true, share: false }, // dribble_stand_flair_6
    RecordScript { row: 677, still: 0, heading: true, share: false }, // dribble_stand_dance
    RecordScript { row: 678, still: 0, heading: true, share: false }, // dribble_stand_elbow
    RecordScript { row: 141, still: 8, heading: true, share: false }, // dribble_crouch
    RecordScript { row: 704, still: 0, heading: true, share: false }, // dribble_backdown_1
    RecordScript { row: 79, still: 35, heading: true, share: true }, // dribble_run_1
    RecordScript { row: 80, still: 15, heading: true, share: true }, // dribble_run_3
    RecordScript { row: 88, still: 16, heading: true, share: true }, // dribble_run_4
    RecordScript { row: 90, still: 15, heading: true, share: true }, // dribble_run_bishop
    RecordScript { row: 92, still: 16, heading: true, share: true }, // dribble_run_thegirl
    RecordScript { row: 82, still: 13, heading: true, share: true }, // dribble_run_thekid
    RecordScript { row: 84, still: 16, heading: true, share: true }, // dribble_run_hookup
    RecordScript { row: 86, still: 16, heading: true, share: true }, // dribble_run_penthouse
    RecordScript { row: 94, still: 15, heading: true, share: true }, // dribble_run_drj
    RecordScript { row: 96, still: 14, heading: true, share: true }, // dribble_run_bird
    RecordScript { row: 98, still: 14, heading: true, share: true }, // dribble_run_magic
    RecordScript { row: 100, still: 15, heading: true, share: true }, // dribble_run_pistol
    RecordScript { row: 102, still: 14, heading: true, share: true }, // dribble_run_dawkins
    RecordScript { row: 104, still: 14, heading: true, share: true }, // dribble_run_female
    RecordScript { row: 106, still: 13, heading: true, share: true }, // dribble_run_female_2
    RecordScript { row: 108, still: 13, heading: true, share: true }, // dribble_run_wilt
    RecordScript { row: 126, still: 18, heading: true, share: true }, // dribble_skip_4
    RecordScript { row: 111, still: 7, heading: true, share: true }, // dribble_cocky_1
    RecordScript { row: 113, still: 21, heading: true, share: true }, // dribble_cocky_2
    RecordScript { row: 142, still: 24, heading: true, share: true }, // dribble_walk_2
    RecordScript { row: 156, still: 21, heading: true, share: true }, // dribble_walk_3
    RecordScript { row: 144, still: 23, heading: true, share: true }, // dribble_walk_bishop
    RecordScript { row: 146, still: 21, heading: true, share: true }, // dribble_walk_thegirl
    RecordScript { row: 148, still: 23, heading: true, share: true }, // dribble_walk_thekid
    RecordScript { row: 152, still: 24, heading: true, share: true }, // dribble_walk_hookup
    RecordScript { row: 154, still: 22, heading: true, share: true }, // dribble_walk_penthouse
    RecordScript { row: 158, still: 21, heading: true, share: true }, // dribble_walk_drj
    RecordScript { row: 160, still: 22, heading: true, share: true }, // dribble_walk_bird
    RecordScript { row: 162, still: 19, heading: true, share: true }, // dribble_walk_magic
    RecordScript { row: 164, still: 19, heading: true, share: true }, // dribble_walk_pistol
    RecordScript { row: 166, still: 21, heading: true, share: true }, // dribble_walk_dawkins
    RecordScript { row: 168, still: 18, heading: true, share: true }, // dribble_walk_female
    RecordScript { row: 170, still: 17, heading: true, share: true }, // dribble_walk_female_2
    RecordScript { row: 172, still: 20, heading: true, share: true }, // dribble_walk_wilt
    RecordScript { row: 114, still: 0, heading: true, share: true }, // dribble_2hand_underleg
    RecordScript { row: 129, still: 3, heading: true, share: true }, // dribble_2hand_xover
    RecordScript { row: 116, still: 2, heading: true, share: true }, // dribble_2hand_creep
    RecordScript { row: 121, still: 3, heading: true, share: true }, // dribble_2hand_creep_2
    RecordScript { row: 118, still: 1, heading: true, share: true }, // dribble_fakepass
    RecordScript { row: 123, still: 0, heading: true, share: true }, // dribble_underleg
    RecordScript { row: 131, still: 5, heading: true, share: true }, // dribble_dance
    RecordScript { row: 132, still: 22, heading: true, share: true }, // dribble_elbow
    RecordScript { row: 136, still: 6, heading: true, share: true }, // dribble_behind
    RecordScript { row: 140, still: 1, heading: true, share: true }, // dribble_underknee
    RecordScript { row: 138, still: 15, heading: true, share: true }, // dribble_thrulegs_behindback
    RecordScript { row: 134, still: 18, heading: true, share: true }, // dribble_spin
    RecordScript { row: 120, still: 13, heading: true, share: true }, // dribble_2hand_skip
    RecordScript { row: 125, still: 6, heading: true, share: true }, // dribble_2hand_hop
];

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

/// The rows record `index`'s script blends to, for the live hand to pick
/// from (`rows[hand == 2]`): most records' scripts take their rows as the
/// table holds them by the live hand (`8e f8`); the shuffles front and
/// back (records 2 and 3, `8e f2`) take their second row for a left-hander
/// whatever the hand (the third attract match's defender at update 812
/// shuffles on 42, not 43); record 72's script names its two rows the
/// other way round from the table.
pub fn script_rows(index: usize, rows: [u16; 2], left_handed: bool) -> [u16; 2] {
    match index {
        2 | 3 => {
            let r = if left_handed { rows[0] } else { rows[1] };
            [r, r]
        }
        72 => [rows[1], rows[0]],
        _ => rows,
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
