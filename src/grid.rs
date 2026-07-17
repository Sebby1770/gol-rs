use crate::{Pattern, Rule};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// How cells beyond a grid edge are treated.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Boundary {
    /// Opposite edges are connected.
    #[default]
    Toroidal,
    /// Cells outside the grid are always dead.
    Finite,
}

impl fmt::Display for Boundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Toroidal => "toroidal",
            Self::Finite => "finite",
        })
    }
}

impl FromStr for Boundary {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "toroidal" | "torus" | "wrap" | "wrapped" => Ok(Self::Toroidal),
            "finite" | "fixed" | "dead" => Ok(Self::Finite),
            other => Err(format!(
                "unknown boundary {other:?}; expected toroidal or finite"
            )),
        }
    }
}

/// The smallest axis-aligned rectangle containing all live cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundingBox {
    pub min_x: usize,
    pub min_y: usize,
    pub max_x: usize,
    pub max_y: usize,
}

impl BoundingBox {
    #[must_use]
    pub const fn width(&self) -> usize {
        self.max_x - self.min_x + 1
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.max_y - self.min_y + 1
    }
}

/// Per-generation cell transition counts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StepStats {
    pub population: usize,
    pub births: usize,
    pub deaths: usize,
    pub survivors: usize,
}

impl StepStats {
    #[must_use]
    pub const fn changed(&self) -> usize {
        self.births + self.deaths
    }

    #[must_use]
    pub fn population_delta(&self) -> isize {
        self.births as isize - self.deaths as isize
    }
}

/// A rectangular grid of dead and live cells.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grid {
    width: usize,
    height: usize,
    cells: Vec<bool>,
    next: Vec<bool>,
}

impl Grid {
    /// Allocate an empty grid. Dimensions must be non-zero and their product
    /// must fit in `usize`.
    pub fn new(width: usize, height: usize) -> Result<Self, GridError> {
        if width == 0 || height == 0 {
            return Err(GridError::EmptyDimension);
        }
        let len = width
            .checked_mul(height)
            .ok_or(GridError::DimensionsOverflow { width, height })?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(len)
            .map_err(|_| GridError::AllocationFailed { width, height })?;
        cells.resize(len, false);
        let mut next = Vec::new();
        next.try_reserve_exact(len)
            .map_err(|_| GridError::AllocationFailed { width, height })?;
        next.resize(len, false);
        Ok(Self {
            width,
            height,
            cells,
            next,
        })
    }

    /// Construct a grid from live-cell coordinates.
    pub fn from_live_cells<I>(width: usize, height: usize, cells: I) -> Result<Self, GridError>
    where
        I: IntoIterator<Item = (usize, usize)>,
    {
        let mut grid = Self::new(width, height)?;
        for (x, y) in cells {
            grid.set(x, y, true)?;
        }
        Ok(grid)
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.population() == 0
    }

    /// Cell states in row-major order.
    #[must_use]
    pub fn cells(&self) -> &[bool] {
        &self.cells
    }

    /// Return a cell, or `None` for an out-of-bounds coordinate.
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<bool> {
        self.index(x, y).map(|index| self.cells[index])
    }

    /// Set a cell, returning an error rather than silently clipping.
    pub fn set(&mut self, x: usize, y: usize, alive: bool) -> Result<(), GridError> {
        let index = self.index(x, y).ok_or(GridError::CoordinateOutOfBounds {
            x,
            y,
            width: self.width,
            height: self.height,
        })?;
        self.cells[index] = alive;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.cells.fill(false);
        self.next.fill(false);
    }

    /// Fill from a deterministic xorshift random stream.
    pub fn randomize(&mut self, rng: &mut SeededRng, density: f64) -> Result<(), GridError> {
        if !density.is_finite() || !(0.0..=1.0).contains(&density) {
            return Err(GridError::InvalidDensity(density));
        }
        for cell in &mut self.cells {
            *cell = rng.next_f64() < density;
        }
        Ok(())
    }

    /// Place an entire pattern. The operation is atomic: if its declared
    /// rectangle does not fit, no cells are changed.
    pub fn place_pattern(
        &mut self,
        pattern: &Pattern,
        origin_x: usize,
        origin_y: usize,
    ) -> Result<(), PlacementError> {
        let end_x = origin_x
            .checked_add(pattern.width())
            .ok_or(PlacementError::OffsetOverflow)?;
        let end_y = origin_y
            .checked_add(pattern.height())
            .ok_or(PlacementError::OffsetOverflow)?;
        if end_x > self.width || end_y > self.height {
            return Err(PlacementError::DoesNotFit {
                pattern_width: pattern.width(),
                pattern_height: pattern.height(),
                origin_x,
                origin_y,
                grid_width: self.width,
                grid_height: self.height,
            });
        }
        for (x, y) in pattern.live_cells() {
            // The checked pattern rectangle above proves these additions and
            // coordinates are in range.
            let target_x = origin_x + x;
            let target_y = origin_y + y;
            let index = target_y * self.width + target_x;
            self.cells[index] = true;
        }
        Ok(())
    }

