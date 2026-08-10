use crate::rule::Rule;

/// Threshold: grids with this many cells (or more) use multi-threaded step.
pub const PARALLEL_THRESHOLD: usize = 20_000;

/// Cell state constants for multi-state automata.
pub mod state {
    /// Dead / empty.
    pub const DEAD: u8 = 0;
    /// Live (binary) or firing (Brian's Brain).
    pub const LIVE: u8 = 1;
    /// Refractory (Brian's Brain only).
    pub const REFRACTORY: u8 = 2;
}

/// Cellular-automaton grid with double buffering, cell ages, and topology.
///
/// Cells are `u8` states: for binary Life-like rules, `0` = dead and `1` = live.
/// Multi-state rules (e.g. Brian's Brain) use more values; see [`state`].
#[derive(Clone, Debug)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    /// When `true` (default), edges wrap toroidally; when `false`, out-of-bounds
    /// neighbors count as dead.
    pub wrap: bool,
    /// Maximum number of distinct cell states (2 = binary, 3 = Brian's Brain).
    pub states: u8,
    /// Cell values: `0` = dead; non-zero meaning depends on automaton.
    pub cells: Vec<u8>,
    /// Age of each cell: 0 = dead, 1 = newborn, increases each step while non-zero.
    pub ages: Vec<u16>,
    next: Vec<u8>,
    next_ages: Vec<u16>,
}

/// Births and deaths produced by one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StepStats {
    pub births: usize,
    pub deaths: usize,
}

impl Grid {
    /// Create a toroidal (wrap-around) binary grid.
    pub fn new(w: usize, h: usize) -> Self {
        Self::with_wrap(w, h, true)
    }

    /// Create a binary grid with explicit topology.
    pub fn with_wrap(w: usize, h: usize, wrap: bool) -> Self {
        Self::with_states(w, h, wrap, 2)
    }

    /// Create a grid with explicit topology and max state count.
    pub fn with_states(w: usize, h: usize, wrap: bool, states: u8) -> Self {
        let states = states.max(2);
        Self {
            w,
            h,
            wrap,
            states,
            cells: vec![0; w * h],
            ages: vec![0; w * h],
            next: vec![0; w * h],
            next_ages: vec![0; w * h],
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    /// True if cell is non-dead (any non-zero state).
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.get_state(x, y) != 0
    }

    /// Raw cell state at (x, y), or 0 if out of bounds.
    pub fn get_state(&self, x: usize, y: usize) -> u8 {
        if x < self.w && y < self.h {
            self.cells[self.idx(x, y)]
        } else {
            0
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

    /// Set binary live/dead (state 0 or 1).
    pub fn set(&mut self, x: usize, y: usize, v: bool) {
        self.set_state(x, y, if v { state::LIVE } else { state::DEAD });
    }

    /// Set an arbitrary cell state (clamped to `states - 1`).
    pub fn set_state(&mut self, x: usize, y: usize, v: u8) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            let max = self.states.saturating_sub(1);
            let v = v.min(max);
            self.cells[i] = v;
            self.ages[i] = if v != 0 { 1 } else { 0 };
        }
    }

    pub fn clear(&mut self) {
        self.cells.fill(0);
        self.ages.fill(0);
    }

    /// Count non-zero neighbors (Life-like).
    pub fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        count_neighbors_slice(&self.cells, self.w, self.h, self.wrap, x, y, None)
    }

    /// Count neighbors whose state equals `match_state` (e.g. firing = 1).
    pub fn count_neighbors_state(&self, x: usize, y: usize, match_state: u8) -> u8 {
        count_neighbors_slice(
            &self.cells,
            self.w,
            self.h,
            self.wrap,
            x,
            y,
            Some(match_state),
        )
    }

    /// Advance one generation under Conway's rules (B3/S23).
    #[inline]
    #[allow(dead_code)] // used by tests; convenience over step_with
    pub fn step(&mut self) -> StepStats {
        self.step_with(&Rule::CONWAY)
    }

    /// Advance one generation under the given Life-like rule (binary).
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

