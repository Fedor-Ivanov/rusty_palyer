//! Обложки альбомов и фото исполнителей — единственный источник картинок для всего
//! интерфейса: сеток и текущего трека. Обложки не хранятся в треках (тысячи обложек заняли
//! бы слишком много памяти), поэтому для показанных на экране они читаются из тегов
//! в фоновых потоках, уменьшаются до нужного размера и превращаются в текстуры. Хранятся
//! только недавно показанные: при прокрутке большой библиотеки память не растёт. Фото из
//! интернета (Deezer) загружаются по ссылке прямо в память и на диск не пишутся.
//!
//! При быстрой прокрутке заказов больше, чем потоки успевают: первыми готовятся те, что
//! на экране сейчас, затем соседние ряды (`prefetch`), а уехавшие с экрана выбрасываются.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use crate::core::library::metadata::{read_cover, read_folder_cover};

/// Сколько обложек держать: миниатюра ~400 КБ, итого до ~120 МБ видеопамяти
const MAX_COVERS: usize = 300;
/// Заказ, которого не хотели столько кадров, выбрасывается: картинка уехала с экрана.
/// Пока заказы ждут, кадры идут каждые 50 мс — это ~0,5 с
const STALE_FRAMES: u64 = 10;
/// Готовая картинка проявляется за столько секунд, а не возникает резко
const APPEAR_SECS: f32 = 0.15;

/// Размер обложки: от него зависит, до скольких пикселей её уменьшать
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverSize {
    /// Плитка сетки
    Thumbnail,
    /// Обложка текущего трека
    Large,
    /// Размытая обложка — фон деталей текущего трека. Маленькая: при растягивании на всю
    /// область линейная фильтрация сама сглаживает её
    Backdrop,
}

impl CoverSize {
    /// Сторона в пикселях — с запасом на экраны с двойной плотностью
    fn pixels(self) -> u32 {
        match self {
            Self::Thumbnail => 320,
            Self::Large => 640,
            Self::Backdrop => 128,
        }
    }
}

/// Сила размытия фона: сигма Гаусса в пикселях обложки `CoverSize::Backdrop`
const BACKDROP_BLUR_SIGMA: f32 = 8.0;

/// Откуда картинка: файлы (в кэше — по первому из кандидатов) или ссылка
#[derive(Clone, PartialEq, Eq, Hash)]
enum Origin {
    File(PathBuf),
    Url(String),
}

/// Картинка в кэше — по происхождению и размеру
type Key = (Origin, CoverSize);
/// Потоков для обложек из файлов. Загрузки по ссылкам идут в своём потоке: медленная сеть
/// не задерживает обложки альбомов
const WORKERS: usize = 2;

pub enum Cover<'a> {
    /// Ещё читается
    Loading,
    /// В тегах обложки нет
    Missing,
    /// Готова. Второе — насколько проявилась: 0.0..=1.0, см. `APPEAR_SECS`
    Ready(&'a TextureHandle, f32),
}

enum Entry {
    Loading,
    Missing {
        last_used: u64,
    },
    Ready {
        texture: TextureHandle,
        /// Хеш исходной картинки: по нему одинаковые обложки разных треков делят одну текстуру
        content: u64,
        last_used: u64,
        /// Когда стала готова — для проявления
        ready_at: Instant,
    },
}

/// Прочитанная обложка: картинка и хеш исходных байтов
type Loaded = Option<(ColorImage, u64)>;

/// Заказ фоновому потоку
struct Job {
    key: Key,
    /// Файлы, в тегах которых искать обложку, по порядку (у ссылки — пусто)
    candidates: Vec<PathBuf>,
    /// Кадр, в котором картинку хотели последний раз
    wanted: u64,
    /// В том кадре она была на экране, а не в соседнем ряду
    visible: bool,
}

