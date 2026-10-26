//! Undo and redo of a model's edits (ctrl+Z, ctrl+shift+Z or ctrl+Y): the model as it was before each edit, kept
//! whole. An edit is a step of its own (a node added, deleted, pasted, a connection, a setting), but the changes
//! made while a button of the mouse is held (nodes dragged, a slider moved) are one step.

use terraflux_graph::Model;

/// The steps kept.
const KEPT: usize = 200;

#[derive(Default)]
pub struct History {
    undo: Vec<Model>,
    redo: Vec<Model>,
    /// The model as it is after the last step (none while a step is being made).
    last: Option<Model>,
}

impl History {
    pub fn new(m: &Model) -> History {
        History { last: Some(m.clone()), ..Default::default() }
    }

    /// After a change of the model: the model as it was before it kept, once for the changes of a step; what was
    /// undone cannot be done again.
    pub fn changed(&mut self, m: &Model) {
        if self.last.as_ref() == Some(m) {
            return;
        }
        if let Some(before) = self.last.take() {
            self.undo.push(before);
            if self.undo.len() > KEPT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    /// Once a frame, before the edits: a step ends unless a button of the mouse is `held`.
    pub fn tick(&mut self, m: &Model, held: bool) {
        if self.last.is_none() && !held {
            self.last = Some(m.clone());
        }
    }

    /// The model as it was before the last step; whether there was one.
    pub fn undo(&mut self, m: &mut Model) -> bool {
        let Some(before) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(m, before));
        self.last = Some(m.clone());
        true
    }

    /// The step undone last made again; whether there was one.
    pub fn redo(&mut self, m: &mut Model) -> bool {
        let Some(after) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(m, after));
        self.last = Some(m.clone());
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Node;

    #[test]
    fn steps_are_undone_and_done_again() {
        let mut m = Model::new("t");
        let mut h = History::new(&m);
        // a step: a node added
        m.nodes.push(Node::new("a", "raster.calc"));
        h.changed(&m);
        h.tick(&m, false);
        let one = m.clone();
        // another: the node dragged, frame after frame with the button held
        for k in 0..5 {
            m.nodes[0].pos = Some([k as f32, 0.0]);
            h.changed(&m);
            h.tick(&m, true);
        }
        h.tick(&m, false);
        let two = m.clone();
        // nothing changed: no step
        h.changed(&m);
        h.tick(&m, false);
        assert!(h.undo(&mut m));
        assert_eq!(m, one);
        assert!(h.undo(&mut m));
        assert!(m.nodes.is_empty());
        assert!(!h.undo(&mut m));
        assert!(h.redo(&mut m) && h.redo(&mut m));
        assert_eq!(m, two);
        assert!(!h.redo(&mut m));
        // a change after an undo: what was undone is gone
        assert!(h.undo(&mut m));
        m.nodes.push(Node::new("c", "raster.calc"));
        h.changed(&m);
        assert!(!h.can_redo());
    }
}
