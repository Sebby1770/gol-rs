# gol-rs

`gol-rs` is a fast, dependency-free terminal laboratory for Conway's Game of
Life and other Life-like cellular automata. It combines an animated CLI with a
reusable Rust engine, strict RLE import/export, deterministic runs, generation
telemetry, and bounded exact-state cycle detection.

```text
gol-rs  gen 42  pop 38  +6 -4  B3/S23 toroidal
    ████
      ██
  ██  ██
```

## What changed in 1.0

- Reusable library split into grid, rule, pattern, RLE, and simulation modules
- Arbitrary Life-like `B/S` rules, including HighLife (`B36/S23`)
- Toroidal and finite boundary modes
- Strict Life RLE loading from a file or stdin and final-state RLE export
- Ten built-in patterns, from a block and blinker to methuselahs and a Gosper gun
- Deterministic random boards with configurable density
- Birth, death, survivor, population, and peak-population telemetry
- Memory-bounded, exact-state stability and oscillator detection
- Animated terminal, plain text, and JSON Lines output modes
- Checked dimensions and placement with no silent pattern clipping
- Library and end-to-end CLI tests plus CI, Clippy, formatting, and package checks

The binary still has zero runtime dependencies.

## Install

Requires Rust 1.85 or newer.

```sh
git clone https://github.com/Sebby1770/gol-rs.git
cd gol-rs
cargo install --path .
```

Or build and install with the existing helpers:

```sh
make release
PREFIX="$HOME/.local" ./install.sh
```

## Quick start

```sh
# Animated random field when stdout is a terminal
gol

# A finite HighLife simulation
gol --pattern acorn --rule B36/S23 --boundary finite --gens 1000

# Exact period detection with machine-readable output
gol --pattern blinker --width 9 --height 9 --headless --stop-on-cycle

# Load RLE, override its rule, and export the final live bounding box
gol --rle examples/glider.rle --rule B3/S23 --gens 40 --export-rle final.rle

# Discover built-ins
gol patterns
```

`--gens N` means N transitions after generation zero, so a complete run emits
N+1 generation records. Non-terminal output automatically uses plain text and
never emits ANSI control sequences. Use `--output terminal` to request ANSI
explicitly, or `--output jsonl`/`--headless` for fast automation.

## Inputs and rules

Built-in patterns:

| Pattern | Size | Population | Type |
| --- | ---: | ---: | --- |
| `block` | 2x2 | 4 | still life |
| `blinker` | 3x1 | 3 | period-2 oscillator |
| `toad` | 4x2 | 6 | period-2 oscillator |
| `beacon` | 4x4 | 8 | period-2 oscillator |
| `glider` | 3x3 | 5 | spaceship |
| `r-pentomino` | 3x3 | 5 | methuselah |
| `acorn` | 7x3 | 7 | methuselah |
| `diehard` | 8x3 | 7 | finite-lived methuselah |
| `pulsar` | 13x13 | 48 | period-3 oscillator |
| `gosper-glider-gun` | 36x9 | 36 | period-30 gun |

Rules use canonical `B<births>/S<survivals>` notation. `23/3` legacy notation
is also accepted. An explicit `--rule` wins over an RLE header; otherwise the
RLE rule wins over Conway's `B3/S23` default.

RLE input accepts comments, CRLF, wrapped bodies, multi-digit runs, and stdin
via `--rle -`. It rejects missing terminators, invalid or zero runs, trailing
data, dimension overflow, and bodies that exceed declared dimensions. Untrusted
RLE grids are capped at 16 million cells, and encoded input is capped at 8 MiB
before parsing.

## Cycle detection

```sh
gol --pattern pulsar --detect-cycles --cycle-limit 5000 --headless
gol --pattern block --stop-on-cycle --output plain
```

Detection records generation zero and retains complete board states, so a hash
collision cannot report a false cycle. A block is period 1 and a blinker period
2. Detection is exact: a translating glider is not period 4 unless the complete
board returns to an identical state. States are bit-packed, and tracking stops
cleanly at either `--cycle-limit` or a 64 MiB packed-state payload budget;
simulation continues unless `--stop-on-cycle` finds a repeat.

## JSON Lines

`--headless` emits one object per generation and a final summary:

```json
{"type":"generation","generation":2,"population":3,"births":2,"deaths":2,"survivors":1,"changed":4,"rule":"B3/S23","boundary":"toroidal","cycle":{"kind":"oscillator","period":2,"first_generation":0,"repeated_generation":2}}
{"type":"summary","source":"builtin:blinker","final_generation":2,"final_population":3,"peak_population":3,"cycle_tracking_saturated":false,"cycle":{"kind":"oscillator","period":2,"first_generation":0,"repeated_generation":2}}
```

## Library API

```rust
use gol_rs::{Boundary, Grid, Rule, Simulation, builtin_pattern};

let pattern = builtin_pattern("glider").unwrap();
let mut grid = Grid::new(20, 20)?;
grid.place_pattern_centered(&pattern)?;

let mut simulation = Simulation::new(grid, Rule::CONWAY, Boundary::Finite);
let generation_one = simulation.step();
println!("population: {}", generation_one.population);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The engine performs no terminal I/O. `Grid::step` returns transition counts,
and pattern placement is atomic: a pattern either fits completely or no cells
are changed.

## Development

```sh
make check          # fmt check, Clippy -D warnings, and all tests
cargo build --release
cargo package
```

The suite covers grid semantics, custom rules, small toruses, RLE round trips
and hostile inputs, exact cycle periods, non-TTY output, stdin import, CLI
validation, and final-state export.

## License

MIT — see [LICENSE](LICENSE).