/// Очередь заказов, общая для потоков. Поток берёт самый нужный: хотели позже всех,
/// а при равенстве — видимый раньше соседнего
#[derive(Default)]
struct Jobs {
    items: Vec<Job>,
    closed: bool,
}

type Queue = Arc<(Mutex<Jobs>, Condvar)>;

pub struct CoverCache {
    /// Заказы обложек из файлов
    files: Queue,
    /// Заказы загрузок по ссылкам
    downloads: Queue,
    results: Receiver<(Key, Loaded)>,
    entries: HashMap<Key, Entry>,
    /// Номер кадра — для вытеснения давно не показанных миниатюр и брошенных заказов
    frame: u64,
}

impl CoverCache {
    pub fn new() -> Self {
        let (result_tx, results) = mpsc::channel();
        let files = Queue::default();
        let downloads = Queue::default();
        let workers = std::iter::repeat_n(&files, WORKERS).chain(std::iter::once(&downloads));
        for queue in workers {
            let (queue, result_tx) = (Arc::clone(queue), result_tx.clone());
            thread::spawn(move || work(&queue, &result_tx));
        }
        Self {
            files,
            downloads,
            results,
            entries: HashMap::new(),
            frame: 0,
        }
    }

    /// Обложка размера `size` из тегов первого из `candidates`, где она есть: обложка
    /// альбома бывает только у части его треков. Если её ещё нет — заказывает чтение в фоне.
    /// В кэше обложка хранится по первому кандидату
    pub fn get(&mut self, candidates: &[PathBuf], size: CoverSize) -> Cover<'_> {
        let Some(first) = candidates.first() else {
            return Cover::Missing;
        };
        let key = (Origin::File(first.clone()), size);
        self.request(&key, candidates, true);
        self.cover(&key)
    }

    /// Картинка размера `size` по ссылке: загружается в фоне только в память — на диск
    /// не пишется. Не загрузилась (нет сети) — заглушка, пока картинку не вытеснят из кэша
    pub fn get_url(&mut self, url: &str, size: CoverSize) -> Cover<'_> {
        let key = (Origin::Url(url.to_owned()), size);
        self.request(&key, &[], true);
        self.cover(&key)
    }

    /// Заказывает обложку заранее — для соседних с экраном рядов сетки: к прокрутке она
    /// будет готова. Видимые готовятся раньше
    pub fn prefetch(&mut self, candidates: &[PathBuf], size: CoverSize) {
        if let Some(first) = candidates.first() {
            self.request(&(Origin::File(first.clone()), size), candidates, false);
        }
    }

    /// Заказ картинки `key` или, если она уже заказана, отметка, что она всё ещё нужна
    fn request(&mut self, key: &Key, candidates: &[PathBuf], visible: bool) {
        let frame = self.frame;
        let queue = match key.0 {
            Origin::File(_) => &self.files,
            Origin::Url(_) => &self.downloads,
        };
        let (jobs, ready) = &**queue;
        let Ok(mut jobs) = jobs.lock() else {
            return;
        };
        match self.entries.get(key) {
            None => {
                self.entries.insert(key.clone(), Entry::Loading);
                jobs.items.push(Job {
                    key: key.clone(),
                    candidates: candidates.to_vec(),
                    wanted: frame,
                    visible,
                });
                ready.notify_one();
            }
            // Ждёт в очереди — нужен и в этом кадре. Если его уже взял поток, в очереди
            // его нет: он и так скоро будет готов
            Some(Entry::Loading) => {
                if let Some(job) = jobs.items.iter_mut().find(|job| job.key == *key) {
                    job.visible = visible || (job.wanted == frame && job.visible);
                    job.wanted = frame;
                }
            }
            Some(_) => {}
        }
    }

    /// Картинка из кэша по ключу; отмечает, что её показали в этом кадре
    fn cover(&mut self, key: &Key) -> Cover<'_> {
        let frame = self.frame;
        match self.entries.get_mut(key) {
            Some(Entry::Ready {
                texture,
                last_used,
                ready_at,
                ..
            }) => {
                *last_used = frame;
                let appear = (ready_at.elapsed().as_secs_f32() / APPEAR_SECS).min(1.0);
                Cover::Ready(texture, appear)
            }
            Some(Entry::Missing { last_used }) => {
                *last_used = frame;
                Cover::Missing
            }
            _ => Cover::Loading,
        }
    }

    /// Принимает готовые миниатюры, выбрасывает брошенные заказы и вытесняет лишние
    /// миниатюры. Вызывать раз за кадр
    pub fn update(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        while let Ok((key, loaded)) = self.results.try_recv() {
            let entry = match loaded {
                Some((image, content)) => Entry::Ready {
                    // Та же картинка уже есть (у треков одного альбома обложка обычно одна) —
                    // берём её текстуру: смена трека внутри альбома не меняет обложку вовсе
                    texture: self.texture_with(content, key.1).unwrap_or_else(|| {
                        let name = match &key.0 {
                            Origin::File(path) => format!("cover {}", path.display()),
                            Origin::Url(url) => format!("cover {url}"),
                        };
                        ctx.load_texture(name, image, TextureOptions::LINEAR)
                    }),
                    content,
                    last_used: self.frame,
                    ready_at: Instant::now(),
                },
                None => Entry::Missing {
                    last_used: self.frame,
                },
            };
            self.entries.insert(key, entry);
        }
        self.drop_stale_jobs();

        // Ждём миниатюры или проявляем готовые — нужны кадры и без действий пользователя
        if self.entries.values().any(|e| matches!(e, Entry::Loading)) {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        let appearing = self.entries.values().any(|e| {
            matches!(e, Entry::Ready { ready_at, .. } if ready_at.elapsed().as_secs_f32() < APPEAR_SECS)
        });
        if appearing {
            ctx.request_repaint();
        }

        self.evict();
    }

    /// Выбрасывает заказы, которых давно не хотели: картинка уехала с экрана. Покажется
    /// снова — закажется снова
    fn drop_stale_jobs(&mut self) {
        let frame = self.frame;
        for queue in [&self.files, &self.downloads] {
            let Ok(mut jobs) = queue.0.lock() else {
                continue;
            };
            jobs.items.retain(|job| {
                let wanted = job.wanted + STALE_FRAMES >= frame;
                if !wanted {
                    self.entries.remove(&job.key);
                }
                wanted
            });
        }
    }

    /// Уже загруженная текстура обложки с хешем `content` и размером `size`
    fn texture_with(&self, content: u64, size: CoverSize) -> Option<TextureHandle> {
        self.entries
            .iter()
            .find_map(|((_, entry_size), entry)| match entry {
                Entry::Ready {
                    texture,
                    content: other,
                    ..
                } if *other == content && *entry_size == size => Some(texture.clone()),
                _ => None,
            })
    }

    /// Оставляет MAX_COVERS недавно показанных; заказанные, но не готовые не трогает
    fn evict(&mut self) {
        let loaded = self
            .entries
            .values()
            .filter(|e| !matches!(e, Entry::Loading))
            .count();
        if loaded <= MAX_COVERS {
            return;
        }
        let mut by_age: Vec<(u64, Key)> = self
            .entries
            .iter()
            .filter_map(|(key, entry)| match entry {
                Entry::Ready { last_used, .. } | Entry::Missing { last_used } => {
                    Some((*last_used, key.clone()))
                }
                Entry::Loading => None,
            })
            .collect();
        by_age.sort_unstable_by_key(|(last_used, _)| *last_used);
        for (_, key) in by_age.into_iter().take(loaded - MAX_COVERS) {
            self.entries.remove(&key);
        }
    }
}

