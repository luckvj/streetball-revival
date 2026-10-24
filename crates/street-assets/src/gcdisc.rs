//! GameCube disc images, read through the `nod` crate: plain `.iso`/`.gcm`
//! and Dolphin's compressed formats (`.rvz`, `.wia`, `.ciso`, `.gcz`, ...).
//!
//! The other Street games come from here: Street V1 (`GNSE69`) only as a
//! GameCube disc, and Street V3 (`G3VE69`), whose GameCube version alone
//! has the Nintendo All-Stars. Paths are matched without regard to case
//! (`/xf/player.viv`).

use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use crate::{Error, Result};

/// One file on the disc.
#[derive(Clone, Copy, Debug)]
pub struct GcFile {
    node: nod::Node,
    pub size: u64,
}

pub struct GcDisc {
    partition: Box<dyn nod::PartitionBase>,
    /// Lower-cased path ("/xf/player.viv") -> file, plus the paths in disc order.
    files: HashMap<String, GcFile>,
    order: Vec<String>,
    game_id: String,
    title: String,
}

fn nod_error(e: nod::Error) -> Error {
    Error::format(format!("GameCube disc: {e}"))
}

impl GcDisc {
    /// Opens a GameCube disc image in any format `nod` reads.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let disc = nod::Disc::new(path.as_ref()).map_err(nod_error)?;
        let header = disc.header();
        let game_id = String::from_utf8_lossy(&header.game_id).into_owned();
        let title = header.game_title_str().trim().to_owned();
        let mut partition = disc
            .open_partition_kind(nod::PartitionKind::Data)
            .map_err(nod_error)?;
        let meta = partition.meta().map_err(nod_error)?;
        let fst = meta
            .fst()
            .map_err(|e| Error::format(format!("GameCube file table: {e}")))?;
        let mut files = HashMap::new();
        let mut order = Vec::new();
        // Directories end at their `length` index; a stack of (end, prefix)
        // rebuilds each file's full path.
        let mut stack: Vec<(usize, String)> = Vec::new();
        for (index, node, name) in fst.iter() {
            while stack.last().is_some_and(|(end, _)| index >= *end) {
                stack.pop();
            }
            let name = name.map_err(Error::format)?;
            let prefix = stack.last().map_or("", |(_, p)| p.as_str());
            let path = format!("{prefix}/{name}").to_ascii_lowercase();
            if node.is_dir() {
                stack.push((node.length() as usize, path));
            } else if node.is_file() {
                files.insert(
                    path.clone(),
                    GcFile {
                        node,
                        size: node.length(),
                    },
                );
                order.push(path);
            }
        }
        Ok(Self {
            partition,
            files,
            order,
            game_id,
            title,
        })
    }

    /// The game's executable (`main.dol`), which lives outside the file
    /// table.
    pub fn main_dol(&mut self) -> Result<Vec<u8>> {
        Ok(self.partition.meta().map_err(nod_error)?.raw_dol.into_vec())
    }

    /// The six-character game code, e.g. `G3VE69` for Street V3 (USA).
    pub fn game_id(&self) -> &str {
        &self.game_id
    }

    /// The title in the disc header, e.g. the V3 title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Every file's path and size, in disc order.
    pub fn files(&self) -> impl Iterator<Item = (&str, u64)> {
        self.order.iter().map(|p| (p.as_str(), self.files[p].size))
    }

    pub fn stat(&self, path: &str) -> Option<GcFile> {
        self.files.get(&path.to_ascii_lowercase()).copied()
    }

    /// The whole file at `path`.
    pub fn read(&mut self, path: &str) -> Result<Vec<u8>> {
        let size = self
            .stat(path)
            .ok_or_else(|| Error::NotFound(path.to_owned()))?
            .size;
        self.read_range(path, 0, size as usize)
    }

    /// `len` bytes of the file at `path`, starting at `offset`.
    pub fn read_range(&mut self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>> {
        let file = self
            .stat(path)
            .ok_or_else(|| Error::NotFound(path.to_owned()))?;
        if offset
            .checked_add(len as u64)
            .is_none_or(|end| end > file.size)
        {
            return Err(Error::format(format!("read past the end of {path}")));
        }
        let mut stream = self.partition.open_file(file.node)?;
        stream.seek(SeekFrom::Start(offset))?;
        let mut out = vec![0; len];
        stream.read_exact(&mut out)?;
        Ok(out)
    }

    /// The entry table of the BIG archive at `path`, read without loading
    /// the whole archive (V3's `/xf/player.viv` is 106 MB).
    pub fn big_index(&mut self, path: &str) -> Result<Vec<crate::big::BigEntry>> {
        let len = self
            .stat(path)
            .ok_or_else(|| Error::NotFound(path.to_owned()))?
            .size as usize;
        let head = self.read_range(path, 0, 16.min(len))?;
        let header_len = (crate::bytes::u32_be(&head, 12)? as usize).min(len);
        crate::big::parse_table(&self.read_range(path, 0, header_len)?, len)
    }

    /// One BIG archive entry, unpacked if it is RefPack data.
    pub fn big_entry(&mut self, path: &str, entry: &crate::big::BigEntry) -> Result<Vec<u8>> {
        let raw = self.read_range(path, entry.offset as u64, entry.size as usize)?;
        crate::refpack::unpack(&raw)
    }
}
