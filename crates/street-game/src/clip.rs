//! The game's animations used in a match, by role. Every name is a clip in
//! the disc's animation banks (see `anim_report`).

pub const STAND: &str = "STANDRELAX_ANTICIPATION_01_2";
pub const DEFEND: &str = "defensivestand02_faceup";
pub const WALK: &str = "WALK_CYCLE_NOBALL_01_3";
pub const RUN: &str = "RUN_CYCLE_NOBALL_01_2";
pub const TURBO: &str = "RUN_CYCLE_NOBALL_06_1";
pub const STAND_BALL: &str = street_net::disc::gait::STAND_BALL;
pub const WALK_BALL: &str = "WALK_CYCLE_WBALL_01_3_R";
pub const RUN_BALL: &str = "RUN_CYCLE_WBALL_01_4_R";
pub const TURBO_BALL: &str = "RUN_CYCLE_WBALL_06_1_R";
pub const SHOT: &str = "jumpshot_r_3";
pub const DUNK: &str = "dunk_1hnd_r_1";
pub const PASS: &str = "jogpass_2handchest";
pub const STEAL: &str = "swipe_in_mid_l_1";
pub const BLOCK: &str = "STANDBLOCK_FREESTYLE01_2_R";
pub const REBOUND: &str = "rebound_l_1";
/// The celebrations' clips and the loser's reactions at the win, the rows
/// of `street_sim::trick::CELEBRATE_ROWS` and `DEJECT_ROWS` in order (the
/// simulation picks and plays them as reactions).
pub const CELEBRATIONS: [&str; 21] = [
    "celeb01_FINAL",
    "celeb03_FINAL",
    "celeb05_FINAL",
    "jogceleb_bringdownhouse_FINAL",
    "jogceleb_sidehistep_FINAL",
    "stand_dance_1",
    "CELEBRATE_FREESTYLE_08_1",
    "CELEBRATE_FREESTYLE_14_2",
    "CELEBRATE_FREESTYLE_16_2",
    "CELEBRATE_FREESTYLE_16_3",
    "CELEBRATE_FREESTYLE07_1",
    "CELEBRATE_FREESTYLE11_1",
    "CELEBRATE_FREESTYLE02_4",
    "CELEBRATE_FREESTYLE03_1",
    "CELEBRATE_FREESTYLE03_3",
    "CELEBRATE_FREESTYLE05_1",
    "SIG_MOVE_PICKUP_08_4",
    "REACTIONS_FREESTYLE_01_2",
    "REACTIONS_FREESTYLE_02_4",
    "REACTIONS_FREESTYLE_03_1",
    "REACTIONS_FREESTYLE_04_2",
];
/// The winners' outros (`/ANIM/ANIBOUT.BIN`, the generic five).
pub const OUTROS: [&str; 5] = [
    "OUTRO_GENERIC_1_1_L",
    "OUTRO_GENERIC_2_1_L",
    "OUTRO_GENERIC_3_1_L",
    "OUTRO_GENERIC_4_2_L",
    "OUTRO_GENERIC_5_1_L",
];
pub const ADVANCED_DUNK: &str = "SUPERDUNK_360_TOMMAHAWK_R";
pub const COUNTER: &str = "STEALCOUNTER_1_2";
pub const GAMEBREAKER_SHORT: &str = "GBSHOTLVL1_SHORT_RANGE_3_R";
pub const GAMEBREAKER_LONG: &str = "GBSHOTLVL1_LONG_RANGE_5_R";
pub const GAMEBREAKER_2: &str = "GBSHOT_PRAYER_01_8_R";
pub const OOP: &str = "alleyoop05_overhead_r";
pub const KICK_PASS: &str = "KICKPASS01_3_R";
pub const HEEZAY: &str = "PASSOUT_OFFDEFENDER_NODRIBBLE_5_R";
pub const GLASS: &str = "PASSOUT_OFFBACKBOARD_NODRIBBLE_7_R";
pub const LOB: &str = "postpass_2handlob_l";
pub const GIVE_AND_GO: &str = "relaxpass_2handchest";
pub const LAYUP: &str = "1footjoglayup01_notrans_l";
pub const PUMP_FAKE: &str = "STANDDRIB_FAKEPULLUP_2_L";
pub const FADEAWAY: &str = "jumpshot_fade_r_2";
pub const PUTBACK: &str = "DNK_PUTBACK_ONEHAND_01_4_R";
pub const TURBO_BLOCK: &str = "standblock08_goaltend_notrans";
pub const DIVE: &str = "diveforball_notrans";
/// Trick moves by level (1 to 5), picked by the move's variant. The fifth
/// level is the all-turbo advanced trick; it reuses the flashiest clips of
/// the lower levels (the bank's own advanced moves are not mapped yet).
pub const TRICKS: [[&str; 4]; 5] = [
    [
        "jogdribxover02_fin_r_switch",
        "jogdribinsideout01_r",
        "standdrib_spin45_l",
        "jogdribinsideout03_l",
    ],
    [
        "jogdribbehindback_l",
        "jogdrib360spin_l",
        "standdrib_xoverbetweenlegs_l",
        "jogdribxover05_double_l",
    ],
    [
        "standdrib360spinthrulegs_l",
        "standdrib_xoveraroundhead_l",
        "STANDDRIB_SHAMGOD_5_R",
        "jogdribxover07_killer2_r",
    ],
    [
        "STANDDRIB_KICKTHRULEGS_4_R",
        "standdrib_xoverbehindheadthru_l",
        "standdribflair_jumpballxover_l",
        "standdrib_thrudefenderslegs_l",
    ],
    [
        "standdrib360spinthrulegs_l",
        "STANDDRIB_SHAMGOD_5_R",
        "standdribflair_jumpballxover_l",
        "STANDDRIB_KICKTHRULEGS_4_R",
    ],
];
/// Faked-out defenders, picked by player.
pub const STUMBLES: [&str; 4] = [
    "react_brokenankles",
    "react_stumbleback",
    "react_sidestumble_r",
    "standfallbackward_notrans",
];

