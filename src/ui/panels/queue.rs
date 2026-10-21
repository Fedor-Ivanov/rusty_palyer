//! Правая панель: очередь — что слушаем сейчас. Треки сюда попадают из медиатеки, плейлистов или
//! кнопкой «+» мимо библиотеки. Внизу — shuffle и повтор.

use eframe::egui::{self, Align, Align2, FontId, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use crate::{
    core::playback::RepeatMode,
    ui::{
        action::Action,
        app::{ADD_BUTTON_SIZE, ListFocus, PlayerApp},
        components,
        i18n::{fill, t},
        loading::Destination,
        style::icons::{paint_broom, paint_repeat, paint_save_playlist, paint_shuffle},
        style::theme::{TEXT, TEXT_FAINT},
    },
};

// Зазор между кнопками-иконками: повтора в нижнем блоке и кнопками в заголовке
const ICON_BUTTON_GAP: f32 = 4.0;
/// Кнопок в заголовке очереди: очистить, сохранить, «+»
const HEADER_BUTTONS: f32 = 3.0;
/// Наименьший зазор между заголовком и кнопками
const TITLE_BUTTONS_GAP: f32 = 8.0;

impl PlayerApp {
    pub(crate) fn render_queue(&mut self, ui: &mut Ui) {
        // Вся область панели — запоминаем до заголовка, чтобы заглушка встала по её центру
        let panel_rect = ui.max_rect();
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            // Рядом с заголовком — сколько треков в очереди; пустой — без числа:
            // там и так «No tracks»
            let count = self.core.controller.player().track_list.tracks.len();
            // Справа три кнопки — заголовок занимает место левее них и обрезается с «…»,
            // а не уходит под кнопки
            let full = ui.available_rect_before_wrap();
            let buttons_width = HEADER_BUTTONS * ADD_BUTTON_SIZE
                + (HEADER_BUTTONS - 1.0) * ICON_BUTTON_GAP
                + TITLE_BUTTONS_GAP;
            let title_area = Rect::from_min_max(
                full.min,
                pos2(
                    (full.right() - buttons_width).max(full.left()),
                    full.bottom(),
                ),
            );
            let title_row = ui
                .scope_builder(UiBuilder::new().max_rect(title_area), |ui| {
                    if count == 0 {
                        components::block_title(ui, t().queue, Align::LEFT, TEXT)
                    } else {
                        // Связанный плейлист — рядом с заголовком, перед числом треков
                        let tracks = t().count(count, &t().track_count);
                        let note = match self.queue_playlist_name() {
                            Some(name) => format!("· {name} · {tracks}"),
                            None => format!("· {tracks}"),
                        };
                        components::block_title_with_note(ui, t().queue, &note)
                    }
                })
                .inner;
            // Строка кнопок — та же по высоте, но во всю ширину
            let row = Rect::from_x_y_ranges(full.x_range(), title_row.y_range());
            // Слева направо: «+», сохранить, очистить. «+» ставится у правого края переданной
            // строки — даём ему строку, укороченную на две кнопки справа
            let add_row = row.with_max_x(row.right() - 2.0 * (ADD_BUTTON_SIZE + ICON_BUTTON_GAP));
            self.render_add_button(ui, add_row, Destination::Queue);
            self.render_queue_buttons(ui, row, count > 0);
        });

        if self.core.controller.player().track_list.tracks.is_empty() {
            if self.loaders.is_empty() {
                ui.painter().text(
                    panel_rect.center(),
                    Align2::CENTER_CENTER,
                    t().no_tracks,
                    FontId::proportional(14.0),
                    TEXT_FAINT,
                );
            }
            return;
        }

        // Сколько выделенных треков не из медиатеки (добавлены кнопкой «+» мимо неё) — для
        // пункта «Add to library». Удалить из медиатеки отсюда нельзя: очередь — про то, что
        // слушаем, а не про медиатеку. Меню открывается после выделения, так что выделение
        // начала кадра — то, к которому оно относится
        let list = &self.core.controller.player().track_list;
        let library_paths = self.core.library.paths();
        let outside_library = self
            .queue_view
            .selection()
            .iter()
            .filter_map(|&index| list.tracks.get(index))
            .filter(|track| !library_paths.contains(&track.path))
            .count();
        // Меню знает только, сколько строк выделено: какие — узнаём после списка
        let mut remove = false;
        let mut add_to_library = false;
        let current = list.current_index;
        let response = self.queue_view.show(
            ui,
            &list.tracks,
            |index| components::RowState {
                current: current == Some(index),
                missing: false,
            },
            self.focus == ListFocus::Queue,
            |ui, selected| {
                if outside_library > 0 {
                    let label = t().for_tracks(
                        t().add_to_library,
                        t().add_items_to_library,
                        outside_library,
                    );
                    if components::menu_item(ui, &label).clicked() {
                        add_to_library = true;
                    }
                }
                let label =
                    t().for_tracks(t().remove_from_queue, t().remove_items_from_queue, selected);
                if components::menu_item(ui, &label).clicked() {
                    remove = true;
                }
            },
        );

        if response.interacted {
            self.focus = ListFocus::Queue;
        }
        if add_to_library {
            let selected = self.queue_view.selection().iter().copied().collect();
            self.dispatch(Action::AddQueueTracksToLibrary(selected));
        } else if remove || response.delete {
            let selected = self.queue_view.selection().clone();
            self.dispatch(Action::RemoveFromQueue(selected));
        }
        if let Some(index) = response.activated {
            self.dispatch(Action::PlayQueueTrack(index));
        }
    }

    /// Кнопки справа от «+» в заголовке очереди: сохранить её как плейлист (дискета)
    /// и очистить (метла).
    /// Пустую очередь ни сохранить, ни очистить нельзя — кнопки приглушены
    fn render_queue_buttons(&mut self, ui: &mut Ui, row: Rect, enabled: bool) {
        let size = vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE);
        // Места справа налево: 0 — у правого края (очистить), 1 — левее (сохранить),
        // 2 — «+»
        let rect_at = |slot: f32| {
            Rect::from_center_size(
                pos2(
                    row.right()
                        - ADD_BUTTON_SIZE / 2.0
                        - slot * (ADD_BUTTON_SIZE + ICON_BUTTON_GAP),
                    row.center().y,
                ),
                size,
            )
        };

        let save_rect = rect_at(1.0);
        // Очередь связана с плейлистом и не менялась — сохранять нечего
        let save_enabled = enabled && !self.core.queue_matches_playlist();
        let save = components::icon_button(
            ui,
            save_rect,
            components::HitShape::Circle,
            "save_queue",
            save_enabled,
        );
        paint_save_playlist(ui, save_rect, save_enabled, save.hover);
        // Очередь связана с плейлистом — клик перезаписывает его; сохранить как новый —
        // из меню по правому клику. Не связана — клик спрашивает название нового
        let linked = self.core.queue_playlist().zip(self.queue_playlist_name());
        let save_clicked = save.clicked;
        let mut save_as_new = false;
        let mut save_to_linked = false;
        match &linked {
            Some((_, name)) => {
                let save_to = fill(t().save_queue_to, &[("playlist", name)]);
                save.response.context_menu(|ui| {
                    components::menu_style(ui);
                    if components::menu_item(ui, &save_to).clicked() {
                        save_to_linked = true;
                    }
                    if components::menu_item(ui, t().save_as_new_playlist).clicked() {
                        save_as_new = true;
                    }
                });
                save.response.on_hover_text(save_to);
            }
            None => {
                save.response.on_hover_text(t().save_queue);
            }
        }

        let clear_rect = rect_at(0.0);
        let clear = components::icon_button(
            ui,
            clear_rect,
            components::HitShape::Circle,
            "clear_queue",
            enabled,
        );
        paint_broom(ui, clear_rect, enabled, clear.hover);
        let clear_clicked = clear.clicked;
        clear.response.on_hover_text(t().clear_queue);

        match linked {
            Some((id, _)) if save_clicked || save_to_linked => {
                self.dispatch(Action::SaveQueueToPlaylist(id));
            }
            _ if save_clicked || save_as_new => self.dispatch(Action::RequestSaveQueue),
            _ => {}
        }
        if clear_clicked {
            self.dispatch(Action::ClearQueue);
        }
    }

    /// Название плейлиста, с которым связана очередь (см. `Core::queue_playlist`)
    fn queue_playlist_name(&self) -> Option<String> {
        let id = self.core.queue_playlist()?;
        self.core
            .playlists
            .get(id)
            .map(|playlist| playlist.name.clone())
    }

    /// Нижний блок очереди: слева shuffle, справа две кнопки повтора
    pub(crate) fn render_playback_modes(&mut self, ui: &mut Ui) {
        let list = &self.core.controller.player().track_list;
        let (shuffle, repeat) = (list.shuffle, list.repeat);
        let size = vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE);

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let (clicked, active, hover) =
                components::toggle_button(ui, rect, "shuffle", shuffle, t().shuffle);
            paint_shuffle(ui, rect, active, hover);
            if clicked {
                self.dispatch(Action::ToggleShuffle);
            }

            // Повторы прижаты вправо. Включена может быть только одна из кнопок —
            // это обеспечивает сам RepeatMode
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = ICON_BUTTON_GAP;
                // В right_to_left первой добавляется самая правая кнопка
                for (mode, id_salt, tooltip) in [
                    (RepeatMode::All, "repeat_all", t().repeat_all),
                    (RepeatMode::One, "repeat_one", t().repeat_one),
                ] {
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    let (clicked, active, hover) =
                        components::toggle_button(ui, rect, id_salt, repeat == mode, tooltip);
                    paint_repeat(ui, rect, mode == RepeatMode::One, active, hover);
                    if clicked {
                        self.dispatch(Action::ToggleRepeat(mode));
                    }
                }
            });
        });
    }
}
