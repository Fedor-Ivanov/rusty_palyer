//! Библиотека — треки, которые пользователь добавил «насовсем». Хранится между запусками
//! списком путей и тегов в JSON в папке данных приложения; сами файлы остаются на своих местах.
//!
//! Медиатека бывает огромной, поэтому медленное — в фоне: сохранение (файл переписывается
//! целиком) и проверка, какие файлы пропали с диска (по запросу к диску на трек).

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
};

use super::Track;
use crate::core::storage::BackgroundWriter;

const FILE_NAME: &str = "library.json";
/// Версия формата файла. 2 — в треках появился год, 3 — номера трека и диска,
/// 4 — исполнитель альбома: библиотеку старой версии нужно один раз перечитать из тегов
/// (см. `outdated_paths`)
const FORMAT_VERSION: u32 = 4;

#[derive(Deserialize)]
struct Stored {
    version: u32,
    tracks: Vec<Track>,
}

/// То же, что `Stored`, для записи: треки — общим списком, без копии на каждое сохранение
#[derive(Serialize)]
struct Snapshot {
    version: u32,
    tracks: Arc<Vec<Track>>,
}

pub struct Library {
    /// Общий с фоновой записью: она держит снимок, пока пишет. Изменение во время записи
    /// копирует список (`Arc::make_mut`), иначе копии нет
    tracks: Arc<Vec<Track>>,
    /// Пути всех треков — чтобы не добавить один файл дважды
    paths: HashSet<PathBuf>,
    /// Файлы, которых не оказалось на диске при загрузке: их переместили или удалили.
    /// Заполняется фоновой проверкой (`check_missing_in_background`); до неё пусто
    missing: HashSet<PathBuf>,
    /// Ответ фоновой проверки пропавших файлов, пока он не пришёл
    missing_check: Option<Receiver<HashSet<PathBuf>>>,
    /// Фоновая запись в файл библиотеки; `None` — папка данных недоступна, библиотека
    /// живёт только до выхода
    writer: Option<BackgroundWriter<Snapshot>>,
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
        let file = dir.map(|dir| dir.join(FILE_NAME));
        let mut library = Self {
            tracks: Arc::new(Vec::new()),
            paths: HashSet::new(),
            missing: HashSet::new(),
            missing_check: None,
            writer: file
                .clone()
                .map(|file| BackgroundWriter::new(file, "library")),
            version: FORMAT_VERSION,
            revision: 0,
        };
        let Some(file) = &file else {
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

        library.paths = stored.tracks.iter().map(|t| t.path.clone()).collect();
        library.tracks = Arc::new(stored.tracks);
        library.version = stored.version;
        (library, None)
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Проверяет в фоне, каких файлов нет на диске: на огромной медиатеке это секунды,
    /// и запуск их не ждёт. Пока ответа нет, все треки считаются на месте. `wake` будит
    /// приложение, когда ответ готов (см. `poll_missing`)
    pub fn check_missing_in_background(&mut self, wake: impl Fn() + Send + 'static) {
        let paths: Vec<PathBuf> = self.tracks.iter().map(|t| t.path.clone()).collect();
        let (sender, receiver) = mpsc::channel();
        self.missing_check = Some(receiver);
        std::thread::spawn(move || {
            let missing = paths.into_iter().filter(|path| !path.exists()).collect();
            let _ = sender.send(missing);
            wake();
        });
    }

    /// Забирает ответ фоновой проверки, если он пришёл. `true` — пометки пропавших
    /// файлов обновились
    pub fn poll_missing(&mut self) -> bool {
        let Some(receiver) = &self.missing_check else {
            return false;
        };
        let Ok(missing) = receiver.try_recv() else {
            return false;
        };
        self.missing_check = None;
        // Пока проверяли, треки могли убрать из медиатеки — их не помечаем
        self.missing = missing
            .into_iter()
            .filter(|path| self.paths.contains(path))
            .collect();
        true
    }

    /// Ошибка фоновой записи, если была, — для показа пользователю
    pub fn take_save_error(&self) -> Option<String> {
        self.writer.as_ref().and_then(BackgroundWriter::take_error)
    }

    /// Ждёт, пока сохранится всё изменённое: перед выходом
    pub fn flush(&self) {
        if let Some(writer) = &self.writer {
            writer.flush();
        }
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
    pub fn refresh(&mut self, fresh: Vec<Track>) {
        let mut fresh: std::collections::HashMap<PathBuf, Track> =
            fresh.into_iter().map(|t| (t.path.clone(), t)).collect();
        for track in Arc::make_mut(&mut self.tracks) {
            if let Some(updated) = fresh.remove(&track.path) {
                *track = updated;
            }
        }
        self.version = FORMAT_VERSION;
        self.revision += 1;
        self.save();
    }

    /// Пути, которые уже есть в библиотеке: при повторном добавлении папки их не читаем заново
    pub fn paths(&self) -> &HashSet<PathBuf> {
        &self.paths
    }

    /// Файла трека не было на диске при загрузке (по фоновой проверке)
    pub fn is_missing(&self, index: usize) -> bool {
        self.tracks
            .get(index)
            .is_some_and(|track| self.missing.contains(&track.path))
    }

    /// Добавляет треки в конец, пропуская те, что уже есть, и сохраняет библиотеку
    pub fn add(&mut self, tracks: Vec<Track>) {
        let new: Vec<Track> = tracks
            .into_iter()
            .filter(|track| self.paths.insert(track.path.clone()))
            .collect();
        for track in &new {
            self.missing.remove(&track.path);
        }
        if new.is_empty() {
            return;
        }
        Arc::make_mut(&mut self.tracks).extend(new);
        self.revision += 1;
        self.save();
    }

    /// Треки с номерами `indices` для воспроизведения — без тех, чьих файлов нет на диске:
    /// сыграть их всё равно не выйдет
    pub fn playable_tracks(&self, indices: &[usize]) -> Vec<Track> {
        indices
            .iter()
            .filter(|&&index| !self.is_missing(index))
            .filter_map(|&index| self.tracks.get(index).cloned())
            .collect()
    }

    /// Пути треков с номерами `indices`. Удаление спрашивает подтверждение по путям, а не
    /// по номерам: пока ждём ответа, номера могут измениться, а пути — нет
    pub fn paths_of(&self, indices: &[usize]) -> HashSet<PathBuf> {
        indices
            .iter()
            .filter_map(|&index| self.tracks.get(index))
            .map(|track| track.path.clone())
            .collect()
    }

    /// Номера треков файлов `paths` — в порядке библиотеки. Файлов, которых в ней нет,
    /// в ответе нет
    pub fn indices_of(&self, paths: &HashSet<PathBuf>) -> Vec<usize> {
        self.tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| paths.contains(&track.path))
            .map(|(index, _)| index)
            .collect()
    }

    /// Удаляет треки файлов `paths` (сами файлы не трогает) и сохраняет библиотеку
    pub fn remove_paths(&mut self, paths: &HashSet<PathBuf>) {
        let indices = self.indices_of(paths).into_iter().collect();
        self.remove(&indices);
    }

    /// Удаляет треки с номерами `indices` (сами файлы не трогает) и сохраняет библиотеку
    pub fn remove(&mut self, indices: &BTreeSet<usize>) {
        let mut index = 0;
        let (paths, missing) = (&mut self.paths, &mut self.missing);
        Arc::make_mut(&mut self.tracks).retain(|track| {
            let keep = !indices.contains(&index);
            index += 1;
            if !keep {
                paths.remove(&track.path);
                missing.remove(&track.path);
            }
            keep
        });
        self.revision += 1;
        self.save();
    }

    /// Отправляет библиотеку на запись в фоне. Ошибка придёт позже — `take_save_error`
    fn save(&self) {
        if let Some(writer) = &self.writer {
            writer.write(Snapshot {
                version: self.version,
                tracks: Arc::clone(&self.tracks),
            });
        }
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
        let dir = std::env::temp_dir().join(format!("lmp_library_{}", std::process::id()));
        let music = dir.join("music");
        fs::create_dir_all(&music).unwrap();
        let [a, b, c] = ["a", "b", "c"].map(|name| music.join(format!("{name}.mp3")));
        for path in [&a, &b, &c] {
            fs::write(path, b"").unwrap();
        }

        let (mut library, error) = Library::load(Some(dir.clone()));
        assert!(error.is_none());
        library.add(vec![track(&a), track(&b)]);
        // Повторное добавление того же файла пропускается
        library.add(vec![track(&b), track(&c)]);
        assert_eq!(titles(&library), ["a", "b", "c"]);

        library.remove(&BTreeSet::from([0]));
        library.flush();
        fs::remove_file(&c).unwrap();

        // После перезапуска — то же содержимое, а пропавший файл помечен — когда закончится
        // фоновая проверка
        let (mut library, error) = Library::load(Some(dir.clone()));
        assert!(error.is_none());
        assert_eq!(titles(&library), ["b", "c"]);
        assert!(!library.is_missing(1));
        library.check_missing_in_background(|| {});
        while !library.poll_missing() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(!library.is_missing(0));
        assert!(library.is_missing(1));
        assert!(library.paths().contains(&b));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn old_library_is_refreshed_once() {
        let dir = std::env::temp_dir().join(format!("lmp_v1_{}", std::process::id()));
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
        library.add(vec![track(&dir.join("b.mp3"))]);
        library.flush();
        let (mut library, _) = Library::load(Some(dir.clone()));
        assert!(library.outdated_paths().is_some());

        let mut fresh = track(&path);
        fresh.year = Some(1999);
        library.refresh(vec![fresh]);
        library.flush();
        let (library, _) = Library::load(Some(dir.clone()));
        assert_eq!(library.outdated_paths(), None);
        assert_eq!(library.tracks()[0].year, Some(1999));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn damaged_file_is_set_aside() {
        let dir = std::env::temp_dir().join(format!("lmp_damaged_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("library.json"), "not json").unwrap();

        let (library, error) = Library::load(Some(dir.clone()));
        assert!(library.tracks().is_empty());
        assert!(error.unwrap().contains("damaged"));
        assert!(dir.join("library.json.bak").exists());

        fs::remove_dir_all(&dir).unwrap();
    }
}
