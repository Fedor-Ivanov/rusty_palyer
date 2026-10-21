//! Сохранённые плейлисты: именованные списки треков. Хранятся между запусками в
//! `playlists.json` в папке данных. Треки хранятся целиком (путь и теги), а не ссылками
//! на медиатеку: в плейлисте могут быть и треки, добавленные мимо неё (кнопкой «+» у
//! очереди), и удаление трека из медиатеки не портит плейлисты.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    io::ErrorKind,
    path::PathBuf,
};

use super::{library::Track, storage::write_atomically};

const FILE_NAME: &str = "playlists.json";
const FORMAT_VERSION: u32 = 1;

/// Номер плейлиста: не меняется, пока плейлист существует, и не переиспользуется — на него
/// ссылается открытая страница
pub type PlaylistId = u64;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub tracks: Vec<Track>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    next_id: PlaylistId,
    playlists: Vec<Playlist>,
}

pub struct Playlists {
    /// В порядке создания
    items: Vec<Playlist>,
    next_id: PlaylistId,
    /// Файлы треков, которых не было на диске при загрузке
    missing: HashSet<PathBuf>,
    /// Куда сохранять; `None` — папка данных недоступна, плейлисты живут только до выхода
    file: Option<PathBuf>,
    /// Растёт при каждом изменении — по ней видно, что производное (сетка) устарело
    revision: u64,
}

