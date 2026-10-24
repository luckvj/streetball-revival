//! Original saves supplied by the user, independent of an emulator.
//!
//! PS2 `GAMEDATA` consists of six little-endian length-prefixed sections.
//! `FUN_0022b...` dispatches section objects in this order; each object's
//! serializer stores its length then raw payload. Unknown fields are retained,
//! so parsing and writing an original file is lossless.
use std::collections::BTreeSet;

use crate::{Error, Result};

/// Exact section payload lengths for the US PS2 release, SLUS-20651.
/// The GameCube card file's name (the header's bytes 8..25), as the
/// original game writes it.
pub const GCI_FILENAME: [u8; 17] = [
    0x4e, 0x42, 0x41, 0x20, 0x53, 0x54, 0x52, 0x45, 0x45, 0x54, 0x20, 0x56, 0x6f, 0x6c, 0x2e, 0x20,
    0x32,
];

pub const SECTION_LENGTHS: [usize; 6] = [0x60, 0x18, 0xa64, 0xbc * 32, 0xc88 * 10, 0x70 * 10];
/// Size including six four-byte length prefixes.
pub const PS2_SAVE_LENGTH: usize = 42020;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Section {
    Unlocks,
    Options,
    HighScores,
    Users,
    Careers,
    Teams,
}

impl Section {
    pub const ALL: [Self; 6] = [
        Self::Unlocks,
        Self::Options,
        Self::HighScores,
        Self::Users,
        Self::Careers,
        Self::Teams,
    ];
}

/// Validated original PS2 game data. No guessed fields are discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ps2Save {
    sections: [Vec<u8>; 6],
}

impl Ps2Save {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PS2_SAVE_LENGTH {
            return Err(Error::format(format!(
                "PS2 game data is {} bytes; expected {PS2_SAVE_LENGTH}",
                bytes.len()
            )));
        }
        let mut offset = 0;
        let mut sections: [Vec<u8>; 6] = std::array::from_fn(|_| Vec::new());
        for (index, expected) in SECTION_LENGTHS.into_iter().enumerate() {
            let length = le32(bytes, offset)? as usize;
            if length != expected {
                return Err(Error::format(format!(
                    "PS2 save section {index} has length {length:#x}; expected {expected:#x}"
                )));
            }
            offset += 4;
            sections[index] = bytes[offset..offset + length].to_vec();
            offset += length;
        }
        Ok(Self { sections })
    }

    /// Read a raw game file or a MAX Drive archive containing one.
    pub fn read(bytes: &[u8]) -> Result<Self> {
        if bytes.starts_with(b"Ps2PowerSave") {
            MaxSave::parse(bytes)?.game_data()
        } else {
            Self::parse(bytes)
        }
    }

    pub fn section(&self, section: Section) -> &[u8] {
        &self.sections[section as usize]
    }

    /// Fixed-size access preserves section lengths while editing traced fields.
    pub fn section_mut(&mut self, section: Section) -> &mut [u8] {
        &mut self.sections[section as usize]
    }

    /// Write the original six-section envelope, preserving every payload byte.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(PS2_SAVE_LENGTH);
        for section in &self.sections {
            output.extend_from_slice(&(section.len() as u32).to_le_bytes());
            output.extend_from_slice(section);
        }
        output
    }

    /// Fixed 0xbc-byte user slot (32 slots); semantic fields require tracing.
    pub fn user(&self, index: usize) -> Option<&[u8]> {
        record(self.section(Section::Users), index, 0xbc)
    }

    /// Fixed 0xc88-byte career slot (10 slots).
    pub fn career(&self, index: usize) -> Option<&[u8]> {
        record(self.section(Section::Careers), index, 0xc88)
    }

    /// Fixed 0x70-byte created-team slot (10 slots).
    pub fn team(&self, index: usize) -> Option<&[u8]> {
        record(self.section(Section::Teams), index, 0x70)
    }

    pub fn user_info(&self, index: usize) -> Option<User<'_>> {
        self.user(index).map(|bytes| User { bytes })
    }

    pub fn career_info(&self, index: usize) -> Option<Career<'_>> {
        self.career(index).map(|bytes| Career { bytes })
    }

    pub fn team_info(&self, index: usize) -> Option<Team<'_>> {
        self.team(index).map(|bytes| Team { bytes })
    }
}

/// View of an original 0xbc-byte user record, with object-header offsets removed.
/// `FUN_001ee168` binds the payload at object +0x18; the following offsets
/// come from its getters and setters, not printable-byte heuristics.
#[derive(Clone, Copy, Debug)]
pub struct User<'a> {
    bytes: &'a [u8],
}

