//! Библиотека — треки, которые пользователь добавил «насовсем». Хранится между запусками
//! списком путей и тегов в JSON в папке данных приложения; сами файлы остаются на своих местах.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::PathBuf,
};

use super::Track;
use crate::core::storage::write_atomically;

const FILE_NAME: &str = "library.json";
/// Версия формата файла. 2 — в треках появился год, 3 — номера трека и диска,
/// 4 — исполнитель альбома: библиотеку старой версии нужно один раз перечитать из тегов
/// (см. `outdated_paths`)
const FORMAT_VERSION: u32 = 4;

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    tracks: Vec<Track>,
}

pub struct Library {
    tracks: Vec<Track>,
    /// Пути всех треков — чтобы не добавить один файл дважды
    paths: HashSet<PathBuf>,
    /// Файлы, которых не оказалось на диске при загрузке: их переместили или удалили
    missing: HashSet<PathBuf>,
    /// Куда сохранять; `None` — папка данных недоступна, библиотека живёт только до выхода
    file: Option<PathBuf>,
    /// Версия формата, с которой сохранять. Пока старая библиотека не перечитана из тегов,
    /// сохраняется со старой версией — иначе после перезапуска её бы сочли обновлённой
    version: u32,
    /// Растёт при каждом изменении: по нему видно, что производные данные (группы по
    /// исполнителям и альбомам) пора пересчитать
    revision: u64,
}

