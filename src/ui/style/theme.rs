use eframe::egui::Color32;

// Glassmorphism: тёмное стекло — системное размытие под прозрачным окном
// и графитовая тонировка, одна на всё окно.
// Белый с альфой задаётся premultiplied: (a, a, a, a).

/// Основной текст и иконки
pub const TEXT: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF7);
/// Второстепенный текст: исполнитель, время, неактивные треки
pub const TEXT_MUTED: Color32 = Color32::from_rgba_premultiplied(180, 180, 180, 180);
/// Подсказки и заглушки
pub const TEXT_FAINT: Color32 = Color32::from_rgba_premultiplied(110, 110, 110, 110);

// Иконки рисуются из перекрывающихся фигур, поэтому их цвета непрозрачные:
// с альфой наложения были бы видны как более плотные пятна
pub const ICON: Color32 = TEXT;
/// Иконка в покое, светлеет до ICON при наведении
pub const ICON_SOFT: Color32 = Color32::from_rgb(0xC8, 0xC8, 0xCE);
/// Выключенный переключатель
pub const ICON_OFF: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x92);
/// Недоступная кнопка
pub const ICON_DISABLED: Color32 = Color32::from_rgb(0x5C, 0x5C, 0x63);

/// Графитовая тонировка поверх системного размытия — единственная заливка стекла,
/// одна на всё окно. Чем больше альфа, тем темнее и тем меньше видно, что за окном
pub const WINDOW_TINT: Color32 = Color32::from_rgba_unmultiplied_const(24, 24, 26, 195);

/// Линия изменения ширины панели на границе панелей: приглушённый белый (~18%),
/// чтобы делила разделы, но не бросалась в глаза
pub const RESIZE_LINE: Color32 = Color32::from_rgba_premultiplied(45, 45, 45, 45);

/// Подсветка при наведении
pub const HIGHLIGHT: Color32 = Color32::from_rgba_premultiplied(18, 18, 18, 18);
/// Подсветка активного элемента, подложка круглых кнопок
pub const HIGHLIGHT_STRONG: Color32 = Color32::from_rgba_premultiplied(40, 40, 40, 40);

/// Толщина рамки выделения: выделенная строка, рамка выделения областью, плитка
/// с открытым контекстным меню. Цвет рамки — HIGHLIGHT_STRONG
pub const SELECTION_STROKE_WIDTH: f32 = 1.5;

/// Рамка выделения областью мыши — в тон выделенным строкам: обводка того же цвета,
/// что рамка выделения и подсветка играющего трека, заливка — как подсветка при наведении
pub const MARQUEE_FILL: Color32 = HIGHLIGHT;
pub const MARQUEE_STROKE: Color32 = HIGHLIGHT_STRONG;

/// Главная кнопка действия («Play all»): светлая, с тёмным текстом — заметнее остальных
pub const PRIMARY_BUTTON_FILL: Color32 = TEXT;
pub const PRIMARY_BUTTON_TEXT: Color32 = Color32::from_rgb(0x16, 0x16, 0x18);
pub const PRIMARY_BUTTON_HOVER: Color32 = Color32::from_rgb(0xD6, 0xD6, 0xDA);
/// Кнопка удаления: красная, при наведении темнее
pub const DESTRUCTIVE_BUTTON_FILL: Color32 = Color32::from_rgb(0xD9, 0x3F, 0x3F);
pub const DESTRUCTIVE_BUTTON_HOVER: Color32 = Color32::from_rgb(0xB8, 0x32, 0x32);
/// Затемнение окна под модальным диалогом
pub const MODAL_BACKDROP: Color32 = Color32::from_black_alpha(120);

// Фон деталей текущего трека — его размытая обложка: непрозрачность обложки (стекло окна
// под ней остаётся видно) и затемнение поверх неё ради читаемости текста — сильнее сверху,
// где текст, слабее внизу
pub const TRACK_BACKDROP_OPACITY: f32 = 0.55;
pub const TRACK_BACKDROP_SHADE_TOP: Color32 = Color32::from_black_alpha(150);
pub const TRACK_BACKDROP_SHADE_BOTTOM: Color32 = Color32::from_black_alpha(70);
/// Проявление фона, когда размытая обложка готова
pub const TRACK_BACKDROP_FADE_SECS: f32 = 0.4;
/// Второстепенная кнопка в покое — HIGHLIGHT_STRONG, при наведении — светлее
pub const SECONDARY_BUTTON_HOVER: Color32 = Color32::from_rgba_premultiplied(66, 66, 66, 66);

/// Незаполненная часть ползунка
pub const SLIDER_TRACK: Color32 = Color32::from_rgba_premultiplied(55, 55, 55, 55);

pub const RED: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B); // error, debug color
/// Фон плашек сверху окна (ошибка, прогресс добавления): почти непрозрачный —
/// они лежат поверх контента и должны читаться
pub const BANNER_FILL: Color32 = Color32::from_rgba_unmultiplied_const(28, 28, 30, 245);
/// Рамка плашки прогресса — нейтральная, в отличие от красной у ошибки
pub const BANNER_BORDER: Color32 = Color32::from_rgba_premultiplied(45, 45, 45, 45);

// Длительность плавных переходов: наведение и смена активного элемента
pub const HOVER_FADE_SECS: f32 = 0.15;
pub const TRACK_ACTIVE_FADE_SECS: f32 = 0.2;
