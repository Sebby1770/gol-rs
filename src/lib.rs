//! gol-rs — Life-like cellular automata library (zero crates.io deps).
//!
//! The CLI binary (`gol`) lives in `main.rs` and depends on this crate.

pub mod grid;
pub mod history;
pub mod patterns;
pub mod render;
pub mod rng;
pub mod rule;
pub mod theme;
pub mod transform;

pub use grid::{Grid, StepStats};
pub use history::{hash_grid, CycleCheck, History};
pub use patterns::{
    dump_ascii, dump_rle, list_patterns, load_file, load_rle_into, save_file, seed_pattern,
    to_rle, PATTERN_NAMES,
};
pub use render::{export_ppm, render_frame, CursorGuard, RenderOpts, Style};
pub use rng::Rng;
pub use rule::{list_rules, Rule};
pub use theme::Theme;
pub use transform::{flip_h, flip_v, rotate90};
