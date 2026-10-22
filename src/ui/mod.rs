//! Интерфейс на egui.
//!
//! - `app` — `PlayerApp`: состояние окна и раскладка панелей;
//! - `action` — действия пользователя с данными и единственное место, где они выполняются;
//! - `navigation` — что открыто в центре (`Route`);
//! - `panels` — всегда на экране: медиатека, плейлист, плеер;
//! - `pages` — страницы центральной области;
//! - `dialogs` — модальные окна;
//! - `components` — переиспользуемые элементы, ничего не знают о плеере;
//! - `style` — цвета, иконки, стекло окна; `i18n` — переводы;
//! - `loading` — добавление файлов; `covers` — кэш обложек.

mod action;
mod app;
mod components;
mod covers;
mod dialogs;
mod file_drop;
mod i18n;
mod loading;
mod navigation;
mod pages;
mod panels;
mod shortcuts;
pub mod style;
mod system_media;

pub use app::PlayerApp;
