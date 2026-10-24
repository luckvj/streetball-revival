//! The game's locale text: `c0 fb` archives (`/LOCALE/XLOC_FE.EUS` for the
//! front end and `/LOCALE/XLOC_GM.EUS` for the game), whose text ids are
//! `file << 8 | index` (the loader at 0x1d4ef0; the file-name table at
//! 0x3ec2f0 in `SLUS_206.51`, see `docs/TRACE.md` §2).
//!
//! The container holds other front-end archives too (`XAFESCRN.BIN`,
//! `SPCHHDRS.BIG` and the font/palette banks carry files with empty names,
//! which the game addresses by index).
//!
//! - `c0 fb`, then a BE u16 header length at 2 and a BE u16 file count at 4,
//!   then per file (from byte 6) a BE u24 offset, BE u24 size and a
//!   NUL-terminated name (empty in `/FE`; `docs/TRACE.md` §4).
//! - Each file is an `LOCH` chunk (0x14 bytes) then an `LOCL` chunk: `LOCL`,
//!   u32 size, u32 0, u32 string count, then count u32 offsets, counted from
//!   the start of the chunk, to NUL-terminated UTF-16LE strings.
//!   Punctuation is stored as literal `\xNN` text (so a comma is the four
//!   bytes `\x2c`), which this decodes.

use std::io::{Read, Seek};

use crate::{Error, Result, bytes, iso::Iso};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C0fbEntry {
    pub name: String,
    pub offset: u32,
    pub size: u32,
}

fn u24_be(b: &[u8], off: usize) -> Result<u32> {
    let s = bytes::range(b, off, 3)?;
    Ok((s[0] as u32) << 16 | (s[1] as u32) << 8 | s[2] as u32)
}

fn u16_be(b: &[u8], off: usize) -> Result<u16> {
    let s = bytes::range(b, off, 2)?;
    Ok(u16::from_be_bytes([s[0], s[1]]))
}

/// The `c0 fb` entry table of `data`.
pub fn c0fb(data: &[u8]) -> Result<Vec<C0fbEntry>> {
    if bytes::range(data, 0, 2)? != *b"\xc0\xfb" {
        return Err(Error::format(format!(
            "not a c0 fb archive (magic {:02x?})",
            data.first()
        )));
    }
    let count = u16_be(data, 4)? as usize;
    // Six bytes plus a name's worst case (a lone NUL) per entry.
    if count > data.len() / 7 {
        return Err(Error::format(format!(
            "c0 fb file count {count} too large for {} bytes",
            data.len()
        )));
    }
    let mut entries = Vec::with_capacity(count);
    let mut pos = 6;
    for _ in 0..count {
        let offset = u24_be(data, pos)?;
        let size = u24_be(data, pos + 3)?;
        let name = bytes::cstr(data, pos + 6)?;
        pos += 6 + name.len() + 1;
        if offset as usize + size as usize > data.len() {
            return Err(Error::format(format!(
                "c0 fb file {name:?} lies outside the archive"
            )));
        }
        entries.push(C0fbEntry { name, offset, size });
    }
    Ok(entries)
}