impl User<'_> {
    /// `FUN_001ee348`: nine characters followed by a terminator.
    pub fn name(&self) -> String {
        text(&self.bytes[..10])
    }

    pub fn is_used(&self) -> bool {
        self.bytes[0] != 0
    }

    /// Nine signed aggregate counters (`FUN_001ee328`, `FUN_001ee420`).
    /// Their labels are deliberately left to the consuming gameplay layer.
    pub fn counter(&self, index: usize) -> Option<i32> {
        if index >= 9 {
            return None;
        }
        Some(word(self.bytes, 0x0c + index * 4) as i32)
    }

    /// Packed win/progression flags (`FUN_001ee3e0`, `FUN_001ee790`).
    pub fn progress_flags(&self) -> u16 {
        half(self.bytes, 0x30) as u16
    }

    pub fn current_court(&self) -> i16 {
        half(self.bytes, 0x32)
    }

    /// Court results mask (`FUN_001ee548`, `FUN_001ee790`).
    pub fn court_results(&self) -> u32 {
        word(self.bytes, 0x34)
    }

    /// Five challenge-group completion bits (`FUN_001ee6e0`, `FUN_001ee9a0`).
    pub fn challenge_groups(&self) -> u32 {
        word(self.bytes, 0x38)
    }

    /// Native user +0x54; bit0 marks the School completion payout consumed.
    pub fn school_flags(&self) -> u32 {
        word(self.bytes, 0x3c)
    }

    /// All 33 challenge nibbles use reversed words and reversed nibbles:
    /// word 4 holds challenges 0..7, highest nibble first (`FUN_001eef88`).
    /// Bit3 records completion; low bits retain an outcome code. School
    /// failure writes4, success preserves that prior code (FUN_001eecd8).
    pub fn challenge(&self, index: usize) -> Option<Challenge> {
        if index >= 33 {
            return None;
        }
        let packed = word(self.bytes, 0x40 + (4 - index / 8) * 4);
        let value = ((packed >> ((7 - index % 8) * 4)) & 15) as u8;
        Some(Challenge {
            completed: value & 8 != 0,
            result: value & 7,
        })
    }

    /// Per-challenge execution counters, 26 entries (`FUN_001eeca8`).
    pub fn challenge_attempts(&self, index: usize) -> Option<i32> {
        if index >= 26 {
            return None;
        }
        Some(word(self.bytes, 0x54 + index * 4) as i32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Challenge {
    pub completed: bool,
    pub result: u8,
}

/// View of one original 0xc88-byte Be a Legend record (`FUN_001e6938`).
#[derive(Clone, Copy, Debug)]
pub struct Career<'a> {
    bytes: &'a [u8],
}

impl Career<'_> {
    pub fn flags(&self) -> u32 {
        word(self.bytes, 0)
    }

    /// Four signed counters (`FUN_001e70a8`); counter 3 starts at 1500.
    pub fn counter(&self, index: usize) -> Option<i32> {
        if index >= 4 {
            return None;
        }
        Some(word(self.bytes, 0x0c + index * 4) as i32)
    }

    /// Seventeen byte-sized event completion tallies (`FUN_001e7ff8`).
    pub fn event_tally(&self, index: usize) -> Option<u8> {
        if index >= 17 {
            return None;
        }
        Some(self.bytes[0x1c + index])
    }

    /// The embedded 0x70-byte created-team record (`FUN_001e6e48`).
    pub fn team(&self) -> Team<'_> {
        Team {
            bytes: &self.bytes[0x664..0x6d4],
        }
    }

    /// Player IDs for the six roster places, including -1 sentinels.
    /// Team serialization rebinds these IDs at object +0x6de (`FUN_001e6e48`).
    pub fn player_ids(&self) -> [i16; 6] {
        std::array::from_fn(|i| half(self.bytes, 0x6c6 + i * 2))
    }
}

/// An original 0x70-byte team record, serialized by `FUN_001ec330`.
#[derive(Clone, Copy, Debug)]
pub struct Team<'a> {
    bytes: &'a [u8],
}

