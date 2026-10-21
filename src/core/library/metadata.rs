//! Теги аудиофайлов: название, исполнитель, альбом, длительность, обложка.

use lofty::{
    config::ParseOptions,
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::{Accessor, ItemKey},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub title: String,
    pub artist: String,
    /// Исполнитель альбома (Album Artist) — по нему треки собираются в альбомы. Пусто,
    /// если в тегах его нет
    pub album_artist: String,
    pub album: String,
    pub duration_secs: u64,
    pub year: Option<u16>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
}

pub fn get_file_metadata(path: &Path) -> Result<FileMetadata, String> {
    // Обложку здесь не читаем: метаданные читаются для всего списка, а обложки нужны только
    // видимым на экране — их по требованию читает кэш обложек (см. read_cover)
    let tagged = Probe::open(path)
        .map_err(|e| format!("Could not open the file: {e}"))?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .map_err(|e| format!("Could not read the metadata: {e}"))?;

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let title = tag
        .and_then(|t| t.title().map(|s| fix_cp1251(&s)))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned()
        });

    let artist = tag
        .and_then(|t| t.artist().map(|s| fix_cp1251(&s)))
        .unwrap_or_default();

    let album_artist = tag
        .and_then(|t| t.get_string(ItemKey::AlbumArtist).map(fix_cp1251))
        .unwrap_or_default();

    let album = tag
        .and_then(|t| t.album().map(|s| fix_cp1251(&s)))
        .unwrap_or_default();

    let duration_secs = tagged.properties().duration().as_secs();
    // Год — из даты записи или отдельного поля года, смотря что есть в тегах
    let year = tag.and_then(|t| t.date()).map(|date| date.year);
    // Номер трека и диска — для порядка треков в альбоме
    let track_number = tag.and_then(|t| t.track());
    let disc_number = tag.and_then(|t| t.disk());

    Ok(FileMetadata {
        title,
        artist,
        album_artist,
        album,
        duration_secs,
        year,
        track_number,
        disc_number,
    })
}

/// Обложка альбома из тегов файла. Читается по требованию — кэшем обложек интерфейса.
pub fn read_cover(path: &Path) -> Option<Vec<u8>> {
    let tagged = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    tag.pictures()
        .first()
        .map(|picture| picture.data().to_vec())
}

/// Обложка альбома картинкой рядом с аудиофайлами в папке `dir` — так её часто хранят
/// вместо тегов. Берётся картинка с говорящим именем (`cover.jpg`, `Folder.png`, `front.jpg`…),
/// а если таких нет — единственная картинка в папке. Из нескольких картинок с другими
/// именами (сканы буклета, задник диска) не угадываем: показать не ту хуже, чем никакую
pub fn read_folder_cover(dir: &Path) -> Option<Vec<u8>> {
    const IMAGE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];
    const COVER_NAMES: [&str; 5] = ["cover", "folder", "front", "albumart", "album"];

    let mut images: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        })
        .collect();
    images.sort();

    let named = |name: &str| {
        images.iter().find(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.to_lowercase() == name)
        })
    };
    let cover = COVER_NAMES
        .iter()
        .find_map(|name| named(name))
        .or(match images.as_slice() {
            [single] => Some(single),
            _ => None,
        })?;
    fs::read(cover).ok()
}

/// Чинит теги в Windows-1251, прочитанные как Latin-1.
///
/// Старые MP3 часто хранят русские теги в кодировке Windows-1251, но в ID3 указана Latin-1.
/// Тогда каждый байт кириллицы читается как латинская буква с диакритикой:
/// «Полет шмеля» превращается в «Ïîëåò øìåëÿ». Такую строку собираем обратно в байты
/// и декодируем как Windows-1251.
///
/// Чтобы не испортить настоящий латинский текст с диакритикой («Beyoncé», «Mötley Crüe»),
/// перекодируем, только если большинство букв — из диапазона U+00C0..=U+00FF, где
/// в Windows-1251 лежит кириллица.
fn fix_cp1251(text: &str) -> String {
    // Строка из Latin-1 состоит только из символов до U+00FF — иначе это уже не наш случай
    if text.chars().any(|c| c as u32 > 0xFF) {
        return text.to_owned();
    }

    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    let cyrillic_in_disguise = text
        .chars()
        .filter(|&c| ('\u{C0}'..='\u{FF}').contains(&c))
        .count();
    if cyrillic_in_disguise < 2 || cyrillic_in_disguise * 2 < letters {
        return text.to_owned();
    }

    let bytes: Vec<u8> = text.chars().map(|c| c as u8).collect();
    let (decoded, _) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(&bytes);
    decoded.into_owned()
}

#[cfg(test)]
mod tests {
    use super::{fix_cp1251, read_folder_cover};
    use std::fs;

    /// Новая пустая временная папка с файлами `names`; содержимое файла — его имя
    fn folder(names: &[&str]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lmp_cover_{}", fastrand::u64(..)));
        fs::create_dir_all(&dir).unwrap();
        for name in names {
            fs::write(dir.join(name), name).unwrap();
        }
        dir
    }

    #[test]
    fn folder_cover_prefers_cover_names_then_single_image() {
        let read = |names: &[&str]| {
            let dir = folder(names);
            let cover = read_folder_cover(&dir).map(|bytes| String::from_utf8(bytes).unwrap());
            fs::remove_dir_all(dir).unwrap();
            cover
        };
        // Единственная картинка — обложка, как бы она ни называлась
        assert_eq!(
            read(&["01 Song.mp3", "Dot Hacker - N°3.jpg"]).as_deref(),
            Some("Dot Hacker - N°3.jpg")
        );
        // Говорящее имя важнее остальных картинок, регистр не важен
        assert_eq!(
            read(&["back.jpg", "Folder.PNG", "cd.jpg"]).as_deref(),
            Some("Folder.PNG")
        );
        // Несколько картинок без говорящих имён — не угадываем
        assert_eq!(read(&["back.jpg", "booklet.jpg"]), None);
        assert_eq!(read(&["01 Song.mp3"]), None);
    }

    #[test]
    fn decodes_cp1251_read_as_latin1() {
        assert_eq!(
            fix_cp1251("\"Ïîëåò øìåëÿ\" èç îïåðû \"Ñêàçêà î öàðå Ñàëòàíå\""),
            "\"Полет шмеля\" из оперы \"Сказка о царе Салтане\""
        );
    }

    #[test]
    fn keeps_latin_with_diacritics() {
        assert_eq!(fix_cp1251("Beyoncé"), "Beyoncé");
        assert_eq!(fix_cp1251("Mötley Crüe"), "Mötley Crüe");
    }

    #[test]
    fn keeps_correct_text() {
        assert_eq!(fix_cp1251("Полет шмеля"), "Полет шмеля");
        assert_eq!(fix_cp1251("Shakira, Burna Boy"), "Shakira, Burna Boy");
    }
}