    /// Place a pattern centred in the grid.
    pub fn place_pattern_centered(&mut self, pattern: &Pattern) -> Result<(), PlacementError> {
        if pattern.width() > self.width || pattern.height() > self.height {
            return Err(PlacementError::DoesNotFit {
                pattern_width: pattern.width(),
                pattern_height: pattern.height(),
                origin_x: 0,
                origin_y: 0,
                grid_width: self.width,
                grid_height: self.height,
            });
        }
        self.place_pattern(
            pattern,
            (self.width - pattern.width()) / 2,
            (self.height - pattern.height()) / 2,
        )
    }

    #[must_use]
    pub fn population(&self) -> usize {
        self.cells.iter().filter(|cell| **cell).count()
    }

    pub fn live_cells(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, alive)| **alive)
            .map(|(index, _)| (index % self.width, index / self.width))
    }

    #[must_use]
    pub fn bounding_box(&self) -> Option<BoundingBox> {
        let mut cells = self.live_cells();
        let (first_x, first_y) = cells.next()?;
        let mut bounds = BoundingBox {
            min_x: first_x,
            min_y: first_y,
            max_x: first_x,
            max_y: first_y,
        };
        for (x, y) in cells {
            bounds.min_x = bounds.min_x.min(x);
            bounds.min_y = bounds.min_y.min(y);
            bounds.max_x = bounds.max_x.max(x);
            bounds.max_y = bounds.max_y.max(y);
        }
        Some(bounds)
    }

    /// Count live Moore-neighbourhood cells under the selected boundary.
    #[must_use]
    pub fn neighbour_count(&self, x: usize, y: usize, boundary: Boundary) -> Option<u8> {
        self.index(x, y)?;
        Some(match boundary {
            Boundary::Finite => self.finite_neighbour_count(x, y),
            Boundary::Toroidal => self.toroidal_neighbour_count(x, y),
        })
    }

    /// Advance one generation in place.
    pub fn step(&mut self, rule: Rule, boundary: Boundary) -> StepStats {
        let mut stats = StepStats::default();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let alive = self.cells[index];
                let neighbours = match boundary {
                    Boundary::Finite => self.finite_neighbour_count(x, y),
                    Boundary::Toroidal => self.toroidal_neighbour_count(x, y),
                };
                let next = rule.next_state(alive, neighbours);
                self.next[index] = next;
                match (alive, next) {
                    (false, true) => stats.births += 1,
                    (true, false) => stats.deaths += 1,
                    (true, true) => stats.survivors += 1,
                    (false, false) => {}
                }
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        stats.population = stats.births + stats.survivors;
        stats
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        (x < self.width && y < self.height).then_some(y * self.width + x)
    }

    fn finite_neighbour_count(&self, x: usize, y: usize) -> u8 {
        let mut count = 0;
        for dy in -1isize..=1 {
            for dx in -1isize..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let Some(nx) = x.checked_add_signed(dx) else {
                    continue;
                };
                let Some(ny) = y.checked_add_signed(dy) else {
                    continue;
                };
                if self.get(nx, ny) == Some(true) {
                    count += 1;
                }
            }
        }
        count
    }

    fn toroidal_neighbour_count(&self, x: usize, y: usize) -> u8 {
        // Deduplication gives intuitive behaviour on one- and two-cell axes,
        // where several wrapped offsets otherwise refer to the same cell.
        let origin = y * self.width + x;
        let mut seen = [usize::MAX; 8];
        let mut seen_len = 0;
        let mut count = 0;
        for dy in [self.height - 1, 0, 1] {
            for dx in [self.width - 1, 0, 1] {
                let nx = (x + dx) % self.width;
                let ny = (y + dy) % self.height;
                let index = ny * self.width + nx;
                if index == origin || seen[..seen_len].contains(&index) {
                    continue;
                }
                seen[seen_len] = index;
                seen_len += 1;
                if self.cells[index] {
                    count += 1;
                }
            }
        }
        count
    }
}

/// A tiny deterministic PRNG suitable for repeatable initial grids.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeededRng(u64);

impl SeededRng {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0xDEAD_BEEF_CAFE_BABE
        } else {
            seed
        })
    }

    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    #[must_use]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1_u64 << 53) as f64)
    }
}