/// Every clip the game plays.
pub fn all() -> Vec<&'static str> {
    let mut all = vec![
        CREATE_STAND,
        CREATE_LOOK,
        STAND,
        DEFEND,
        WALK,
        RUN,
        TURBO,
        STAND_BALL,
        WALK_BALL,
        RUN_BALL,
        TURBO_BALL,
        SHOT,
        DUNK,
        PASS,
        STEAL,
        BLOCK,
        REBOUND,
        ADVANCED_DUNK,
        COUNTER,
        GAMEBREAKER_SHORT,
        GAMEBREAKER_LONG,
        GAMEBREAKER_2,
        OOP,
        KICK_PASS,
        HEEZAY,
        GLASS,
        LOB,
        GIVE_AND_GO,
        LAYUP,
        PUMP_FAKE,
        FADEAWAY,
        PUTBACK,
        TURBO_BLOCK,
        DIVE,
    ];
    all.extend(TRICKS.iter().flatten());
    all.extend(CELEBRATIONS);
    all.extend(OUTROS);
    all.extend(STUMBLES);
    all
}

/// The create-a-player idle (front-end bank), for the Create A Baller preview:
/// animation 0x477 of the table at `0x3de8a0` (`FUN_002dc4e0`).
pub const CREATE_STAND: &str = "createplayer_stand";
/// The viewer's head pose (animation 0x476, `FUN_002dc4a8`): Look, Hair and
/// Facial Hair.
pub const CREATE_LOOK: &str = "createplayer_look";

/// Database record and equipped XDBMOVE rows for each on-court player.
#[derive(bevy::prelude::Resource)]
pub struct PlayerMoves(pub Vec<(usize, [Vec<u8>; 3])>);

/// The PS2 turbo bits used by the executable's move-mask tables.
fn turbo_mask(buttons: u16) -> u32 {
    use street_sim::button;
    [
        (button::L1, 0x4000),
        (button::R1, 0x8000),
        (button::L2, 0x10000),
        (button::R2, 0x20000),
    ]
    .into_iter()
    .filter(|&(bit, _)| buttons & bit != 0)
    .map(|(_, mask)| mask)
    .sum()
}

