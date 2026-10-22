//! Настройки приложения. Хранятся между запусками в `settings.json` в папке данных —
//! рядом с библиотекой и плейлистом, но отдельно от них: смена настройки не переписывает
//! библиотеку, а формат библиотеки не зависит от настроек.

use serde::{Deserialize, Serialize};
use std::{fs, io::ErrorKind, path::PathBuf};

use crate::{core::playback::Volume, core::storage::write_atomically};

const FILE_NAME: &str = "settings.json";

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Language {
    #[default]
    English,
    Russian,
    French,
    Ukrainian,
}

impl Language {
    /// Все языки — в порядке списка в настройках
    pub const ALL: [Language; 4] = [
        Language::English,
        Language::French,
        Language::Russian,
        Language::Ukrainian,
    ];
}

/// Оттенок стекла — тонировки поверх системного размытия под окном. Сами цвета — в теме
/// интерфейса
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlassTint {
    #[default]
    Graphite,
    Blue,
    Violet,
    Green,
    Rose,
    Amber,
}

impl GlassTint {
    /// Все оттенки — в порядке кружков в настройках
    pub const ALL: [GlassTint; 6] = [
        GlassTint::Graphite,
        GlassTint::Blue,
        GlassTint::Violet,
        GlassTint::Green,
        GlassTint::Rose,
        GlassTint::Amber,
    ];
}

/// Стекло окна: оттенок и прозрачность
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct Glass {
    pub tint: GlassTint,
    /// 0.0 — тонировка непрозрачная, за окном ничего не видно; 1.0 — самое прозрачное
    /// стекло, какое допускает интерфейс
    pub transparency: f32,
}

impl Default for Glass {
    fn default() -> Self {
        Self {
            tint: GlassTint::default(),
            transparency: 0.4,
        }
    }
}

/// Все настройки. Отсутствующее в файле поле получает значение по умолчанию
/// (`serde(default)`): файл от прошлой версии приложения, где этой настройки ещё не было,
/// читается без ошибок — отдельная версия формата не нужна
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Settings {
    language: Language,
    volume: Volume,
    glass: Glass,
    /// Искать в интернете то, чего нет в файлах: фото исполнителей (Deezer) и тексты
    /// песен (LRCLIB). Выключено, пока пользователь не включит: иначе приложение без спроса
    /// отправляло бы сервисам имена исполнителей и названия треков
    online_lookups: bool,
}

impl Settings {
    pub fn language(&self) -> Language {
        self.language
    }

    pub fn update_language(&mut self, language: Language) {
        self.language = language;
    }

    pub fn volume(&self) -> Volume {
        self.volume
    }

    pub fn update_volume(&mut self, volume: Volume) {
        self.volume = volume;
    }

    pub fn glass(&self) -> Glass {
        self.glass
    }

    pub fn online_lookups(&self) -> bool {
        self.online_lookups
    }

    pub fn update_online_lookups(&mut self, online: bool) {
        self.online_lookups = online;
    }

    pub fn update_glass(&mut self, glass: Glass) {
        self.glass = Glass {
            transparency: glass.transparency.clamp(0.0, 1.0),
            ..glass
        };
    }
}

/// Файл настроек. Помнит, что в нём лежит, — неизменившиеся настройки не перезаписывает
pub struct SettingsStore {
    /// Куда сохранять; `None` — папка данных недоступна, настройки живут только до выхода
    file: Option<PathBuf>,
    saved: Settings,
}

impl SettingsStore {
    /// Загружает настройки из папки данных `dir`. Файла нет — настройки по умолчанию.
    /// Третьим значением — ошибка для показа пользователю, если файл не прочитался
    pub fn load(dir: Option<PathBuf>) -> (Self, Settings, Option<String>) {
        let file = dir.map(|dir| dir.join(FILE_NAME));
        let (settings, error) = match file.as_ref().map(fs::read_to_string) {
            None => (Settings::default(), None),
            Some(Err(e)) if e.kind() == ErrorKind::NotFound => (Settings::default(), None),
            Some(Err(e)) => (
                Settings::default(),
                Some(format!("Could not read the settings: {e}")),
            ),
            Some(Ok(text)) => match serde_json::from_str(&text) {
                Ok(settings) => (settings, None),
                Err(e) => (
                    Settings::default(),
                    Some(format!("The settings file is damaged, using defaults: {e}")),
                ),
            },
        };
        let store = Self {
            file,
            saved: settings.clone(),
        };
        (store, settings, error)
    }

    /// Сохраняет настройки, если они изменились с прошлого сохранения
    pub fn save_if_changed(&mut self, settings: &Settings) -> Result<(), String> {
        if *settings == self.saved {
            return Ok(());
        }
        // Сохранёнными считаем и при ошибке: иначе запись повторялась бы каждый кадр.
        // Следующее изменение попробует снова
        self.saved = settings.clone();
        let Some(file) = &self.file else {
            return Ok(());
        };
        write_atomically(file, settings).map_err(|e| format!("Could not save the settings: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Glass, GlassTint, Language, Settings, SettingsStore};
    use crate::core::playback::Volume;
    use std::fs;

    #[test]
    fn saves_loads_and_fills_missing_fields() {
        let dir = std::env::temp_dir().join(format!("lmp_settings_{}", fastrand::u64(..)));

        let (mut store, mut settings, error) = SettingsStore::load(Some(dir.clone()));
        assert!(error.is_none());
        assert_eq!(settings, Settings::default());

        settings.update_language(Language::Russian);
        settings.update_volume(Volume {
            level: 0.3,
            muted: true,
        });
        settings.update_glass(Glass {
            tint: GlassTint::Violet,
            transparency: 0.7,
        });
        store.save_if_changed(&settings).unwrap();
        let (_, loaded, error) = SettingsStore::load(Some(dir.clone()));
        assert!(error.is_none());
        assert_eq!(loaded, settings);

        // Файл старой версии без громкости: язык читается, громкость — по умолчанию
        fs::write(dir.join(super::FILE_NAME), r#"{"language":"Russian"}"#).unwrap();
        let (_, loaded, error) = SettingsStore::load(Some(dir.clone()));
        assert!(error.is_none());
        assert_eq!(loaded.language(), Language::Russian);
        assert_eq!(loaded.volume(), Volume::default());
        assert_eq!(loaded.glass(), Glass::default());
        // Поиск в интернете — только если пользователь включил
        assert!(!loaded.online_lookups());

        fs::remove_dir_all(dir).unwrap();
    }
}
