//! the original game's movies, read from the user's disc.
//!
//! - [`nfx`]: the `.NFX` container (`MPCh` chunks of MPEG-2 video, with
//!   a SCHl soundtrack between them)
//! - [`mpeg2`]: a small MPEG-2 video decoder for them
//! - [`tables`]: the standard's code tables

pub mod mpeg2;
pub mod nfx;
pub mod tables;
