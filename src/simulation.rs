use crate::{Boundary, Grid, Rule};
use std::collections::HashMap;
use std::fmt;

/// Default upper bound for retained, packed cycle-state payloads.
pub const DEFAULT_MAX_CYCLE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct StateKey {
    width: usize,
    height: usize,
    cells: Box<[u64]>,
}

impl StateKey {
    fn from_grid(grid: &Grid) -> Option<Self> {
        let word_count = grid.len().div_ceil(u64::BITS as usize);
        let mut cells = Vec::new();
        cells.try_reserve_exact(word_count).ok()?;
        cells.resize(word_count, 0u64);
        for (index, alive) in grid.cells().iter().copied().enumerate() {
            if alive {
                cells[index / u64::BITS as usize] |= 1u64 << (index % u64::BITS as usize);
            }
        }
        Some(Self {
            width: grid.width(),
            height: grid.height(),
            cells: cells.into_boxed_slice(),
        })
    }

    fn payload_bytes(&self) -> usize {
        self.cells.len().saturating_mul(std::mem::size_of::<u64>())
    }
}

/// Statistics for a fully materialized generation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GenerationStats {
    pub generation: u64,
    pub population: usize,
    pub births: usize,
    pub deaths: usize,
    pub survivors: usize,
}

impl GenerationStats {
    #[must_use]
    pub const fn changed(&self) -> usize {
        self.births + self.deaths
    }
}

/// A reusable simulation that owns a grid and its evolution settings.
#[derive(Clone, Debug)]
pub struct Simulation {
    grid: Grid,
    rule: Rule,
    boundary: Boundary,
    generation: u64,
}

impl Simulation {
    #[must_use]
    pub const fn new(grid: Grid, rule: Rule, boundary: Boundary) -> Self {
        Self {
            grid,
            rule,
            boundary,
            generation: 0,
        }
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn rule(&self) -> Rule {
        self.rule
    }

    #[must_use]
    pub const fn boundary(&self) -> Boundary {
        self.boundary
    }

    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn grid_mut(&mut self) -> &mut Grid {
        &mut self.grid
    }

    #[must_use]
    pub fn current_stats(&self) -> GenerationStats {
        GenerationStats {
            generation: self.generation,
            population: self.grid.population(),
            ..GenerationStats::default()
        }
    }

    /// Advance one generation and report the transitions that produced it.
    pub fn step(&mut self) -> GenerationStats {
        let transitions = self.grid.step(self.rule, self.boundary);
        self.generation = self.generation.saturating_add(1);
        GenerationStats {
            generation: self.generation,
            population: transitions.population,
            births: transitions.births,
            deaths: transitions.deaths,
            survivors: transitions.survivors,
        }
    }
}

/// Human-readable classification of an exact repeated state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CycleKind {
    Stable,
    Oscillator,
}

impl fmt::Display for CycleKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Stable => "stable",
            Self::Oscillator => "oscillator",
        })
    }
}

/// An exact-state repeat. Translating spaceships are intentionally not treated
/// as cycles unless the complete board is identical.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cycle {
    pub first_generation: u64,
    pub repeated_generation: u64,
    pub period: u64,
    pub kind: CycleKind,
}

/// Bounded exact-state cycle detection.
///
/// Complete cell vectors are retained, so hash collisions cannot create false
/// cycles. States are bit-packed, and the detector stops accepting new states
/// once either its state-count or packed-payload budget is reached.
#[derive(Clone, Debug)]
pub struct CycleDetector {
    seen: HashMap<StateKey, u64>,
    max_states: usize,
    max_bytes: usize,
    tracked_bytes: usize,
    saturated: bool,
}

impl CycleDetector {
    #[must_use]
    pub fn new(max_states: usize) -> Self {
        Self::with_limits(max_states, DEFAULT_MAX_CYCLE_BYTES)
    }

    /// Construct a detector with explicit state-count and packed-payload limits.
    #[must_use]
    pub fn with_limits(max_states: usize, max_bytes: usize) -> Self {
        Self {
            seen: HashMap::new(),
            max_states,
            max_bytes,
            tracked_bytes: 0,
            saturated: false,
        }
    }

    #[must_use]
    pub const fn max_states(&self) -> usize {
        self.max_states
    }

    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    #[must_use]
    pub fn tracked_states(&self) -> usize {
        self.seen.len()
    }