/// The `XLOC_FE` files a front-end layout's text id picks with its high
/// byte (`file << 8 | index`), in the game's own order: the table at
/// 0x3ec2f0, 101 entries (the last, `challengercomes.txt`, at 0x64).
pub const FE_TEXT_FILES_ALL: [&str; 101] = [
    "common.txt",
    "common_fe.txt",
    "help.txt",
    "mainmenu.txt",
    "main_help.txt",
    "myteam.txt",
    "credits.txt",
    "score.txt",
    "entername.txt",
    "poster.txt",
    "chooseteam.txt",
    "chooseteam1p.txt",
    "chooseplyr.txt",
    "choosecrt.txt",
    "setchallenge.txt",
    "choosergn.txt",
    "options.txt",
    "nbachooseplyr.txt",
    "playit.txt",
    "pickupintro.txt",
    "tourneyintro.txt",
    "streetchallengeintro.txt",
    "bigradio.txt",
    "chooselogo.txt",
    "load.txt",
    "storymainmenu.txt",
    "storytestbed.txt",
    "controlconfig.txt",
    "mapscreen.txt",
    "createteam.txt",
    "nbatcend.txt",
    "bealegendend.txt",
    "createplyr.txt",
    "createplyr_createorchoose.txt",
    "createplyr_choose.txt",
    "createplyr_build.txt",
    "createplyr_clothes.txt",
    "createplyr_confirm.txt",
    "createplyr_facial.txt",
    "createplyr_gender.txt",
    "createplyr_hair.txt",
    "createplyr_height.txt",
    "createplyr_look.txt",
    "createplyr_name.txt",
    "createplyr_skills.txt",
    "wdtm.txt",
    "delete.txt",
    "jointeam.txt",
    "editplyr.txt",
    "editplyr_clothes.txt",
    "editplyr_shoes.txt",
    "editplyr_hair.txt",
    "editplyr_facial.txt",
    "editplyr_skills.txt",
    "editplyr_moves.txt",
    "editplyr_moves_replace.txt",
    "whatsnew.txt",
    "streetTips.txt",
    "singleko.txt",
    "doubleko.txt",
    "bestof3.txt",
    "updateTeam.txt",
    "gamedata.txt",
    "reputation.txt",
    "contconfig.txt",
    "random_first.txt",
    "random_first_female.txt",
    "random_last.txt",
    "creditsintro.txt",
    "dvdcontent.txt",
    "dvdnewin2.txt",
    "dvdbobbito.txt",
    "difficulty.txt",
    "memcard_ps2.txt",
    "options_ps2.txt",
    "control_chk_ps2.txt",
    "titlescreen_ps2.txt",
    "entername_ps2.txt",
    "memcard_gc.txt",
    "control_chk_gc.txt",
    "titlescreen_gc.txt",
    "entername_gc.txt",
    "memcard_xbox.txt",
    "options_xbox.txt",
    "control_chk_xbox.txt",
    "titlescreen_xbox.txt",
    "entername_xbox.txt",
    "common_gm.txt",
    "pause.txt",
    "overlays.txt",
    "school.txt",
    "pickuprewards.txt",
    "bealegendrewards.txt",
    "bealegendrewards2.txt",
    "topballer.txt",
    "ballerbreakdown.txt",
    "rundown.txt",
    "milestone.txt",
    "givenickname.txt",
    "nicknames.txt",
    "challengercomes.txt",
];

/// The first 30 of [`FE_TEXT_FILES_ALL`] (`common.txt`..`createteam.txt`):
/// the files the screens built before Create Baller's panels resolve
/// their layouts' words through; the later screens read the whole table.
pub const FE_TEXT_FILES: &[&str] = FE_TEXT_FILES_ALL.split_at(30).0;

/// One locale text file: a name (`common.txt`, `overlays.txt`, ...) and its
/// strings, in file order. A string id into it is its index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocaleText {
    pub name: String,
    pub texts: Vec<String>,
}

/// The strings of one file's LOCL chunk; string offsets count from the
/// chunk's own start (at [`bytes::range`]-checked `loc`).
fn texts(file: &[u8]) -> Result<Vec<String>> {
    let loc = file
        .windows(4)
        .position(|w| w == *b"LOCL")
        .ok_or_else(|| Error::format("no LOCL chunk"))?;
    let count = bytes::u32_le(file, loc + 12)? as usize;
    if count > file.len() / 4 {
        return Err(Error::format(format!("LOCL string count {count}")));
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let off = bytes::u32_le(file, loc + 16 + 4 * i)? as usize;
        let start = loc + off;
        let mut end = start;
        while end + 2 <= file.len() && file[end..end + 2] != [0, 0] {
            end += 2;
        }
        if end + 2 > file.len() {
            return Err(Error::format("unterminated LOCL string"));
        }
        out.push(unescape(utf16le(&file[start..end])));
    }
    Ok(out)
}

