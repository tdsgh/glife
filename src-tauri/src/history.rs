//! The undo history: the changes made to the cells since the universe last
//! advanced, newest last, and the steps undone since the last change.

use std::collections::{HashMap, VecDeque};

use glife_engine::{Capture, CellState, Universe};

/// The history keeps at most this many undo steps.
const LIMIT: usize = 50;

/// One undo step: the cells it changed as they were before and after, and
/// the generation and revision before and after.
pub struct Entry {
    pub before: Cells,
    pub after: Cells,
    pub generation: [u64; 2],
    pub revision: [u64; 2],
}

/// The cells one step changed: whole rectangles, or the cells a stroke
/// touched, one each.
pub enum Cells {
    Rects(Capture),
    Each(Vec<CellState>),
}

impl Cells {
    /// Puts the recorded cells back; their trail is set to 0.
    pub fn restore(&self, universe: &mut Universe) {
        match self {
            Cells::Rects(capture) => universe.restore(capture),
            Cells::Each(cells) => cells.iter().for_each(|&cell| universe.restore_cell(cell)),
        }
    }

    #[cfg(test)]
    fn bytes(&self) -> usize {
        match self {
            Cells::Rects(capture) => capture.bytes(),
            Cells::Each(cells) => cells.capacity() * std::mem::size_of::<CellState>(),
        }
    }
}

#[derive(Default)]
pub struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    /// The stroke the newest entry still collects, with the index in its
    /// `Each` lists of every cell it holds.
    stroke: Option<(u64, HashMap<(u32, u32), usize>)>,
}

impl History {
    /// Records a new step, discarding what could have been redone and the
    /// oldest step past the limit.
    pub fn push(&mut self, entry: Entry) {
        self.stroke = None;
        self.redo.clear();
        if self.undo.len() == LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(entry);
    }

    /// The id of the stroke the next `edit` may continue.
    pub fn stroke(&self) -> Option<u64> {
        self.stroke.as_ref().map(|(id, _)| *id)
    }

    /// Ends the open stroke: the next `edit` starts a new step.
    pub fn end_stroke(&mut self) {
        self.stroke = None;
    }

    /// Records one batch of edited `cells`, with each one's state `before`
    /// and `after` the batch. A batch of the open stroke joins its step:
    /// a cell keeps the state from before the stroke first touched it and
    /// takes its latest state after. Any other batch is a new step, and one
    /// with a stroke id opens that stroke.
    pub fn record_cells(
        &mut self,
        stroke: Option<u64>,
        cells: &[(u32, u32)],
        before: Vec<CellState>,
        after: Vec<CellState>,
        generation: [u64; 2],
        revision: [u64; 2],
    ) {
        if stroke.is_none() || stroke != self.stroke() {
            self.push(Entry {
                before: Cells::Each(Vec::new()),
                after: Cells::Each(Vec::new()),
                generation,
                revision,
            });
            self.stroke = stroke.map(|id| (id, HashMap::new()));
        }
        let entry = self.undo.back_mut().expect("an entry was just pushed or is open");
        entry.revision[1] = revision[1];
        let (Cells::Each(first), Cells::Each(latest)) = (&mut entry.before, &mut entry.after) else {
            unreachable!("a stroke entry holds single cells");
        };
        let mut no_stroke = HashMap::new();
        let index = self.stroke.as_mut().map_or(&mut no_stroke, |(_, index)| index);
        for ((&cell, b), a) in cells.iter().zip(before).zip(after) {
            match index.get(&cell) {
                Some(&i) => latest[i] = a,
                None => {
                    index.insert(cell, first.len());
                    first.push(b);
                    latest.push(a);
                }
            }
        }
    }

    /// Moves the newest step to the redo list and returns it.
    pub fn undo(&mut self) -> Option<&Entry> {
        self.stroke = None;
        self.redo.push(self.undo.pop_back()?);
        self.redo.last()
    }

    /// Moves the step undone last back to the undo list and returns it.
    pub fn redo(&mut self) -> Option<&Entry> {
        self.stroke = None;
        self.undo.push_back(self.redo.pop()?);
        self.undo.back()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.stroke = None;
    }

    /// Memory the recorded cells hold, in bytes.
    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(|e| e.before.bytes() + e.after.bytes())
            .sum()
    }

    #[cfg(test)]
    pub fn len(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
}