impl Team<'_> {
    pub fn id(&self) -> i16 {
        half(self.bytes, 0)
    }
    pub fn name(&self) -> String {
        text(&self.bytes[2..0x22])
    }
    pub fn name_part(&self, index: usize) -> Option<String> {
        if index >= 3 {
            return None;
        }
        let offset = 2 + index * 32;
        Some(text(&self.bytes[offset..offset + 32]))
    }
    pub fn player_ids(&self) -> [i16; 6] {
        std::array::from_fn(|i| half(self.bytes, 0x62 + i * 2))
    }
    /// Team database field 10, copied at `FUN_001ec330`.
    pub fn field_10(&self) -> i16 {
        half(self.bytes, 0x6e)
    }
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn half(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn record(bytes: &[u8], index: usize, stride: usize) -> Option<&[u8]> {
    let start = index.checked_mul(stride)?;
    bytes.get(start..start.checked_add(stride)?)
}

/// One file in a MAX Drive archive. Parsing never writes archive paths to disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// MAX Drive archive validated before decompression and record access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaxSave {
    pub directory: String,
    pub title: String,
    pub files: Vec<SaveFile>,
}

impl MaxSave {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        const LIMIT: usize = 16 * 1024 * 1024;
        if bytes.len() < 92 || bytes.len() > LIMIT || &bytes[..12] != b"Ps2PowerSave" {
            return Err(Error::format("invalid MAX Drive archive header"));
        }
        let checksum = le32(bytes, 12)?;
        let actual = crc32(bytes.iter().enumerate().map(
            |(i, &b)| {
                if (12..16).contains(&i) { 0 } else { b }
            },
        ));
        if checksum != actual {
            return Err(Error::format("MAX Drive CRC32 mismatch"));
        }
        let compressed_length = le32(bytes, 80)? as usize;
        let count = le32(bytes, 84)? as usize;
        let length = le32(bytes, 88)? as usize;
        // Some producers put the uncompressed length in the compressed field.
        if compressed_length != length && compressed_length.checked_add(88) != Some(bytes.len()) {
            return Err(Error::format("MAX Drive compressed length mismatch"));
        }
        if count == 0 || count > 256 || length > LIMIT || count * 36 > length {
            return Err(Error::format("MAX Drive record/output limits exceeded"));
        }
        let directory = safe_name(&bytes[16..48])?;
        let title = text(&bytes[48..80]);
        let body = crate::lzari::decode(&bytes[92..], length)?;
        let mut offset = 0usize;
        let mut files = Vec::with_capacity(count);
        let mut names = BTreeSet::new();
        for _ in 0..count {
            let header = body
                .get(offset..offset + 36)
                .ok_or_else(|| Error::format("truncated MAX Drive file header"))?;
            let file_length = le32(header, 0)? as usize;
            let name = safe_name(&header[4..36])?;
            if !names.insert(name.clone()) {
                return Err(Error::format("duplicate MAX Drive filename"));
            }
            offset += 36;
            let end = offset
                .checked_add(file_length)
                .ok_or_else(|| Error::format("MAX Drive file length overflow"))?;
            let payload = body
                .get(offset..end)
                .ok_or_else(|| Error::format("truncated MAX Drive file"))?;
            files.push(SaveFile {
                name,
                bytes: payload.to_vec(),
            });
            offset = (end + 8).div_ceil(16) * 16 - 8;
            if offset > body.len() {
                return Err(Error::format("truncated MAX Drive record alignment"));
            }
        }
        if offset != body.len() {
            return Err(Error::format("unaccounted MAX Drive archive bytes"));
        }
        Ok(Self {
            directory,
            title,
            files,
        })
    }

    /// Import only this game's US PS2 save, avoiding same-sized unrelated files.
    pub fn game_data(&self) -> Result<Ps2Save> {
        if self.directory != "BASLUS-20651GAMEDATA" {
            return Err(Error::format("MAX archive is not SLUS-20651 game data"));
        }
        let file = self
            .files
            .iter()
            .find(|f| f.name == self.directory)
            .ok_or_else(|| Error::NotFound(self.directory.clone()))?;
        Ps2Save::parse(&file.bytes)
    }
}

/// GCI container metadata and lossless access to the six US game payloads.
/// Header layout: Dolphin `GCMemcard.h::DEntry`; numeric fields are big endian.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GciSave {
    pub game_code: [u8; 4],
    pub maker_code: [u8; 2],
    pub filename: String,
    pub blocks: u16,
    pub payload: Vec<u8>,
}

