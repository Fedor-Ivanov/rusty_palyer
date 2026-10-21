//! Клавиши плеера, пока окно в фокусе: пробел — play/pause, ⌘→ / ⌘← (Ctrl на Windows) —
//! следующий и предыдущий трек, как в Apple Music. Медиа-кнопки клавиатуры работают
//! и без фокуса — см. `system_media`.

use eframe::egui::{self, Key, Modifiers};

use super::{action::Action, app::PlayerApp};

impl PlayerApp {
    /// Вызывается в начале кадра, до виджетов: пробел забирается раньше, чем его увидит
    /// кнопка в фокусе, — иначе он нажал бы её, а не поставил паузу.
    /// Пока вводят текст (поиск, название плейлиста), клавиши остаются полю
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.text_edit_focused() {
            return;
        }
        let (toggle, next, previous) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::Space),
                input.consume_key(Modifiers::COMMAND, Key::ArrowRight),
                input.consume_key(Modifiers::COMMAND, Key::ArrowLeft),
            )
        });
        if toggle {
            self.dispatch(Action::TogglePlay);
        }
        if next {
            self.dispatch(Action::Next);
        }
        if previous {
            self.dispatch(Action::Previous);
        }
    }
}
