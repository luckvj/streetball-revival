//! Music.
//!
//! - The front end plays whole songs, `/MUSIC/FE_MUSIC/*.ASF` (one stereo
//!   44.1 kHz SCHl stream each; `INTRO.ASF` is the title).
//! - In a game the "interactive" songs play: `/MUSIC/<FOLDER>/MAIN.MUS` (the
//!   song cut into SCHl segments, stereo 36 kHz), `TRICKS.MUS` and
//!   `WHOOSH.MUS` (BNKl banks of stingers), and `<FOLDER>.MPF`, an the publisher
//!   PathFinder map (`PFDx`, version 3.4) that picks the next segment from
//!   the game's events ([`crate::path`]).
//! - `/CSCRIPTS/PATHSONG.CFG` lists the in-game songs; see [`songs`].
//! - `/MUSIC/LOADLOOP/*.BNK` are looping loading-screen beds.

/// One in-game song from `PATHSONG.CFG`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    /// Folder under `/MUSIC`.
    pub folder: String,
    /// Bit flags of the artist intro lines the announcer may say.
    pub artist_speech: u32,
    pub title: String,
    pub artist: String,
    pub album: String,
}

impl Song {
    pub fn main(&self) -> String {
        format!("/MUSIC/{}/MAIN.MUS", self.folder.to_ascii_uppercase())
    }

    pub fn tricks(&self) -> String {
        format!("/MUSIC/{}/TRICKS.MUS", self.folder.to_ascii_uppercase())
    }

    pub fn whoosh(&self) -> String {
        format!("/MUSIC/{}/WHOOSH.MUS", self.folder.to_ascii_uppercase())
    }

    /// The PathFinder map ([`crate::path`]).
    pub fn map(&self) -> String {
        let f = self.folder.to_ascii_uppercase();
        format!("/MUSIC/{f}/{f}.MPF")
    }
}

/// The front end's songs, in the order of the Trax menu.
pub const FRONT_END_SONGS: [&str; 9] = [
    "/MUSIC/FE_MUSIC/PETEROCK.ASF",
    "/MUSIC/FE_MUSIC/NELLY.ASF",
    "/MUSIC/FE_MUSIC/BSHEEP.ASF",
    "/MUSIC/FE_MUSIC/LORDS.ASF",
    "/MUSIC/FE_MUSIC/DILAT.ASF",
    "/MUSIC/FE_MUSIC/LYTE.ASF",
    "/MUSIC/FE_MUSIC/BENZ.ASF",
    "/MUSIC/FE_MUSIC/ERICK.ASF",
    "/MUSIC/FE_MUSIC/NATE.ASF",
];

pub const TITLE_SONG: &str = "/MUSIC/FE_MUSIC/INTRO.ASF";

pub const PATHSONG: &str = "/CSCRIPTS/PATHSONG.CFG";

/// Parses `PATHSONG.CFG`: lines `pathsong <folder> <flags> "title" "artist"
/// "album"`; `//` starts a comment.
pub fn songs(cfg: &str) -> Vec<Song> {
    let mut out = Vec::new();
    for line in cfg.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        let Some(rest) = line.strip_prefix("pathsong") else {
            continue;
        };
        let mut words = rest.split_whitespace();
        let (Some(folder), Some(flags)) = (words.next(), words.next()) else {
            continue;
        };
        let quoted: Vec<&str> = rest.split('"').skip(1).step_by(2).collect();
        out.push(Song {
            folder: folder.to_owned(),
            artist_speech: flags.parse().unwrap_or(0),
            title: quoted.first().copied().unwrap_or("").to_owned(),
            artist: quoted.get(1).copied().unwrap_or("").to_owned(),
            album: quoted.get(2).copied().unwrap_or("").to_owned(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pathsong_lines() {
        let cfg = "// comment\r\n//pathsong old 1 \"a\" \"b\" \"c\"\r\n\
                   pathsong getup 64 \"Get Up\" \"Someone\" \"An Album\"\r\n";
        let s = songs(cfg);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].folder, "getup");
        assert_eq!(s[0].artist_speech, 64);
        assert_eq!(
            (s[0].title.as_str(), s[0].artist.as_str()),
            ("Get Up", "Someone")
        );
        assert_eq!(s[0].main(), "/MUSIC/GETUP/MAIN.MUS");
    }
}
