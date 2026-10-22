//! Картинки исполнителей. Сначала своя — файл `artist.jpg` (`.png`, `.webp`) рядом
//! с музыкой исполнителя, так их кладут Kodi и Plex. Нет своей — фото из Deezer.
//!
//! Фото Deezer на диск не сохраняются: так требуют права на них. В папке данных
//! (`artists/`) хранится только ссылка (и ответ «не найдено») — чтобы не спрашивать Deezer
//! при каждом запуске. Сама картинка загружается по ссылке, когда она нужна на экране,
//! и живёт только в памяти (см. `ui::covers`).
//!
//! Вся работа — в фоновом потоке, интерфейс её не ждёт: после загрузки медиатеки в очередь
//! встают все исполнители, и картинки появляются со временем. Исполнители на экране
//! обрабатываются раньше остальных. К Deezer — по одному запросу, с паузой, и только
//! если в настройках включён поиск в интернете (`set_online`). Без него фото Deezer
//! не показываются вовсе — загрузка по ссылке тоже обращение к интернету.

mod deezer;

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

use super::storage::{now_secs, stable_hash, write_atomically};

const DIR_NAME: &str = "artists";
/// 2 — вместо картинки хранится ссылка на неё. Ответы версии 1 не годятся: картинки
/// версии 1 удаляются (`remove_stored_pictures`), и исполнителя спрашиваем заново
const FORMAT_VERSION: u32 = 2;
/// Имя своей картинки исполнителя — с одним из этих расширений
const LOCAL_NAME: &str = "artist";
const LOCAL_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];
/// «Не найдено» запоминается на столько: потом спросим снова — фото могли добавить
const NOT_FOUND_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// Не дозвались — спросим снова не раньше чем через столько
const FAILED_RETRY_AFTER: Duration = Duration::from_secs(5 * 60);
/// Пауза после запроса к Deezer: у него ограничение — 50 запросов за 5 секунд
const REQUEST_GAP: Duration = Duration::from_millis(200);

/// Что показать вместо заглушки
#[derive(Debug, Clone, PartialEq)]
pub enum ArtistImage {
    /// Своя картинка — файл
    File(PathBuf),
    /// Фото Deezer — загрузить по ссылке, только в память
    Url(String),
    /// Ищется в фоне
    Searching,
    /// Картинки нет — заглушка
    Missing,
}

/// Исполнитель для фонового поиска
pub struct ArtistJob {
    pub name: String,
    /// Папки его треков: в них ищется своя картинка
    pub music_dirs: Vec<PathBuf>,
}

/// Найденная картинка исполнителя
#[derive(Debug, Clone, PartialEq)]
enum Picture {
    File(PathBuf),
    Url(String),
}

/// Ответ Deezer в кеше — ссылка на фото, не само фото
#[derive(Serialize, Deserialize)]
struct Cached {
    version: u32,
    /// Когда спрашивали — секунды Unix
    checked_at: u64,
    /// `None` — не найдено
    url: Option<String>,
}

/// Состояние исполнителя в этом запуске
enum State {
    /// В очереди фонового потока; `urgent` — уже поставлен вперёд
    Queued {
        urgent: bool,
    },
    Done(Option<Picture>),
    /// Ни своей, ни известной ссылки нет, а спросить Deezer нельзя — поиск в интернете выключен.
    /// Включат — спросим (`set_online`)
    Offline,
    Failed(Instant),
}

/// Ответ фонового потока об исполнителе
enum Outcome {
    Done(Option<Picture>),
    Offline,
    Failed(String),
}

/// Очередь фонового потока: сначала срочные (на экране), потом остальные
#[derive(Default)]
struct Queue {
    urgent: VecDeque<ArtistJob>,
    background: VecDeque<ArtistJob>,
    closed: bool,
}

type Answer = (String, Outcome);

pub struct ArtistImages {
    /// По имени в нижнем регистре
    states: HashMap<String, State>,
    queue: Arc<(Mutex<Queue>, Condvar)>,
    answers: Receiver<Answer>,
    /// Можно ли спрашивать Deezer — общий с фоновым потоком
    online: Arc<AtomicBool>,
}

