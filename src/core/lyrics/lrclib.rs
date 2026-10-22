//! Запрос текста песни в LRCLIB (https://lrclib.net) — открытой базе текстов, без ключей.
//! Трек ищется по сигнатуре: исполнитель, название, альбом и длительность (сервис допускает
//! расхождение длительности в пару секунд).

use serde::{Deserialize, Serialize};

use crate::core::net;

const GET_URL: &str = "https://lrclib.net/api/get";
const SEARCH_URL: &str = "https://lrclib.net/api/search";

/// По чему ищется трек
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrackSignature {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub duration_secs: u64,
}

/// Ответ LRCLIB, как он хранится в кеше
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Remote {
    Found {
        /// Синхронизированный текст в формате LRC
        synced: Option<String>,
        plain: Option<String>,
        /// Трек без слов
        instrumental: bool,
    },
    NotFound,
}

/// Запись LRCLIB — только нужные поля. Так отвечают и `/api/get`, и `/api/search` (списком)
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Track {
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    artist_name: String,
    /// Секунды
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

/// Наибольшее расхождение длительности у записи из поиска — столько же допускает `/api/get`
const DURATION_TOLERANCE_SECS: f64 = 2.0;

/// Спрашивает LRCLIB о треке: сначала точной сигнатурой (`/api/get`), а не нашлось —
/// поиском по исполнителю и названию: альбом в тегах часто назван иначе, чем в LRCLIB
/// («… (Limited Edition)»). Ошибка — сеть, сервер занят (503) и прочее временное:
/// такой ответ не запоминается, его можно спросить снова
pub fn fetch(signature: &TrackSignature) -> Result<Remote, String> {
    let agent = net::agent();

    let response = agent
        .get(GET_URL)
        .query("artist_name", &signature.artist)
        .query("track_name", &signature.title)
        .query("album_name", &signature.album)
        .query("duration", signature.duration_secs.to_string())
        .call();
    let track: Option<Track> = read(response)?;
    if let Some(track) = track {
        return Ok(found(track));
    }

    let response = agent
        .get(SEARCH_URL)
        .query("artist_name", &signature.artist)
        .query("track_name", &signature.title)
        .call();
    let candidates: Vec<Track> = read(response)?.unwrap_or_default();
    Ok(best_match(candidates, signature).map_or(Remote::NotFound, found))
}

/// Тело ответа: `None` — 404, ошибка — всё, кроме 200 и 404
fn read<T: serde::de::DeserializeOwned>(
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<Option<T>, String> {
    let mut response = response.map_err(|e| format!("Could not reach LRCLIB: {e}"))?;
    match response.status().as_u16() {
        200 => response
            .body_mut()
            .read_json()
            .map(Some)
            .map_err(|e| format!("LRCLIB sent an unexpected answer: {e}")),
        404 => Ok(None),
        status => Err(format!("LRCLIB answered with HTTP {status}")),
    }
}

fn found(track: Track) -> Remote {
    let non_empty = |text: Option<String>| text.filter(|text| !text.trim().is_empty());
    Remote::Found {
        synced: non_empty(track.synced_lyrics),
        plain: non_empty(track.plain_lyrics),
        instrumental: track.instrumental,
    }
}

/// Запись из поиска, которая точно про этот трек: то же название и исполнитель (без учёта
/// регистра), длительность — в пределах `DURATION_TOLERANCE_SECS`. Поиск нечёткий,
/// и без этих проверок можно показать текст другой песни. Из подходящих — с синхронизированным
/// текстом, затем с ближайшей длительностью
fn best_match(candidates: Vec<Track>, signature: &TrackSignature) -> Option<Track> {
    let same = |a: &str, b: &str| a.trim().to_lowercase() == b.trim().to_lowercase();
    let duration = signature.duration_secs as f64;
    candidates
        .into_iter()
        .filter(|track| {
            same(&track.track_name, &signature.title)
                && same(&track.artist_name, &signature.artist)
                && (track.duration - duration).abs() <= DURATION_TOLERANCE_SECS
                && (track.instrumental
                    || track.synced_lyrics.is_some()
                    || track.plain_lyrics.is_some())
        })
        .min_by(|a, b| {
            let rank = |track: &Track| {
                (
                    track.synced_lyrics.is_none(),
                    ((track.duration - duration).abs() * 1000.0) as u64,
                )
            };
            rank(a).cmp(&rank(b))
        })
}

#[cfg(test)]
mod tests {
    use super::{Track, TrackSignature, best_match};

    fn track(name: &str, artist: &str, duration: f64, synced: bool) -> Track {
        Track {
            track_name: name.into(),
            artist_name: artist.into(),
            duration,
            instrumental: false,
            plain_lyrics: Some("Line".into()),
            synced_lyrics: synced.then(|| "[00:01.00]Line".into()),
        }
    }

    #[test]
    fn picks_matching_search_result() {
        let signature = TrackSignature {
            artist: "HIM".into(),
            title: "Join Me In Death".into(),
            album: "Razorblade Romance (Limited Edition)".into(),
            duration_secs: 217,
        };
        let candidates = vec![
            // Другая песня, другой исполнитель, далёкая длительность — не подходят
            track("Join Me", "HIM", 217.0, true),
            track("Join Me In Death", "HIM - Topic", 217.0, true),
            track("Join Me In Death", "HIM", 222.0, true),
            // Подходят: без синхронизированного текста и с ним чуть дальше по длительности
            track("Join Me In Death", "HIM", 217.0, false),
            track("Join Me in Death", "Him", 218.0, true),
        ];
        let best = best_match(candidates, &signature).unwrap();
        assert_eq!(best.artist_name, "Him");
        assert!(best.synced_lyrics.is_some());

        assert!(best_match(vec![track("Other", "HIM", 217.0, true)], &signature).is_none());
    }
}