/// A UTF-16LE slice without its terminator; bad halves become U+FFFD.
fn utf16le(b: &[u8]) -> String {
    b.as_chunks::<2>()
        .0
        .iter()
        .map(|c| char::from_u32(u16::from_le_bytes([c[0], c[1]]) as u32).unwrap_or('\u{fffd}'))
        .collect()
}

/// Punctuation is stored as literal `\xNN` text (`\x2c` a comma, `\x27` an
/// apostrophe, `\xae` the registered mark). Decode those; anything else keeps
/// the backslash as written.
fn unescape(s: String) -> String {
    if !s.contains("\\x") {
        return s;
    }
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\' && c[i + 1..].first() == Some(&'x') {
            let hex: String = c.iter().skip(i + 2).take(2).collect();
            if hex.len() == 2 && hex.chars().all(|h| h.is_ascii_hexdigit()) {
                // Any two hex digits decode, as the game's own dump does;
                // controls are rare but present (a trailing \x0d et al).
                let code = u32::from_str_radix(&hex, 16).expect("hex digits");
                out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                i += 4;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// The text files of the locale archive at `path` on the disc.
pub fn read<R: Read + Seek>(iso: &mut Iso<R>, path: &str) -> Result<Vec<LocaleText>> {
    let size = iso
        .stat(path)
        .ok_or_else(|| Error::NotFound(path.to_owned()))?
        .size as usize;
    let data = iso.read_range(path, 0, size)?;
    parse(&data)
}

/// The text files of a `c0 fb` locale archive.
pub fn parse(data: &[u8]) -> Result<Vec<LocaleText>> {
    let entries = c0fb(data)?;
    let mut out = Vec::with_capacity(entries.len());
    for e in &entries {
        let name = e.name.clone();
        let chunk = bytes::range(data, e.offset as usize, e.size as usize)?;
        out.push(LocaleText {
            name,
            texts: texts(chunk)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `\xc0\xfb`, one file `a.txt`, two strings: "hi,!" with a direct
    /// UTF-16 comma, then a literal `\x27` escape encoding an apostrophe.
    fn sample() -> Vec<u8> {
        let u16le = |c: char| (c as u16).to_le_bytes();
        let mut chunk = b"LOCH".to_vec(); // 4
        chunk.resize(10, 0); // + the LOCH padding: 6 zeros
        chunk.extend_from_slice(b"LOCL"); // 10..14
        chunk.extend_from_slice(&0u32.to_le_bytes()); // size, unused
        chunk.extend_from_slice(&0u32.to_le_bytes());
        chunk.extend_from_slice(&2u32.to_le_bytes()); // two strings
        chunk.extend_from_slice(&(24u32).to_le_bytes()); // their offsets, from
        chunk.extend_from_slice(&(34u32).to_le_bytes()); // the chunk's start
        for c in "hi,!".chars() {
            chunk.extend_from_slice(&u16le(c));
        }
        chunk.extend_from_slice(&[0, 0]);
        for c in ['\\', 'x', '2', '7'] {
            chunk.extend_from_slice(&u16le(c));
        }
        chunk.extend_from_slice(&[0, 0]);
        let mut out = b"\xc0\xfb".to_vec();
        out.extend_from_slice(&[0, 2]); // header length
        out.extend_from_slice(&1u16.to_be_bytes()); // file count
        out.extend_from_slice(&[0, 0, 0x12]); // the chunk follows the header + name
        out.extend_from_slice(&(chunk.len() as u32).to_be_bytes()[1..]); // size u24
        out.extend_from_slice(b"a.txt\0");
        out.extend_from_slice(&chunk);
        out
    }

    #[test]
    fn round_trips_through_the_container() {
        let d = sample();
        let files = parse(&d).expect("parses");
        // The direct comma decodes as itself; the \x27 escape decodes too.
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "a.txt");
        assert_eq!(files[0].texts, vec!["hi,!".to_owned(), "'".to_owned()]);
    }

    #[test]
    fn rejects_bad_magic_and_oob_sizes() {
        assert!(c0fb(b"BIGF012").is_err());
        let mut d = sample();
        d[6] = 0xff; // offset outside the archive
        assert!(parse(&d).is_err());
    }
}
