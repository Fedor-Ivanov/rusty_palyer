//! Фоновое добавление треков. Чтение тегов и проверка формата для тысяч файлов занимают
//! секунды и минуты — в потоке интерфейса окно бы на это время зависло. Здесь файлы
//! обрабатываются в нескольких потоках; треки копятся и отдаются разом, когда готово всё —
//! так в списке сразу весь выбор, и нужный трек из конца папки можно найти.
//!
//! Отменить добавление — просто уронить `Loader`: потоки заметят закрытый канал и остановятся.

use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use super::{scan::find_audio_files, track::Track};

/// Больше потоков не даёт выигрыша: упираемся в чтение с диска
const MAX_WORKERS: usize = 8;

/// Что добавляем
pub enum Source {
    Files(Vec<PathBuf>),
    /// Все аудиофайлы в папке, включая вложенные
    Folder(PathBuf),
}

enum Message {
    /// Сколько файлов будет обработано и сколько пропущено, потому что они уже есть
    /// у получателя, — известно после обхода папки
    Total(usize, usize),
    /// Результат для файла с номером `usize` в исходном порядке
    Loaded(usize, Result<Track, String>),
    /// Папку не удалось прочитать
    Failed(String),
}

pub struct Loader {
    messages: Receiver<Message>,
    started: Instant,
    total: Option<usize>,
    skipped: usize,
    /// Номер следующего результата по порядку — треки собираются строго в исходном порядке
    next: usize,
    /// Результаты, пришедшие раньше своей очереди: потоки работают параллельно
    pending: BTreeMap<usize, Result<Track, String>>,
    tracks: Vec<Track>,
    errors: Vec<String>,
    failed: bool,
}

impl Loader {
    /// Начинает добавление. Файлы из `skip` пропускаются без чтения — например, те,
    /// что уже есть в библиотеке
    pub fn start(source: Source, skip: HashSet<PathBuf>) -> Self {
        let (tx, messages) = mpsc::channel();
        thread::spawn(move || run(source, skip, tx));
        Self {
            messages,
            started: Instant::now(),
            total: None,
            skipped: 0,
            next: 0,
            pending: BTreeMap::new(),
            tracks: Vec::new(),
            errors: Vec::new(),
            failed: false,
        }
    }

    /// Принимает результаты, готовые к этому моменту. Не блокирует. Вызывать каждый кадр
    pub fn update(&mut self) {
        loop {
            match self.messages.try_recv() {
                Ok(Message::Total(total, skipped)) => {
                    self.total = Some(total);
                    self.skipped = skipped;
                }
                Ok(Message::Loaded(index, result)) => {
                    self.pending.insert(index, result);
                }
                Ok(Message::Failed(error)) => {
                    self.errors.push(error);
                    self.failed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // Все потоки завершились; если какой-то упал, недостающих треков
                    // уже не будет — не ждём их вечно
                    if self
                        .total
                        .is_none_or(|total| self.next + self.pending.len() < total)
                    {
                        self.failed = true;
                    }
                    break;
                }
            }
        }

        while let Some(result) = self.pending.remove(&self.next) {
            self.next += 1;
            match result {
                Ok(track) => self.tracks.push(track),
                Err(error) => self.errors.push(error),
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        self.failed || self.total.is_some_and(|total| self.next >= total)
    }

    /// Сколько файлов обработано и сколько всего; `None`, пока идёт обход папки
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.total
            .map(|total| (self.next + self.pending.len(), total))
    }

    /// Сколько прошло с начала добавления
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Итог: треки в исходном порядке, ошибки по файлам и сколько файлов нашлось
    pub fn finish(self) -> Finished {
        Finished {
            tracks: self.tracks,
            errors: self.errors,
            total: self.total,
            skipped: self.skipped,
        }
    }
}

pub struct Finished {
    pub tracks: Vec<Track>,
    /// Ошибки по файлам, которые не удалось добавить
    pub errors: Vec<String>,
    /// `None`, если папку не удалось прочитать
    pub total: Option<usize>,
    /// Сколько файлов пропущено, потому что они уже есть у получателя
    pub skipped: usize,
}

fn run(source: Source, skip: HashSet<PathBuf>, tx: Sender<Message>) {
    let mut paths = match source {
        Source::Files(paths) => paths,
        Source::Folder(dir) => match find_audio_files(&dir) {
            Ok(paths) => paths,
            Err(error) => {
                let _ = tx.send(Message::Failed(error));
                return;
            }
        },
    };
    let found = paths.len();
    paths.retain(|path| !skip.contains(path));
    let _ = tx.send(Message::Total(paths.len(), found - paths.len()));

    // Потоки берут файлы по очереди из общего счётчика: быстрые файлы не ждут медленных
    let paths = Arc::new(paths);
    let next = Arc::new(AtomicUsize::new(0));
    let workers = thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(MAX_WORKERS);
    for _ in 0..workers {
        let (paths, next, tx) = (paths.clone(), next.clone(), tx.clone());
        thread::spawn(move || {
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = paths.get(index) else {
                    break;
                };
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let result = Track::from_path(path.clone()).map_err(|e| format!("{name}: {e}"));
                // Получатель пропал — интерфейс закрыт, работать дальше незачем
                if tx.send(Message::Loaded(index, result)).is_err() {
                    break;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{Loader, Source};
    use std::{fs, path::Path, thread, time::Duration};

    /// Минимальный корректный WAV: 16-бит моно, 8 кГц, 0,1 с тишины
    fn write_wav(path: &Path) {
        let samples = 800u32;
        let data_len = samples * 2;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_len).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes()); // PCM
        bytes.extend(1u16.to_le_bytes()); // моно
        bytes.extend(8000u32.to_le_bytes());
        bytes.extend(16000u32.to_le_bytes()); // байт в секунду
        bytes.extend(2u16.to_le_bytes()); // байт на сэмпл
        bytes.extend(16u16.to_le_bytes()); // бит на сэмпл
        bytes.extend(b"data");
        bytes.extend(data_len.to_le_bytes());
        bytes.resize(bytes.len() + data_len as usize, 0);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn loads_folder_in_order_and_reports_errors() {
        let dir = std::env::temp_dir().join(format!("rusty_player_loader_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let names: Vec<String> = (0..40).map(|i| format!("{i:02}.wav")).collect();
        for name in &names {
            write_wav(&dir.join(name));
        }
        // Битый файл с подходящим расширением — должен попасть в ошибки, а не в список
        fs::write(dir.join("99.mp3"), b"not audio").unwrap();

        let skip = [dir.join("05.wav")].into_iter().collect();
        let mut loader = Loader::start(Source::Folder(dir.clone()), skip);
        for _ in 0..500 {
            loader.update();
            if loader.is_finished() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        fs::remove_dir_all(&dir).unwrap();

        assert!(loader.is_finished());
        // 05.wav был в `skip` — его не читали
        assert_eq!(loader.progress(), Some((40, 40)));
        let result = loader.finish();
        let loaded: Vec<String> = result
            .tracks
            .iter()
            .map(|t| t.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        let expected: Vec<&String> = names.iter().filter(|n| *n != "05.wav").collect();
        assert_eq!(loaded.iter().collect::<Vec<_>>(), expected);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].starts_with("99.mp3"));
        assert_eq!(result.total, Some(40));
        assert_eq!(result.skipped, 1);
    }
}
