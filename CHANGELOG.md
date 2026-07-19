# Changelog

All notable changes to this project are documented in this file.

## [0.2.0] — 2026-07-19

### Added
- Modular layout: `grid`, `patterns`, `render`, `rng` modules (still a single `gol` binary)
- Many more built-in patterns: blinker, toad, beacon, block, beehive, lwss, mwss, hwss,
  rpentomino, acorn, diehard, plus `spaceship` alias for glider
- `--list-patterns` to print available seeds
- RLE load/save (Life RLE subset + Life 1.05 plain): `--load`, `--save`, `--save-every N`
- `--dump [rle|ascii]` to print the final pattern to stdout
- Render styles: `--style block|braille|dots`
- `--until-stable` stops when the grid equals the previous generation
- `--quiet` / `--once` final-stats-only mode
- `--no-color` plain output
- Per-step births/deaths in the status line
- `--stats out.csv` population history (`gen,pop`)
- GitHub Actions CI (`cargo test` + `cargo build --release`)
- Expanded unit tests (RLE roundtrip, oscillators, still lifes, until-stable)

### Changed
- Version bump to 0.2.0
- Cargo edition set to `2021` for broader toolchain support
- README rewritten for the new CLI surface

### Unchanged
- Zero external crates
- Toroidal wrap-around grid
- MIT license

## [0.1.0] — initial

- Single-file Conway's Game of Life CLI
- Patterns: random, glider, pulsar, gosper
- ANSI block renderer with cursor guard
