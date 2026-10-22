//! HTTP для онлайн-сервисов: тексты песен (LRCLIB), картинки исполнителей (Deezer).

use std::time::Duration;

/// Сервисы просят, чтобы приложение представлялось: название, версия и ссылка
const USER_AGENT: &str = concat!(
    "LMP ",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Fedor-Ivanov/rusty_palyer)"
);
const TIMEOUT: Duration = Duration::from_secs(15);

/// Клиент с общими настройками. Коды ответа — не ошибки: 404 у сервисов значит
/// «не найдено», его разбирает вызывающий
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .build()
        .into()
}
