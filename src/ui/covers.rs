//! Обложки альбомов — единственный источник для всего интерфейса: сетки альбомов и текущего
//! трека. Обложки не хранятся в треках (тысячи обложек заняли бы слишком много памяти),
//! поэтому для показанных на экране они читаются из тегов в фоновых потоках, уменьшаются
//! до нужного размера и превращаются в текстуры. Хранятся только недавно показанные:
//! при прокрутке большой библиотеки память не растёт.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use crate::core::library::metadata::read_cover;

/// Сколько обложек держать: миниатюра ~400 КБ, итого до ~120 МБ видеопамяти
const MAX_COVERS: usize = 300;

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

/// Обложка в кэше — по первому из файлов-кандидатов и размеру
type Key = (PathBuf, CoverSize);
/// Заказ потоку: ключ и файлы, в тегах которых искать обложку, по порядку
type Request = (Key, Vec<PathBuf>);
const WORKERS: usize = 2;

pub enum Cover<'a> {
    /// Ещё читается
    Loading,
    /// В тегах обложки нет
    Missing,
    Ready(&'a TextureHandle),
}

enum Entry {
    Loading,
    Missing {
        last_used: u64,
    },
    Ready {
        texture: TextureHandle,
        last_used: u64,
    },
}

pub struct CoverCache {
    requests: Sender<Request>,
    results: Receiver<(Key, Option<ColorImage>)>,
    entries: HashMap<Key, Entry>,
    /// Номер кадра — для вытеснения давно не показанных миниатюр
    frame: u64,
}

impl CoverCache {
    pub fn new() -> Self {
        let (requests, request_rx) = mpsc::channel::<Request>();
        let (result_tx, results) = mpsc::channel();
        // Потоки делят одну очередь запросов
        let request_rx = Arc::new(Mutex::new(request_rx));
        for _ in 0..WORKERS {
            let (request_rx, result_tx) = (request_rx.clone(), result_tx.clone());
            thread::spawn(move || {
                loop {
                    let Ok(key) = request_rx.lock().map(|rx| rx.recv()) else {
                        break;
                    };
                    let Ok((key, candidates)) = key else {
                        break;
                    };
                    // Первый файл, в тегах которого обложка есть: обычно это первый же
                    let image = candidates.iter().find_map(|path| load_cover(path, key.1));
                    if result_tx.send((key, image)).is_err() {
                        break;
                    }
                }
            });
        }
        Self {
            requests,
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
        let frame = self.frame;
        let key = (first.clone(), size);
        if !self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), Entry::Loading);
            let _ = self.requests.send((key.clone(), candidates.to_vec()));
        }
        match self.entries.get_mut(&key) {
            Some(Entry::Ready { texture, last_used }) => {
                *last_used = frame;
                Cover::Ready(texture)
            }
            Some(Entry::Missing { last_used }) => {
                *last_used = frame;
                Cover::Missing
            }
            _ => Cover::Loading,
        }
    }

    /// Принимает готовые миниатюры и вытесняет лишние. Вызывать раз за кадр
    pub fn update(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        while let Ok((key, image)) = self.results.try_recv() {
            let entry = match image {
                Some(image) => Entry::Ready {
                    texture: ctx.load_texture(
                        format!("cover {}", key.0.display()),
                        image,
                        TextureOptions::LINEAR,
                    ),
                    last_used: self.frame,
                },
                None => Entry::Missing {
                    last_used: self.frame,
                },
            };
            self.entries.insert(key, entry);
        }

        // Ждём миниатюры — нужны кадры и без действий пользователя
        if self.entries.values().any(|e| matches!(e, Entry::Loading)) {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        self.evict();
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

/// Обложка из тегов, уменьшенная так, чтобы большая сторона была не больше
/// `size.pixels()`; для фона — ещё и размытая
fn load_cover(path: &Path, size: CoverSize) -> Option<ColorImage> {
    let bytes = read_cover(path)?;
    let pixels = size.pixels();
    let mut image = image::load_from_memory(&bytes)
        .ok()?
        .thumbnail(pixels, pixels)
        .to_rgba8();
    if size == CoverSize::Backdrop {
        image = image::imageops::blur(&image, BACKDROP_BLUR_SIGMA);
    }
    let size = [image.width() as usize, image.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
}
