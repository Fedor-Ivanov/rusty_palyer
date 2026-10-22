//! Запись файлов данных: медиатека, плейлист и настройки хранятся JSON-файлами в папке
//! данных приложения. Каждый модуль сам решает, что и когда сохранять, а пишет — здесь.

use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
};

/// Пишет `value` в JSON-файл `file`: во временный файл и переименовывает его поверх старого.
/// Если запись прервётся, старый файл останется целым, а не полузаписанным
pub fn write_atomically(file: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
    let temp = file.with_extension("json.tmp");
    fs::write(&temp, json).map_err(|e| e.to_string())?;
    fs::rename(&temp, file).map_err(|e| e.to_string())
}

/// Хеш строки для имён файлов кеша (тексты песен, картинки исполнителей) — FNV-1a.
/// Не `DefaultHasher`: тот может считать иначе в другой версии Rust, и кеш потерялся бы
pub fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Сейчас — в секундах Unix: когда спрашивали сервис, для срока жизни ответа в кеше
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Запись файла в фоновом потоке — для больших файлов (медиатека), чтобы интерфейс
/// не ждал записи. Пишет по порядку; если изменения идут чаще, чем успевает запись,
/// промежуточные состояния пропускаются — пишется последнее. При уничтожении дожидается
/// записи всего отправленного
pub struct BackgroundWriter<T> {
    jobs: Option<Sender<Job<T>>>,
    errors: Receiver<String>,
    thread: Option<JoinHandle<()>>,
}

enum Job<T> {
    Write(T),
    /// Ответить, когда всё отправленное раньше записано
    Flush(Sender<()>),
}

impl<T: Serialize + Send + 'static> BackgroundWriter<T> {
    /// `what` — что пишется, для текста ошибки: «Could not save the {what}»
    pub fn new(file: PathBuf, what: &'static str) -> Self {
        let (jobs, inbox) = mpsc::channel::<Job<T>>();
        let (report, errors) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let write = |value: &T| {
                if let Err(e) = write_atomically(&file, value) {
                    let _ = report.send(format!("Could not save the {what}: {e}"));
                }
            };
            // Ждём задачу, затем забираем всё, что успело накопиться: из записей нужна
            // только последняя, а просьбу дождаться выполняем после записи того, что до неё
            while let Ok(job) = inbox.recv() {
                let mut latest = None;
                let mut flushes = Vec::new();
                for job in std::iter::once(job).chain(inbox.try_iter()) {
                    match job {
                        Job::Write(value) => latest = Some(value),
                        Job::Flush(done) => {
                            if let Some(value) = latest.take() {
                                write(&value);
                            }
                            flushes.push(done);
                        }
                    }
                }
                if let Some(value) = latest {
                    write(&value);
                }
                for done in flushes {
                    let _ = done.send(());
                }
            }
        });
        Self {
            jobs: Some(jobs),
            errors,
            thread: Some(thread),
        }
    }

    /// Отправляет `value` на запись и сразу возвращается
    pub fn write(&self, value: T) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(Job::Write(value));
        }
    }

    /// Ждёт, пока всё отправленное запишется: перед выходом и в тестах
    pub fn flush(&self) {
        let (done, wait) = mpsc::channel();
        if let Some(jobs) = &self.jobs
            && jobs.send(Job::Flush(done)).is_ok()
        {
            let _ = wait.recv();
        }
    }

    /// Ошибка записи, если была, — для показа пользователю
    pub fn take_error(&self) -> Option<String> {
        self.errors.try_iter().last()
    }
}

impl<T> Drop for BackgroundWriter<T> {
    fn drop(&mut self) {
        // Без отправителя поток допишет оставшееся и закончится
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BackgroundWriter;
    use std::fs;

    #[test]
    fn writes_latest_value_in_background() {
        let dir = std::env::temp_dir().join(format!("lmp_writer_{}", fastrand::u64(..)));
        let file = dir.join("data.json");
        let writer = BackgroundWriter::new(file.clone(), "data");
        for value in 0..100 {
            writer.write(value);
        }
        writer.flush();
        assert_eq!(fs::read_to_string(&file).unwrap(), "99");
        assert!(writer.take_error().is_none());

        // При уничтожении отправленное дописывается
        writer.write(7);
        drop(writer);
        assert_eq!(fs::read_to_string(&file).unwrap(), "7");
        fs::remove_dir_all(dir).unwrap();
    }
}
