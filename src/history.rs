use crate::coords::{Calibration, ImageCoordSystem};
use crate::track::Track;

#[derive(Clone, Debug, PartialEq)]
pub struct EditSnapshot {
    pub tracks: Vec<Track>,
    pub active: usize,
    pub coords: ImageCoordSystem,
    pub calibration: Calibration,
}

pub struct UndoStack {
    states: Vec<EditSnapshot>,
    index: usize,
    saved: usize,
}

impl UndoStack {
    pub fn new(initial: EditSnapshot) -> Self {
        Self {
            states: vec![initial],
            index: 0,
            saved: 0,
        }
    }

    pub fn reset(&mut self, initial: EditSnapshot) {
        self.states.clear();
        self.states.push(initial);
        self.index = 0;
        self.saved = 0;
    }

    /// Record the state after an edit. Redo entries are discarded.
    pub fn record(&mut self, after: EditSnapshot) {
        if self.states.get(self.index) == Some(&after) {
            return;
        }
        if self.saved > self.index {
            // The saved state lived in the redo branch we are about to drop.
            self.saved = usize::MAX;
        }
        self.states.truncate(self.index + 1);
        self.states.push(after);
        self.index = self.states.len() - 1;
    }

    pub fn can_undo(&self) -> bool {
        self.index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.index + 1 < self.states.len()
    }

    pub fn undo(&mut self) -> Option<EditSnapshot> {
        if !self.can_undo() {
            return None;
        }
        self.index -= 1;
        self.states.get(self.index).cloned()
    }

    pub fn redo(&mut self) -> Option<EditSnapshot> {
        if !self.can_redo() {
            return None;
        }
        self.index += 1;
        self.states.get(self.index).cloned()
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.index;
    }

    pub fn dirty(&self) -> bool {
        self.index != self.saved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(y: f32) -> EditSnapshot {
        let mut coords = ImageCoordSystem::default();
        coords.base.origin = [0.0, y as f64];
        EditSnapshot {
            tracks: Vec::new(),
            active: 0,
            coords,
            calibration: Calibration::default(),
        }
    }

    #[test]
    fn undo_redo_restores_after_states() {
        let mut stack = UndoStack::new(snapshot(0.0));
        stack.record(snapshot(1.0));
        assert!(stack.dirty());
        assert_eq!(stack.undo(), Some(snapshot(0.0)));
        assert!(!stack.dirty());
        assert_eq!(stack.redo(), Some(snapshot(1.0)));
        assert!(stack.dirty());
        stack.mark_saved();
        assert!(!stack.dirty());
    }

    #[test]
    fn edit_after_undo_invalidates_saved_redo() {
        let mut stack = UndoStack::new(snapshot(0.0));
        stack.record(snapshot(1.0));
        stack.mark_saved();
        stack.undo();
        stack.record(snapshot(2.0));
        assert!(stack.dirty());
        assert!(!stack.can_redo());
    }
}
