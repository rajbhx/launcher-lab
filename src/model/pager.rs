//! Home-screen pager math: drag tracking, edge clamp and velocity settle.

#[derive(Clone, Debug)]
pub struct Pager {
    pub pages: usize,
    pub current: usize,
    /// Extra camera offset in px on top of the page alignment.
    /// Positive = dragging towards the next page (finger moved left).
    pub offset: f32,
}

impl Pager {
    pub fn new(pages: usize) -> Self {
        Self {
            pages: pages.max(1),
            current: 0,
            offset: 0.0,
        }
    }

    /// Apply a per-frame finger delta in px.
    ///
    /// Dragging left (negative `dx`) moves towards the next page; the offset
    /// is clamped so the first/last page cannot be dragged past.
    pub fn drag(&mut self, dx: f32, w: f32) {
        let lower = if self.current > 0 { -w } else { 0.0 };
        let upper = if self.current + 1 < self.pages { w } else { 0.0 };
        self.offset = (self.offset - dx).clamp(lower, upper);
    }

    /// Snap after release, taking release velocity into account.
    /// Returns `true` when the page changed.
    pub fn settle(&mut self, vx: f32, w: f32) -> bool {
        let projected = self.offset - vx * 0.12;
        let threshold = w * 0.28;
        let target = if projected > threshold && self.current + 1 < self.pages {
            self.current + 1
        } else if projected < -threshold && self.current > 0 {
            self.current - 1
        } else {
            self.current
        };
        let changed = target != self.current;
        self.current = target;
        changed
    }

    /// Camera x position (content space) for the current page + offset.
    pub fn camera_x(&self, w: f32) -> f32 {
        self.current as f32 * w + self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drags_and_clamps_on_first_page() {
        let mut p = Pager::new(3);
        p.drag(120.0, 1000.0); // drag right on the first page: no previous page
        assert_eq!(p.offset, 0.0);
        p.drag(-320.0, 1000.0); // drag left towards the next page
        assert!((p.offset - 320.0).abs() < 0.01);
    }

    #[test]
    fn clamps_on_last_page() {
        let mut p = Pager::new(2);
        p.current = 1;
        p.drag(-500.0, 1000.0); // no next page
        assert_eq!(p.offset, 0.0);
        p.drag(150.0, 1000.0); // previous page exists
        assert!((p.offset + 150.0).abs() < 0.01);
    }

    #[test]
    fn settle_moves_to_next_page() {
        let mut p = Pager::new(3);
        p.drag(-400.0, 1000.0);
        assert!(p.settle(0.0, 1000.0));
        assert_eq!(p.current, 1);
    }

    #[test]
    fn settle_snaps_back_below_threshold() {
        let mut p = Pager::new(3);
        p.drag(-100.0, 1000.0);
        assert!(!p.settle(0.0, 1000.0));
        assert_eq!(p.current, 0);
    }

    #[test]
    fn velocity_flick_advances_and_last_page_clamps() {
        let mut p = Pager::new(3);
        p.drag(-60.0, 1000.0);
        assert!(p.settle(-2000.0, 1000.0));
        assert_eq!(p.current, 1);

        let mut q = Pager::new(1);
        q.drag(-400.0, 1000.0);
        assert!(!q.settle(-2000.0, 1000.0));
        assert_eq!(q.current, 0);
    }

    #[test]
    fn camera_follows_page_and_offset() {
        let mut p = Pager::new(3);
        p.current = 2;
        p.offset = -50.0;
        assert!((p.camera_x(500.0) - 950.0).abs() < 0.01);
    }
}