impl GciSave {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 64 {
            return Err(Error::format("truncated GCI directory entry"));
        }
        let blocks = u16::from_be_bytes([bytes[0x38], bytes[0x39]]);
        if blocks == 0 || bytes.len() != 64 + usize::from(blocks) * 8192 {
            return Err(Error::format("GCI block count/length mismatch"));
        }
        let game_code: [u8; 4] = bytes[..4].try_into().unwrap();
        let maker_code: [u8; 2] = bytes[4..6].try_into().unwrap();
        if !game_code
            .iter()
            .chain(&maker_code)
            .all(u8::is_ascii_alphanumeric)
        {
            return Err(Error::format("invalid GCI game/maker identity"));
        }
        Ok(Self {
            game_code,
            maker_code,
            filename: text(&bytes[8..40]),
            blocks,
            payload: bytes[64..].to_vec(),
        })
    }

    /// The supplied US GCI has the same six fixed payload lengths as PS2,
    /// padded to GameCube card blocks rather than prefixed with lengths.
    /// Each payload has a BE delimiter `0xdeadfade + length`. This is an
    /// integrity/shape check, not a checksum of the payload's contents.
    pub fn section(&self, section: Section) -> Result<&[u8]> {
        if !self.is_vol_2_usa() || self.blocks != 7 || self.filename.as_bytes() != GCI_FILENAME {
            return Err(Error::format(
                "unsupported GameCube save identity or layout",
            ));
        }
        const OFFSETS: [usize; 6] = [0x3200, 0x3600, 0x3800, 0x4400, 0x5c00, 0xda00];
        let index = section as usize;
        let at = OFFSETS[index];
        let length = SECTION_LENGTHS[index];
        let bytes = self
            .payload
            .get(at..at + length)
            .ok_or_else(|| Error::format("truncated GameCube game section"))?;
        let footer = self
            .payload
            .get(at + length..at + length + 4)
            .ok_or_else(|| Error::format("truncated GameCube section delimiter"))?;
        if u32::from_be_bytes(footer.try_into().unwrap()) != 0xdeadfade + length as u32 {
            return Err(Error::format("invalid GameCube section delimiter"));
        }
        Ok(bytes)
    }

    /// Validate every game section before interpreting any fields.
    pub fn validate_sections(&self) -> Result<()> {
        for section in [
            Section::Unlocks,
            Section::Options,
            Section::HighScores,
            Section::Users,
            Section::Careers,
            Section::Teams,
        ] {
            self.section(section)?;
        }
        Ok(())
    }

    pub fn is_vol_2_usa(&self) -> bool {
        self.game_code == *b"GNZE" && self.maker_code == *b"69"
    }
}

fn le32(bytes: &[u8], offset: usize) -> Result<u32> {
    let word = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::format("truncated save integer"))?;
    Ok(u32::from_le_bytes(word.try_into().unwrap()))
}

fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn safe_name(bytes: &[u8]) -> Result<String> {
    let name = text(bytes);
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .bytes()
            .any(|b| !(32..127).contains(&b) || b"/\\:<>\"|?*".contains(&b))
    {
        return Err(Error::format("unsafe MAX Drive filename"));
    }
    Ok(name)
}

