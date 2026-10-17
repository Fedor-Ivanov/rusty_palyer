use lofty::{
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::Accessor,
};
use std::{fs::File, path::Path};
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
    pub album_cover: Option<Vec<u8>>,
}

pub fn get_file_metadata(path: &Path) -> Result<FileMetadata, String> {
    let tagged = Probe::open(path)
        .map_err(|e| format!("Не удалось открыть файл: {e}"))?
        .read()
        .map_err(|e| format!("Не удалось прочитать метаданные: {e}"))?;

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

    let album_cover = tag.and_then(|t| t.pictures().first().map(|p| p.data().to_vec()));

    let duration_secs = tagged.properties().duration().as_secs();

    Ok(FileMetadata {
        title,
        artist,
        album,
        duration_secs,
        album_cover,
    })
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
        .ok_or_else(|| "Неподдерживаемый формат файла".to_string())?;

    let file = File::open(path).map_err(|e| format!("Не удалось открыть файл: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    hint.with_extension(&ext);

    // Уровень 1: контейнер
    let format = get_probe()
        .probe(&hint, mss, Default::default(), Default::default())
        .map_err(|_| "Неподдерживаемый или повреждённый файл".to_string())?;

    // Берём первую аудиодорожку
    let params = format
        .tracks()
        .iter()
        .find_map(|t| match &t.codec_params {
            Some(CodecParameters::Audio(p)) => Some(p),
            _ => None,
        })
        .ok_or_else(|| "В файле нет аудиодорожки".to_string())?;

    // Уровень 2: кодек
    get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|_| "Этот аудиокодек не поддерживается".to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fix_cp1251;

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
