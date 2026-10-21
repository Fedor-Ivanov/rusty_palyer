//! Кнопки-иконки с точной областью нажатия и плавным наведением.

use eframe::egui::{self, Align2, CursorIcon, FontId, Pos2, Rect, Response, Sense, Ui, vec2};

use crate::ui::style::theme::{
    DESTRUCTIVE_BUTTON_FILL, DESTRUCTIVE_BUTTON_HOVER, HIGHLIGHT_STRONG, HOVER_FADE_SECS,
    PRIMARY_BUTTON_FILL, PRIMARY_BUTTON_HOVER, PRIMARY_BUTTON_TEXT, SECONDARY_BUTTON_HOVER, TEXT,
    TRACK_ACTIVE_FADE_SECS,
};

const PILL_HEIGHT: f32 = 32.0;
const PILL_PADDING: f32 = 18.0;
const PILL_TEXT_SIZE: f32 = 14.0;

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

/// Вид кнопки-«пилюли»
#[derive(Clone, Copy)]
pub enum ButtonStyle {
    /// Главное действие: светлая заливка и тёмный текст
    Primary,
    /// Полупрозрачная
    Secondary,
    /// Действие, которое что-то удаляет: красная
    Destructive,
}

/// Кнопка-«пилюля» с текстом. При наведении плавно меняет яркость
pub fn pill_button(ui: &mut Ui, label: &str, style: ButtonStyle) -> Response {
    let font = FontId::proportional(PILL_TEXT_SIZE);
    let text_width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), TEXT)
        .size()
        .x;
    let (rect, response) = ui.allocate_exact_size(
        vec2(text_width + 2.0 * PILL_PADDING, PILL_HEIGHT),
        Sense::click(),
    );
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("hover"),
        response.hovered(),
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );
    let (fill, text) = match style {
        // Светлая кнопка при наведении чуть темнеет
        ButtonStyle::Primary => (
            PRIMARY_BUTTON_FILL.lerp_to_gamma(PRIMARY_BUTTON_HOVER, hover),
            PRIMARY_BUTTON_TEXT,
        ),
        // Полупрозрачная — при наведении светлеет
        ButtonStyle::Secondary => (
            HIGHLIGHT_STRONG.lerp_to_gamma(SECONDARY_BUTTON_HOVER, hover),
            TEXT,
        ),
        ButtonStyle::Destructive => (
            DESTRUCTIVE_BUTTON_FILL.lerp_to_gamma(DESTRUCTIVE_BUTTON_HOVER, hover),
            TEXT,
        ),
    };
    ui.painter().rect_filled(rect, PILL_HEIGHT / 2.0, fill);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, label, font, text);
    response
}
