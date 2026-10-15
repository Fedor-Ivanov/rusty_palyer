use serde::Serialize;
use std::path::PathBuf;

use crate::metadata::{check_supported, get_file_metadata};

#[derive(Serialize, Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: u64,
}

impl Track {
    /// Создаёт трек из файла: проверяет, что его можно воспроизвести, и читает метаданные.
    pub fn from_path(path: PathBuf) -> Result<Self, String> {
        check_supported(&path)?;
        let meta = get_file_metadata(&path)?;

        Ok(Self {
            path,
            title: meta.title,
            artist: meta.artist,
            album: meta.album,
            duration_secs: meta.duration_secs,
        })
    }
}
