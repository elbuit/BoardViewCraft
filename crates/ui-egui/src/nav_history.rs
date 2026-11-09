//! Browser-like navigation history of board selections (toolbar Back / Forward).
//!
//! Each entry pairs a selection with the view it was seen in. Recording a new selection after
//! going back drops the forward entries; moving back or forward never records anything. The
//! model is generic over the selection and view types so it can be tested without a board.

use crate::board_tabs::BoardTab;
use bvc_core::{Point, Side};

/// Maximum number of entries kept; the oldest is dropped beyond it.
pub const DEFAULT_CAP: usize = 100;

/// What is selected on the board.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NavSelection {
    pub part: Option<usize>,
    pub pin: Option<usize>,
    pub via: Option<usize>,
    pub net: Option<String>,
}

impl NavSelection {
    pub fn is_empty(&self) -> bool {
        self.part.is_none() && self.pin.is_none() && self.via.is_none() && self.net.is_none()
    }
}

/// Where a selection was looked at: the tab, the active canvas and each canvas' center/scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavView {
    pub tab: BoardTab,
    pub active: Side,
    pub top: (Point, f32),
    pub bottom: (Point, f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry<S, V> {
    pub selection: S,
    pub view: V,
}

#[derive(Debug, Clone)]
pub struct NavHistory<S, V> {
    entries: Vec<Entry<S, V>>,
    /// Index of the current entry (meaningless while `entries` is empty).
    cursor: usize,
    cap: usize,
}

impl<S, V> Default for NavHistory<S, V> {
    fn default() -> Self {
        Self::with_cap(DEFAULT_CAP)
    }
}

impl<S, V> NavHistory<S, V> {
    pub fn with_cap(cap: usize) -> Self {
        Self { entries: Vec::new(), cursor: 0, cap: cap.max(1) }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }

    pub fn current(&self) -> Option<&Entry<S, V>> {
        self.entries.get(self.cursor)
    }

    pub fn can_back(&self) -> bool {
        !self.entries.is_empty() && self.cursor > 0
    }

    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    /// Replaces the view of the current entry (the view the user leaves it in).
    pub fn update_current_view(&mut self, view: V) {
        if let Some(entry) = self.entries.get_mut(self.cursor) {
            entry.view = view;
        }
    }

    /// Moves to the previous entry, saving `view_now` into the one being left.
    pub fn back(&mut self, view_now: V) -> Option<&Entry<S, V>> {
        if !self.can_back() {
            return None;
        }
        self.update_current_view(view_now);
        self.cursor -= 1;
        self.entries.get(self.cursor)
    }

    /// Moves to the next entry, saving `view_now` into the one being left.
    pub fn forward(&mut self, view_now: V) -> Option<&Entry<S, V>> {
        if !self.can_forward() {
            return None;
        }
        self.update_current_view(view_now);
        self.cursor += 1;
        self.entries.get(self.cursor)
    }
}

impl<S: PartialEq, V> NavHistory<S, V> {
    /// Records `selection` as the new current entry. A selection equal to the current one is
    /// skipped (returns `false`); forward entries are dropped; the oldest entry goes past the cap.
    pub fn record(&mut self, selection: S, view: V) -> bool {
        if self.current().is_some_and(|e| e.selection == selection) {
            return false;
        }
        if !self.entries.is_empty() {
            self.entries.truncate(self.cursor + 1);
        }
        self.entries.push(Entry { selection, view });
        if self.entries.len() > self.cap {
            let excess = self.entries.len() - self.cap;
            self.entries.drain(..excess);
        }
        self.cursor = self.entries.len() - 1;
        true
    }
}

/// The app's selection history: watches the selection once per frame and records changes.
#[derive(Debug, Default)]
pub struct Navigation {
    pub history: NavHistory<NavSelection, NavView>,
    /// Selection last seen (recorded, restored or cleared), to detect changes.
    seen: NavSelection,
    /// View at the end of the previous frame: the view a selection is left in.
    last_view: Option<NavView>,
}

impl Navigation {
    /// Call once per frame after all UI ran. A changed, non-empty selection becomes a new entry,
    /// after the entry being left keeps the view it was left in.
    pub fn observe(&mut self, selection: &NavSelection, view: NavView) {
        if *selection != self.seen {
            self.seen = selection.clone();
            if !selection.is_empty() {
                if let Some(left) = self.last_view {
                    self.history.update_current_view(left);
                }
                self.history.record(selection.clone(), view);
            }
        }
        self.last_view = Some(view);
    }

    /// Goes back one entry; the caller restores the returned selection and view.
    pub fn back(&mut self, view_now: NavView) -> Option<Entry<NavSelection, NavView>> {
        let entry = self.history.back(view_now).cloned();
        self.restored(entry)
    }

    /// Goes forward one entry; the caller restores the returned selection and view.
    pub fn forward(&mut self, view_now: NavView) -> Option<Entry<NavSelection, NavView>> {
        let entry = self.history.forward(view_now).cloned();
        self.restored(entry)
    }

    fn restored(&mut self, entry: Option<Entry<NavSelection, NavView>>) -> Option<Entry<NavSelection, NavView>> {
        if let Some(e) = &entry {
            self.seen = e.selection.clone();
            self.last_view = Some(e.view);
        }
        entry
    }

    /// Forgets everything (board opened or closed).
    pub fn clear(&mut self) {
        self.history.clear();
        self.seen = NavSelection::default();
        self.last_view = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type H = NavHistory<u32, u32>;

    fn sel(h: &H) -> Option<u32> {
        h.current().map(|e| e.selection)
    }

    #[test]
    fn empty_history_cannot_move() {
        let mut h = H::default();
        assert!(h.is_empty() && !h.can_back() && !h.can_forward());
        assert!(h.back(0).is_none());
        assert!(h.forward(0).is_none());
    }

    #[test]
    fn record_appends_and_becomes_current() {
        let mut h = H::default();
        assert!(h.record(1, 10));
        assert!(h.record(2, 20));
        assert_eq!(h.len(), 2);
        assert_eq!(sel(&h), Some(2));
        assert!(h.can_back() && !h.can_forward());
    }

    #[test]
    fn back_and_forward_move_without_recording() {
        let mut h = H::default();
        h.record(1, 10);
        h.record(2, 20);
        h.record(3, 30);
        let e = h.back(33).unwrap();
        assert_eq!((e.selection, e.view), (2, 20));
        let e = h.back(22).unwrap();
        assert_eq!((e.selection, e.view), (1, 10));
        assert!(h.back(0).is_none(), "already at the oldest entry");
        let e = h.forward(11).unwrap();
        assert_eq!((e.selection, e.view), (2, 22), "the view left behind was saved");
        let e = h.forward(0).unwrap();
        assert_eq!((e.selection, e.view), (3, 33));
        assert!(h.forward(0).is_none());
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn recording_after_back_truncates_forward_entries() {
        let mut h = H::default();
        h.record(1, 0);
        h.record(2, 0);
        h.record(3, 0);
        h.back(0);
        h.back(0);
        assert!(h.record(4, 0));
        assert_eq!(h.len(), 2);
        assert_eq!(sel(&h), Some(4));
        assert!(!h.can_forward());
        assert_eq!(h.back(0).map(|e| e.selection), Some(1));
    }

    #[test]
    fn history_is_capped_dropping_the_oldest() {
        let mut h = NavHistory::<u32, u32>::with_cap(3);
        for i in 0..5 {
            h.record(i, 0);
        }
        assert_eq!(h.len(), 3);
        assert_eq!(sel(&h), Some(4));
        h.back(0);
        h.back(0);
        assert_eq!(sel(&h), Some(2));
        assert!(!h.can_back());
        assert_eq!(H::default().cap, DEFAULT_CAP);
    }

    #[test]
    fn duplicates_of_the_current_entry_are_skipped() {
        let mut h = H::default();
        assert!(h.record(1, 10));
        assert!(!h.record(1, 99));
        assert_eq!(h.len(), 1);
        assert_eq!(h.current().unwrap().view, 10);
        h.record(2, 0);
        assert!(h.record(1, 0), "only the current entry dedupes");
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn update_current_view_and_clear() {
        let mut h = H::default();
        h.update_current_view(5);
        h.record(1, 10);
        h.update_current_view(15);
        assert_eq!(h.current().unwrap().view, 15);
        h.clear();
        assert!(h.is_empty() && h.current().is_none());
    }

    #[test]
    fn empty_selection_detection() {
        assert!(NavSelection::default().is_empty());
        assert!(!NavSelection { net: Some("GND".into()), ..Default::default() }.is_empty());
    }
    fn view(x: f64) -> NavView {
        NavView { tab: BoardTab::Top, active: Side::Top, top: (Point { x, y: 0.0 }, 1.0), bottom: (Point::ZERO, 1.0) }
    }

    fn part(i: usize) -> NavSelection {
        NavSelection { part: Some(i), ..Default::default() }
    }

    #[test]
    fn navigation_records_selection_changes_with_the_view_left_behind() {
        let mut nav = Navigation::default();
        nav.observe(&part(1), view(1.0));
        nav.observe(&part(1), view(2.0)); // panned while part 1 stays selected
        nav.observe(&part(2), view(3.0));
        assert_eq!(nav.history.len(), 2);
        let e = nav.back(view(4.0)).unwrap();
        assert_eq!(e.selection, part(1));
        assert_eq!(e.view, view(2.0), "part 1 comes back where it was left");
        // The restored selection is not recorded again.
        nav.observe(&part(1), view(2.0));
        assert_eq!(nav.history.len(), 2);
        let e = nav.forward(view(2.0)).unwrap();
        assert_eq!((e.selection, e.view), (part(2), view(4.0)));
    }

    #[test]
    fn navigation_ignores_cleared_selection_and_resets() {
        let mut nav = Navigation::default();
        nav.observe(&NavSelection::default(), view(0.0));
        assert!(nav.history.is_empty());
        nav.observe(&part(1), view(1.0));
        nav.observe(&NavSelection::default(), view(1.0));
        nav.observe(&part(1), view(1.0));
        assert_eq!(nav.history.len(), 1, "re-selecting the current entry is a duplicate");
        nav.clear();
        assert!(nav.history.is_empty() && nav.back(view(0.0)).is_none());
    }
}