impl Playlists {
    /// Загружает плейлисты из папки данных `dir`. Файла нет — плейлистов нет.
    /// Вторым значением — ошибка для показа пользователю
    pub fn load(dir: Option<PathBuf>) -> (Self, Option<String>) {
        let mut playlists = Self {
            items: Vec::new(),
            next_id: 1,
            missing: HashSet::new(),
            file: dir.map(|dir| dir.join(FILE_NAME)),
            revision: 0,
        };
        let Some(file) = &playlists.file else {
            return (playlists, None);
        };
        let text = match fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => return (playlists, None),
            Err(e) => {
                return (
                    playlists,
                    Some(format!("Could not read the playlists: {e}")),
                );
            }
        };
        let stored: Stored = match serde_json::from_str(&text) {
            Ok(stored) => stored,
            Err(e) => {
                // Повреждённый файл не перезаписываем при первом же изменении —
                // откладываем, чтобы плейлисты можно было восстановить
                let backup = file.with_extension("json.bak");
                let _ = fs::rename(file, &backup);
                return (
                    playlists,
                    Some(format!(
                        "The playlists file is damaged and was set aside as {}: {e}",
                        backup.display()
                    )),
                );
            }
        };
        playlists.missing = stored
            .playlists
            .iter()
            .flat_map(|playlist| &playlist.tracks)
            .filter(|track| !track.path.exists())
            .map(|track| track.path.clone())
            .collect();
        playlists.next_id = stored.next_id;
        playlists.items = stored.playlists;
        (playlists, None)
    }

    pub fn all(&self) -> &[Playlist] {
        &self.items
    }

    pub fn get(&self, id: PlaylistId) -> Option<&Playlist> {
        self.items.iter().find(|playlist| playlist.id == id)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Файла трека не было на диске при загрузке
    pub fn is_missing(&self, track: &Track) -> bool {
        self.missing.contains(&track.path)
    }

    /// Треки плейлиста для воспроизведения — без тех, чьих файлов нет на диске
    pub fn playable_tracks(&self, id: PlaylistId) -> Vec<Track> {
        self.get(id)
            .map(|playlist| {
                playlist
                    .tracks
                    .iter()
                    .filter(|track| !self.is_missing(track))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Создаёт плейлист и сохраняет. Возвращает его номер
    pub fn create(&mut self, name: &str, tracks: Vec<Track>) -> Result<PlaylistId, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Playlist {
            id,
            name: name.trim().to_owned(),
            tracks,
        });
        self.changed()?;
        Ok(id)
    }

    pub fn rename(&mut self, id: PlaylistId, name: &str) -> Result<(), String> {
        let Some(playlist) = self.items.iter_mut().find(|playlist| playlist.id == id) else {
            return Ok(());
        };
        playlist.name = name.trim().to_owned();
        self.changed()
    }

    pub fn delete(&mut self, id: PlaylistId) -> Result<(), String> {
        self.items.retain(|playlist| playlist.id != id);
        self.changed()
    }

    /// Заменяет все треки плейлиста
    pub fn set_tracks(&mut self, id: PlaylistId, tracks: Vec<Track>) -> Result<(), String> {
        let Some(playlist) = self.items.iter_mut().find(|playlist| playlist.id == id) else {
            return Ok(());
        };
        playlist.tracks = tracks;
        self.changed()
    }

    /// Добавляет треки в конец плейлиста. Повторы допустимы — как в очереди
    pub fn add_tracks(&mut self, id: PlaylistId, tracks: Vec<Track>) -> Result<(), String> {
        let Some(playlist) = self.items.iter_mut().find(|playlist| playlist.id == id) else {
            return Ok(());
        };
        playlist.tracks.extend(tracks);
        self.changed()
    }

    /// Убирает из плейлиста треки с номерами `indices`
    pub fn remove_tracks(
        &mut self,
        id: PlaylistId,
        indices: &BTreeSet<usize>,
    ) -> Result<(), String> {
        let Some(playlist) = self.items.iter_mut().find(|playlist| playlist.id == id) else {
            return Ok(());
        };
        let mut index = 0;
        playlist.tracks.retain(|_| {
            let keep = !indices.contains(&index);
            index += 1;
            keep
        });
        self.changed()
    }

    /// После любого изменения: новая ревизия и сохранение
    fn changed(&mut self) -> Result<(), String> {
        self.revision += 1;
        let Some(file) = &self.file else {
            return Ok(());
        };
        let stored = Stored {
            version: FORMAT_VERSION,
            next_id: self.next_id,
            playlists: self.items.clone(),
        };
        write_atomically(file, &stored).map_err(|e| format!("Could not save the playlists: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::Playlists;
    use crate::core::library::Track;
    use std::{collections::BTreeSet, fs, path::PathBuf};

    fn track(name: &str) -> Track {
        Track {
            path: PathBuf::from(format!("/music/{name}.mp3")),
            title: name.into(),
            artist: "Artist".into(),
            album_artist: String::new(),
            duration_secs: 60,
            album: "Album".into(),
            year: None,
            track_number: None,
            disc_number: None,
        }
    }

    fn titles(playlists: &Playlists, id: u64) -> Vec<String> {
        let playlist = playlists.get(id).unwrap();
        playlist.tracks.iter().map(|t| t.title.clone()).collect()
    }

    #[test]
    fn crud_survives_restart() {
        let dir =
            std::env::temp_dir().join(format!("rusty_player_playlists_{}", fastrand::u64(..)));
        let (mut playlists, error) = Playlists::load(Some(dir.clone()));
        assert!(error.is_none());

        let chill = playlists
            .create("  Chill ", vec![track("a"), track("b")])
            .unwrap();
        let party = playlists.create("Party", vec![track("c")]).unwrap();
        assert_ne!(chill, party);
        assert_eq!(playlists.get(chill).unwrap().name, "Chill");

        playlists.add_tracks(chill, vec![track("c")]).unwrap();
        playlists
            .remove_tracks(chill, &BTreeSet::from([0]))
            .unwrap();
        playlists.rename(party, "Weekend").unwrap();
        assert_eq!(titles(&playlists, chill), ["b", "c"]);

        let (mut restored, error) = Playlists::load(Some(dir.clone()));
        assert!(error.is_none());
        assert_eq!(titles(&restored, chill), ["b", "c"]);
        assert_eq!(restored.get(party).unwrap().name, "Weekend");

        // Удалённый номер не переиспользуется — даже после перезапуска
        restored.delete(chill).unwrap();
        let next = restored.create("New", Vec::new()).unwrap();
        assert!(next > party);
        let names: Vec<_> = restored.all().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Weekend", "New"]);

        fs::remove_dir_all(dir).unwrap();
    }
}