    /// Force sequential Life-like step (for tests / comparison).
    pub fn step_with_sequential(&mut self, rule: &Rule) -> StepStats {
        let mut births = 0usize;
        let mut deaths = 0usize;
        for y in 0..self.h {
            for x in 0..self.w {
                let n = self.count_neighbors(x, y);
                let i = self.idx(x, y);
                let alive = self.cells[i] != 0;
                let next_alive = rule.next_alive(alive, n);
                if next_alive {
                    self.next[i] = state::LIVE;
                    if alive {
                        self.next_ages[i] = self.ages[i].saturating_add(1);
                    } else {
                        self.next_ages[i] = 1;
                        births += 1;
                    }
                } else {
                    self.next[i] = state::DEAD;
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

        let mut next = std::mem::take(&mut self.next);
        let mut next_ages = std::mem::take(&mut self.next_ages);
        if next.len() != w * h {
            next.resize(w * h, 0);
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
                            let n = count_neighbors_slice(cells, w, h, wrap, x, y, None);
                            let gi = y * w + x;
                            let li = ly * w + x;
                            let alive = cells[gi] != 0;
                            let next_alive = rule.next_alive(alive, n);
                            if next_alive {
                                nchunk[li] = state::LIVE;
                                if alive {
                                    achunk[li] = ages[gi].saturating_add(1);
                                } else {
                                    achunk[li] = 1;
                                    births += 1;
                                }
                            } else {
                                nchunk[li] = state::DEAD;
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

        std::mem::swap(&mut self.cells, &mut next);
        std::mem::swap(&mut self.ages, &mut next_ages);
        self.next = next;
        self.next_ages = next_ages;
        stats
    }

    /// Brian's Brain step: 0=dead, 1=firing, 2=refractory.
    ///
    /// - dead → firing if exactly 2 firing neighbors
    /// - firing → refractory
    /// - refractory → dead
    ///
    /// Births = dead→firing; deaths = non-zero→dead (refractory dying).
    pub fn step_brians_brain(&mut self) -> StepStats {
        self.states = self.states.max(3);
        if self.w * self.h >= PARALLEL_THRESHOLD && self.h >= 2 {
            self.step_brians_brain_parallel()
        } else {
            self.step_brians_brain_sequential()
        }
    }

    fn step_brians_brain_sequential(&mut self) -> StepStats {
        let mut births = 0usize;
        let mut deaths = 0usize;
        for y in 0..self.h {
            for x in 0..self.w {
                let i = self.idx(x, y);
                let cur = self.cells[i];
                let (next, birth, death) = brians_next(
                    cur,
                    count_neighbors_slice(
                        &self.cells,
                        self.w,
                        self.h,
                        self.wrap,
                        x,
                        y,
                        Some(state::LIVE),
                    ),
                    self.ages[i],
                );
                self.next[i] = next.0;
                self.next_ages[i] = next.1;
                births += birth;
                deaths += death;
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        std::mem::swap(&mut self.ages, &mut self.next_ages);
        StepStats { births, deaths }
    }

    fn step_brians_brain_parallel(&mut self) -> StepStats {
        let w = self.w;
        let h = self.h;
        let wrap = self.wrap;

        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, h);
        let chunk_h = (h + n_threads - 1) / n_threads;

        let mut next = std::mem::take(&mut self.next);
        let mut next_ages = std::mem::take(&mut self.next_ages);
        if next.len() != w * h {
            next.resize(w * h, 0);
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
                            let gi = y * w + x;
                            let li = ly * w + x;
                            let n =
                                count_neighbors_slice(cells, w, h, wrap, x, y, Some(state::LIVE));
                            let (next, birth, death) = brians_next(cells[gi], n, ages[gi]);
                            nchunk[li] = next.0;
                            achunk[li] = next.1;
                            births += birth;
                            deaths += death;
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

        std::mem::swap(&mut self.cells, &mut next);
        std::mem::swap(&mut self.ages, &mut next_ages);
        self.next = next;
        self.next_ages = next_ages;
        stats
    }

    /// After bulk-filling `cells` (e.g. random seed), sync ages so non-zero cells are age 1.
    pub fn sync_ages_from_cells(&mut self) {
        for i in 0..self.cells.len() {
            self.ages[i] = if self.cells[i] != 0 { 1 } else { 0 };
        }
    }

    /// Population = number of non-dead cells.
    pub fn population(&self) -> usize {
        self.cells.iter().filter(|c| **c != 0).count()
    }

    /// Count cells in a specific state.
    pub fn count_state(&self, s: u8) -> usize {
        self.cells.iter().filter(|c| **c == s).count()
    }

    /// Maximum age among non-dead cells (0 if empty).
    pub fn max_age(&self) -> u16 {
        self.ages.iter().copied().max().unwrap_or(0)
    }

    /// True if every cell matches `other` (same dimensions required). Ages ignored.
    pub fn equals(&self, other: &Grid) -> bool {
        self.w == other.w && self.h == other.h && self.cells == other.cells
    }

    /// Bounding box of non-dead cells: (min_x, min_y, max_x, max_y), or None if empty.
    pub fn live_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let mut min_x = self.w;
        let mut min_y = self.h;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut any = false;
        for y in 0..self.h {
            for x in 0..self.w {
                if self.cells[self.idx(x, y)] != 0 {
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

/// `(next_state, next_age)`, birth flag (0/1), death flag (0/1).
#[inline]
fn brians_next(cur: u8, firing_neighbors: u8, age: u16) -> ((u8, u16), usize, usize) {
    match cur {
        state::DEAD => {
            if firing_neighbors == 2 {
                ((state::LIVE, 1), 1, 0)
            } else {
                ((state::DEAD, 0), 0, 0)
            }
        }
        state::LIVE => ((state::REFRACTORY, 1), 0, 0),
        state::REFRACTORY => ((state::DEAD, 0), 0, 1),
        _ => {
            // Unknown: treat as dead
            let _ = age;
            ((state::DEAD, 0), 0, 0)
        }
    }
}

/// Neighbor count against a cell slice.
///
/// If `match_state` is `None`, counts any non-zero cell; otherwise counts cells
/// equal to that state (used for Brian's Brain firing neighbors).
#[inline]
pub fn count_neighbors_slice(
    cells: &[u8],
    w: usize,
    h: usize,
    wrap: bool,
    x: usize,
    y: usize,
    match_state: Option<u8>,
) -> u8 {
    let mut n = 0u8;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            let cell = if wrap {
                let nx = ((nx % w as i32 + w as i32) % w as i32) as usize;
                let ny = ((ny % h as i32 + h as i32) % h as i32) as usize;
                cells[ny * w + nx]
            } else if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                cells[ny as usize * w + nx as usize]
            } else {
                continue;
            };
            let hit = match match_state {
                Some(s) => cell == s,
                None => cell != 0,
            };
            if hit {
                n += 1;
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
    use crate::rng::Rng;
    use crate::rule::Rule;

    #[test]
    fn blinker_oscillates() {
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        g.step();
        assert!(!g.get(1, 2));
        assert!(!g.get(3, 2));
        assert!(g.get(2, 1));
        assert!(g.get(2, 2));
        assert!(g.get(2, 3));
        g.step();
        assert!(g.get(1, 2));
        assert!(g.get(2, 2));
        assert!(g.get(3, 2));
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
        assert_eq!(tor.count_neighbors(2, 2), 1);
        assert_eq!(fin.count_neighbors(2, 2), 0);
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
        let w = 200;
        let h = 100;
        let mut rng = Rng::new(0xBEEF_CAFE);
        let mut seq = Grid::new(w, h);
        for c in seq.cells.iter_mut() {
            *c = if rng.next_f64() < 0.3 { 1 } else { 0 };
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
        let h = 130;
        let mut rng = Rng::new(99);
        let mut seq = Grid::with_wrap(w, h, false);
        for c in seq.cells.iter_mut() {
            *c = if rng.next_f64() < 0.25 { 1 } else { 0 };
        }
        seq.sync_ages_from_cells();
        let mut par = seq.clone();
        for _ in 0..3 {
            seq.step_with_sequential(&Rule::HIGHLIFE);
            par.step_with_parallel(&Rule::HIGHLIFE);
            assert_eq!(seq.cells, par.cells);
        }
    }

    #[test]
    fn brians_brain_firing_to_refractory_to_dead() {
        let mut g = Grid::with_states(5, 5, true, 3);
        // Two firing neighbors for center dead cell → birth
        g.set_state(1, 2, state::LIVE);
        g.set_state(3, 2, state::LIVE);
        // Center is dead with exactly 2 firing neighbors
        assert_eq!(g.get_state(2, 2), 0);
        assert_eq!(g.count_neighbors_state(2, 2, state::LIVE), 2);
        g.step_brians_brain();
        assert_eq!(g.get_state(2, 2), state::LIVE); // born firing
                                                    // Original firing cells become refractory
        assert_eq!(g.get_state(1, 2), state::REFRACTORY);
        assert_eq!(g.get_state(3, 2), state::REFRACTORY);
        g.step_brians_brain();
        // Refractory → dead
        assert_eq!(g.get_state(1, 2), state::DEAD);
        assert_eq!(g.get_state(3, 2), state::DEAD);
        // Center firing → refractory
        assert_eq!(g.get_state(2, 2), state::REFRACTORY);
        g.step_brians_brain();
        assert_eq!(g.get_state(2, 2), state::DEAD);
    }

    #[test]
    fn brians_brain_no_birth_with_one_neighbor() {
        let mut g = Grid::with_states(5, 5, true, 3);
        g.set_state(2, 2, state::LIVE);
        g.step_brians_brain();
        // No new firings from lonely cell
        assert_eq!(g.count_state(state::LIVE), 0);
        assert_eq!(g.get_state(2, 2), state::REFRACTORY);
    }

    #[test]
    fn brians_brain_parallel_matches_seq() {
        let w = 200;
        let h = 100;
        let mut rng = Rng::new(0xBB);
        let mut seq = Grid::with_states(w, h, true, 3);
        for c in seq.cells.iter_mut() {
            let r = rng.next_f64();
            *c = if r < 0.15 {
                1
            } else if r < 0.25 {
                2
            } else {
                0
            };
        }
        seq.sync_ages_from_cells();
        let mut par = seq.clone();
        for _ in 0..4 {
            let s1 = seq.step_brians_brain_sequential();
            let s2 = par.step_brians_brain_parallel();
            assert_eq!(s1, s2);
            assert_eq!(seq.cells, par.cells);
        }
    }
}