impl ArtistImages {
    /// `data_dir` — папка данных (`None` — недоступна: только свои картинки). `wake` будит
    /// приложение, когда картинка готова
    pub fn new(data_dir: Option<PathBuf>, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let cache = data_dir.map(|dir| dir.join(DIR_NAME));
        let queue: Arc<(Mutex<Queue>, Condvar)> = Arc::default();
        let (reply, answers) = mpsc::channel();
        let online = Arc::new(AtomicBool::new(false));
        let (worker_queue, worker_online) = (Arc::clone(&queue), Arc::clone(&online));
        std::thread::spawn(move || {
            work(
                &worker_queue,
                cache.as_deref(),
                &worker_online,
                &reply,
                &*wake,
            );
        });
        Self {
            states: HashMap::new(),
            queue,
            answers,
            online,
        }
    }

    /// Разрешает или запрещает спрашивать Deezer. После включения исполнители, которых
    /// не искали из-за выключенного поиска, снова считаются неизвестными: их можно ставить
    /// в очередь (`prefetch`, `image`)
    pub fn set_online(&mut self, online: bool) {
        self.online.store(online, Ordering::Relaxed);
        if online {
            self.states
                .retain(|_, state| !matches!(state, State::Offline));
        }
    }

    /// Ставит в фоновую очередь всех исполнителей, о которых ещё ничего не известно, —
    /// после загрузки или изменения медиатеки
    pub fn prefetch(&mut self, artists: impl IntoIterator<Item = ArtistJob>) {
        let jobs: Vec<ArtistJob> = artists
            .into_iter()
            .filter(|job| self.should_queue(&key(&job.name)))
            .collect();
        if jobs.is_empty() {
            return;
        }
        for job in &jobs {
            self.states
                .insert(key(&job.name), State::Queued { urgent: false });
        }
        self.push(jobs, false);
    }

    /// Картинка исполнителя, который на экране. Ещё не готова — он встаёт в начало очереди.
    /// `music_dirs` вызывается, только если исполнителя нужно поставить в очередь
    pub fn image(&mut self, name: &str, music_dirs: impl FnOnce() -> Vec<PathBuf>) -> ArtistImage {
        let key = key(name);
        if key.is_empty() {
            return ArtistImage::Missing;
        }
        match self.states.get(&key) {
            Some(State::Done(picture)) => {
                return match picture {
                    Some(Picture::File(path)) => ArtistImage::File(path.clone()),
                    // Загрузка по ссылке — обращение к интернету: только с разрешения
                    Some(Picture::Url(url)) if self.online.load(Ordering::Relaxed) => {
                        ArtistImage::Url(url.clone())
                    }
                    _ => ArtistImage::Missing,
                };
            }
            Some(State::Queued { urgent: true }) => return ArtistImage::Searching,
            Some(State::Offline) => return ArtistImage::Missing,
            Some(State::Failed(_)) if !self.should_queue(&key) => return ArtistImage::Missing,
            _ => {}
        }
        // Нового или ждущего в общей очереди — вперёд
        self.states.insert(key, State::Queued { urgent: true });
        self.push(
            vec![ArtistJob {
                name: name.to_owned(),
                music_dirs: music_dirs(),
            }],
            true,
        );
        ArtistImage::Searching
    }

    /// Забирает готовые картинки. `true` — что-то пришло: стоит перерисовать
    pub fn poll(&mut self) -> bool {
        let mut arrived = false;
        while let Ok((key, outcome)) = self.answers.try_recv() {
            arrived = true;
            let state = match outcome {
                Outcome::Done(image) => State::Done(image),
                Outcome::Offline => State::Offline,
                Outcome::Failed(e) => {
                    eprintln!("Could not get the picture of {key}: {e}");
                    State::Failed(Instant::now())
                }
            };
            self.states.insert(key, state);
        }
        arrived
    }