/// Grid construction or coordinate error.
#[derive(Clone, Debug, PartialEq)]
pub enum GridError {
    EmptyDimension,
    DimensionsOverflow {
        width: usize,
        height: usize,
    },
    AllocationFailed {
        width: usize,
        height: usize,
    },
    CoordinateOutOfBounds {
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    },
    InvalidDensity(f64),
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDimension => f.write_str("grid width and height must both be at least 1"),
            Self::DimensionsOverflow { width, height } => {
                write!(f, "grid dimensions {width}x{height} overflow address space")
            }
            Self::AllocationFailed { width, height } => {
                write!(f, "could not allocate grid dimensions {width}x{height}")
            }
            Self::CoordinateOutOfBounds {
                x,
                y,
                width,
                height,
            } => write!(f, "coordinate ({x},{y}) is outside {width}x{height} grid"),
            Self::InvalidDensity(density) => {
                write!(f, "density must be a finite number in 0..=1, got {density}")
            }
        }
    }
}

impl Error for GridError {}

/// Pattern placement error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlacementError {
    OffsetOverflow,
    DoesNotFit {
        pattern_width: usize,
        pattern_height: usize,
        origin_x: usize,
        origin_y: usize,
        grid_width: usize,
        grid_height: usize,
    },
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OffsetOverflow => {
                f.write_str("pattern origin plus dimensions overflow address space")
            }
            Self::DoesNotFit {
                pattern_width,
                pattern_height,
                origin_x,
                origin_y,
                grid_width,
                grid_height,
            } => write!(
                f,
                "{pattern_width}x{pattern_height} pattern at ({origin_x},{origin_y}) does not fit in {grid_width}x{grid_height} grid"
            ),
        }
    }
}

impl Error for PlacementError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(width: usize, height: usize, cells: &[(usize, usize)]) -> Grid {
        Grid::from_live_cells(width, height, cells.iter().copied()).unwrap()
    }

    #[test]
    fn finite_and_toroidal_boundaries_differ_at_corner() {
        let grid = grid(5, 5, &[(0, 0)]);
        assert_eq!(grid.neighbour_count(4, 4, Boundary::Finite), Some(0));
        assert_eq!(grid.neighbour_count(4, 4, Boundary::Toroidal), Some(1));
    }

    #[test]
    fn tiny_torus_does_not_double_count_cells() {
        let grid = grid(2, 2, &[(0, 0), (1, 0), (0, 1)]);
        assert_eq!(grid.neighbour_count(1, 1, Boundary::Toroidal), Some(3));
    }

    #[test]
    fn blinker_oscillates_and_reports_transitions() {
        let mut grid = grid(5, 5, &[(1, 2), (2, 2), (3, 2)]);
        let stats = grid.step(Rule::CONWAY, Boundary::Finite);
        assert_eq!(stats.population, 3);
        assert_eq!(stats.births, 2);
        assert_eq!(stats.deaths, 2);
        assert_eq!(grid.get(2, 1), Some(true));
        grid.step(Rule::CONWAY, Boundary::Finite);
        assert_eq!(grid.get(1, 2), Some(true));
    }

    #[test]
    fn highlife_rule_births_with_six_neighbours() {
        let rule: Rule = "B36/S23".parse().unwrap();
        let mut grid = grid(3, 3, &[(0, 0), (1, 0), (2, 0), (0, 1), (2, 1), (0, 2)]);
        grid.step(rule, Boundary::Finite);
        assert_eq!(grid.get(1, 1), Some(true));
    }

    #[test]
    fn rejects_invalid_dimensions_and_coordinates() {
        assert_eq!(Grid::new(0, 1), Err(GridError::EmptyDimension));
        assert!(Grid::new(usize::MAX, 2).is_err());
        assert!(Grid::new(usize::MAX, 1).is_err());
        assert!(Grid::new(2, 2).unwrap().set(2, 0, true).is_err());
    }

    #[test]
    fn randomization_is_reproducible() {
        let mut first = Grid::new(20, 10).unwrap();
        let mut second = Grid::new(20, 10).unwrap();
        first.randomize(&mut SeededRng::new(42), 0.3).unwrap();
        second.randomize(&mut SeededRng::new(42), 0.3).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn bounding_box_tracks_live_extent() {
        let grid = grid(10, 10, &[(7, 2), (3, 8), (5, 4)]);
        assert_eq!(
            grid.bounding_box(),
            Some(BoundingBox {
                min_x: 3,
                min_y: 2,
                max_x: 7,
                max_y: 8
            })
        );
    }
}
