//! BIG archives (`BIGF`). Vol. 2 stores courts as `.VIV` files in this
//! format, and the large `/XF/XF00000n` files are BIG archives with empty
//! entry names (the game addresses those entries by index).
//!
//! Layout: magic, u32 LE total size, u32 BE entry count, u32 BE header size,
//! then per entry a u32 BE offset, u32 BE size and NUL-terminated name.

use std::io::{Read, Seek};

use crate::{Error, Result, bytes, iso::Iso};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigEntry {
    pub name: String,
    pub offset: u32,
    pub size: u32,
}

pub struct Big<'a> {
    data: &'a [u8],
    entries: Vec<BigEntry>,
}

/// The entry table in `header` (the start of an archive of `archive_len`
/// bytes), for readers of other disc kinds ([`crate::gcdisc`]).
pub(crate) fn parse_table(header: &[u8], archive_len: usize) -> Result<Vec<BigEntry>> {
    table(header, archive_len)
}

/// Parse the entry table in `header` (the start of an archive of `archive_len` bytes).
fn table(header: &[u8], archive_len: usize) -> Result<Vec<BigEntry>> {
    let magic = bytes::range(header, 0, 4)?;
    if !matches!(magic, b"BIGF" | b"BIG4" | b"BIGH") {
        return Err(Error::format(format!(
            "not a BIG archive (magic {magic:02x?})"
        )));
    }
    let count = bytes::u32_be(header, 8)? as usize;
    // Each entry needs at least 9 bytes; a larger count is corrupt, not a big archive.
    if count > header.len() / 9 {
        return Err(Error::format(format!(
            "BIG entry count {count} too large for {} bytes",
            header.len()
        )));
    }
    let mut entries = Vec::with_capacity(count);
    let mut pos = 16;
    for _ in 0..count {
        let offset = bytes::u32_be(header, pos)?;
        let size = bytes::u32_be(header, pos + 4)?;
        let name = bytes::cstr(header, pos + 8)?;
        pos += 8 + name.len() + 1;
        if offset as usize + size as usize > archive_len {
            return Err(Error::format(format!(
                "BIG entry {name:?} lies outside the archive"
            )));
        }
        entries.push(BigEntry { name, offset, size });
    }
    Ok(entries)
}

/// The entry table of the BIG archive at `path` on the disc, read without
/// loading the whole archive (the player archives are 78 and 134 MB).
pub fn read_index<R: Read + Seek>(iso: &mut Iso<R>, path: &str) -> Result<Vec<BigEntry>> {
    let len = iso
        .stat(path)
        .ok_or_else(|| Error::NotFound(path.to_owned()))?
        .size as usize;
    let head = iso.read_range(path, 0, 16.min(len))?;
    let header_len = (bytes::u32_be(&head, 12)? as usize).min(len);
    table(&iso.read_range(path, 0, header_len)?, len)
}

/// One entry of the archive at `path`, read on its own.
pub fn read_entry<R: Read + Seek>(
    iso: &mut Iso<R>,
    path: &str,
    entry: &BigEntry,
) -> Result<Vec<u8>> {
    iso.read_range(path, entry.offset as u64, entry.size as usize)
}

impl<'a> Big<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let entries = table(data, data.len())?;
        Ok(Self { data, entries })
    }

    pub fn entries(&self) -> &[BigEntry] {
        &self.entries
    }

    /// The bytes of `entry`. Entries were bounds-checked in [`Big::parse`].
    pub fn bytes(&self, entry: &BigEntry) -> &'a [u8] {
        &self.data[entry.offset as usize..(entry.offset + entry.size) as usize]
    }

    /// The entry named `name`, compared case-insensitively.
    pub fn find(&self, name: &str) -> Option<&BigEntry> {
        self.entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// The bytes of the entry named `name`.
    pub fn get(&self, name: &str) -> Result<&'a [u8]> {
        self.find(name)
            .map(|e| self.bytes(e))
            .ok_or_else(|| Error::NotFound(name.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let header_len: usize = 16 + files.iter().map(|(n, _)| 8 + n.len() + 1).sum::<usize>();
        let mut out = b"BIGF".to_vec();
        out.extend(0u32.to_le_bytes());
        out.extend((files.len() as u32).to_be_bytes());
        out.extend((header_len as u32).to_be_bytes());
        let mut offset = header_len;
        for (name, data) in files {
            out.extend((offset as u32).to_be_bytes());
            out.extend((data.len() as u32).to_be_bytes());
            out.extend(name.as_bytes());
            out.push(0);
            offset += data.len();
        }
        for (_, data) in files {
            out.extend(*data);
        }
        out
    }

    #[test]
    fn finds_entries_by_name() {
        let bytes = archive(&[("court.o", b"abc"), ("COURT.SSH", b"de")]);
        let big = Big::parse(&bytes).unwrap();
        assert_eq!(big.entries().len(), 2);
        assert_eq!(big.get("court.ssh").unwrap(), b"de");
        assert_eq!(big.get("COURT.O").unwrap(), b"abc");
        assert!(big.get("missing").is_err());
    }

    #[test]
    fn rejects_entries_outside_the_archive() {
        let mut bytes = archive(&[("a", b"xyz")]);
        bytes.truncate(bytes.len() - 1);
        assert!(Big::parse(&bytes).is_err());
    }
}