    /// Неизвестного или давно не дозвавшегося — можно ставить в очередь
    fn should_queue(&self, key: &str) -> bool {
        match self.states.get(key) {
            None => true,
            Some(State::Failed(at)) => at.elapsed() >= FAILED_RETRY_AFTER,
            Some(_) => false,
        }
    }

    fn push(&self, jobs: Vec<ArtistJob>, urgent: bool) {
        let (queue, ready) = &*self.queue;
        let Ok(mut queue) = queue.lock() else {
            return;
        };
        if urgent {
            queue.urgent.extend(jobs);
        } else {
            queue.background.extend(jobs);
        }
        ready.notify_one();
    }
}

impl Drop for ArtistImages {
    fn drop(&mut self) {
        // Фоновый поток доделает текущего исполнителя и закончится
        let (queue, ready) = &*self.queue;
        if let Ok(mut queue) = queue.lock() {
            queue.closed = true;
            ready.notify_one();
        }
    }
}

fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Фоновый поток: берёт исполнителей из очереди по одному — сначала срочных
fn work(
    queue: &(Mutex<Queue>, Condvar),
    cache: Option<&Path>,
    online: &AtomicBool,
    reply: &Sender<Answer>,
    wake: &(dyn Fn() + Send + Sync),
) {
    if let Some(cache) = cache {
        remove_stored_pictures(cache);
    }
    let (queue, ready) = queue;
    // Кто уже обработан: исполнитель мог встать в очередь дважды — фоном и срочно
    let mut done: HashSet<String> = HashSet::new();
    loop {
        let job = {
            let Ok(mut queue) = queue.lock() else {
                return;
            };
            loop {
                if queue.closed {
                    return;
                }
                if let Some(job) = queue
                    .urgent
                    .pop_front()
                    .or_else(|| queue.background.pop_front())
                {
                    break job;
                }
                let Ok(next) = ready.wait(queue) else {
                    return;
                };
                queue = next;
            }
        };
        let key = key(&job.name);
        if done.contains(&key) {
            continue;
        }
        let online = online.load(Ordering::Relaxed);
        let (outcome, asked_deezer) = find_image(&key, &job.music_dirs, cache, online);
        // Не дозвались или не спрашивали — в следующий раз можно снова
        if matches!(outcome, Outcome::Done(_)) {
            done.insert(key.clone());
        }
        if reply.send((key, outcome)).is_err() {
            return;
        }
        wake();
        if asked_deezer {
            std::thread::sleep(REQUEST_GAP);
        }
    }
}

/// Картинка исполнителя: своя, из кеша или — если `online` — из Deezer. Второе значение —
/// спрашивали ли Deezer: после запроса нужна пауза
fn find_image(
    key: &str,
    music_dirs: &[PathBuf],
    cache: Option<&Path>,
    online: bool,
) -> (Outcome, bool) {
    if let Some(local) = find_local(music_dirs) {
        return (Outcome::Done(Some(Picture::File(local))), false);
    }
    let Some(cache) = cache else {
        return (Outcome::Done(None), false);
    };
    if let Some(cached) = read_cached(cache, key) {
        return (Outcome::Done(cached.map(Picture::Url)), false);
    }
    if !online {
        return (Outcome::Offline, false);
    }
    let outcome = match fetch_and_store(cache, key) {
        Ok(url) => Outcome::Done(url.map(Picture::Url)),
        Err(e) => Outcome::Failed(e),
    };
    (outcome, true)
}

/// Своя картинка исполнителя в одной из папок `dirs`
fn find_local(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter()
        .flat_map(|dir| {
            LOCAL_EXTENSIONS
                .iter()
                .map(move |ext| dir.join(format!("{LOCAL_NAME}.{ext}")))
        })
        .find(|path| path.is_file())
}

/// Файл кеша исполнителя: ответ Deezer
fn cache_file(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{:016x}.json", stable_hash(key)))
}

