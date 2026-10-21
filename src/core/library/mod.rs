//! Файлы на диске: описание трека, теги, поиск аудиофайлов, фоновое добавление,
//! библиотека — сохранённый между запусками список треков.
//! Ничего не знает ни о воспроизведении, ни об интерфейсе.

pub mod groups;
pub mod loader;
pub mod metadata;
pub mod scan;
mod store;
pub mod track;

pub use store::Library;
pub use track::Track;
