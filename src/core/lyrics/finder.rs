//! Текст песни для трека: свой (файл `.lrc`, теги) или из LRCLIB. Ответы LRCLIB хранятся
//! в папке данных (`lyrics/`, файл на трек) — один трек спрашивается один раз. Запросы идут
//! в фоне: пока ответа нет, показывается то, что есть локально.

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::{
    Lyrics, load_lyrics,
    lrclib::{self, Remote, TrackSignature},
    parse_lyrics,
};
use crate::core::{library::Track, storage::write_atomically};

const DIR_NAME: &str = "lyrics";
const FORMAT_VERSION: u32 = 1;
/// «Не найден» запоминается на столько: потом спросим снова — текст могли добавить
const NOT_FOUND_TTL: Duration = Duration::from_secs(14 * 24 * 60 * 60);
/// Сеть или сервер подвели (LRCLIB нередко отвечает «занят») — запрос повторяется
/// с такими паузами
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
];
/// Не дозвались и после повторов — спросим снова не раньше чем через столько
const FAILED_RETRY_AFTER: Duration = Duration::from_secs(5 * 60);

/// Что показать для трека
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrackLyrics {
    /// Лучший из найденных текстов: синхронизированный важнее обычного
    pub lyrics: Option<Lyrics>,
    /// LRCLIB ещё отвечает — текст может появиться или смениться синхронизированным
    pub searching: bool,
    /// LRCLIB знает, что в треке нет слов
    pub instrumental: bool,
    /// LRCLIB недавно не ответил (сеть, сервер занят) — спросим позже
    pub unavailable: bool,
}

/// Ответ LRCLIB в кеше
#[derive(Serialize, Deserialize)]
struct Cached {
    version: u32,
    /// Когда спрашивали — секунды Unix
    checked_at: u64,
    remote: Remote,
}

