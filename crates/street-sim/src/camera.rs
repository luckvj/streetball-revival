//! The camera manager's placements that its handlers make at a message
//! inside the game's update. A courtside eye (`FUN_00303c80` with its last
//! argument zero) takes four values of the shared generator (`0x303d48`,
//! `0x303de4`, `0x303e4c`, `0x303ebc`: its scale, x, y and depth), whatever
//! the eye. Two of the manager's handlers place one at a message the update
//! posts:
//!
//! - slot 0x204 (`FUN_00303650`, posted by `FUN_00159cf0`): a gamebreaker
//!   goes up, unless a Level 2 one's spot stages it (`FUN_00141298`);
//! - slot 0x1a4 (`FUN_00302488`, the shot clock's violation in
//!   `FUN_0013f1f0`): a courtside eye on the side's last carrier, unless a
//!   gamebreaker is on.
//!
//! The simulation draws the four values there and keeps them in
//! [`World::camera_draws`]; `street-game`'s camera places the eye from them
//! after the update. Its other placements (a replay's start and end, the
//! attract's cuts) stay the camera's, on the match's generator after the
//! update. (A violation within the half-second blend after a gamebreaker,
//! which the original's handler skips, still draws here.)

use crate::World;

/// The values a courtside eye takes.
pub const COURTSIDE_DRAWS: usize = 4;

impl World {
    /// A courtside eye's four values, drawn now for the camera.
    pub(crate) fn camera_courtside(&mut self) {
        for _ in 0..COURTSIDE_DRAWS {
            let r = self.rng.next_u32();
            self.camera_draws.push(r);
        }
    }
}
