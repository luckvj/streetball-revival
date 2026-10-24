//! the original game's audio, read from the user's disc: the publisher's PS2 formats.
//!
//! - [`adpcm`]: the XA and PS-ADPCM codecs
//! - [`patch`]: `PT` sound headers
//! - [`schl`]: `SCHl` streams (songs, music segments, crowd beds, commentary)
//! - [`bank`]: `BNKl` banks and `.ABK` files (sound effects, crowd calls)
//! - [`aems`]: the AEMS scripts in the `.ABK` files and the `.AEM` event
//!   maps, and a runtime that plays them (which sound each game event makes)
//! - [`path`]: the PathFinder maps that sequence the in-game songs
//! - [`speech`]: the commentary archive (`/SPEECH/SPCHDATS.BIG`)
//! - [`music`]: the song list (`/CSCRIPTS/PATHSONG.CFG`) and music files
//!
//! Which bank each court loads is in [`courts`]; which game moments fire
//! which events, in `docs/TRACE.md` (audio section).

pub mod adpcm;
pub mod aems;
pub mod bank;
pub mod music;
pub mod patch;
pub mod path;
pub mod schl;
pub mod speech;
mod speech_lookup;

/// Decoded audio: interleaved signed 16-bit samples.
#[derive(Clone, Debug)]
pub struct Clip {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
    /// Loop start and end in sample frames.
    pub looping: Option<(u32, u32)>,
}

impl Clip {
    /// Sample frames (samples per channel).
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / self.rate.max(1) as f32
    }
}

/// The banks a match loads, from `AuAemsManager::Init` (0x19e160).
pub mod courts {
    /// Banks every match loads.
    pub const COMMON: [&str; 6] = [
        "/SFX/SFX_COUR.ABK",
        "/SFX/SFX_EXER.ABK",
        "/SFX/SFX_SIGN.ABK",
        "/SFX/SFX_SWEE.ABK",
        "/SFX/SFX_HUD_.ABK",
        "/SFX/SFX_TRAN.ABK",
    ];

    /// The front end's bank: menu sounds by index (0x19df08).
    pub const FRONT_END: &str = "/SFX/FE_MOD.ABK";

    /// A court's ambience bank, crowd bank and crowd stream, by `XDBCOURT`
    /// record (`None` for Street School, which the game plays at the Rec
    /// Center ambience with the Broad Street crowd).
    pub fn banks(court: Option<usize>) -> (&'static str, &'static str, &'static str) {
        match court {
            None => (
                "/SFX/SFX_REC_.ABK",
                "/SFX/CRD_STRT.ABK",
                "/SFX/CRD_STRT.AST",
            ),
            Some(0) => (
                "/SFX/SFX_REC_.ABK",
                "/SFX/CRD_RECK.ABK",
                "/SFX/CRD_RECK.AST",
            ),
            Some(1) => (
                "/SFX/SFX_PHIL.ABK",
                "/SFX/CRD_STRT.ABK",
                "/SFX/CRD_STRT.AST",
            ),
            Some(2) => ("/SFX/SFX_FOST.ABK", "/SFX/CRD_FOS.ABK", "/SFX/CRD_FOS.AST"),
            Some(3) => (
                "/SFX/SFX_SOUL.ABK",
                "/SFX/CRD_SOUL.ABK",
                "/SFX/CRD_SOUL.AST",
            ),
            Some(4) => (
                "/SFX/SFX_4TH_.ABK",
                "/SFX/CRD_CAGE.ABK",
                "/SFX/CRD_CAGE.AST",
            ),
            Some(5) => (
                "/SFX/SFX_MOSS.ABK",
                "/SFX/CRD_MOSW.ABK",
                "/SFX/CRD_MOSW.AST",
            ),
            Some(6) => (
                "/SFX/SFX_ROOF.ABK",
                "/SFX/CRD_PARK.ABK",
                "/SFX/CRD_PARK.AST",
            ),
            Some(7) => (
                "/SFX/SFX_SEAT.ABK",
                "/SFX/CRD_SEAT.ABK",
                "/SFX/CRD_SEAT.AST",
            ),
            Some(8) => ("/SFX/SFX_INDO.ABK", "/SFX/CRD_IND.ABK", "/SFX/CRD_IND.AST"),
            Some(9) => (
                "/SFX/SFX_RUCK.ABK",
                "/SFX/CRD_RUCK.ABK",
                "/SFX/CRD_RUCK.AST",
            ),
            // Rucker Park '78 and anything newer.
            Some(_) => (
                "/SFX/SFX_PHIL.ABK",
                "/SFX/CRD_RUC1.ABK",
                "/SFX/CRD_RUC1.AST",
            ),
        }
    }
}