fn crc32(bytes: impl Iterator<Item = u8>) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_save() -> Vec<u8> {
        let mut output = Vec::new();
        for (index, length) in SECTION_LENGTHS.into_iter().enumerate() {
            output.extend_from_slice(&(length as u32).to_le_bytes());
            output.extend((0..length).map(|i| (i + index) as u8));
        }
        output
    }

    #[test]
    fn exact_original_envelope_and_records_round_trip() {
        let bytes = synthetic_save();
        let parsed = Ps2Save::parse(&bytes).unwrap();
        assert_eq!(parsed.to_bytes(), bytes);
        assert_eq!(parsed.user(31).unwrap().len(), 0xbc);
        assert_eq!(parsed.career(9).unwrap().len(), 0xc88);
        assert_eq!(parsed.team(9).unwrap().len(), 0x70);
        assert!(parsed.user(32).is_none());
        assert!(parsed.career(10).is_none());
        assert!(parsed.team(usize::MAX).is_none());
    }

    #[test]
    fn malformed_save_lengths_are_rejected() {
        let bytes = synthetic_save();
        assert!(Ps2Save::parse(&bytes[..bytes.len() - 1]).is_err());
        for offset in [0, 0x64, 0x80, 0xae8, 0x226c, 0x9fc0] {
            let mut bad = bytes.clone();
            bad[offset] ^= 1;
            assert!(Ps2Save::parse(&bad).is_err());
        }
    }

    #[test]
    fn gci_metadata_uses_big_endian_and_rejects_truncation() {
        let mut bytes = vec![0; 64 + 8192];
        bytes[..6].copy_from_slice(b"GNZE69");
        bytes[8..12].copy_from_slice(b"TEST");
        bytes[0x38..0x3a].copy_from_slice(&1u16.to_be_bytes());
        let parsed = GciSave::parse(&bytes).unwrap();
        assert!(parsed.is_vol_2_usa());
        assert_eq!(parsed.blocks, 1);
        assert_eq!(parsed.filename, "TEST");
        assert!(Ps2Save::read(&bytes).is_err());
        assert!(GciSave::parse(&bytes[..bytes.len() - 1]).is_err());
        bytes[0x38..0x3a].copy_from_slice(&1u16.to_le_bytes());
        assert!(GciSave::parse(&bytes).is_err());
    }

    #[test]
    fn gci_section_delimiters_are_checked_independently() {
        let mut bytes = vec![0; 64 + 7 * 8192];
        bytes[..6].copy_from_slice(b"GNZE69");
        bytes[8..25].copy_from_slice(&GCI_FILENAME);
        bytes[0x38..0x3a].copy_from_slice(&7u16.to_be_bytes());
        for (offset, length) in [0x3240, 0x3640, 0x3840, 0x4440, 0x5c40, 0xda40]
            .into_iter()
            .zip(SECTION_LENGTHS)
        {
            bytes[offset + length..offset + length + 4]
                .copy_from_slice(&(0xdeadfadeu32 + length as u32).to_be_bytes());
        }
        let save = GciSave::parse(&bytes).unwrap();
        save.validate_sections().unwrap();
        assert_eq!(save.section(Section::Careers).unwrap().len(), 0x7d50);
        for (offset, length) in [0x3240, 0x3640, 0x3840, 0x4440, 0x5c40, 0xda40]
            .into_iter()
            .zip(SECTION_LENGTHS)
        {
            let mut bad = bytes.clone();
            bad[offset + length + 3] ^= 1;
            assert!(GciSave::parse(&bad).unwrap().validate_sections().is_err());
        }
    }

    #[test]
    fn crc_and_archive_bounds_are_checked_before_decode() {
        assert_eq!(crc32(b"123456789".iter().copied()), 0xcbf43926);
        let mut bytes = vec![0; 92];
        bytes[..12].copy_from_slice(b"Ps2PowerSave");
        assert!(MaxSave::parse(&bytes).is_err());
        let checksum = crc32(bytes.iter().copied());
        bytes[12..16].copy_from_slice(&checksum.to_le_bytes());
        assert!(MaxSave::parse(&bytes).is_err());
        assert!(safe_name(b"../bad").is_err());
        assert!(safe_name(b"x\\y").is_err());
    }

    #[test]
    fn traced_user_and_team_offsets_and_reversed_challenge_order() {
        let mut save = Ps2Save::parse(&synthetic_save()).unwrap();
        let users = save.section_mut(Section::Users);
        users[..0xbc].fill(0);
        users[..4].copy_from_slice(b"TEST");
        users[0x0c..0x10].copy_from_slice(&13u32.to_le_bytes());
        users[0x32..0x34].copy_from_slice(&27i16.to_le_bytes());
        // Challenges 0 and 7 share the last word, opposite ends of the word.
        users[0x50..0x54].copy_from_slice(&0xb0000008u32.to_le_bytes());
        users[0x40..0x44].copy_from_slice(&0xe0000000u32.to_le_bytes());
        let user = save.user_info(0).unwrap();
        assert_eq!(user.name(), "TEST");
        assert_eq!(user.counter(0), Some(13));
        assert_eq!(user.counter(9), None);
        assert_eq!(user.current_court(), 27);
        assert_eq!(
            user.challenge(0),
            Some(Challenge {
                completed: true,
                result: 3
            })
        );
        assert_eq!(
            user.challenge(7),
            Some(Challenge {
                completed: true,
                result: 0
            })
        );
        assert_eq!(
            user.challenge(32),
            Some(Challenge {
                completed: true,
                result: 6
            })
        );
        assert_eq!(user.challenge(33), None);
        assert_eq!(user.challenge_attempts(26), None);
        let teams = save.section_mut(Section::Teams);
        teams[..0x70].fill(0);
        teams[2..6].copy_from_slice(b"CREW");
        teams[0x62..0x64].copy_from_slice(&(-1i16).to_le_bytes());
        teams[0x64..0x66].copy_from_slice(&157i16.to_le_bytes());
        let team = save.team_info(0).unwrap();
        assert_eq!(team.name(), "CREW");
        assert_eq!(team.player_ids(), [-1, 157, 0, 0, 0, 0]);
        assert_eq!(team.name_part(3), None);
    }
}
