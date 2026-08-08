use crate::rule::Rule;

/// Toroidal cellular-automaton grid with double buffering and cell ages.
#[derive(Clone, Debug)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
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
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
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

    /// Advance one generation under Conway's rules (B3/S23).
    #[inline]
    #[allow(dead_code)] // used by tests; convenience over step_with
    pub fn step(&mut self) -> StepStats {
        self.step_with(&Rule::CONWAY)
    }

    /// Advance one generation under the given Life-like rule.
    ///
    /// Ages: dead → 0; newly born → 1; surviving live cells age +1 (saturates at u16::MAX).
    pub fn step_with(&mut self, rule: &Rule) -> StepStats {
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
                        // survive — age increments
                        self.next_ages[i] = self.ages[i].saturating_add(1);
                    } else {
                        // birth
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
        // dead cells stay 0
        assert_eq!(g.age_at(0, 0), 0);
    }

    #[test]
    fn age_newborn_is_one() {
        // blinker: corners die, new cells born at vertical positions
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        g.step();
        // vertical blinker at (2,1),(2,2),(2,3) — (2,1) and (2,3) are newborns
        assert!(g.get(2, 1));
        assert_eq!(g.age_at(2, 1), 1);
        assert_eq!(g.age_at(2, 3), 1);
        // center survived
        assert_eq!(g.age_at(2, 2), 2);
    }

    #[test]
    fn highlife_births_on_6() {
        // Construct a dead cell with exactly 6 live neighbors.
        // Neighbors of (2,2): all around except leave two empty.
        // Fill: (1,1)(2,1)(3,1)(1,2)(3,2)(1,3) = 6 neighbors, (2,2) dead.
        let mut g = Grid::new(5, 5);
        for &(x, y) in &[(1, 1), (2, 1), (3, 1), (1, 2), (3, 2), (1, 3)] {
            g.set(x, y, true);
        }
        assert!(!g.get(2, 2));
        assert_eq!(g.count_neighbors(2, 2), 6);

        // Conway: no birth on 6
        let mut gc = g.clone();
        gc.step_with(&Rule::CONWAY);
        assert!(!gc.get(2, 2));

        // HighLife: birth on 6
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
        // Single seed: B2 births only; a lone cell dies, pairs explode.
        let mut g = Grid::new(6, 6);
        // Two adjacent cells each have 1 neighbor — neither survives, no birth.
        g.set(2, 2, true);
        g.set(3, 2, true);
        g.step_with(&Rule::SEEDS);
        // Under Seeds, live cells never survive; births only on exactly 2.
        // Each of the two had 1 neighbor → die. Positions with 2 neighbors of the pair:
        // (2,1),(3,1),(2,3),(3,3) each see both → birth.
        assert!(!g.get(2, 2));
        assert!(!g.get(3, 2));
        assert!(g.get(2, 1));
        assert!(g.get(3, 1));
        assert!(g.get(2, 3));
        assert!(g.get(3, 3));
    }
}
