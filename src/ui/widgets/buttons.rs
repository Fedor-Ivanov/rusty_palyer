//! Кнопки-иконки с точной областью нажатия и плавным наведением.

use eframe::egui::{self, CursorIcon, Pos2, Rect, Response, Sense, Ui};

use crate::ui::theme::{HOVER_FADE_SECS, TRACK_ACTIVE_FADE_SECS};

/// Форма области, в которой кнопка реагирует на курсор
pub enum HitShape {
    Rect,
    /// Круг, вписанный в прямоугольник кнопки
    Circle,
}

pub struct IconButton {
    pub response: Response,
    pub clicked: bool,
    /// Плавный hover для отрисовки: 0.0 — без наведения, 1.0 — наведено
    pub hover: f32,
}

/// Кнопка-иконка: клик, курсор-рука и плавный hover — только внутри `shape`
pub fn icon_button(
    ui: &mut Ui,
    rect: Rect,
    shape: HitShape,
    id_salt: &str,
    enabled: bool,
) -> IconButton {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.scope_id().with(id_salt), sense);

    // egui проверяет попадание по прямоугольнику, круг уточняем сами
    let inside = |pos: Pos2| match shape {
        HitShape::Rect => true,
        HitShape::Circle => pos.distance(rect.center()) <= rect.width() / 2.0,
    };
    let hovered = enabled && response.hover_pos().is_some_and(inside);
    let clicked = response.clicked() && response.interact_pointer_pos().is_some_and(inside);

    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("hover"),
        hovered,
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );

    IconButton {
        response,
        clicked,
        hover,
    }
}

/// Кнопка-переключатель в панели списка. Возвращает клик и плавные (0.0..=1.0)
/// значения включения и наведения для отрисовки иконки.
pub fn toggle_button(
    ui: &mut Ui,
    rect: Rect,
    id_salt: &str,
    on: bool,
    tooltip: &str,
) -> (bool, f32, f32) {
    let button = icon_button(ui, rect, HitShape::Circle, id_salt, true);
    let active = ui.ctx().animate_bool_with_time_and_easing(
        button.response.id.with("active"),
        on,
        TRACK_ACTIVE_FADE_SECS,
        egui::emath::easing::cubic_in_out,
    );
    let clicked = button.clicked;
    button.response.on_hover_text(tooltip);
    (clicked, active, button.hover)
}