/// Select the equipped script through the same row-to-id map as the original.
/// Animation VM callbacks are not simulated here; the first loaded primary
/// clip is used. Handed alternatives and later phases remain in the parser.
pub struct Selection<'a> {
    pub name: &'a str,
    pub mirrored: bool,
}

/// Native slot positions, with a bounded migration for older compact saves.
/// Fully sized or sentinel-bearing lists already carry native positions.
pub fn native_move_slots(disc: &crate::setup::Disc, kind: usize, rows: &[u8]) -> Vec<u8> {
    use street_sim::story::{EMPTY_MOVE_SLOT, MOVE_SLOTS};
    let mut slots = vec![EMPTY_MOVE_SLOT; MOVE_SLOTS[kind]];
    let valid = |row: u8| {
        disc.moves
            .get(usize::from(row))
            .is_some_and(|m| m.kind == kind as i16)
    };
    if rows.len() == slots.len() || rows.contains(&EMPTY_MOVE_SLOT) || kind == 0 {
        for (dst, &row) in slots.iter_mut().zip(rows) {
            if valid(row) {
                *dst = row;
            }
        }
    } else {
        for &row in rows {
            let Some(m) = disc.moves.get(usize::from(row)).filter(|_| valid(row)) else {
                continue;
            };
            let range = match m.level {
                4 => 0..1,
                3 => 1..5,
                2 => 5..11,
                _ => continue,
            };
            if let Some(slot) = range.into_iter().find(|&i| slots[i] == EMPTY_MOVE_SLOT) {
                slots[slot] = row;
            }
        }
    }
    slots
}

/// The record and equipped rows a player's moves are looked up by: a
/// roster.json player's animation source's (`street_assets::roster_edits`).
fn routed(moves: &PlayerMoves, player: usize) -> Option<(usize, [Vec<u8>; 3])> {
    let (record, equipped) = moves.0.get(player)?;
    Some(street_assets::roster_edits::route_moves(*record, equipped))
}

/// 0x13a1f8 / 0x1d8778: read a specific equipped slot, then native defaults.
pub fn move_script_id(
    clips: &crate::setup::GameClips,
    moves: &PlayerMoves,
    disc: &crate::setup::Disc,
    player: usize,
    kind: street_assets::moves::Kind,
    slot: usize,
) -> Option<u16> {
    use street_assets::moves::Kind;
    // roster.json: the animation source's record and equipped rows.
    let (record, equipped) = &routed(moves, player)?;
    let category = match kind {
        Kind::Shot => 0,
        Kind::Dunk => 1,
        Kind::Trick => 2,
    };
    let rows = native_move_slots(disc, category, &equipped[category]);
    let row = *rows.get(slot)?;
    let equipped_row = (row != street_sim::story::EMPTY_MOVE_SLOT).then_some(usize::from(row));
    // 0x13a1f8 branches on the record's field 3 (created, generic, other).
    let team = disc.players.get(*record).map(|p| p.team);
    clips
        .scripts
        .slot_script(kind, *record, team, slot, equipped_row)
}

/// Resolve all direct dribble slots at match setup, retaining equipped rows
/// separately from native script ids. Native mask levels come from 0x43d408;
/// equipped slots cannot move an action into a different value class.
pub fn resolved_trick_slots(
    clips: &crate::setup::GameClips,
    moves: &PlayerMoves,
    disc: &crate::setup::Disc,
    player: usize,
) -> Option<street_sim::NativeTricks> {
    let (_, equipped) = &routed(moves, player)?;
    let rows = native_move_slots(disc, 2, &equipped[2]);
    let mut slots = [street_sim::TrickIdentity {
        slot: 0,
        script: 0,
        level: 0,
        equipped_row: None,
    }; 11];
    for (slot, identity) in slots.iter_mut().enumerate() {
        let script = move_script_id(
            clips,
            moves,
            disc,
            player,
            street_assets::moves::Kind::Trick,
            slot,
        )?;
        clips.scripts.tricks.get(usize::from(script))?;
        *identity = street_sim::TrickIdentity {
            slot: slot as u8,
            script,
            level: street_sim::player::TRICK_SLOT_LEVELS[slot],
            equipped_row: (rows[slot] != street_sim::story::EMPTY_MOVE_SLOT).then_some(rows[slot]),
        };
    }
    Some(street_sim::NativeTricks { slots })
}

