//! Логика плеера без интерфейса: медиатека, воспроизведение, настройки и их сохранение
//! на диск. Ничего не знает об egui — интерфейс (`crate::ui`) только читает отсюда данные
//! и вызывает методы.

pub mod library;
pub mod playback;
pub mod settings;
pub mod state;
pub mod storage;
