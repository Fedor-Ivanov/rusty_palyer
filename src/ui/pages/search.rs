//! Поиск на страницах медиатеки: запрос и найденные песни.

use crate::{core::library::Track, ui::app::PlayerApp};

/// Поиск в виде библиотеки. Запрос один на открытый вид: при смене вида он очищается
/// (см. `PlayerApp::navigate`). Artists ищет по имени исполнителя, Albums — по названию
/// альбома, Songs — по названию трека и имени исполнителя
#[derive(Default)]
pub(crate) struct LibrarySearch {
    pub(crate) query: String,
    /// Найденные треки Songs; `None` — запрос пуст, показана вся библиотека
    pub(crate) found_songs: Option<FoundSongs>,
}

impl LibrarySearch {
    pub(crate) fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
}

/// Треки, найденные в Songs. Пересчитываются, только когда изменился запрос или библиотека:
/// в библиотеке могут быть десятки тысяч треков, а кадры рисуются постоянно
pub(crate) struct FoundSongs {
    pub(crate) revision: u64,
    /// Запрос в нижнем регистре, по которому искали
    pub(crate) query: String,
    /// Номера найденных треков в библиотеке
    pub(crate) indices: Vec<usize>,
    /// Копии найденных треков — их показывает список
    pub(crate) tracks: Vec<Track>,
}

/// Запрос поиска для сравнения: без пробелов по краям, в нижнем регистре
pub(crate) fn normalized_query(query: &str) -> String {
    query.trim().to_lowercase()
}

/// Есть ли `query` (уже из `normalized_query`) в `text` без учёта регистра.
/// Пустой запрос подходит ко всему
pub(crate) fn contains_query(text: &str, query: &str) -> bool {
    query.is_empty() || text.to_lowercase().contains(query)
}

impl PlayerApp {
    /// Пересчитывает найденные в Songs треки, если изменился запрос или библиотека
    pub(crate) fn refresh_found_songs(&mut self) {
        let query = normalized_query(&self.search.query);
        if query.is_empty() {
            self.search.found_songs = None;
            return;
        }
        let revision = self.state.library.revision();
        let up_to_date = self
            .search
            .found_songs
            .as_ref()
            .is_some_and(|found| found.revision == revision && found.query == query);
        if up_to_date {
            return;
        }
        let (indices, tracks) = self
            .state
            .library
            .tracks()
            .iter()
            .enumerate()
            .filter(|(_, track)| {
                contains_query(&track.title, &query) || contains_query(&track.artist, &query)
            })
            .map(|(index, track)| (index, track.clone()))
            .unzip();
        self.search.found_songs = Some(FoundSongs {
            revision,
            query,
            indices,
            tracks,
        });
    }
}
