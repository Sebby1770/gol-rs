use crate::Rule;
use std::error::Error;
use std::fmt;

/// An immutable rectangular cell pattern, optionally carrying an RLE rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pattern {
    width: usize,
    height: usize,
    cells: Vec<bool>,
    rule: Option<Rule>,
}

impl Pattern {
    /// Build a pattern from live-cell coordinates.
    pub fn from_live_cells<I>(width: usize, height: usize, cells: I) -> Result<Self, PatternError>
    where
        I: IntoIterator<Item = (usize, usize)>,
    {
        if width == 0 || height == 0 {
            return Err(PatternError::EmptyDimension);
        }
        let len = width
            .checked_mul(height)
            .ok_or(PatternError::DimensionsOverflow { width, height })?;
        let mut states = Vec::new();
        states
            .try_reserve_exact(len)
            .map_err(|_| PatternError::AllocationFailed { width, height })?;
        states.resize(len, false);
        for (x, y) in cells {
            if x >= width || y >= height {
                return Err(PatternError::CoordinateOutOfBounds {
                    x,
                    y,
                    width,
                    height,
                });
            }
            states[y * width + x] = true;
        }
        Ok(Self {
            width,
            height,
            cells: states,
            rule: None,
        })
    }

    pub(crate) fn set_live_run(&mut self, y: usize, start_x: usize, end_x: usize) {
        debug_assert!(y < self.height);
        debug_assert!(start_x <= end_x && end_x <= self.width);
        let row_start = y * self.width;
        self.cells[row_start + start_x..row_start + end_x].fill(true);
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
    pub const fn rule(&self) -> Option<Rule> {
        self.rule
    }

    #[must_use]
    pub fn with_rule(mut self, rule: Rule) -> Self {
        self.rule = Some(rule);
        self
    }

    #[must_use]
    pub fn population(&self) -> usize {
        self.cells.iter().filter(|cell| **cell).count()
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<bool> {
        (x < self.width && y < self.height).then(|| self.cells[y * self.width + x])
    }

    pub fn live_cells(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, alive)| **alive)
            .map(|(index, _)| (index % self.width, index / self.width))
    }
}

/// Metadata for one built-in pattern.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatternInfo {
    pub name: &'static str,
    pub width: usize,
    pub height: usize,
    pub population: usize,
    pub description: &'static str,
}

const CATALOGUE: &[PatternInfo] = &[
    PatternInfo {
        name: "block",
        width: 2,
        height: 2,
        population: 4,
        description: "period-1 still life",
    },
    PatternInfo {
        name: "blinker",
        width: 3,
        height: 1,
        population: 3,
        description: "period-2 oscillator",
    },
    PatternInfo {
        name: "toad",
        width: 4,
        height: 2,
        population: 6,
        description: "period-2 oscillator",
    },
    PatternInfo {
        name: "beacon",
        width: 4,
        height: 4,
        population: 8,
        description: "period-2 oscillator",
    },
    PatternInfo {
        name: "glider",
        width: 3,
        height: 3,
        population: 5,
        description: "small diagonal spaceship",
    },
    PatternInfo {
        name: "r-pentomino",
        width: 3,
        height: 3,
        population: 5,
        description: "chaotic methuselah (1103 generations)",
    },
    PatternInfo {
        name: "acorn",
        width: 7,
        height: 3,
        population: 7,
        description: "long-lived methuselah (5206 generations)",
    },
    PatternInfo {
        name: "diehard",
        width: 8,
        height: 3,
        population: 7,
        description: "methuselah that vanishes after 130 generations",
    },
    PatternInfo {
        name: "pulsar",
        width: 13,
        height: 13,
        population: 48,
        description: "large period-3 oscillator",
    },
    PatternInfo {
        name: "gosper-glider-gun",
        width: 36,
        height: 9,
        population: 36,
        description: "period-30 glider gun",
    },
];

/// All built-in patterns, in catalogue order.
#[must_use]
pub const fn catalogue() -> &'static [PatternInfo] {
    CATALOGUE
}

