use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{metadata::get_file_metadata, scan::check_supported};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    /// Исполнитель альбома из тегов (Album Artist); пусто, если его нет. В старых файлах
    /// библиотеки поля нет — тогда пусто
    #[serde(default)]
    pub album_artist: String,
    pub duration_secs: u64,
    pub album: String,
    /// Год из тегов. В старых файлах библиотеки поля нет — тогда `None`
    #[serde(default)]
    pub year: Option<u16>,
    /// Номер трека и диска из тегов — порядок треков в альбоме. В старых файлах
    /// библиотеки полей нет — тогда `None`
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub disc_number: Option<u32>,
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
            album_artist: meta.album_artist,
            album: meta.album,
            duration_secs: meta.duration_secs,
            year: meta.year,
            track_number: meta.track_number,
            disc_number: meta.disc_number,
        })
    }
}