    /// Bytes used by retained packed cell payloads (excluding map overhead).
    #[must_use]
    pub const fn tracked_bytes(&self) -> usize {
        self.tracked_bytes
    }

    #[must_use]
    pub const fn is_saturated(&self) -> bool {
        self.saturated
    }

    /// Observe a state at its generation. Call this for generation zero before
    /// stepping to detect stable states and oscillator periods correctly.
    pub fn observe(&mut self, generation: u64, grid: &Grid) -> Option<Cycle> {
        let Some(state) = StateKey::from_grid(grid) else {
            self.saturated = true;
            return None;
        };
        if let Some(first_generation) = self.seen.get_mut(&state) {
            let previous_generation = *first_generation;
            *first_generation = generation;
            let period = generation.saturating_sub(previous_generation);
            return Some(Cycle {
                first_generation: previous_generation,
                repeated_generation: generation,
                period,
                kind: if period == 1 {
                    CycleKind::Stable
                } else {
                    CycleKind::Oscillator
                },
            });
        }
        let state_bytes = state.payload_bytes();
        if self.seen.len() >= self.max_states
            || state_bytes > self.max_bytes.saturating_sub(self.tracked_bytes)
        {
            self.saturated = true;
            return None;
        }
        self.seen.insert(state, generation);
        self.tracked_bytes += state_bytes;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simulation(cells: &[(usize, usize)]) -> Simulation {
        Simulation::new(
            Grid::from_live_cells(7, 7, cells.iter().copied()).unwrap(),
            Rule::CONWAY,
            Boundary::Finite,
        )
    }

    #[test]
    fn still_life_is_period_one_from_generation_zero() {
        let mut simulation = simulation(&[(2, 2), (3, 2), (2, 3), (3, 3)]);
        let mut detector = CycleDetector::new(10);
        assert_eq!(detector.observe(0, simulation.grid()), None);
        let stats = simulation.step();
        assert_eq!(stats.generation, 1);
        assert_eq!(
            detector.observe(1, simulation.grid()),
            Some(Cycle {
                first_generation: 0,
                repeated_generation: 1,
                period: 1,
                kind: CycleKind::Stable,
            })
        );
    }

    #[test]
    fn blinker_is_period_two() {
        let mut simulation = simulation(&[(2, 3), (3, 3), (4, 3)]);
        let mut detector = CycleDetector::new(10);
        detector.observe(0, simulation.grid());
        simulation.step();
        assert_eq!(detector.observe(1, simulation.grid()), None);
        simulation.step();
        assert_eq!(detector.observe(2, simulation.grid()).unwrap().period, 2);
    }

    #[test]
    fn detector_is_bounded() {
        let grid = Grid::new(2, 2).unwrap();
        let mut detector = CycleDetector::new(0);
        assert_eq!(detector.observe(0, &grid), None);
        assert!(detector.is_saturated());
        assert_eq!(detector.tracked_states(), 0);
    }

    #[test]
    fn detector_packs_states_and_honours_byte_budget() {
        let grid = Grid::new(65, 1).unwrap();
        let mut detector = CycleDetector::with_limits(10, 16);
        assert_eq!(detector.observe(0, &grid), None);
        assert_eq!(detector.tracked_states(), 1);
        assert_eq!(detector.tracked_bytes(), 16);

        let other = Grid::from_live_cells(65, 1, [(0, 0)]).unwrap();
        assert_eq!(detector.observe(1, &other), None);
        assert!(detector.is_saturated());
        assert_eq!(detector.tracked_states(), 1);
        assert_eq!(detector.tracked_bytes(), 16);
    }

    #[test]
    fn dimensions_are_part_of_cycle_identity() {
        let wide = Grid::new(3, 2).unwrap();
        let tall = Grid::new(2, 3).unwrap();
        let mut detector = CycleDetector::new(10);
        assert_eq!(detector.observe(0, &wide), None);
        assert_eq!(detector.observe(1, &tall), None);
    }

    #[test]
    fn repeated_observations_keep_the_true_local_period() {
        let grid = Grid::new(2, 2).unwrap();
        let mut detector = CycleDetector::new(10);
        detector.observe(0, &grid);
        assert_eq!(detector.observe(1, &grid).unwrap().period, 1);
        assert_eq!(detector.observe(2, &grid).unwrap().period, 1);
    }
}
