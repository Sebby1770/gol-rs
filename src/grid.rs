use crate::rule::Rule;

/// Threshold: grids with this many cells (or more) use multi-threaded step.
pub const PARALLEL_THRESHOLD: usize = 20_000;

/// Cellular-automaton grid with double buffering, cell ages, and topology.
#[derive(Clone, Debug)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    /// When `true` (default), edges wrap toroidally; when `false`, out-of-bounds
    /// neighbors count as dead.
    pub wrap: bool,
    pub cells: Vec<bool>,
    /// Age of each cell: 0 = dead, 1 = newborn, increases each step while alive.
    pub ages: Vec<u16>,
    next: Vec<bool>,
    next_ages: Vec<u16>,
}

/// Births and deaths produced by one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StepStats {
    pub births: usize,
    pub deaths: usize,
}

impl Grid {
    /// Create a toroidal (wrap-around) grid.
    pub fn new(w: usize, h: usize) -> Self {
        Self::with_wrap(w, h, true)
    }

    /// Create a grid with explicit topology.
    pub fn with_wrap(w: usize, h: usize, wrap: bool) -> Self {
        Self {
            w,
            h,
            wrap,
            cells: vec![false; w * h],
            ages: vec![0; w * h],
            next: vec![false; w * h],
            next_ages: vec![0; w * h],
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

    /// Age of cell at (x, y), or 0 if out of bounds / dead.
    #[allow(dead_code)] // used by tests; public grid API
    pub fn age_at(&self, x: usize, y: usize) -> u16 {
        if x < self.w && y < self.h {
            self.ages[self.idx(x, y)]
        } else {
            0
        }
    }

    pub fn set(&mut self, x: usize, y: usize, v: bool) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            self.cells[i] = v;
            // Newborn if setting live; dead age 0.
            self.ages[i] = if v { 1 } else { 0 };
        }
    }

    pub fn clear(&mut self) {
        self.cells.fill(false);
        self.ages.fill(0);
    }

