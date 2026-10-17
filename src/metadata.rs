use lofty::{
    config::ParseOptions,
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::Accessor,
};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};
use symphonia::{
    core::{
        codecs::{CodecParameters, audio::AudioDecoderOptions},
        formats::probe::Hint,
        io::MediaSourceStream,
    },
    default::{get_codecs, get_probe},
};

/// Расширения поддерживаемых аудиофайлов (в нижнем регистре).
pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac"];

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: u64,
}

pub fn get_file_metadata(path: &Path) -> Result<FileMetadata, String> {
    // Обложку здесь не читаем: метаданные читаются для всего списка, а обложка нужна
    // только текущему треку (см. read_cover). Держать тысячи обложек в памяти дорого
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

    let album = tag
        .and_then(|t| t.album().map(|s| fix_cp1251(&s)))
        .unwrap_or_default();

    let duration_secs = tagged.properties().duration().as_secs();

    Ok(FileMetadata {
        title,
        artist,
        album,
        duration_secs,
    })
}

/// Обложка альбома из тегов файла. Читается по требованию — для текущего трека.
pub fn read_cover(path: &Path) -> Option<Vec<u8>> {
    let tagged = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    tag.pictures()
        .first()
        .map(|picture| picture.data().to_vec())
}

/// Все аудиофайлы поддерживаемых форматов в папке, включая вложенные, по алфавиту пути
/// без учёта регистра.
/// Остальные файлы (обложки, плейлисты и т. п.) пропускаются; вложенные папки,
/// которые не удалось прочитать, — тоже, чтобы одна такая папка не срывала всё добавление.
pub fn find_audio_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                let _ = walk(&path, found);
            } else if is_supported_extension(&path) {
                found.push(path);
            }
        }
        Ok(())
    }

    let mut found = Vec::new();
    walk(dir, &mut found).map_err(|e| format!("Could not read the folder: {e}"))?;
    // Без учёта регистра, как в Finder и Проводнике
    found.sort_by_key(|path| path.to_string_lossy().to_lowercase());
    Ok(found)
}

fn is_supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
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

/// Проверяет, что файл можно воспроизвести: известное расширение, контейнер и кодек.
pub fn check_supported(path: &Path) -> Result<(), String> {
    // Уровень 0: расширение
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| SUPPORTED_EXTENSIONS.contains(&e.as_str()))
        .ok_or_else(|| "Unsupported file format".to_string())?;

    let file = File::open(path).map_err(|e| format!("Could not open the file: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    hint.with_extension(&ext);

    // Уровень 1: контейнер
    let format = get_probe()
        .probe(&hint, mss, Default::default(), Default::default())
        .map_err(|_| "Unsupported or damaged file".to_string())?;

    // Берём первую аудиодорожку
    let params = format
        .tracks()
        .iter()
        .find_map(|t| match &t.codec_params {
            Some(CodecParameters::Audio(p)) => Some(p),
            _ => None,
        })
        .ok_or_else(|| "The file has no audio track".to_string())?;

    // Уровень 2: кодек
    get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|_| "This audio codec is not supported".to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{find_audio_files, fix_cp1251};
    use std::fs;

    #[test]
    fn finds_audio_files_recursively() {
        let dir = std::env::temp_dir().join(format!("rusty_player_test_{}", std::process::id()));
        fs::create_dir_all(dir.join("CD2")).unwrap();
        for name in ["b.mp3", "a.FLAC", "cover.jpg", "CD2/c.ogg", "CD2/notes.txt"] {
            fs::write(dir.join(name), b"").unwrap();
        }

        let found = find_audio_files(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        let names: Vec<_> = found
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.FLAC", "b.mp3", "CD2/c.ogg"]);
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