/// Who the shot pick sees (`FUN_0016b4b8`): the animation source's record,
/// its field 12 (female) and the six shot slots' ids (`0x13a1f8(record, 2,
/// +0x258, slot)`).
pub fn resolved_shooter(
    clips: &crate::setup::GameClips,
    moves: &PlayerMoves,
    disc: &crate::setup::Disc,
    player: usize,
) -> Option<street_sim::Shooter> {
    let (record, _) = routed(moves, player)?;
    let mut slots = [0u8; 6];
    for (slot, id) in slots.iter_mut().enumerate() {
        let script = move_script_id(
            clips,
            moves,
            disc,
            player,
            street_assets::moves::Kind::Shot,
            slot,
        )?;
        *id = u8::try_from(script).ok().filter(|&id| id < 27)?;
    }
    Some(street_sim::Shooter {
        record: u16::try_from(record).ok()?,
        female: disc.players.get(record).is_some_and(|p| p.female),
        slots,
    })
}

/// The dunk slots (a bit each, eleven) whose dunk is advanced: the dunk
/// id's flag word at `0x43f5f0` has 0x800 (`FUN_00171de8`). The default set
/// has one, its four-turbo slot (dunk 48); a created player's equipped
/// dunks bring their own.
pub fn advanced_dunk_slots(
    clips: &crate::setup::GameClips,
    moves: &PlayerMoves,
    disc: &crate::setup::Disc,
    player: usize,
) -> Option<u16> {
    let mut mask = 0;
    for slot in 0..11 {
        let id = move_script_id(
            clips,
            moves,
            disc,
            player,
            street_assets::moves::Kind::Dunk,
            slot,
        )?;
        let flags = clips.scripts.dunk_flags.get(usize::from(id))?;
        if flags & street_assets::scripts::ADVANCED_DUNK != 0 {
            mask |= 1 << slot;
        }
    }
    Some(mask)
}

pub fn move_selection<'a>(
    clips: &'a crate::setup::GameClips,
    moves: &PlayerMoves,
    disc: &crate::setup::Disc,
    player: usize,
    kind: street_assets::moves::Kind,
    buttons: u16,
    _level: u8,
    variant: usize,
) -> Option<Selection<'a>> {
    use street_assets::moves::{Kind, TURBO_SHOTS};
    let mask = turbo_mask(buttons);
    let slot = match kind {
        Kind::Shot => TURBO_SHOTS.iter().position(|&m| m == mask).unwrap_or(0),
        _ => street_sim::NativeTricks::slot(buttons).unwrap_or(5 + variant % 6),
    };
    let id = move_script_id(clips, moves, disc, player, kind, slot)?;
    script_selection_with_variant(clips, kind, id, variant)
}

/// Render a script identity already selected by authoritative simulation.
#[allow(dead_code)]
pub fn script_selection(
    clips: &crate::setup::GameClips,
    kind: street_assets::moves::Kind,
    id: u16,
) -> Option<Selection<'_>> {
    script_selection_with_variant(clips, kind, id, 0)
}

/// Render a script identity taking into account move variants and alternate blend targets.
/// The clip a native trick plays: its first blend's primary target, or on
/// the live left hand (`side` 1) its 0xf8 alternate (`FUN_001978b0`). The
/// other prefixes' conditions are not evaluated; they play the primary.
pub fn trick_target(
    clips: &crate::setup::GameClips,
    script: u16,
    side: u8,
) -> Option<&street_assets::scripts::ClipTarget> {
    street_net::disc::gait::trick_target(&clips.scripts, &clips.clips, script, side)
}

