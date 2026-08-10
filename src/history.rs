use crate::grid::Grid;
use std::collections::HashMap;

/// FNV-1a 64-bit offset basis and prime.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

/// Hash grid cell states with FNV-1a (supports multi-state `u8` cells).
pub fn hash_grid(grid: &Grid) -> u64 {
    let mut h = FNV_OFFSET;
    // Mix dimensions so differently-sized empty grids differ.
    h = fnv_byte(h, grid.w as u8);
    h = fnv_byte(h, (grid.w >> 8) as u8);
    h = fnv_byte(h, grid.h as u8);
    h = fnv_byte(h, (grid.h >> 8) as u8);
    h = fnv_byte(h, grid.states);
    // For binary grids pack 8 cells/bit; multi-state hashes raw u8 values.
    if grid.states <= 2 {
        let mut acc = 0u8;
        let mut bit = 0u8;
        for &cell in &grid.cells {
            if cell != 0 {
                acc |= 1 << bit;
            }
            bit += 1;
            if bit == 8 {
                h = fnv_byte(h, acc);
                acc = 0;
                bit = 0;
            }
        }
        if bit != 0 {
            h = fnv_byte(h, acc);
        }
    } else {
        for &cell in &grid.cells {
            h = fnv_byte(h, cell);
        }
    }
    h
}

#[inline]
fn fnv_byte(h: u64, b: u8) -> u64 {
    (h ^ b as u64).wrapping_mul(FNV_PRIME)
}

/// Result of checking the history ring for a cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleCheck {
    /// No prior match within the window.
    Novel,
    /// Current state matches a state seen `period` generations ago.
    Cycle { period: u64 },
}

/// Ring of recent generation hashes for oscillator / cycle detection.
///
/// Maps hash → generation number. On collision of the same hash at two gens,
/// reports the period (difference). Period-1 is a still life / stable empty.
pub struct History {
    /// hash → first generation at which that hash was seen (within window).
    seen: HashMap<u64, u64>,
    /// Ring of (generation, hash) for eviction when capacity is exceeded.
    ring: Vec<(u64, u64)>,
    /// Next write index into the ring.
    head: usize,
    /// How many slots are filled (≤ capacity).
    len: usize,
    capacity: usize,
}

impl History {
    /// Create a history that retains at most `capacity` recent generations.
    /// Capacity of 0 is treated as 1.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            seen: HashMap::with_capacity(capacity),
            ring: vec![(0, 0); capacity],
            head: 0,
            len: 0,
            capacity,
        }
    }

    /// Default window: enough for long-period oscillators (e.g. pentadecathlon = 15).
    pub fn default_window() -> Self {
        Self::new(4096)
    }

    /// Record the grid at `generation` and check for a cycle.
    ///
    /// Call this **before** stepping, so gen 0 is recorded, then after each step.
    /// If the current hash matches one seen at gen `g0`, returns
    /// `Cycle { period: generation - g0 }`.
    pub fn observe(&mut self, grid: &Grid, generation: u64) -> CycleCheck {
        let h = hash_grid(grid);
        if let Some(&prev_gen) = self.seen.get(&h) {
            let period = generation.saturating_sub(prev_gen);
            if period > 0 {
                return CycleCheck::Cycle { period };
            }
            // Same generation re-observe: treat as novel (period 0).
            return CycleCheck::Novel;
        }

        // Evict oldest if full.
        if self.len == self.capacity {
            let (old_gen, old_h) = self.ring[self.head];
            // Only remove from map if it still points at the evicted generation.
            if self.seen.get(&old_h).copied() == Some(old_gen) {
                self.seen.remove(&old_h);
            }
        } else {
            self.len += 1;
        }

        self.ring[self.head] = (generation, h);
        self.seen.insert(h, generation);
        self.head = (self.head + 1) % self.capacity;

        CycleCheck::Novel
    }

    /// Number of distinct generations currently tracked.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;
    use crate::rule::Rule;

    #[test]
    fn hash_deterministic() {
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        let h1 = hash_grid(&g);
        let h2 = hash_grid(&g);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_differs_for_different_patterns() {
        let mut a = Grid::new(5, 5);
        a.set(2, 2, true);
        let mut b = Grid::new(5, 5);
        b.set(2, 3, true);
        assert_ne!(hash_grid(&a), hash_grid(&b));
    }

    #[test]
    fn blinker_period_2() {
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);

        let mut hist = History::new(64);
        // gen 0
        assert_eq!(hist.observe(&g, 0), CycleCheck::Novel);
        g.step_with(&Rule::CONWAY);
        // gen 1 — vertical
        assert_eq!(hist.observe(&g, 1), CycleCheck::Novel);
        g.step_with(&Rule::CONWAY);
        // gen 2 — back to horizontal → period 2
        match hist.observe(&g, 2) {
            CycleCheck::Cycle { period } => assert_eq!(period, 2),
            CycleCheck::Novel => panic!("expected period-2 cycle"),
        }
    }

    #[test]
    fn block_period_1_stable() {
        let mut g = Grid::new(4, 4);
        g.set(1, 1, true);
        g.set(2, 1, true);
        g.set(1, 2, true);
        g.set(2, 2, true);

        let mut hist = History::new(16);
        assert_eq!(hist.observe(&g, 0), CycleCheck::Novel);
        g.step_with(&Rule::CONWAY);
        match hist.observe(&g, 1) {
            CycleCheck::Cycle { period } => assert_eq!(period, 1),
            CycleCheck::Novel => panic!("still life should be period 1"),
        }
    }

    #[test]
    fn empty_is_stable() {
        let mut g = Grid::new(4, 4);
        let mut hist = History::new(8);
        hist.observe(&g, 0);
        g.step_with(&Rule::CONWAY);
        match hist.observe(&g, 1) {
            CycleCheck::Cycle { period } => assert_eq!(period, 1),
            CycleCheck::Novel => panic!("empty should stay empty (period 1)"),
        }
    }
}
