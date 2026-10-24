//! The export folder: writes files under it and counts what it wrote.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use street_assets::ssh;

pub struct Out {
    root: PathBuf,
    bytes: AtomicU64,
    files: AtomicU64,
}

impl Out {
    pub fn new(root: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(root)?;
        Ok(Self {
            root: root.to_path_buf(),
            bytes: AtomicU64::new(0),
            files: AtomicU64::new(0),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Write `data` at `rel` (forward slashes), making its folders.
    pub fn bytes(&self, rel: &str, data: &[u8]) -> std::io::Result<()> {
        let path = self.root.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, data)?;
        self.bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
        self.files.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn png(&self, rel: &str, t: &ssh::Texture) -> std::io::Result<()> {
        self.bytes(rel, &crate::picture::encode(t.width, t.height, &t.rgba))
    }

    pub fn json(&self, rel: &str, value: &serde_json::Value) -> std::io::Result<()> {
        let mut text = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
        text.push(b'\n');
        self.bytes(rel, &text)
    }

    /// Bytes and files written so far.
    pub fn written(&self) -> (u64, u64) {
        (
            self.bytes.load(Ordering::Relaxed),
            self.files.load(Ordering::Relaxed),
        )
    }
}
