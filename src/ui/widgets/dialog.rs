//! Модальный диалог подтверждения.

use eframe::egui::{self, Align, Id, Label, Layout, RichText, Stroke};

use super::buttons::{ButtonStyle, pill_button};
use crate::ui::theme::{BANNER_BORDER, BANNER_FILL, MODAL_BACKDROP, TEXT, TEXT_MUTED};

const DIALOG_WIDTH: f32 = 360.0;

/// Ответ на диалог подтверждения
pub enum Confirm {
    /// Ещё не ответили
    Pending,
    Confirmed,
    /// «Cancel», Esc или клик мимо диалога
    Cancelled,
}

/// Модальный диалог поверх затемнённого окна: заголовок, пояснение и две кнопки —
/// «Cancel» и подтверждение `confirm_label` (красная: подтверждают удаление).
/// Показывать каждый кадр, пока не ответили
pub fn confirm_dialog(
    ctx: &egui::Context,
    id_salt: &str,
    title: &str,
    message: &str,
    confirm_label: &str,
) -> Confirm {
    let mut answer = Confirm::Pending;
    let modal = egui::Modal::new(Id::unique(id_salt))
        .backdrop_color(MODAL_BACKDROP)
        .frame(
            egui::Frame::new()
                .fill(BANNER_FILL)
                .stroke(Stroke::new(1.0, BANNER_BORDER))
                .corner_radius(14)
                .inner_margin(20),
        )
        .show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            ui.add(Label::new(RichText::new(title).size(17.0).color(TEXT)).selectable(false));
            ui.add_space(6.0);
            ui.add(
                Label::new(RichText::new(message).size(14.0).color(TEXT_MUTED))
                    .selectable(false)
                    .wrap(),
            );
            ui.add_space(18.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // В right_to_left первой добавляется самая правая кнопка
                if pill_button(ui, confirm_label, ButtonStyle::Destructive).clicked() {
                    answer = Confirm::Confirmed;
                }
                if pill_button(ui, "Cancel", ButtonStyle::Secondary).clicked() {
                    answer = Confirm::Cancelled;
                }
            });
        });
    if matches!(answer, Confirm::Pending) && modal.should_close() {
        answer = Confirm::Cancelled;
    }
    answer
}
