//! Bevy rendering for Streetball Revival: turns what `street-assets` decodes from the
//! disc into meshes, materials and skinned, animated players.
//!
//! - [`convert`]: textures and meshes to Bevy assets
//! - [`court`]: spawn a court's models where the game puts them
//! - [`crowd`]: the animated crowd in its seats
//! - [`hires`]: HD texture packs standing in for the disc's images
//! - [`net`]: the nets, a cloth hanging from each rim
//! - [`player`]: load and spawn a skinned player
//! - [`props`]: stand-alone models such as the ball
//! - [`shadow`]: players' shadows, projected along the court's sun
//! - [`animation`]: play and crossfade the game's animations on a player
//! - [`app`]: finding the disc, screenshots and other program plumbing
//!
//! Add [`StreetRenderPlugin`] to an app to drive animation.

pub mod animation;
pub mod app;
pub mod convert;
pub mod court;
pub mod crowd;
pub mod hires;
pub mod net;
pub mod player;
pub mod props;
pub mod screen_fx;
pub mod shadow;

use bevy::prelude::*;

pub struct StreetRenderPlugin;

impl Plugin for StreetRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            animation::pose_skeletons.before(TransformSystems::Propagate),
        );
        app.add_systems(Update, app::sync_court_sun);
        shadow::plugin(app);
        net::plugin(app);
        crowd::plugin(app);
        app.add_plugins(screen_fx::ScreenFxPlugin);
    }
}
