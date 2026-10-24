//! Read-only ISO9660 access to a disc image.
//!
//! Paths are stored upper-case with a leading `/`, the version suffix (`;1`)
//! removed, and a trailing `.` removed for names without an extension
//! (`/XF/XF000001.` on disc becomes `/XF/XF000001`). Lookups are case-insensitive.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use crate::{Error, Result, bytes};

const SECTOR: u64 = 2048;
/// Deeper than any real disc; stops a corrupt image from recursing forever.
const MAX_DEPTH: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsoFile {
    pub lba: u32,
    pub size: u32,
}

pub struct Iso<R> {
    reader: R,
    files: BTreeMap<String, IsoFile>,
}

impl Iso<BufReader<File>> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(BufReader::new(File::open(path)?))
    }
}

impl<R: Read + Seek> Iso<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let pvd = read_at(&mut reader, 16 * SECTOR, SECTOR as usize)?;
        if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
            return Err(Error::format(
                "no ISO9660 primary volume descriptor at sector 16",
            ));
        }
        let root = bytes::range(&pvd, 156, 34)?;
        let (lba, size) = (bytes::u32_le(root, 2)?, bytes::u32_le(root, 10)?);
        let mut files = BTreeMap::new();
        walk(&mut reader, lba, size, "", 0, &mut files)?;
        Ok(Self { reader, files })
    }

    /// Every file on the disc, in path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, IsoFile)> {
        self.files.iter().map(|(k, v)| (k.as_str(), *v))
    }

    pub fn stat(&self, path: &str) -> Option<IsoFile> {
        self.files.get(&normalise(path)).copied()
    }

    /// The whole file at `path`.
    pub fn read(&mut self, path: &str) -> Result<Vec<u8>> {
        let f = self
            .stat(path)
            .ok_or_else(|| Error::NotFound(path.to_owned()))?;
        read_at(&mut self.reader, f.lba as u64 * SECTOR, f.size as usize)
    }

    /// `len` bytes at `offset` inside the file at `path`.
    pub fn read_range(&mut self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>> {
        let f = self
            .stat(path)
            .ok_or_else(|| Error::NotFound(path.to_owned()))?;
        if offset + len as u64 > f.size as u64 {
            return Err(Error::format(format!(
                "{path}: range {offset:#x}+{len:#x} past end of file"
            )));
        }
        read_at(&mut self.reader, f.lba as u64 * SECTOR + offset, len)
    }
}

fn normalise(path: &str) -> String {
    let p = path.trim_start_matches('/').to_ascii_uppercase();
    format!("/{p}")
}

fn read_at<R: Read + Seek>(r: &mut R, pos: u64, len: usize) -> Result<Vec<u8>> {
    r.seek(SeekFrom::Start(pos))?;
    let mut buf = vec![0; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

fn walk<R: Read + Seek>(
    r: &mut R,
    lba: u32,
    size: u32,
    prefix: &str,
    depth: usize,
    out: &mut BTreeMap<String, IsoFile>,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(Error::format("directory tree too deep"));
    }
    let dir = read_at(r, lba as u64 * SECTOR, size as usize)?;
    let mut i = 0;
    while i < dir.len() {
        let len = dir[i] as usize;
        if len == 0 {
            // Records never straddle a sector; skip the padding to the next one.
            i = (i / SECTOR as usize + 1) * SECTOR as usize;
            continue;
        }
        let rec = bytes::range(&dir, i, len)?;
        i += len;
        let name_len =
            *rec.get(32)
                .ok_or_else(|| Error::format("short directory record"))? as usize;
        let raw = bytes::range(rec, 33, name_len)?;
        if raw == [0] || raw == [1] {
            continue; // "." and ".."
        }
        let name: String = raw.iter().map(|&c| c as char).collect();
        let name = name
            .split(';')
            .next()
            .unwrap_or_default()
            .trim_end_matches('.');
        let path = format!("{prefix}/{}", name.to_ascii_uppercase());
        let (child_lba, child_size) = (bytes::u32_le(rec, 2)?, bytes::u32_le(rec, 10)?);
        if rec[25] & 2 != 0 {
            walk(r, child_lba, child_size, &path, depth + 1, out)?;
        } else {
            out.insert(
                path,
                IsoFile {
                    lba: child_lba,
                    size: child_size,
                },
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A minimal image: PVD at sector 16, root directory at sector 18 with one file.
    fn tiny_iso() -> Vec<u8> {
        let mut img = vec![0u8; 20 * SECTOR as usize];
        let pvd = 16 * SECTOR as usize;
        img[pvd] = 1;
        img[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        let root = pvd + 156;
        img[root] = 34;
        img[root + 2..root + 6].copy_from_slice(&18u32.to_le_bytes());
        img[root + 10..root + 14].copy_from_slice(&(SECTOR as u32).to_le_bytes());
        img[root + 25] = 2;

        let dir = 18 * SECTOR as usize;
        let name = b"HELLO.TXT;1";
        let rec_len = 33 + name.len() + 1;
        img[dir] = rec_len as u8;
        img[dir + 2..dir + 6].copy_from_slice(&19u32.to_le_bytes());
        img[dir + 10..dir + 14].copy_from_slice(&5u32.to_le_bytes());
        img[dir + 32] = name.len() as u8;
        img[dir + 33..dir + 33 + name.len()].copy_from_slice(name);
        let data = 19 * SECTOR as usize;
        img[data..data + 5].copy_from_slice(b"hello");
        img
    }

    #[test]
    fn lists_and_reads_files_case_insensitively() {
        let mut iso = Iso::new(Cursor::new(tiny_iso())).unwrap();
        assert_eq!(
            iso.files().map(|(p, _)| p).collect::<Vec<_>>(),
            ["/HELLO.TXT"]
        );
        assert_eq!(iso.read("hello.txt").unwrap(), b"hello");
        assert_eq!(iso.read_range("/HELLO.TXT", 1, 3).unwrap(), b"ell");
        assert!(matches!(iso.read("/MISSING"), Err(Error::NotFound(_))));
    }

    #[test]
    fn rejects_non_iso_data() {
        assert!(Iso::new(Cursor::new(vec![0u8; 40 * 1024])).is_err());
    }
}