/// Ответ из кеша: `Some(Some(ссылка))`, `Some(None)` — «не найдено», пока не устарело;
/// `None` — в кеше ничего нет (или устарело, или старой версии)
fn read_cached(dir: &Path, key: &str) -> Option<Option<String>> {
    let text = fs::read_to_string(cache_file(dir, key)).ok()?;
    let cached: Cached = serde_json::from_str(&text).ok()?;
    if cached.version != FORMAT_VERSION {
        return None;
    }
    if cached.url.is_some() {
        return Some(cached.url);
    }
    let age = now_secs().saturating_sub(cached.checked_at);
    (age < NOT_FOUND_TTL.as_secs()).then_some(None)
}

/// Спрашивает Deezer и сохраняет в кеш ответ — ссылку на фото, не само фото
fn fetch_and_store(dir: &Path, key: &str) -> Result<Option<String>, String> {
    let url = deezer::find_picture(key)?;
    let cached = Cached {
        version: FORMAT_VERSION,
        checked_at: now_secs(),
        url: url.clone(),
    };
    write_atomically(&cache_file(dir, key), &cached)?;
    Ok(url)
}

/// Удаляет фото, которые сохраняла версия 1: хранить фото Deezer на диске нельзя
fn remove_stored_pictures(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.extension().is_some_and(|ext| ext == "jpg") {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtistImage, ArtistImages, ArtistJob, Cached, FORMAT_VERSION, cache_file};
    use crate::core::storage::{now_secs, write_atomically};
    use std::{fs, sync::Arc, time::Duration};

    /// Ждёт, пока фоновый поток ответит про `name`
    fn wait(images: &mut ArtistImages, name: &str) -> ArtistImage {
        for _ in 0..2000 {
            images.poll();
            let image = images.image(name, Vec::new);
            if image != ArtistImage::Searching {
                return image;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("no answer for {name}");
    }

    #[test]
    fn finds_local_pictures_and_cached_links_in_background() {
        let root = std::env::temp_dir().join(format!("lmp_artists_{}", fastrand::u64(..)));
        let music = root.join("music");
        fs::create_dir_all(&music).unwrap();
        fs::write(music.join("artist.png"), b"png").unwrap();

        // В кеше — ссылка на фото одного исполнителя, «не найдено» другого и фото,
        // сохранённое старой версией: его быть не должно
        let data = root.join("data");
        let cache = data.join("artists");
        fs::create_dir_all(&cache).unwrap();
        let found = Cached {
            version: FORMAT_VERSION,
            checked_at: now_secs(),
            url: Some("https://cdn/him.jpg".into()),
        };
        write_atomically(&cache_file(&cache, "him"), &found).unwrap();
        let missing = Cached { url: None, ..found };
        write_atomically(&cache_file(&cache, "nobody"), &missing).unwrap();
        let old_picture = cache.join("0123456789abcdef.jpg");
        fs::write(&old_picture, b"jpg").unwrap();

        let mut images = ArtistImages::new(Some(data), Arc::new(|| {}));
        images.prefetch([ArtistJob {
            name: "Artist".into(),
            music_dirs: vec![music.clone()],
        }]);
        assert_eq!(
            wait(&mut images, "Artist"),
            ArtistImage::File(music.join("artist.png"))
        );
        // Поиск в интернете выключен (так по умолчанию): фото Deezer не показываем
        // и Deezer не спрашиваем
        assert_eq!(wait(&mut images, "HIM"), ArtistImage::Missing);
        assert_eq!(wait(&mut images, "Unknown Artist"), ArtistImage::Missing);
        assert!(!old_picture.exists());

        // Включили — известная ссылка показывается сразу, без запроса
        images.set_online(true);
        assert_eq!(
            images.image("HIM", Vec::new),
            ArtistImage::Url("https://cdn/him.jpg".into())
        );
        // «Не найдено» из кеша — тоже без запроса
        assert_eq!(wait(&mut images, "Nobody"), ArtistImage::Missing);
        drop(images);
        fs::remove_dir_all(root).unwrap();
    }
}
