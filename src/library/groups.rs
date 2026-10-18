//! Группировка треков библиотеки по исполнителям и альбомам — для видов «Artists» и «Albums».

use std::{collections::HashMap, path::PathBuf};

use super::Track;

pub const UNKNOWN_ARTIST: &str = "Unknown Artist";
pub const UNKNOWN_ALBUM: &str = "Unknown Album";

pub struct ArtistGroup {
    pub name: String,
    /// Номера треков в библиотеке
    pub tracks: Vec<usize>,
}

pub struct AlbumGroup {
    pub title: String,
    pub artist: String,
    /// Самый ранний год среди треков альбома
    pub year: Option<u16>,
    /// Файл, из тегов которого берётся обложка, — первый трек альбома
    pub cover_from: PathBuf,
    /// Номера треков в библиотеке — в порядке альбома: по номеру диска и трека
    pub tracks: Vec<usize>,
}

/// Исполнители и альбомы библиотеки, по алфавиту без учёта регистра.
/// Альбом — это пара «название + исполнитель»: одноимённые альбомы разных исполнителей
/// не сливаются в один
pub fn group(tracks: &[Track]) -> (Vec<ArtistGroup>, Vec<AlbumGroup>) {
    let name_or = |text: &str, unknown: &str| {
        let text = text.trim();
        if text.is_empty() { unknown } else { text }.to_owned()
    };

    let mut artists: HashMap<String, ArtistGroup> = HashMap::new();
    let mut albums: HashMap<(String, String), AlbumGroup> = HashMap::new();
    for (index, track) in tracks.iter().enumerate() {
        let artist = name_or(&track.artist, UNKNOWN_ARTIST);
        let album = name_or(&track.album, UNKNOWN_ALBUM);

        artists
            .entry(artist.to_lowercase())
            .or_insert_with(|| ArtistGroup {
                name: artist.clone(),
                tracks: Vec::new(),
            })
            .tracks
            .push(index);

        let group = albums
            .entry((album.to_lowercase(), artist.to_lowercase()))
            .or_insert_with(|| AlbumGroup {
                title: album,
                artist,
                year: None,
                cover_from: track.path.clone(),
                tracks: Vec::new(),
            });
        group.tracks.push(index);
        group.year = match (group.year, track.year) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }

    let mut artists: Vec<ArtistGroup> = artists.into_values().collect();
    artists.sort_by_cached_key(|a| a.name.to_lowercase());
    let mut albums: Vec<AlbumGroup> = albums.into_values().collect();
    // Треки альбома — по номеру диска, затем по номеру трека. Без номера — в конце диска,
    // в порядке библиотеки; без номера диска считаем первым диском
    for album in &mut albums {
        album.tracks.sort_by_key(|&index| {
            let track = &tracks[index];
            (
                track.disc_number.unwrap_or(1),
                track.track_number.unwrap_or(u32::MAX),
                index,
            )
        });
    }
    albums.sort_by_cached_key(|a| (a.title.to_lowercase(), a.artist.to_lowercase()));
    (artists, albums)
}

#[cfg(test)]
mod tests {
    use super::{UNKNOWN_ALBUM, UNKNOWN_ARTIST, group};
    use crate::library::Track;
    use std::path::PathBuf;

    fn numbered(name: &str, disc: Option<u32>, number: Option<u32>) -> Track {
        Track {
            disc_number: disc,
            track_number: number,
            ..track(name, "Band", "Album", None)
        }
    }

    #[test]
    fn album_tracks_follow_disc_and_track_numbers() {
        let tracks = [
            numbered("d2-1", Some(2), Some(1)),
            numbered("no number", None, None),
            numbered("d1-2", Some(1), Some(2)),
            numbered("d1-1", None, Some(1)),
        ];
        let (_, albums) = group(&tracks);
        let order: Vec<_> = albums[0]
            .tracks
            .iter()
            .map(|&i| tracks[i].title.as_str())
            .collect();
        // Без номера диска — первый диск; без номера трека — в конце своего диска
        assert_eq!(order, ["d1-1", "d1-2", "no number", "d2-1"]);
    }

    fn track(name: &str, artist: &str, album: &str, year: Option<u16>) -> Track {
        Track {
            path: PathBuf::from(format!("{name}.mp3")),
            title: name.into(),
            artist: artist.into(),
            duration_secs: 0,
            album: album.into(),
            year,
            track_number: None,
            disc_number: None,
        }
    }

    #[test]
    fn groups_by_artist_and_album() {
        let tracks = [
            track("1", "Muse", "Origin of Symmetry", Some(2001)),
            track("2", "muse", "Origin of Symmetry", Some(2002)),
            track("3", "Adele", "21", None),
            track("4", "Adele", "Greatest Hits", Some(2015)),
            track("5", "Björk", "Greatest Hits", Some(2002)),
            track("6", "", "", None),
        ];
        let (artists, albums) = group(&tracks);

        let names: Vec<_> = artists.iter().map(|a| a.name.as_str()).collect();
        // Регистр не различается, имя — как у первого встреченного трека
        assert_eq!(names, ["Adele", "Björk", "Muse", UNKNOWN_ARTIST]);
        assert_eq!(artists[2].tracks, [0, 1]);

        let albums: Vec<_> = albums
            .iter()
            .map(|a| (a.title.as_str(), a.artist.as_str(), a.year, a.tracks.len()))
            .collect();
        assert_eq!(
            albums,
            [
                ("21", "Adele", None, 1),
                // Одноимённые альбомы разных исполнителей — разные альбомы
                ("Greatest Hits", "Adele", Some(2015), 1),
                ("Greatest Hits", "Björk", Some(2002), 1),
                // Год альбома — самый ранний из его треков
                ("Origin of Symmetry", "Muse", Some(2001), 2),
                (UNKNOWN_ALBUM, UNKNOWN_ARTIST, None, 1),
            ]
        );
    }
}
