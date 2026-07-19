/// Toroidal Game of Life grid with double buffering.
#[derive(Clone, Debug)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub cells: Vec<bool>,
    next: Vec<bool>,
}

/// Births and deaths produced by one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StepStats {
    pub births: usize,
    pub deaths: usize,
}

impl Grid {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![false; w * h],
            next: vec![false; w * h],
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        if x < self.w && y < self.h {
            self.cells[self.idx(x, y)]
        } else {
            false
        }
    }

    pub fn set(&mut self, x: usize, y: usize, v: bool) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            self.cells[i] = v;
        }
    }

    pub fn clear(&mut self) {
        self.cells.fill(false);
    }

    pub fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        let mut n = 0u8;
        let xoffsets = [self.w - 1, 0, 1];
        let yoffsets = [self.h - 1, 0, 1];
        for &dy in &yoffsets {
            for &dx in &xoffsets {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = (x + dx) % self.w;
                let ny = (y + dy) % self.h;
                if self.cells[self.idx(nx, ny)] {
                    n += 1;
                }
            }
        }
        n
    }

    /// Advance one generation. Returns birth/death counts for this step.
    pub fn step(&mut self) -> StepStats {
        let mut births = 0usize;
        let mut deaths = 0usize;
        for y in 0..self.h {
            for x in 0..self.w {
                let n = self.count_neighbors(x, y);
                let i = self.idx(x, y);
                let alive = self.cells[i];
                let next_alive = matches!((alive, n), (true, 2) | (true, 3) | (false, 3));
                self.next[i] = next_alive;
                if !alive && next_alive {
                    births += 1;
                } else if alive && !next_alive {
                    deaths += 1;
                }
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        StepStats { births, deaths }
    }

    pub fn population(&self) -> usize {
        self.cells.iter().filter(|c| **c).count()
    }

    /// True if every cell matches `other` (same dimensions required).
    pub fn equals(&self, other: &Grid) -> bool {
        self.w == other.w && self.h == other.h && self.cells == other.cells
    }

    /// Bounding box of live cells: (min_x, min_y, max_x, max_y), or None if empty.
    pub fn live_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let mut min_x = self.w;
        let mut min_y = self.h;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut any = false;
        for y in 0..self.h {
            for x in 0..self.w {
                if self.cells[self.idx(x, y)] {
                    any = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        if any {
            Some((min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }
}

impl PartialEq for Grid {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Eq for Grid {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blinker_oscillates() {
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        g.step();
        assert!(!g.cells[g.idx(1, 2)]);
        assert!(!g.cells[g.idx(3, 2)]);
        assert!(g.cells[g.idx(2, 1)]);
        assert!(g.cells[g.idx(2, 2)]);
        assert!(g.cells[g.idx(2, 3)]);
        g.step();
        assert!(g.cells[g.idx(1, 2)]);
        assert!(g.cells[g.idx(2, 2)]);
        assert!(g.cells[g.idx(3, 2)]);
    }

    #[test]
    fn block_is_still_life() {
        let mut g = Grid::new(4, 4);
        g.set(1, 1, true);
        g.set(2, 1, true);
        g.set(1, 2, true);
        g.set(2, 2, true);
        let snapshot = g.cells.clone();
        let stats = g.step();
        assert_eq!(g.cells, snapshot);
        assert_eq!(stats.births, 0);
        assert_eq!(stats.deaths, 0);
    }

    #[test]
    fn beehive_is_still_life() {
        // Beehive:
        //  .oo.
        // o..o
        //  .oo.
        let mut g = Grid::new(6, 5);
        g.set(2, 1, true);
        g.set(3, 1, true);
        g.set(1, 2, true);
        g.set(4, 2, true);
        g.set(2, 3, true);
        g.set(3, 3, true);
        let before = g.clone();
        g.step();
        assert!(g.equals(&before));
    }

    #[test]
    fn toroidal_wrap_counts_corner_neighbor() {
        let mut g = Grid::new(5, 5);
        g.set(0, 0, true);
        assert_eq!(g.count_neighbors(4, 4), 1);
    }

    #[test]
    fn lonely_cell_dies() {
        let mut g = Grid::new(5, 5);
        g.set(2, 2, true);
        let stats = g.step();
        assert_eq!(g.population(), 0);
        assert_eq!(stats.deaths, 1);
        assert_eq!(stats.births, 0);
    }

    #[test]
    fn equality_detects_stable() {
        let mut g = Grid::new(4, 4);
        g.set(1, 1, true);
        g.set(2, 1, true);
        g.set(1, 2, true);
        g.set(2, 2, true);
        let prev = g.clone();
        g.step();
        assert_eq!(g, prev);
    }

    #[test]
    fn step_stats_births() {
        // Three cells in a row → blinker births two, kills two corners conceptually.
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        let stats = g.step();
        assert_eq!(stats.births, 2);
        assert_eq!(stats.deaths, 2);
        assert_eq!(g.population(), 3);
    }
}
