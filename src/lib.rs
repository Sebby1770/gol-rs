//! A small, dependency-free engine for Conway's Game of Life and other
//! two-state, Moore-neighbourhood Life-like cellular automata.
//!
//! The crate deliberately keeps I/O out of the simulation engine. Applications
//! can build a [`Grid`], choose a [`Rule`] and [`Boundary`], and then drive a
//! [`Simulation`] one generation at a time.

mod grid;
mod pattern;
mod rle;
mod rule;
mod simulation;

pub use grid::{Boundary, BoundingBox, Grid, GridError, PlacementError, SeededRng, StepStats};
pub use pattern::{Pattern, PatternError, PatternInfo, builtin_pattern, catalogue};
pub use rle::{MAX_RLE_CELLS, MAX_RLE_INPUT_BYTES, RleError, export_rle, parse_rle};
pub use rule::{Rule, RuleParseError};
pub use simulation::{
    Cycle, CycleDetector, CycleKind, DEFAULT_MAX_CYCLE_BYTES, GenerationStats, Simulation,
};
