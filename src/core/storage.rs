//! Запись файлов данных: медиатека, плейлист и настройки хранятся JSON-файлами в папке
//! данных приложения. Каждый модуль сам решает, что и когда сохранять, а пишет — здесь.

use serde::Serialize;
use std::{fs, path::Path};

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
