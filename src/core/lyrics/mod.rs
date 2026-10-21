//! Тексты песен: из файла `.lrc` рядом с треком, из тегов или из LRCLIB (см. `finder`).
//! Синхронизированный текст (формат LRC: строки с метками времени `[01:23.45]`) знает,
//! какая строка поётся сейчас; обычный — просто строки.

pub mod finder;
pub mod lrclib;

pub use finder::{LyricsFinder, TrackLyrics};

use lofty::{file::TaggedFileExt, probe::Probe, tag::ItemKey};
use std::{fs, path::Path};

use crate::core::library::metadata::fix_cp1251;

#[derive(Debug, Clone, PartialEq)]
pub struct Lyrics {
    /// По времени, если текст синхронизирован; иначе — по порядку в тексте
    pub lines: Vec<LyricLine>,
    /// У строк есть время: можно подсвечивать текущую
    pub synced: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    /// Когда строка начинается, в миллисекундах; `None` у несинхронизированного текста
    pub start_ms: Option<u64>,
    pub text: String,
}

impl Lyrics {
    /// Строка, которая поётся на `position_ms`: последняя из начавшихся. `None` — текст
    /// не синхронизирован или первая строка ещё не началась
    pub fn current_line(&self, position_ms: u64) -> Option<usize> {
        if !self.synced {
            return None;
        }
        // Строки отсортированы по времени: число начавшихся — номер следующей
        let started = self
            .lines
            .partition_point(|line| line.start_ms.is_some_and(|start| start <= position_ms));
        started.checked_sub(1)
    }
}

/// Текст песни трека `path`. Сначала — файл `.lrc` с тем же именем в той же папке
/// (так хранят скачанные тексты), затем — тексты в тегах. Синхронизированный
/// предпочитается обычному. `None` — текста нигде нет
pub fn load_lyrics(path: &Path) -> Option<Lyrics> {
    let mut candidates = Vec::new();
    if let Some(text) = read_lrc_file(&path.with_extension("lrc")) {
        candidates.push(text);
    }
    if let Ok(tagged) = Probe::open(path).and_then(|probe| probe.read()) {
        for tag in tagged.tags() {
            for key in [ItemKey::Lyrics, ItemKey::UnsyncLyrics] {
                if let Some(text) = tag.get_string(key) {
                    candidates.push(fix_cp1251(text));
                }
            }
        }
    }
    let parsed: Vec<Lyrics> = candidates
        .iter()
        .filter_map(|text| parse_lyrics(text))
        .collect();
    parsed
        .iter()
        .find(|lyrics| lyrics.synced)
        .or_else(|| parsed.first())
        .cloned()
}

/// Файл `.lrc`: обычно UTF-8, но старые бывают в Windows-1251
fn read_lrc_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    Some(match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1251.decode(bytes).0.into_owned(),
    })
}

/// Разбирает текст песни. Есть строки с метками времени — синхронизированный (LRC),
/// иначе — обычный. Пустой текст — `None`
pub fn parse_lyrics(text: &str) -> Option<Lyrics> {
    let mut offset_ms: i64 = 0;
    let mut timed: Vec<LyricLine> = Vec::new();
    let mut plain: Vec<String> = Vec::new();

    // Строки разделяют по-разному: \n, \r\n и одиночным \r (старый формат Mac — так бывает
    // в тегах MP3). `lines` знает только первые два
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    for raw in text.lines() {
        let line = raw.trim();
        let (times, rest) = split_timestamps(line);
        if times.is_empty() {
            // Служебные строки LRC: [ar:…], [ti:…], [offset:…] — не текст
            if let Some(tag) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']'))
                && let Some((name, value)) = tag.split_once(':')
                && name.chars().all(|c| c.is_ascii_alphabetic())
            {
                if name.eq_ignore_ascii_case("offset") {
                    offset_ms = value.trim().parse().unwrap_or(0);
                }
                continue;
            }
            plain.push(line.to_owned());
            continue;
        }
        let text = strip_word_timestamps(rest);
        // Повторяющийся припев пишут одной строкой с несколькими метками
        for start in times {
            timed.push(LyricLine {
                start_ms: Some(start),
                text: text.clone(),
            });
        }
    }

    if !timed.is_empty() {
        // Положительный offset — текст должен появляться раньше
        for line in &mut timed {
            line.start_ms = line
                .start_ms
                .map(|start| (start as i64 - offset_ms).max(0) as u64);
        }
        timed.sort_by_key(|line| line.start_ms);
        return Some(Lyrics {
            lines: timed,
            synced: true,
        });
    }

    // Обычный текст — без пустых строк по краям
    let first = plain.iter().position(|line| !line.is_empty())?;
    let last = plain.iter().rposition(|line| !line.is_empty())?;
    Some(Lyrics {
        lines: plain[first..=last]
            .iter()
            .map(|text| LyricLine {
                start_ms: None,
                text: text.clone(),
            })
            .collect(),
        synced: false,
    })
}