/// Load a built-in pattern by name. Names are ASCII case-insensitive and `_`
/// is accepted in place of `-`.
#[must_use]
pub fn builtin_pattern(name: &str) -> Option<Pattern> {
    let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "block" => from_ascii("oo\noo"),
        "blinker" => from_ascii("ooo"),
        "toad" => from_ascii(".ooo\nooo."),
        "beacon" => from_ascii("oo..\noo..\n..oo\n..oo"),
        "glider" => from_ascii(".o.\n..o\nooo"),
        "r-pentomino" | "rpentomino" => from_ascii(".oo\noo.\n.o."),
        "acorn" => from_ascii(".o.....\n...o...\noo..ooo"),
        "diehard" => from_ascii("......o.\noo......\n.o...ooo"),
        "pulsar" => from_ascii(PULSAR),
        "gosper" | "gosper-gun" | "gosper-glider-gun" => from_ascii(GOSPER),
        _ => return None,
    }
    .ok()
}

fn from_ascii(art: &str) -> Result<Pattern, PatternError> {
    let lines: Vec<&str> = art.lines().collect();
    let height = lines.len();
    let width = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let cells = lines.iter().enumerate().flat_map(|(y, line)| {
        line.chars()
            .enumerate()
            .filter(|(_, cell)| matches!(cell, 'o' | 'O' | '#' | '*'))
            .map(move |(x, _)| (x, y))
    });
    Pattern::from_live_cells(width, height, cells)
}

const PULSAR: &str = "\
..ooo...ooo..
.............
o....o.o....o
o....o.o....o
o....o.o....o
..ooo...ooo..
.............
..ooo...ooo..
o....o.o....o
o....o.o....o
o....o.o....o
.............
..ooo...ooo..";

const GOSPER: &str = "\
........................o...........
......................o.o...........
............oo......oo............oo
...........o...o....oo............oo
oo........o.....o...oo..............
oo........o...o.oo....o.o...........
..........o.....o.......o...........
...........o...o....................
............oo......................";

/// Pattern construction error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternError {
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
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDimension => f.write_str("pattern width and height must both be at least 1"),
            Self::DimensionsOverflow { width, height } => {
                write!(
                    f,
                    "pattern dimensions {width}x{height} overflow address space"
                )
            }
            Self::AllocationFailed { width, height } => {
                write!(f, "could not allocate pattern dimensions {width}x{height}")
            }
            Self::CoordinateOutOfBounds {
                x,
                y,
                width,
                height,
            } => write!(f, "pattern cell ({x},{y}) is outside {width}x{height}"),
        }
    }
}

impl Error for PatternError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_metadata_matches_patterns() {
        for info in catalogue() {
            let pattern = builtin_pattern(info.name).unwrap();
            assert_eq!(pattern.width(), info.width, "{} width", info.name);
            assert_eq!(pattern.height(), info.height, "{} height", info.name);
            assert_eq!(
                pattern.population(),
                info.population,
                "{} population",
                info.name
            );
        }
    }

    #[test]
    fn aliases_are_supported() {
        assert_eq!(
            builtin_pattern("GOSPER"),
            builtin_pattern("gosper_glider_gun")
        );
        assert_eq!(
            builtin_pattern("rpentomino"),
            builtin_pattern("r-pentomino")
        );
    }

    #[test]
    fn placement_is_atomic_when_pattern_does_not_fit() {
        let pattern = builtin_pattern("glider").unwrap();
        let mut grid = crate::Grid::new(3, 3).unwrap();
        assert!(grid.place_pattern(&pattern, 1, 0).is_err());
        assert!(grid.is_empty());
    }

    #[test]
    fn placement_checks_offset_overflow() {
        let pattern = builtin_pattern("block").unwrap();
        let mut grid = crate::Grid::new(4, 4).unwrap();
        assert_eq!(
            grid.place_pattern(&pattern, usize::MAX, 0),
            Err(crate::PlacementError::OffsetOverflow)
        );
    }

    #[test]
    fn huge_non_overflowing_allocation_returns_error() {
        assert!(Pattern::from_live_cells(usize::MAX, 1, std::iter::empty()).is_err());
    }
}
