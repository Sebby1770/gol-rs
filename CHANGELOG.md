# Changelog

All notable changes to this project are documented in this file.

## [0.3.0] — 2026-08-08

### Added
- **Multi-rule Life-like automata** (`src/rule.rs`): birth/survive bitmasks, parse `B3/S23` and `23/3`,
  named presets: `conway`, `highlife`, `seeds`, `daynight`, `life-without-death`/`lwd`, `maze`, `replicator`
- CLI: `--rule NAME|B#/S#`, `--list-rules`
- **Cell age tracking** on the grid (`ages: Vec<u16>`): 0 dead, 1 newborn, +1 per step while alive
- CLI: `--age` heat-map colouring (ANSI 256 green → yellow → red)
- **Cycle detection** (`src/history.rs`): FNV-1a grid hashes, ring buffer of recent generations
- CLI: `--until-cycle` stops when a prior generation reappears and reports the period
- `--until-stable` remains period-1 (still life / empty) via the same history path
- **Interactive mode** (`-i` / `--interactive`): Space pause, `.` step, `+`/`-` speed, `r` reseed, `q` quit
  - Raw terminal + non-blocking stdin via thin `extern "C"` termios/poll (no crates; macOS/Linux)
  - RAII restore of termios and cursor
- New patterns: `pentadecathlon`, `glider-pair`, `infinite1`
- `Grid::step_with(&Rule)`; `step()` still defaults to Conway
- Tests: rule parse, HighLife birth-on-6, blinker cycle period 2, age increments, Conway blinker

### Changed
- Version bump to **0.3.0**
- README and HELP rewritten for multi-rule / age / cycle / interactive
- Render header can show rule name, max age, and interactive status

### Unchanged
- Zero crates.io dependencies
- Toroidal wrap-around grid
- MIT license

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