    pub fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        count_neighbors_slice(&self.cells, self.w, self.h, self.wrap, x, y)
    }

    /// Advance one generation under Conway's rules (B3/S23).
    #[inline]
    #[allow(dead_code)] // used by tests; convenience over step_with
    pub fn step(&mut self) -> StepStats {
        self.step_with(&Rule::CONWAY)
    }

    /// Advance one generation under the given Life-like rule.
    ///
    /// Ages: dead → 0; newly born → 1; surviving live cells age +1 (saturates at u16::MAX).
    /// Large grids (≥ [`PARALLEL_THRESHOLD`] cells) use a multi-threaded fill.
    pub fn step_with(&mut self, rule: &Rule) -> StepStats {
        if self.w * self.h >= PARALLEL_THRESHOLD && self.h >= 2 {
            self.step_with_parallel(rule)
        } else {
            self.step_with_sequential(rule)
        }
    }

    /// Force sequential step (for tests / comparison).
    pub fn step_with_sequential(&mut self, rule: &Rule) -> StepStats {
        let mut births = 0usize;
        let mut deaths = 0usize;
        for y in 0..self.h {
            for x in 0..self.w {
                let n = self.count_neighbors(x, y);
                let i = self.idx(x, y);
                let alive = self.cells[i];
                let next_alive = rule.next_alive(alive, n);
                self.next[i] = next_alive;
                if next_alive {
                    if alive {
                        self.next_ages[i] = self.ages[i].saturating_add(1);
                    } else {
                        self.next_ages[i] = 1;
                        births += 1;
                    }
                } else {
                    self.next_ages[i] = 0;
                    if alive {
                        deaths += 1;
                    }
                }
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        std::mem::swap(&mut self.ages, &mut self.next_ages);
        StepStats { births, deaths }
    }

    /// Multi-threaded next-buffer fill via `std::thread::scope`, then swap.
    fn step_with_parallel(&mut self, rule: &Rule) -> StepStats {
        let w = self.w;
        let h = self.h;
        let wrap = self.wrap;

        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, h);
        let chunk_h = (h + n_threads - 1) / n_threads;

        // Take next buffers so we can split them while reading cells/ages.
        let mut next = std::mem::take(&mut self.next);
        let mut next_ages = std::mem::take(&mut self.next_ages);
        if next.len() != w * h {
            next.resize(w * h, false);
            next_ages.resize(w * h, 0);
        }

        let cells = self.cells.as_slice();
        let ages = self.ages.as_slice();

        let stats = std::thread::scope(|scope| {
            let mut next_rest = next.as_mut_slice();
            let mut ages_rest = next_ages.as_mut_slice();
            let mut handles = Vec::with_capacity(n_threads);

            let mut y0 = 0usize;
            while y0 < h {
                let y1 = (y0 + chunk_h).min(h);
                let nrows = y1 - y0;
                let (nchunk, nrest) = next_rest.split_at_mut(nrows * w);
                let (achunk, arest) = ages_rest.split_at_mut(nrows * w);
                next_rest = nrest;
                ages_rest = arest;

                let handle = scope.spawn(move || {
                    let mut births = 0usize;
                    let mut deaths = 0usize;
                    for ly in 0..nrows {
                        let y = y0 + ly;
                        for x in 0..w {
                            let n = count_neighbors_slice(cells, w, h, wrap, x, y);
                            let gi = y * w + x;
                            let li = ly * w + x;
                            let alive = cells[gi];
                            let next_alive = rule.next_alive(alive, n);
                            nchunk[li] = next_alive;
                            if next_alive {
                                if alive {
                                    achunk[li] = ages[gi].saturating_add(1);
                                } else {
                                    achunk[li] = 1;
                                    births += 1;
                                }
                            } else {
                                achunk[li] = 0;
                                if alive {
                                    deaths += 1;
                                }
                            }
                        }
                    }
                    StepStats { births, deaths }
                });
                handles.push(handle);
                y0 = y1;
            }

            let mut total = StepStats::default();
            for handle in handles {
                let s = handle.join().expect("worker panicked");
                total.births += s.births;
                total.deaths += s.deaths;
            }
            total
        });

        // next holds new state; swap into cells, keep old as buffer.
        std::mem::swap(&mut self.cells, &mut next);
        std::mem::swap(&mut self.ages, &mut next_ages);
        self.next = next;
        self.next_ages = next_ages;
        stats
    }

    /// After bulk-filling `cells` (e.g. random seed), sync ages so live cells are age 1.
    pub fn sync_ages_from_cells(&mut self) {
        for i in 0..self.cells.len() {
            self.ages[i] = if self.cells[i] { 1 } else { 0 };
        }
    }

    pub fn population(&self) -> usize {
        self.cells.iter().filter(|c| **c).count()
    }

    /// Maximum age among live cells (0 if empty).
    pub fn max_age(&self) -> u16 {
        self.ages.iter().copied().max().unwrap_or(0)
    }

    /// True if every cell matches `other` (same dimensions required). Ages ignored.
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

/// Neighbor count against a cell slice (shared by sequential + parallel paths).
#[inline]
pub fn count_neighbors_slice(cells: &[bool], w: usize, h: usize, wrap: bool, x: usize, y: usize) -> u8 {
    let mut n = 0u8;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if wrap {
                let nx = ((nx % w as i32 + w as i32) % w as i32) as usize;
                let ny = ((ny % h as i32 + h as i32) % h as i32) as usize;
                if cells[ny * w + nx] {
                    n += 1;
                }
            } else if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                if cells[ny as usize * w + nx as usize] {
                    n += 1;
                }
            }
        }
    }
    n
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
    use crate::rule::Rule;
    use crate::rng::Rng;

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
    fn finite_corner_has_no_wrap_neighbor() {
        // 3×3 finite: live cell at (0,0) is not a neighbor of (2,2).
        let mut g = Grid::with_wrap(3, 3, false);
        g.set(0, 0, true);
        assert_eq!(g.count_neighbors(2, 2), 0);
        assert_eq!(g.count_neighbors(0, 1), 1);
        assert_eq!(g.count_neighbors(1, 0), 1);
        assert_eq!(g.count_neighbors(1, 1), 1);
    }

    #[test]
    fn finite_vs_toroidal_corner_3x3() {
        let mut tor = Grid::with_wrap(3, 3, true);
        let mut fin = Grid::with_wrap(3, 3, false);
        tor.set(0, 0, true);
        fin.set(0, 0, true);
        // Corner (2,2): toroidal sees (0,0) via wrap; finite does not.
        assert_eq!(tor.count_neighbors(2, 2), 1);
        assert_eq!(fin.count_neighbors(2, 2), 0);
        // Center always sees the corner cell.
        assert_eq!(tor.count_neighbors(1, 1), 1);
        assert_eq!(fin.count_neighbors(1, 1), 1);
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
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        let stats = g.step();
        assert_eq!(stats.births, 2);
        assert_eq!(stats.deaths, 2);
        assert_eq!(g.population(), 3);
    }

    #[test]
    fn age_increments_for_still_life() {
        let mut g = Grid::new(4, 4);
        g.set(1, 1, true);
        g.set(2, 1, true);
        g.set(1, 2, true);
        g.set(2, 2, true);
        assert_eq!(g.age_at(1, 1), 1);
        g.step();
        assert_eq!(g.age_at(1, 1), 2);
        g.step();
        assert_eq!(g.age_at(1, 1), 3);
        assert_eq!(g.age_at(2, 2), 3);
        assert_eq!(g.age_at(0, 0), 0);
    }

    #[test]
    fn age_newborn_is_one() {
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        g.step();
        assert!(g.get(2, 1));
        assert_eq!(g.age_at(2, 1), 1);
        assert_eq!(g.age_at(2, 3), 1);
        assert_eq!(g.age_at(2, 2), 2);
    }

    #[test]
    fn highlife_births_on_6() {
        let mut g = Grid::new(5, 5);
        for &(x, y) in &[(1, 1), (2, 1), (3, 1), (1, 2), (3, 2), (1, 3)] {
            g.set(x, y, true);
        }
        assert!(!g.get(2, 2));
        assert_eq!(g.count_neighbors(2, 2), 6);

        let mut gc = g.clone();
        gc.step_with(&Rule::CONWAY);
        assert!(!gc.get(2, 2));

        g.step_with(&Rule::HIGHLIFE);
        assert!(g.get(2, 2));
        assert_eq!(g.age_at(2, 2), 1);
    }

    #[test]
    fn rule_conway_blinker_still_works() {
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        let rule = Rule::parse("conway").unwrap();
        g.step_with(&rule);
        assert!(g.get(2, 1) && g.get(2, 2) && g.get(2, 3));
        g.step_with(&rule);
        assert!(g.get(1, 2) && g.get(2, 2) && g.get(3, 2));
    }

    #[test]
    fn seeds_no_survival() {
        let mut g = Grid::new(6, 6);
        g.set(2, 2, true);
        g.set(3, 2, true);
        g.step_with(&Rule::SEEDS);
        assert!(!g.get(2, 2));
        assert!(!g.get(3, 2));
        assert!(g.get(2, 1));
        assert!(g.get(3, 1));
        assert!(g.get(2, 3));
        assert!(g.get(3, 3));
    }

    #[test]
    fn parallel_matches_sequential() {
        // Force large enough for parallel path: 200×100 = 20_000
        let w = 200;
        let h = 100;
        let mut rng = Rng::new(0xBEEF_CAFE);
        let mut seq = Grid::new(w, h);
        for c in seq.cells.iter_mut() {
            *c = rng.next_f64() < 0.3;
        }
        seq.sync_ages_from_cells();
        let mut par = seq.clone();

        for _ in 0..5 {
            let s1 = seq.step_with_sequential(&Rule::CONWAY);
            let s2 = par.step_with_parallel(&Rule::CONWAY);
            assert_eq!(s1, s2, "step stats differ");
            assert_eq!(seq.cells, par.cells, "cells differ");
            assert_eq!(seq.ages, par.ages, "ages differ");
        }
    }

    #[test]
    fn parallel_matches_sequential_finite() {
        let w = 160;
        let h = 130; // 20_800
        let mut rng = Rng::new(99);
        let mut seq = Grid::with_wrap(w, h, false);
        for c in seq.cells.iter_mut() {
            *c = rng.next_f64() < 0.25;
        }
        seq.sync_ages_from_cells();
        let mut par = seq.clone();
        for _ in 0..3 {
            seq.step_with_sequential(&Rule::HIGHLIFE);
            par.step_with_parallel(&Rule::HIGHLIFE);
            assert_eq!(seq.cells, par.cells);
        }
    }
}
