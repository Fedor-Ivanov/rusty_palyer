//! Центральная область: информация о текущем треке и обложка, без треков — «Open files».

use eframe::egui::{self, CursorIcon, Rect, RichText, Ui};

use super::{PlayerApp, loading::Destination};
use crate::ui::{
    format::format_secs,
    theme::{TEXT, TEXT_MUTED},
    widgets,
};

impl PlayerApp {
    /// Кнопка по центру всего оставшегося места
    pub(super) fn render_add_tracks_block(&mut self, ui: &mut Ui) {
        let text = RichText::new("Open files").size(18.0).color(TEXT);

        // Layout в egui не центрирует по вертикали, поэтому считаем размер кнопки
        // заранее и ставим её в центр свободной области сами
        let text_size = egui::WidgetText::from(text.clone())
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            )
            .size();
        let button_size = text_size + 2.0 * ui.spacing().button_padding;
        let button_rect =
            Rect::from_center_size(ui.available_rect_before_wrap().center(), button_size);

        let open = egui::Button::new(text).frame(false);
        if ui
            .put(button_rect, open)
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Open files")
            .clicked()
        {
            self.add_files(Destination::Playlist);
        }
    }

    pub(super) fn render_current_track_info(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            if let Some(track) = &self.controller.player().current_track {
                // Название ярче, остальное — второстепенным цветом
                for (line, color) in [
                    (track.title.as_str(), TEXT),
                    (&track.artist, TEXT_MUTED),
                    (&track.album, TEXT_MUTED),
                    (format_secs(track.duration_secs as f32).as_str(), TEXT_MUTED),
                ] {
                    if !line.is_empty() {
                        ui.label(RichText::new(line).size(16.0).color(color));
                    }
                }
                ui.add_space(8.0);

                ui.vertical_centered(|ui| {
                    widgets::album_cover(ui, track.album_cover.as_deref());
                });
            }
        });
    }
}
