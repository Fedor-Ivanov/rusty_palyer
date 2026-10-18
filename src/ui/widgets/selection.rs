//! Выделение в списках треков и сетках плиток: клики с модификаторами и выделение
//! областью мыши.

use eframe::egui::{self, Pos2};
use std::collections::BTreeSet;

/// Выделение областью мыши. Начало хранится в координатах содержимого (списка или сетки),
/// а не экрана: пока тянут, содержимое может прокручиваться, и экранная точка начала
/// уезжает вместе с ним
pub(super) struct Marquee {
    pub(super) start: Pos2,
    /// Что было выделено до начала (с ⌘/Ctrl область добавляется к нему)
    pub(super) base: BTreeSet<usize>,
}

/// Выделение — как в Finder: клик выделяет один элемент, ⌘/Ctrl+клик добавляет или убирает
/// элемент, Shift+клик выделяет диапазон от последнего клика. Элементы — номера строк
/// списка или плиток сетки
#[derive(Default)]
pub(super) struct Selection {
    indices: BTreeSet<usize>,
    /// Откуда считается диапазон при Shift+клике
    anchor: Option<usize>,
}

impl Selection {
    pub(super) fn click(&mut self, index: usize, modifiers: egui::Modifiers) {
        if modifiers.shift
            && let Some(anchor) = self.anchor
        {
            // ⌘/Ctrl вместе с Shift добавляет диапазон к уже выделенному
            if !modifiers.command {
                self.indices.clear();
            }
            self.indices.extend(anchor.min(index)..=anchor.max(index));
        } else if modifiers.command {
            if !self.indices.remove(&index) {
                self.indices.insert(index);
            }
            self.anchor = Some(index);
        } else {
            self.indices = BTreeSet::from([index]);
            self.anchor = Some(index);
        }
    }

    /// Выделение областью: `base` плюс строки `range`. `anchor` — строка, с которой начали
    /// тянуть: от неё считается диапазон при следующем Shift+клике
    pub(super) fn select_range(
        &mut self,
        base: &BTreeSet<usize>,
        range: std::ops::RangeInclusive<usize>,
        anchor: usize,
    ) {
        self.select_many(base, range, anchor);
    }

    /// Выделение областью в сетке: `base` плюс плитки `items` — рамка захватывает
    /// прямоугольник плиток, а не диапазон подряд. `anchor` — как у `select_range`
    pub(super) fn select_many(
        &mut self,
        base: &BTreeSet<usize>,
        items: impl IntoIterator<Item = usize>,
        anchor: usize,
    ) {
        self.indices = base.iter().copied().chain(items).collect();
        self.anchor = Some(anchor);
    }

    pub(super) fn select_all(&mut self, len: usize) {
        self.indices = (0..len).collect();
    }

    pub(super) fn clear(&mut self) {
        self.indices.clear();
        self.anchor = None;
    }

    pub(super) fn contains(&self, index: usize) -> bool {
        self.indices.contains(&index)
    }

    pub(super) fn len(&self) -> usize {
        self.indices.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub(super) fn indices(&self) -> &BTreeSet<usize> {
        &self.indices
    }
}

#[cfg(test)]
mod tests {
    use super::Selection;
    use eframe::egui::Modifiers;

    fn selected(selection: &Selection) -> Vec<usize> {
        selection.indices().iter().copied().collect()
    }

    #[test]
    fn click_selects_only_one_track() {
        let mut selection = Selection::default();
        selection.click(2, Modifiers::NONE);
        selection.click(5, Modifiers::NONE);
        assert_eq!(selected(&selection), [5]);
    }

    #[test]
    fn command_click_toggles_tracks() {
        let mut selection = Selection::default();
        selection.click(2, Modifiers::NONE);
        selection.click(5, Modifiers::COMMAND);
        selection.click(7, Modifiers::COMMAND);
        selection.click(2, Modifiers::COMMAND);
        assert_eq!(selected(&selection), [5, 7]);
    }

    #[test]
    fn shift_click_selects_range_from_anchor() {
        let mut selection = Selection::default();
        selection.click(5, Modifiers::NONE);
        selection.click(2, Modifiers::SHIFT);
        assert_eq!(selected(&selection), [2, 3, 4, 5]);
        // Якорь остаётся на 5: новый Shift+клик заменяет диапазон, а не расширяет его
        selection.click(7, Modifiers::SHIFT);
        assert_eq!(selected(&selection), [5, 6, 7]);
    }

    #[test]
    fn command_shift_click_adds_range() {
        let mut selection = Selection::default();
        selection.click(0, Modifiers::NONE);
        selection.click(5, Modifiers::COMMAND);
        selection.click(7, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(selected(&selection), [0, 5, 6, 7]);
    }

    #[test]
    fn marquee_range_replaces_or_adds_to_base() {
        let mut selection = Selection::default();
        selection.select_range(&Default::default(), 3..=6, 6);
        assert_eq!(selected(&selection), [3, 4, 5, 6]);

        let base = [0, 9].into_iter().collect();
        selection.select_range(&base, 4..=5, 4);
        assert_eq!(selected(&selection), [0, 4, 5, 9]);
        // Shift+клик после области считает диапазон от строки, с которой её начали тянуть
        selection.click(7, Modifiers::SHIFT);
        assert_eq!(selected(&selection), [4, 5, 6, 7]);
    }

    #[test]
    fn grid_marquee_selects_rectangle_of_tiles() {
        // Сетка 4 колонки: рамка задела колонки 1–2 в строках 0–1 — это плитки 1, 2, 5, 6,
        // не диапазон 1..=6
        let mut selection = Selection::default();
        selection.select_many(&Default::default(), [1, 2, 5, 6], 1);
        assert_eq!(selected(&selection), [1, 2, 5, 6]);

        // С ⌘ рамка добавляется к уже выделенному
        let base = [9].into_iter().collect();
        selection.select_many(&base, [1, 5], 1);
        assert_eq!(selected(&selection), [1, 5, 9]);
    }
}
