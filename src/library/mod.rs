//! Файлы на диске: описание трека, теги, поиск аудиофайлов, фоновое добавление.
//! Ничего не знает ни о воспроизведении, ни об интерфейсе.

pub mod loader;
pub mod metadata;
pub mod scan;
mod track;

pub use track::Track;