impl Library {
    /// Загружает библиотеку из папки данных `dir`. Файла нет — пустая библиотека.
    /// Вторым значением — ошибка для показа пользователю, если что-то пошло не так.
    pub fn load(dir: Option<PathBuf>) -> (Self, Option<String>) {
        let mut library = Self {
            tracks: Vec::new(),
            paths: HashSet::new(),
            missing: HashSet::new(),
            file: dir.map(|dir| dir.join(FILE_NAME)),
            version: FORMAT_VERSION,
            revision: 0,
        };
        let Some(file) = &library.file else {
            return (
                library,
                Some("Could not find a folder to store the library: it won't be saved".into()),
            );
        };

        let text = match fs::read_to_string(file) {
            Ok(text) => text,
            // Первый запуск: библиотеки ещё нет
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (library, None),
            Err(e) => return (library, Some(format!("Could not read the library: {e}"))),
        };
        let stored: Stored = match serde_json::from_str(&text) {
            Ok(stored) => stored,
            Err(e) => {
                // Повреждённый файл не перезаписываем пустым при первом же изменении:
                // откладываем его в сторону, чтобы его можно было восстановить
                let backup = file.with_extension("json.bak");
                let _ = fs::rename(file, &backup);
                return (
                    library,
                    Some(format!(
                        "The library file is damaged and was set aside as {}: {e}",
                        backup.display()
                    )),
                );
            }
        };

        library.missing = stored
            .tracks
            .iter()
            .filter(|track| !track.path.exists())
            .map(|track| track.path.clone())
            .collect();
        library.paths = stored.tracks.iter().map(|t| t.path.clone()).collect();
        library.tracks = stored.tracks;
        library.version = stored.version;
        (library, None)
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Библиотека сохранена старой версией и в ней не хватает данных из тегов — пути всех
    /// её треков, чтобы перечитать их. `None` — библиотека актуальна
    pub fn outdated_paths(&self) -> Option<Vec<PathBuf>> {
        (self.version < FORMAT_VERSION && !self.tracks.is_empty())
            .then(|| self.tracks.iter().map(|t| t.path.clone()).collect())
    }

    /// Заменяет данные треков на перечитанные из тегов (по пути) и сохраняет библиотеку уже
    /// новой версией. Треки, которых нет в `fresh` (файл пропал), остаются как были
    pub fn refresh(&mut self, fresh: Vec<Track>) -> Result<(), String> {
        let mut fresh: std::collections::HashMap<PathBuf, Track> =
            fresh.into_iter().map(|t| (t.path.clone(), t)).collect();
        for track in &mut self.tracks {
            if let Some(updated) = fresh.remove(&track.path) {
                *track = updated;
            }
        }
        self.version = FORMAT_VERSION;
        self.revision += 1;
        self.save()
    }

    /// Пути, которые уже есть в библиотеке: при повторном добавлении папки их не читаем заново
    pub fn paths(&self) -> &HashSet<PathBuf> {
        &self.paths
    }

    /// Файла трека не было на диске при загрузке
    pub fn is_missing(&self, index: usize) -> bool {
        self.tracks
            .get(index)
            .is_some_and(|track| self.missing.contains(&track.path))
    }

    /// Добавляет треки в конец, пропуская те, что уже есть, и сохраняет библиотеку
    pub fn add(&mut self, tracks: Vec<Track>) -> Result<(), String> {
        let before = self.tracks.len();
        for track in tracks {
            if self.paths.insert(track.path.clone()) {
                self.missing.remove(&track.path);
                self.tracks.push(track);
            }
        }
        if self.tracks.len() == before {
            return Ok(());
        }
        self.revision += 1;
        self.save()
    }

    /// Удаляет треки с номерами `indices` (сами файлы не трогает) и сохраняет библиотеку
    pub fn remove(&mut self, indices: &BTreeSet<usize>) -> Result<(), String> {
        let mut index = 0;
        self.tracks.retain(|track| {
            let keep = !indices.contains(&index);
            index += 1;
            if !keep {
                self.paths.remove(&track.path);
                self.missing.remove(&track.path);
            }
            keep
        });
        self.revision += 1;
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        write_atomically(
            file,
            &Stored {
                version: self.version,
                tracks: self.tracks.clone(),
            },
        )
        .map_err(|e| format!("Could not save the library: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::Library;
    use crate::core::library::Track;
    use std::{collections::BTreeSet, fs, path::Path};

    fn track(path: &Path) -> Track {
        Track {
            path: path.to_path_buf(),
            title: path.file_stem().unwrap().to_string_lossy().into_owned(),
            artist: "Artist".into(),
            album_artist: String::new(),
            duration_secs: 60,
            year: None,
            track_number: None,
            disc_number: None,
            album: String::new(),
        }
    }

    fn titles(library: &Library) -> Vec<&str> {
        library.tracks().iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn saves_loads_dedupes_and_marks_missing() {
        let dir = std::env::temp_dir().join(format!("rusty_player_library_{}", std::process::id()));
        let music = dir.join("music");
        fs::create_dir_all(&music).unwrap();
        let [a, b, c] = ["a", "b", "c"].map(|name| music.join(format!("{name}.mp3")));
        for path in [&a, &b, &c] {
            fs::write(path, b"").unwrap();
        }

        let (mut library, error) = Library::load(Some(dir.clone()));
        assert!(error.is_none());
        library.add(vec![track(&a), track(&b)]).unwrap();
        // Повторное добавление того же файла пропускается
        library.add(vec![track(&b), track(&c)]).unwrap();
        assert_eq!(titles(&library), ["a", "b", "c"]);

        library.remove(&BTreeSet::from([0])).unwrap();
        fs::remove_file(&c).unwrap();

        // После перезапуска — то же содержимое, а пропавший файл помечен
        let (library, error) = Library::load(Some(dir.clone()));
        assert!(error.is_none());
        assert_eq!(titles(&library), ["b", "c"]);
        assert!(!library.is_missing(0));
        assert!(library.is_missing(1));
        assert!(library.paths().contains(&b));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn old_library_is_refreshed_once() {
        let dir = std::env::temp_dir().join(format!("rusty_player_v1_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.mp3");
        // Библиотека в формате 1: у трека нет года
        fs::write(
            dir.join("library.json"),
            format!(
                r#"{{"version":1,"tracks":[{{"path":{:?},"title":"a","artist":"","duration_secs":1,"album":""}}]}}"#,
                path
            ),
        )
        .unwrap();

        let (mut library, error) = Library::load(Some(dir.clone()));
        assert!(error.is_none());
        assert_eq!(library.tracks()[0].year, None);
        assert_eq!(library.outdated_paths(), Some(vec![path.clone()]));

        // Добавление до обновления не должно пометить библиотеку обновлённой
        library.add(vec![track(&dir.join("b.mp3"))]).unwrap();
        let (mut library, _) = Library::load(Some(dir.clone()));
        assert!(library.outdated_paths().is_some());

        let mut fresh = track(&path);
        fresh.year = Some(1999);
        library.refresh(vec![fresh]).unwrap();
        let (library, _) = Library::load(Some(dir.clone()));
        assert_eq!(library.outdated_paths(), None);
        assert_eq!(library.tracks()[0].year, Some(1999));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn damaged_file_is_set_aside() {
        let dir = std::env::temp_dir().join(format!("rusty_player_damaged_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("library.json"), "not json").unwrap();

        let (library, error) = Library::load(Some(dir.clone()));
        assert!(library.tracks().is_empty());
        assert!(error.unwrap().contains("damaged"));
        assert!(dir.join("library.json.bak").exists());

        fs::remove_dir_all(&dir).unwrap();
    }
}
