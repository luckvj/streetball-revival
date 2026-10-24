//! The disc's data a match's simulation reads beside its game state: the
//! pose bank ([`crate::pose`]), the gait records with the hand flags of the
//! clips they follow ([`crate::gait_select`]), and the staged Level 2
//! gamebreakers' plans with their hand tracks ([`crate::gamebreaker`]).
//!
//! Every machine builds it from its own disc (`street-net`'s `disc`: the
//! game for a match played alone, the referee and every client online,
//! the same way), and a world holds it fixed for the match. Snapshots and
//! checksums leave it out: they carry only the match's own values (each
//! player's gait profile, the staged players' model scales and width
//! ratios), so a world read from one (an online match's) holds those values
//! and no tables until [`World::install`] puts this machine's tables under
//! them, and it refuses to step until then ([`World::step`]). The machines
//! in an online match compare a digest of this data before it starts
//! (`street-net`'s `disc::Disc::digest`).

use std::sync::Arc;

use crate::World;
use crate::gait_select::GaitConfig;
use crate::gamebreaker::Config;
use crate::pose::{Bank, Poses};

/// The disc's tables, with none of a match's own values.
#[derive(Clone, Debug)]
pub struct Data {
    /// The skeleton and every animation record's clip.
    pub poses: Arc<Poses>,
    /// The 108 gait records and the hand flags of the held gait rows, the
    /// standing dribble and the tricks; no profiles.
    pub gait: GaitConfig,
    /// The staged gamebreakers' families, regions, plans and hand tracks;
    /// no players' scales.
    pub staged: Config,
}

impl World {
    /// The match's own disc values with no tables, as a world read from a
    /// snapshot holds them: each player's gait profile and the staged
    /// players' model scales and width ratios. [`World::install`] puts the
    /// tables under them.
    pub fn set_match_values(
        &mut self,
        profiles: Vec<crate::gait_select::GaitProfile>,
        staged_scales: Vec<f32>,
        staged_width_ratios: Vec<f32>,
    ) {
        self.gait = Some(Arc::new(GaitConfig {
            profiles,
            ..GaitConfig::default()
        }));
        self.staged = Some(Arc::new(Config {
            player_scales: staged_scales,
            player_width_ratios: staged_width_ratios,
            ..Config::default()
        }));
        self.poses = Bank::Missing;
    }

    /// Install this machine's disc data under the match's own values: the
    /// pose bank, and the gait records and staged plans each with the
    /// world's profiles and players' scales. A match needs all three (a
    /// world without them would play the generic walk and no staged
    /// gamebreakers, another game), so a world lacking its own values is
    /// refused, and so is one the tables don't fit ([`World::validate`]).
    pub fn install(&mut self, data: &Data) -> Result<(), String> {
        let profiles = self
            .gait
            .as_ref()
            .map(|g| g.profiles.clone())
            .ok_or("the match has no gait profiles")?;
        if profiles.len() != self.players.len() {
            return Err(format!(
                "the match has {} gait profiles for {} players",
                profiles.len(),
                self.players.len()
            ));
        }
        let (scales, ratios) = self
            .staged
            .as_ref()
            .map(|s| (s.player_scales.clone(), s.player_width_ratios.clone()))
            .ok_or("the match has no staged gamebreakers' players")?;
        let staged = Config {
            player_scales: scales,
            player_width_ratios: ratios,
            ..data.staged.clone()
        };
        staged.validate()?;
        self.gait = Some(Arc::new(GaitConfig {
            profiles,
            ..data.gait.clone()
        }));
        self.staged = Some(Arc::new(staged));
        self.poses = Bank::Disc(data.poses.clone());
        self.validate()
            .map_err(|e| format!("the disc's data doesn't fit the match: {e}"))
    }
}

#[cfg(any(test, feature = "stand-ins"))]
impl World {
    /// `original`'s tables under this world's own disc values, as
    /// [`World::install`] puts a machine's disc's: for tests that read a
    /// world back from a snapshot and compare it whole (test builds only).
    pub fn with_tables_of(mut self, original: &World) -> World {
        if let (Some(mine), Some(theirs)) = (&self.gait, &original.gait) {
            self.gait = Some(Arc::new(GaitConfig {
                profiles: mine.profiles.clone(),
                ..(**theirs).clone()
            }));
        }
        if let (Some(mine), Some(theirs)) = (&self.staged, &original.staged) {
            self.staged = Some(Arc::new(Config {
                player_scales: mine.player_scales.clone(),
                player_width_ratios: mine.player_width_ratios.clone(),
                ..(**theirs).clone()
            }));
        }
        self.poses = original.poses.clone();
        self
    }
}