pub struct LyricsFinder {
    /// Папка кеша; `None` — папка данных недоступна, ответы живут до выхода
    dir: Option<PathBuf>,
    /// Ответы, известные в этом запуске: прочитанные из кеша и полученные
    known: HashMap<TrackSignature, Remote>,
    /// Запросы в пути
    pending: HashSet<TrackSignature>,
    /// Не дозвались (сеть, сервер занят) — и когда: спросим снова через `FAILED_RETRY_AFTER`
    failed: HashMap<TrackSignature, Instant>,
    sender: Sender<(TrackSignature, Result<Remote, String>)>,
    results: Receiver<(TrackSignature, Result<Remote, String>)>,
    /// Будит приложение, когда пришёл ответ: оно перечитает текст
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl LyricsFinder {
    pub fn new(data_dir: Option<PathBuf>, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (sender, results) = mpsc::channel();
        Self {
            dir: data_dir.map(|dir| dir.join(DIR_NAME)),
            known: HashMap::new(),
            pending: HashSet::new(),
            failed: HashMap::new(),
            sender,
            results,
            wake,
        }
    }

    /// Текст песни трека. Свой синхронизированный — сразу он. Иначе спрашиваем LRCLIB
    /// (из кеша или запросом в фоне): его синхронизированный важнее своего обычного
    pub fn find(&mut self, track: &Track) -> TrackLyrics {
        let local = load_lyrics(&track.path);
        if local.as_ref().is_some_and(|lyrics| lyrics.synced) {
            return TrackLyrics {
                lyrics: local,
                ..TrackLyrics::default()
            };
        }
        let Some(signature) = signature(track) else {
            return TrackLyrics {
                lyrics: local,
                ..TrackLyrics::default()
            };
        };
        match self.remote(&signature) {
            Some(Remote::Found {
                synced,
                plain,
                instrumental,
            }) => {
                let synced = synced.as_deref().and_then(parse_lyrics);
                let plain = plain.as_deref().and_then(parse_lyrics);
                TrackLyrics {
                    lyrics: synced.or(local).or(plain),
                    instrumental,
                    ..TrackLyrics::default()
                }
            }
            Some(Remote::NotFound) => TrackLyrics {
                lyrics: local,
                ..TrackLyrics::default()
            },
            None => {
                let searching = self.request(signature);
                TrackLyrics {
                    lyrics: local,
                    searching,
                    unavailable: !searching,
                    ..TrackLyrics::default()
                }
            }
        }
    }

    /// Забирает пришедшие ответы и сохраняет их в кеш. `true` — что-то пришло: текст
    /// трека стоит найти заново
    pub fn poll(&mut self) -> bool {
        let mut arrived = false;
        while let Ok((signature, result)) = self.results.try_recv() {
            self.pending.remove(&signature);
            arrived = true;
            match result {
                Ok(remote) => {
                    self.save(&signature, &remote);
                    self.known.insert(signature, remote);
                }
                Err(e) => {
                    eprintln!("Could not get lyrics for {}: {e}", signature.title);
                    self.failed.insert(signature, Instant::now());
                }
            }
        }
        arrived
    }

    /// Ответ LRCLIB, если он уже известен: в этом запуске или из кеша, пока не устарел
    fn remote(&mut self, signature: &TrackSignature) -> Option<Remote> {
        if let Some(remote) = self.known.get(signature) {
            return Some(remote.clone());
        }
        let file = self.dir.as_ref()?.join(file_name(signature));
        let cached: Cached = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
        let age = now_secs().saturating_sub(cached.checked_at);
        if cached.remote == Remote::NotFound && age > NOT_FOUND_TTL.as_secs() {
            return None;
        }
        self.known.insert(signature.clone(), cached.remote.clone());
        Some(cached.remote)
    }

    /// Спрашивает LRCLIB в фоне, если ещё не спрашивали. `true` — ответ ждём
    fn request(&mut self, signature: TrackSignature) -> bool {
        if let Some(failed_at) = self.failed.get(&signature) {
            if failed_at.elapsed() < FAILED_RETRY_AFTER {
                return false;
            }
            self.failed.remove(&signature);
        }
        if self.pending.insert(signature.clone()) {
            let sender = self.sender.clone();
            let wake = Arc::clone(&self.wake);
            std::thread::spawn(move || {
                let mut result = lrclib::fetch(&signature);
                for delay in RETRY_DELAYS {
                    if result.is_ok() {
                        break;
                    }
                    std::thread::sleep(delay);
                    result = lrclib::fetch(&signature);
                }
                let _ = sender.send((signature, result));
                wake();
            });
        }
        true
    }

    fn save(&self, signature: &TrackSignature, remote: &Remote) {
        let Some(dir) = &self.dir else {
            return;
        };
        let cached = Cached {
            version: FORMAT_VERSION,
            checked_at: now_secs(),
            remote: remote.clone(),
        };
        // Кеш — не данные пользователя: не сохранился — спросим ещё раз в другой раз
        if let Err(e) = write_atomically(&dir.join(file_name(signature)), &cached) {
            eprintln!("Could not save the lyrics: {e}");
        }
    }
}

/// Сигнатура трека для LRCLIB. Без исполнителя или названия искать бессмысленно
fn signature(track: &Track) -> Option<TrackSignature> {
    let artist = track.artist.trim();
    let title = track.title.trim();
    if artist.is_empty() || title.is_empty() {
        return None;
    }
    Some(TrackSignature {
        artist: artist.to_owned(),
        title: title.to_owned(),
        album: track.album.trim().to_owned(),
        duration_secs: track.duration_secs,
    })
}

/// Имя файла кеша — хеш сигнатуры без учёта регистра. FNV-1a, а не `DefaultHasher`:
/// тот может считать иначе в другой версии Rust, и кеш потерялся бы
fn file_name(signature: &TrackSignature) -> String {
    let key = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        signature.artist.to_lowercase(),
        signature.title.to_lowercase(),
        signature.album.to_lowercase(),
        signature.duration_secs
    );
    let hash = key.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}.json")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{LyricsFinder, Remote, TrackSignature, file_name};
    use std::{fs, sync::Arc};

    fn signature() -> TrackSignature {
        TrackSignature {
            artist: "Ghost B.C.".into(),
            title: "Jigolo Har Megiddo".into(),
            album: "Infestissumam".into(),
            duration_secs: 238,
        }
    }

    #[test]
    fn file_name_is_stable_and_case_insensitive() {
        let lower = TrackSignature {
            artist: "ghost b.c.".into(),
            ..signature()
        };
        assert_eq!(file_name(&signature()), file_name(&lower));
        // Хеш не должен меняться между версиями — иначе кеш потеряется
        assert_eq!(file_name(&signature()), "33735ca278336967.json");
    }

    #[test]
    fn answers_survive_restart() {
        let dir = std::env::temp_dir().join(format!("lmp_lyrics_{}", fastrand::u64(..)));
        let remote = Remote::Found {
            synced: Some("[00:01.00]Line".into()),
            plain: Some("Line".into()),
            instrumental: false,
        };
        let finder = LyricsFinder::new(Some(dir.clone()), Arc::new(|| {}));
        finder.save(&signature(), &remote);

        let mut restarted = LyricsFinder::new(Some(dir.clone()), Arc::new(|| {}));
        assert_eq!(restarted.remote(&signature()), Some(remote));
        fs::remove_dir_all(dir).unwrap();
    }
}
