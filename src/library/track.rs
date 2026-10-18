use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{metadata::get_file_metadata, scan::check_supported};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub duration_secs: u64,
    pub album: String,
    /// Читается только у текущего трека (см. `PlayerController::set_current_track`): обложки всего
    /// списка заняли бы слишком много памяти. У треков в списке всегда `None`.
    /// В файл библиотеки не сохраняется
    #[serde(skip)]
    pub album_cover: Option<Vec<u8>>,
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
            album_cover: None,
        })
    }
}
