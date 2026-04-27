use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read, Write},
    path::Path,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub const CHUNK_SIZE: usize = 65_536; // 64 KiB

pub struct Bundle {
    /// Raw ZIP bytes (plaintext).
    pub bytes: Vec<u8>,
    /// SHA-256 of `bytes`.
    pub content_hash: [u8; 32],
    /// `bytes` split into CHUNK_SIZE pieces.
    pub chunks: Vec<Vec<u8>>,
}

impl Bundle {
    /// Create a Bundle by zipping the contents of `dir`.
    /// Walks the directory recursively and stores every file with its relative path.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let bytes = zip_dir(dir)?;
        Ok(Self::from_bytes(bytes))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let content_hash: [u8; 32] = hasher.finalize().into();
        let chunks = bytes.chunks(CHUNK_SIZE).map(|c| c.to_vec()).collect();
        Self { bytes, content_hash, chunks }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Hex-encode `content_hash` for use as a manifest variable.
    pub fn content_hash_hex(&self) -> String {
        hex::encode(self.content_hash)
    }

    /// Hex-encode chunk `i` for use as a manifest variable.
    pub fn chunk_hex(&self, i: usize) -> String {
        hex::encode(&self.chunks[i])
    }
}

fn zip_dir(dir: &Path) -> Result<Vec<u8>> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let base = dir.canonicalize().context("Cannot resolve dist directory")?;
    add_dir_to_zip(&mut zip, &base, &base, options)?;

    let cursor = zip.finish().context("Failed to finalise ZIP")?;
    Ok(cursor.into_inner())
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<Cursor<Vec<u8>>>,
    base: &Path,
    current: &Path,
    options: SimpleFileOptions,
) -> Result<()> {
    for entry in std::fs::read_dir(current).context("Failed to read directory")? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(base).unwrap();
        let name = relative.to_string_lossy().replace('\\', "/");

        if path.is_dir() {
            zip.add_directory(&name, options)?;
            add_dir_to_zip(zip, base, &path, options)?;
        } else {
            zip.start_file(&name, options)?;
            let mut file = std::fs::File::open(&path)
                .with_context(|| format!("Cannot open {}", path.display()))?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;
            zip.write_all(&contents)?;
        }
    }
    Ok(())
}