pub fn script_selection_with_variant(
    clips: &crate::setup::GameClips,
    kind: street_assets::moves::Kind,
    id: u16,
    variant: usize,
) -> Option<Selection<'_>> {
    let script = clips.scripts.playback_script(kind, usize::from(id))?;
    for blend in &script.blends {
        if !blend.targets.is_empty() {
            let target_idx = variant % blend.targets.len();
            let target = &blend.targets[target_idx];
            if let Some(name) = target.name.as_deref()
                && clips.clips.get(name).is_some()
            {
                return Some(Selection {
                    name,
                    mirrored: target.mirrored,
                });
            }
        }
    }
    for blend in &script.blends {
        for target in &blend.targets {
            if let Some(name) = target.name.as_deref()
                && clips.clips.get(name).is_some()
            {
                return Some(Selection {
                    name,
                    mirrored: target.mirrored,
                });
            }
        }
    }
    if !script.clip_targets.is_empty() {
        let name = &script.clip_targets[variant % script.clip_targets.len()];
        if clips.clips.get(name.as_str()).is_some() {
            return Some(Selection {
                name: name.as_str(),
                mirrored: false,
            });
        }
    }
    script
        .clip_targets
        .iter()
        .map(String::as_str)
        .find(|&name| clips.clips.get(name).is_some())
        .map(|name| Selection {
            name,
            mirrored: false,
        })
}

