//! Форматирование значений для показа.

/// Секунды в `мм:сс` или `чч:мм:сс`.
pub fn format_secs(seconds: f32) -> String {
    let seconds = seconds.max(0.0) as u64;
    let (h, m, s) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);

    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Число со словом в нужной форме: «1 track», «3 tracks»
pub fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}