/// Метки времени в начале строки (`[01:23.45][02:10.00]текст`) и остаток строки
fn split_timestamps(mut line: &str) -> (Vec<u64>, &str) {
    let mut times = Vec::new();
    while let Some(rest) = line.strip_prefix('[') {
        let Some((stamp, after)) = rest.split_once(']') else {
            break;
        };
        let Some(ms) = parse_timestamp(stamp) else {
            break;
        };
        times.push(ms);
        line = after;
    }
    (times, line.trim())
}

/// `mm:ss`, `mm:ss.xx`, `mm:ss.xxx` или `mm:ss:xx` — в миллисекунды
fn parse_timestamp(stamp: &str) -> Option<u64> {
    let (minutes, rest) = stamp.split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let (seconds, fraction) = match rest.split_once(['.', ':']) {
        Some((seconds, fraction)) => (seconds, fraction),
        None => (rest, ""),
    };
    let seconds: u64 = seconds.trim().parse().ok()?;
    if seconds >= 60 {
        return None;
    }
    // Дробь — доли секунды: «4» — 400 мс, «45» — 450, «456» — 456
    let fraction_ms = if fraction.is_empty() {
        0
    } else {
        let digits: String = fraction.chars().take(3).collect();
        let value: u64 = digits.parse().ok()?;
        value * 10u64.pow(3 - digits.len() as u32)
    };
    Some((minutes * 60 + seconds) * 1000 + fraction_ms)
}

/// Убирает метки отдельных слов расширенного LRC: `<01:23.45>`
fn strip_word_timestamps(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        if parse_timestamp(&rest[start + 1..start + end]).is_none() {
            out.push_str(&rest[..start + end + 1]);
        } else {
            out.push_str(&rest[..start]);
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    // После удаления меток между словами остаются двойные пробелы
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{Lyrics, parse_lyrics};

    fn starts(lyrics: &Lyrics) -> Vec<(Option<u64>, &str)> {
        lyrics
            .lines
            .iter()
            .map(|line| (line.start_ms, line.text.as_str()))
            .collect()
    }

    #[test]
    fn parses_synced_lyrics() {
        let lyrics = parse_lyrics(
            "[ar:Artist]\n[offset:+100]\n[00:12.34]First\n\
             [00:05.00][01:00.5]Chorus\n[00:20]<00:20.00>Word <00:21.00>by word\n[00:30.00]",
        )
        .unwrap();
        assert!(lyrics.synced);
        assert_eq!(
            starts(&lyrics),
            [
                (Some(4_900), "Chorus"),
                (Some(12_240), "First"),
                (Some(19_900), "Word by word"),
                (Some(29_900), ""),
                (Some(60_400), "Chorus"),
            ]
        );
        assert_eq!(lyrics.current_line(0), None);
        assert_eq!(lyrics.current_line(4_900), Some(0));
        assert_eq!(lyrics.current_line(15_000), Some(1));
        assert_eq!(lyrics.current_line(100_000), Some(4));
    }

    #[test]
    fn parses_plain_lyrics() {
        let lyrics = parse_lyrics("\n\nFirst line\n\nSecond line\n\n").unwrap();
        assert!(!lyrics.synced);
        assert_eq!(
            starts(&lyrics),
            [(None, "First line"), (None, ""), (None, "Second line")]
        );
        assert_eq!(lyrics.current_line(10_000), None);
        assert_eq!(parse_lyrics("  \n "), None);

        // Строки через одиночный \r и через \r\n — те же строки
        for text in ["First line\r\rSecond line", "First line\r\n\r\nSecond line"] {
            let lyrics = parse_lyrics(text).unwrap();
            assert_eq!(
                starts(&lyrics),
                [(None, "First line"), (None, ""), (None, "Second line")]
            );
        }
    }
}