#[cfg(test)]
pub fn move_clip<'a>(
    clips: &'a crate::setup::GameClips,
    moves: &PlayerMoves,
    disc: &crate::setup::Disc,
    player: usize,
    kind: street_assets::moves::Kind,
    buttons: u16,
    level: u8,
    variant: usize,
) -> Option<&'a str> {
    move_selection(clips, moves, disc, player, kind, buttons, level, variant).map(|s| s.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_equipped_slots_do_not_cycle_and_legend_defaults_are_slot_specific() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut disc =
            crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = crate::setup::load_clips(&mut disc.iso).unwrap();
        use street_assets::moves::Kind;
        use street_sim::story::EMPTY_MOVE_SLOT;
        let row = disc
            .moves
            .iter()
            .find(|m| m.kind == 2 && clips.scripts.row_script(m.index) == Some(69))
            .unwrap()
            .index as u8;
        let mut equipped: [Vec<u8>; 3] = Default::default();
        equipped[2] = vec![EMPTY_MOVE_SLOT; 11];
        equipped[2][10] = row;
        let moves = PlayerMoves(vec![(234, equipped)]);
        assert_eq!(
            move_script_id(&clips, &moves, &disc, 0, Kind::Trick, 10),
            Some(69)
        );
        assert_eq!(
            move_script_id(&clips, &moves, &disc, 0, Kind::Trick, 5),
            Some(clips.scripts.default_tricks[5])
        );
        let native = resolved_trick_slots(&clips, &moves, &disc, 0).unwrap();
        assert_eq!(
            native.slots[10],
            street_sim::TrickIdentity {
                slot: 10,
                script: 69,
                level: 2,
                equipped_row: Some(row),
            }
        );
        assert_eq!(native.slots[5].script, clips.scripts.default_tricks[5]);
        assert_eq!(native.slots[5].equipped_row, None);
        // A different equipped script replaces only its native slot.
        let replacement = disc
            .moves
            .iter()
            .find(|m| {
                m.kind == 2
                    && m.level == 2
                    && clips.scripts.row_script(m.index).is_some_and(|id| id != 69)
            })
            .unwrap()
            .index as u8;
        let mut replaced = PlayerMoves(moves.0.clone());
        replaced.0[0].1[2][10] = replacement;
        let replacement_native = resolved_trick_slots(&clips, &replaced, &disc, 0).unwrap();
        assert_eq!(
            replacement_native.slots[10].script,
            clips.scripts.row_script(usize::from(replacement)).unwrap()
        );
        assert_eq!(replacement_native.slots[10].equipped_row, Some(replacement));
        assert_eq!(replacement_native.slots[10].level, 2);
        replaced.0[0].1[2][10] = EMPTY_MOVE_SLOT;
        let empty = resolved_trick_slots(&clips, &replaced, &disc, 0).unwrap();
        assert_eq!(empty.slots[10].script, 69);
        assert_eq!(empty.slots[10].equipped_row, None);
        let migrated = native_move_slots(&disc, 2, &[row]);
        assert_eq!(migrated[5], row);
        assert_eq!(migrated[10], EMPTY_MOVE_SLOT);
        let legends = PlayerMoves(vec![(0x91, Default::default())]);
        let native_legend = resolved_trick_slots(&clips, &legends, &disc, 0).unwrap();
        assert_eq!(native_legend.slots[1].script, 0x3c);
        assert_eq!(native_legend.slots[1].level, 3);
        assert_eq!(native_legend.slots[10].script, 69);
        assert_eq!(
            move_script_id(&clips, &legends, &disc, 0, Kind::Trick, 1),
            Some(0x3c)
        );
        assert_eq!(
            move_script_id(&clips, &legends, &disc, 0, Kind::Dunk, 1),
            Some(0x35)
        );
        assert_eq!(
            move_script_id(&clips, &legends, &disc, 0, Kind::Dunk, 0),
            Some(clips.scripts.default_dunks[0])
        );
        assert_eq!(
            move_script_id(&clips, &legends, &disc, 0, Kind::Shot, 1),
            Some(clips.scripts.default_shots[1])
        );
    }

    #[test]
    fn slot_scripts_follow_0x13a1f8_by_the_records_team() {
        // Expected ids straight from the executable: the generic team's
        // fixed rows (FUN_001d9e60, u16 at 0x45fde0/0x45fdf0/0x45fe08)
        // through the row table 0x45f9c8; the created team's equipped rows
        // through the same table; the defaults at 0x43d6c0/0x43d5d0/0x43d498.
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut disc =
            crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = crate::setup::load_clips(&mut disc.iso).unwrap();
        let elf = disc.iso.read("/SLUS_206.51").unwrap();
        let word = |a: u32| street_assets::scripts::executable_words(&elf, a, 1).unwrap()[0];
        let half = |a: u32| (word(a & !3) >> (8 * (a & 2))) & 0xffff;
        use street_assets::moves::Kind;
        use street_assets::scripts::{CREATED_TEAM, GENERIC_TEAM};
        let generic = disc
            .players
            .iter()
            .position(|p| p.team == GENERIC_TEAM)
            .expect("a generic streetballer");
        let created = disc
            .players
            .iter()
            .position(|p| p.team == CREATED_TEAM)
            .expect("a created-player record");
        for (category, kind, rows, defaults) in [
            (0usize, Kind::Shot, 0x45fde0u32, 0x43d6c0u32),
            (1, Kind::Dunk, 0x45fdf0, 0x43d5d0),
            (2, Kind::Trick, 0x45fe08, 0x43d498),
        ] {
            let slots = street_sim::story::MOVE_SLOTS[category];
            let moves = PlayerMoves(vec![(generic, Default::default())]);
            for slot in 0..slots {
                let row = half(rows + 2 * slot as u32);
                let expected = if row == 0xffff {
                    word(defaults + 4 * slot as u32)
                } else {
                    word(0x45f9c8 + 4 * row)
                };
                assert_eq!(
                    move_script_id(&clips, &moves, &disc, 0, kind, slot),
                    Some(expected as u16),
                    "generic {kind:?} slot {slot}"
                );
            }
            // A created player's equipped row, then an empty slot.
            let row = disc
                .moves
                .iter()
                .find(|m| m.kind == category as i16)
                .unwrap()
                .index;
            let mut equipped: [Vec<u8>; 3] = Default::default();
            equipped[category] = vec![street_sim::story::EMPTY_MOVE_SLOT; slots];
            equipped[category][0] = row as u8;
            let moves = PlayerMoves(vec![(created, equipped)]);
            assert_eq!(
                move_script_id(&clips, &moves, &disc, 0, kind, 0),
                Some(word(0x45f9c8 + 4 * row as u32) as u16)
            );
            assert_eq!(
                move_script_id(&clips, &moves, &disc, 0, kind, 1),
                Some(word(defaults + 4) as u16)
            );
        }
    }

    #[test]
    fn the_shooter_carries_the_records_sex_and_shot_slots() {
        // `FUN_0016b4b8`'s view: record 172 (Bird's shot) with the shared
        // set's slots (`0x43d6c0`), and a woman's field 12.
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut disc =
            crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = crate::setup::load_clips(&mut disc.iso).unwrap();
        let moves = PlayerMoves(vec![(172, Default::default())]);
        let bird = resolved_shooter(&clips, &moves, &disc, 0).unwrap();
        assert_eq!(bird.record, 172);
        assert!(!bird.female);
        assert_eq!(bird.slots.map(usize::from), street_sim::timing::SHOT_SLOTS);
        let woman = disc.players.iter().position(|p| p.female).unwrap();
        let moves = PlayerMoves(vec![(woman, Default::default())]);
        assert!(resolved_shooter(&clips, &moves, &disc, 0).unwrap().female);
    }

    #[test]
    fn released_trick_buttons_keep_the_disc_script_selection() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut disc =
            crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = crate::setup::load_clips(&mut disc.iso).unwrap();
        let moves = PlayerMoves(vec![(0, Default::default())]);
        let mut world = street_sim::World::new(
            [
                street_sim::Hoop {
                    rim: [36.95, 11.75, 0.0],
                    rim_radius: 0.75,
                },
                street_sim::Hoop {
                    rim: [-36.96, 11.75, 0.0],
                    rim_radius: 0.75,
                },
            ],
            vec![street_sim::Player::new(0, [0.0, 0.0], 0.0)],
            street_sim::Rules::default(),
            5,
        );
        world.players[0].native_tricks = resolved_trick_slots(&clips, &moves, &disc, 0);
        world.phase = street_sim::Phase::Live;
        world.ball.state = street_sim::BallState::Held(0);
        // Dribbling (a fresh take standing still clears out instead).
        world.players[0].dribble = 1;
        let initiating =
            street_sim::button::TRICK | street_sim::button::L2 | street_sim::button::R2;
        world.step(&[street_sim::Input::default().with(initiating)]);
        let selected = |world: &street_sim::World| {
            let street_sim::Action::Trick { id: identity, .. } = world.players[0].action else {
                panic!("expected an active native trick");
            };
            assert_eq!(identity.level, 2);
            assert_eq!(identity.script, 69);
            script_selection(&clips, street_assets::moves::Kind::Trick, identity.script)
                .unwrap()
                .name
        };
        let initial = selected(&world);
        // L2+R2 is native slot 10, default script 69. Verify against the
        // executable parser rather than a hard-coded animation name.
        let self_toss = &clips.scripts.tricks[69];
        assert!(self_toss.clip_targets.iter().any(|name| name == initial));
        let restored = street_sim::World::from_snapshot(&world.to_snapshot()).unwrap();
        assert_eq!(selected(&restored), initial);
        for _ in 0..8 {
            world.step(&[street_sim::Input::default()]);
            assert_eq!(selected(&world), initial);
        }
    }

    #[test]
    fn equipped_rows_play_their_own_scripts_from_the_disc() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut disc =
            crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = crate::setup::load_clips(&mut disc.iso).unwrap();
        let mut checked = [0; 3];
        for row in &disc.moves {
            let category = row.kind as usize;
            let kind = [
                street_assets::moves::Kind::Shot,
                street_assets::moves::Kind::Dunk,
                street_assets::moves::Kind::Trick,
            ][category];
            let mut equipped: [Vec<u8>; 3] = Default::default();
            equipped[category] =
                vec![street_sim::story::EMPTY_MOVE_SLOT; street_sim::story::MOVE_SLOTS[category]];
            equipped[category][if category == 0 { 0 } else { 5 }] = row.index as u8;
            let moves = PlayerMoves(vec![(234, equipped)]);
            let Some(script) = clips
                .scripts
                .row_script(row.index)
                .and_then(|id| clips.scripts.playback_script(kind, usize::from(id)))
            else {
                continue;
            };
            let chosen = move_clip(
                &clips,
                &moves,
                &disc,
                0,
                kind,
                street_sim::button::L1 | street_sim::button::R1,
                row.level as u8 + 1,
                0,
            )
            .unwrap();
            assert!(
                script.clip_targets.iter().any(|n| n == chosen),
                "{} selected {chosen}",
                row.name
            );
            checked[category] += 1;
        }
        assert!(checked.iter().all(|&n| n > 0), "{checked:?}");
    }
}