/// Фоновый поток: берёт самый нужный заказ и готовит картинку
fn work(queue: &Queue, results: &Sender<(Key, Loaded)>) {
    let (jobs, ready) = &**queue;
    loop {
        let job = {
            let Ok(mut jobs) = jobs.lock() else {
                return;
            };
            loop {
                if jobs.closed {
                    return;
                }
                let best = jobs
                    .items
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, job)| (job.wanted, job.visible))
                    .map(|(index, _)| index);
                if let Some(index) = best {
                    break jobs.items.swap_remove(index);
                }
                let Ok(next) = ready.wait(jobs) else {
                    return;
                };
                jobs = next;
            }
        };
        let image = match &job.key.0 {
            Origin::File(_) => find_cover(&job.candidates, job.key.1),
            Origin::Url(url) => download(url).and_then(|bytes| decode_cover(&bytes, job.key.1)),
        };
        if results.send((job.key, image)).is_err() {
            return;
        }
    }
}

impl Drop for CoverCache {
    fn drop(&mut self) {
        // Потоки доделают текущую картинку и закончатся
        for queue in [&self.files, &self.downloads] {
            if let Ok(mut jobs) = queue.0.lock() {
                jobs.closed = true;
                queue.1.notify_all();
            }
        }
    }
}

/// Обложка для файлов `candidates`: из тегов первого, где она есть (обычно это первый же),
/// а если ни в одном её нет — картинкой из их папки (см. `read_folder_cover`).
/// Вместе с картинкой — хеш исходных байтов
fn find_cover(candidates: &[PathBuf], size: CoverSize) -> Loaded {
    cover_sources(candidates).find_map(|bytes| decode_cover(&bytes, size))
}

