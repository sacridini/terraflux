//! The walk over objects one by one, each in its own box: the frame of an object (its bounding box with a pad of one
//! cell, so that a distance or a flood sees the cells around it) and [`each`], which runs a function over some objects
//! in parallel, the biggest boxes first so that the threads end together, and returns the results in the order of the
//! ids: the same on any number of threads.

use rayon::prelude::*;

use super::Objects;

/// The box of an object with a pad of one cell all round. A cell of the frame is at `r * w + c`; the grid's cell at
/// (`top + r`, `left + c`), which is off the grid in the pad where the box touches the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Frame {
    pub(super) top: i64,
    pub(super) left: i64,
    pub(super) w: usize,
    pub(super) h: usize,
}

impl Frame {
    /// The frame of the object with this id (1..=count).
    pub(super) fn of(o: &Objects, id: u32) -> Frame {
        let [r0, c0, r1, c1] = o.boxes[id as usize - 1];
        Frame { top: r0 as i64 - 1, left: c0 as i64 - 1, w: (c1 - c0) as usize + 3, h: (r1 - r0) as usize + 3 }
    }

    /// The cells of the frame.
    pub(super) fn cells(&self) -> usize {
        self.w * self.h
    }

    /// The index in the grid of a cell of the frame: none off the grid.
    pub(super) fn global(&self, o: &Objects, i: usize) -> Option<usize> {
        self.index(o.grid.width, o.grid.height, i)
    }

    /// [`Frame::global`] for a grid of this width and height, when the labels are not an `Objects`'.
    pub(super) fn index(&self, width: usize, height: usize, i: usize) -> Option<usize> {
        let (r, c) = (self.top + (i / self.w) as i64, self.left + (i % self.w) as i64);
        (r >= 0 && c >= 0 && (r as usize) < height && (c as usize) < width).then(|| r as usize * width + c as usize)
    }

    /// The cells of the object in the frame (false in the pad and on the cells of other objects).
    pub(super) fn mask(&self, o: &Objects, id: u32) -> Vec<bool> {
        (0..self.cells()).map(|i| self.global(o, i).is_some_and(|g| o.labels[g] == id)).collect()
    }
}

/// `f(id, frame)` for each of the ids (objects, ascending), in parallel and the biggest boxes first; the results in
/// the order of `ids`.
pub(super) fn each<T: Send>(o: &Objects, ids: &[u32], f: impl Fn(u32, &Frame) -> T + Sync) -> Vec<T> {
    let frames: Vec<Frame> = ids.iter().map(|&id| Frame::of(o, id)).collect();
    each_with(&frames, |k, fr| f(ids[k], fr))
}

/// `f(k, frame)` for each of the frames, in parallel and the biggest first; the results in the order of `frames`.
/// For boxes that do not come from the objects' own (a label array that changed).
pub(super) fn each_with<T: Send>(frames: &[Frame], f: impl Fn(usize, &Frame) -> T + Sync) -> Vec<T> {
    let mut order: Vec<usize> = (0..frames.len()).collect();
    // (a stable sort: equal boxes stay in their order)
    order.sort_by_key(|&k| std::cmp::Reverse(frames[k].cells()));
    let done: Vec<(usize, T)> = order.par_iter().map(|&k| (k, f(k, &frames[k]))).collect();
    let mut out: Vec<Option<T>> = (0..frames.len()).map(|_| None).collect();
    for (k, t) in done {
        out[k] = Some(t);
    }
    out.into_iter().map(|t| t.expect("each object ran")).collect()
}
