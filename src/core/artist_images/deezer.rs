//! Фото исполнителя из Deezer: поиск исполнителя по имени — открытый API, без ключа.
//! Отсюда — только ссылка на фото: хранить сами фото Deezer нельзя.

use serde::Deserialize;

use crate::core::net;

const SEARCH_URL: &str = "https://api.deezer.com/search/artist";
/// Столько исполнителей просим у поиска: однофамильцев бывает несколько
const SEARCH_LIMIT: &str = "10";

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    data: Vec<Artist>,
    /// Deezer сообщает об ошибках (например, превышен лимит запросов) с кодом 200
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Artist {
    name: String,
    /// Число поклонников — по нему из однофамильцев выбирается известный
    #[serde(default)]
    nb_fan: u64,
    /// 500×500
    picture_big: Option<String>,
}

/// Ссылка на фото исполнителя `name`: `Ok(None)` — в Deezer такого исполнителя нет или
/// у него нет фото. Само фото не скачивается: его загружают по ссылке, когда оно нужно
/// на экране. Ошибка — сеть или сервис: можно спросить снова
pub fn find_picture(name: &str) -> Result<Option<String>, String> {
    let mut response = net::agent()
        .get(SEARCH_URL)
        .query("q", name)
        .query("limit", SEARCH_LIMIT)
        .call()
        .map_err(|e| format!("Could not reach Deezer: {e}"))?;
    if response.status() != 200 {
        return Err(format!("Deezer answered with HTTP {}", response.status()));
    }
    let search: Search = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("Deezer sent an unexpected answer: {e}"))?;
    if let Some(error) = search.error {
        return Err(format!("Deezer answered with an error: {error}"));
    }
    Ok(best_picture(search.data, name))
}

/// Ссылка на фото исполнителя с тем же именем (без учёта регистра); из однофамильцев —
/// самого известного. Похожие имена не годятся: лучше заглушка, чем чужое лицо
fn best_picture(artists: Vec<Artist>, name: &str) -> Option<String> {
    let name = name.trim().to_lowercase();
    artists
        .into_iter()
        .filter(|artist| artist.name.trim().to_lowercase() == name)
        .filter_map(|artist| {
            let url = artist.picture_big?;
            // Без фото Deezer отдаёт ссылку на заглушку с пустым идентификатором картинки
            (!url.contains("/artist//")).then_some((artist.nb_fan, url))
        })
        .max_by_key(|(fans, _)| *fans)
        .map(|(_, url)| url)
}

#[cfg(test)]
mod tests {
    use super::{Artist, best_picture};

    fn artist(name: &str, fans: u64, picture: &str) -> Artist {
        Artist {
            name: name.into(),
            nb_fan: fans,
            picture_big: Some(picture.into()),
        }
    }

    #[test]
    fn picks_most_known_namesake() {
        let artists = vec![
            artist("HIM", 68, "https://img/artist/a/500.jpg"),
            artist("HIM", 210_403, "https://img/artist/b/500.jpg"),
            artist("H.I.M.", 1_000_000, "https://img/artist/c/500.jpg"),
            // Самый известный, но без фото
            artist("him", 5_000_000, "https://img/artist//500.jpg"),
        ];
        assert_eq!(
            best_picture(artists, "HIM").as_deref(),
            Some("https://img/artist/b/500.jpg")
        );
        let ghost = vec![artist("Ghost", 614_630, "https://img/artist/g/500.jpg")];
        assert_eq!(best_picture(ghost, "Ghost B.C."), None);
    }
}