/// Байты обложки для файлов `candidates` как есть, без декодирования — в том же порядке
/// поиска, что и `find_cover`
pub fn cover_bytes(candidates: &[PathBuf]) -> Option<Vec<u8>> {
    cover_sources(candidates).next()
}

/// Найденные картинки по порядку: из тегов файлов, затем из их папок. Читаются лениво —
/// до первой подходящей
fn cover_sources(candidates: &[PathBuf]) -> impl Iterator<Item = Vec<u8>> + '_ {
    // Кандидат-картинка (фото исполнителя) читается как есть, остальные — аудиофайлы
    let tags = candidates.iter().filter_map(|path| {
        if is_image_file(path) {
            std::fs::read(path).ok()
        } else {
            read_cover(path)
        }
    });
    let folders = std::iter::once(()).flat_map(move |()| {
        // Папки по порядку кандидатов, каждая один раз: треки альбома обычно в одной.
        // Папки картинок не смотрим: рядом с фото исполнителя лежат фото других
        let mut dirs: Vec<&Path> = Vec::new();
        for dir in candidates
            .iter()
            .filter(|path| !is_image_file(path))
            .filter_map(|path| path.parent())
        {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        dirs.into_iter().filter_map(read_folder_cover)
    });
    tags.chain(folders)
}

/// Картинка по ссылке — байтами в память
fn download(url: &str) -> Option<Vec<u8>> {
    let mut response = crate::core::net::agent().get(url).call().ok()?;
    if response.status() != 200 {
        return None;
    }
    response.body_mut().read_to_vec().ok()
}

/// Файл картинки, а не аудиофайл — по расширению
fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["jpg", "jpeg", "png", "webp"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

/// Картинка обложки, уменьшенная так, чтобы большая сторона была не больше
/// `size.pixels()`; для фона — ещё и размытая
fn decode_cover(bytes: &[u8], size: CoverSize) -> Loaded {
    let pixels = size.pixels();
    let mut image = image::load_from_memory(bytes)
        .ok()?
        .thumbnail(pixels, pixels)
        .to_rgba8();
    if size == CoverSize::Backdrop {
        image = image::imageops::blur(&image, BACKDROP_BLUR_SIGMA);
    }
    let size = [image.width() as usize, image.height() as usize];
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some((
        ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
        hasher.finish(),
    ))
}
