//! The original's gait records as the simulation picks walks, jogs and runs
//! from them ([`street_sim::gait_select::GaitConfig`]), with the hand flags
//! of the clips whose frames it follows: the held gait rows, the standing
//! dribble, and where each trick ends. Built by every machine from its own
//! disc ([`super::Disc`]); each player's profile is the match's own.

use street_assets::clip::Clips;
use street_assets::gait::GaitData;
use street_assets::scripts::{ClipTarget, MoveScripts};
use street_assets::timeline::ScriptBank;
use street_sim::gait_select::{GaitConfig, GaitRecord, hand_flags};

/// The standing dribble's clip, whose hand flags the held ball follows.
pub const STAND_BALL: &str = "dribble_stand_flair_5_r";

/// The clip a native trick plays: its first blend's primary target, or on
/// the live left hand (`side` 1) its 0xf8 alternate (`FUN_001978b0`). The
/// other prefixes' conditions are not evaluated; they play the primary.
pub fn trick_target<'a>(
    scripts: &'a MoveScripts,
    clips: &Clips,
    script: u16,
    side: u8,
) -> Option<&'a ClipTarget> {
    let script = scripts.playback_script(street_assets::moves::Kind::Trick, usize::from(script))?;
    let blend = script.blends.iter().find(|b| {
        b.targets
            .first()
            .and_then(|t| t.name.as_deref())
            .is_some_and(|name| clips.get(name).is_some())
    })?;
    let alternate = (side == 1)
        .then(|| blend.selectors.iter().position(|&s| s == 0xf8))
        .flatten()
        .and_then(|k| blend.targets.get(k + 1))
        .filter(|t| {
            t.name
                .as_deref()
                .is_some_and(|name| clips.get(name).is_some())
        });
    alternate.or_else(|| blend.targets.first())
}

/// The gait records and the clips' hand flags, with no profiles.
pub fn tables(
    clips: &Clips,
    timeline: &ScriptBank,
    scripts: &MoveScripts,
    gait: &GaitData,
) -> GaitConfig {
    // A clip's hand flags per frame, its state words mirrored as the
    // animation record asks (`FUN_00191440`).
    let hands = |name: &str, mirrored: bool| -> Vec<u8> {
        clips.get(name).map_or_else(Vec::new, |clip| {
            (0..clip.states.len())
                .map(|f| hand_flags(clip.state_at_mirrored(f, mirrored)))
                .collect()
        })
    };
    let row_hands = |row: u16| {
        let a = timeline.animations.get(usize::from(row))?;
        Some(hands(a.name.as_deref()?, a.mirrored))
    };
    let mut rows: Vec<u16> = gait
        .records
        .iter()
        .filter(|r| r.flags & 2 != 0)
        .flat_map(|r| r.rows)
        .collect();
    rows.sort_unstable();
    rows.dedup();
    // Each trick's clip on either side, as the view plays it.
    let frames = |target: Option<&ClipTarget>| {
        target
            .and_then(|t| Some(hands(t.name.as_deref()?, t.mirrored)))
            .unwrap_or_default()
    };
    let trick_hands = (0..scripts.tricks.len())
        .map(|id| {
            let id = id as u16;
            (
                id,
                [
                    frames(trick_target(scripts, clips, id, 0)),
                    frames(trick_target(scripts, clips, id, 1)),
                ],
            )
        })
        .collect();
    GaitConfig {
        records: gait
            .records
            .iter()
            .map(|r| GaitRecord {
                rows: r.rows,
                category_mask: r.category_mask,
                player: r.player_restriction,
                minimum_handles: r.minimum_handles,
                root_distance: r.root_distance,
                motion: [r.minimum_motion, r.maximum_motion],
                flags: r.flags,
            })
            .collect(),
        profiles: Vec::new(),
        row_hands: rows
            .into_iter()
            .filter_map(|row| Some((row, row_hands(row)?)))
            .collect(),
        stand_hands: hands(STAND_BALL, false),
        trick_hands,
    }
}
