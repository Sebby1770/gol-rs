# gol-rs

Life-like cellular automata in your terminal — pure Rust, **zero external crates**.

Default rule is Conway's Game of Life (`B3/S23`). Also ships HighLife, Seeds, Day & Night, and more.

```
gol-rs  gen     0  pop   384  grid 60x30  +12/-8  (Ctrl-C to quit)
  ████  ██  ██████      ██  ████      ██████  ██  ████
  ██  ████      ██████  ██████      ██  ██████  ████
  ...
```

## Features

- Toroidal (wrap-around) **or finite** edges (`--finite` / `--no-wrap`)
- **Multi-rule Life-like CA** — `--rule conway|highlife|seeds|…` or `B3/S23` / `23/3`
- **Colour themes** — `classic`, `neon`, `fire`, `ocean`, `mono` (`--theme`)
- **Cell age heat-map** — `--age` colours young → old within the theme palette
- **Cycle detection** — `--until-cycle` stops on any oscillator; reports period
- **Interactive mode** — `-i` pause/step/speed/reseed/theme/age/wrap from the keyboard
- Built-in patterns: random, glider/spaceship, blinker, toad, beacon, block, beehive,
  lwss/mwss/hwss, r-pentomino, acorn, diehard, pulsar, gosper, pentadecathlon,
  glider-pair, infinite1
- **RLE** load/save (Life RLE subset + Life 1.05 plain text)
- Grid transforms on seed: `--rotate90`, `--flip-h`, `--flip-v`
- Render styles: `block` (█), `braille` (2×4 denser Unicode), `dots` (·)
- **PPM export** of the final frame (`--export-ppm`)
- **Benchmark mode** (`--bench`) — gens/sec and cells/sec
- **Parallel step** for large grids (≥ 20 000 cells) via `std::thread::scope`
- Rich run stats: max/min pop, gen at peak, total births/deaths; CSV `gen,pop,births,deaths`
- Library crate (`gol_rs`) + `gol` binary
- Deterministic RNG seeding for reproducible runs
- Pure stdlib — no crates.io dependencies (interactive mode uses thin `extern "C"` termios)

## Build

Requires a Rust toolchain (edition 2021; see [rustup.rs](https://rustup.rs)).

```sh
make release        # builds target/release/gol
make test           # runs the unit tests
make demo           # builds and runs the Gosper glider gun
make check          # fmt + clippy + test
```

## Install

```sh
./install.sh                          # installs to /usr/local/bin (uses sudo if needed)
PREFIX=$HOME/.local ./install.sh      # user-local install
./install.sh --uninstall              # remove
```

Or via `make`:

```sh
make install                          # PREFIX=/usr/local
PREFIX=$HOME/.local make install
```

## Usage

```sh
gol                                                    # 60x30 random grid (Conway)
gol --pattern gosper --width 80 --height 25 --delay 60
gol --pattern blinker --until-cycle --quiet           # reports period 2
gol --pattern block --until-stable --quiet
gol --rule highlife --pattern random --density 0.2
gol --rule B36/S23 --gens 300
gol --theme neon --age --pattern acorn
gol --finite --pattern glider --width 40 --height 20
gol --bench --width 200 --height 100 --gens 200 --seed 1
gol --pattern acorn --export-ppm final.ppm --quiet --gens 200
gol -i --pattern gosper --theme fire --width 80 --height 25
gol --load gun.rle --rotate90 --flip-h --stats pop.csv
gol --style braille --pattern pentadecathlon --summary
gol --list-patterns
gol --list-rules
gol --help
```

### Interactive keys (`-i` / `--interactive`)

Requires a TTY. Restores terminal settings on exit.

| Key | Action |
| --- | --- |
| `Space` | pause / resume |
| `.` | step one generation (while paused) |
| `+` / `=` | speed up (halve delay) |
| `-` | slow down (double delay) |
| `r` | reseed (same pattern / new random) |
| `t` | cycle colour theme |
| `a` | toggle age heat-map |
| `w` | toggle wrap (toroidal ↔ finite) |
| `q` | quit |

### CLI flags

| Flag | Default | Description |
| --- | --- | --- |
| `-W`, `--width N` | `60` | grid width |
| `-H`, `--height N` | `30` | grid height |
| `-g`, `--gens N` | `500` | max generations |
| `-d`, `--delay MS` | `80` | per-frame delay |
| `-s`, `--seed N` | time-based | RNG seed (`random` pattern) |
| `-p`, `--pattern NAME` | `random` | built-in pattern name |
| `--density F` | `0.25` | 0–1 fill for `random` |
| `--list-patterns` | | list built-in patterns |
| `--load PATH` | | seed from RLE / Life 1.05 file |
| `--save PATH` | | write final grid as RLE |
| `--save-every N` | | also snapshot RLE every N gens |
| `--dump [rle\|ascii]` | | print final pattern to stdout |
| `--style MODE` | `block` | `block` \| `braille` \| `dots` |
| `--rule NAME\|B#/S#` | `conway` | Life-like rule (see `--list-rules`) |
| `--list-rules` | | list built-in rules |
| `--age` | | heat-map colour by cell age |
| `--theme NAME` | `classic` | `classic` \| `neon` \| `fire` \| `ocean` \| `mono` |
| `--finite`, `--no-wrap` | wrap on | hard edges (off-grid = dead) |
| `--rotate90` | | rotate seed 90° CW after load/pattern |
| `--flip-h` | | flip seed horizontally |
| `--flip-v` | | flip seed vertically |
| `--until-stable` | | stop on period-1 (still life / empty) |
| `--until-cycle` | | stop when any prior gen reappears; report period |
| `-i`, `--interactive` | | keyboard control (TTY) |
| `--quiet`, `--once` | | final stats only (no animation) |
| `--summary` | | always print final summary (even when animating) |
| `--no-color` | | disable ANSI colours |
| `--stats PATH` | | CSV history: `gen,pop,births,deaths` |
| `--export-ppm PATH` | | write final frame as P6 PPM |
| `--bench` | | no render; print gens/sec & cells/sec |

### Built-in rules

| Name | Notation | Notes |
| --- | --- | --- |
| `conway` | B3/S23 | classic Game of Life (default) |
| `highlife` | B36/S23 | birth on 6; has replicators |
| `seeds` | B2/S | explosive; no survival |
| `daynight` | B3678/S34678 | Day & Night |
| `life-without-death` / `lwd` | B3/S012345678 | cells never die |
| `maze` | B3/S12345 | maze-like growth |
| `replicator` | B1357/S1357 | Fredkin replicator |

Also accepts `B#/S#` or classic `S/B` form (`23/3` = Conway).

### Themes

| Name | Look |
| --- | --- |
| `classic` | green live cells (default) |
| `neon` | cyan / magenta |
| `fire` | yellow → orange → red |
| `ocean` | pale cyan → deep blue |
| `mono` | greyscale |

With `--age`, the heat-map ramps within the chosen theme. Header accents follow the theme too.

## Library

```toml
# in another crate, path-depend on this package
gol-rs = { path = "…" }
```

```rust
use gol_rs::{Grid, Rule, Theme, seed_pattern, Rng};

fn main() {
    let mut g = Grid::with_wrap(40, 20, true);
    let mut rng = Rng::new(1);
    seed_pattern(&mut g, "gosper", &mut rng, 0.0).unwrap();
    for _ in 0..100 {
        g.step_with(&Rule::CONWAY);
    }
    println!("pop {}", g.population());
    let _ = Theme::NEON;
}
```

## Layout

```
src/
  lib.rs        # library root (gol_rs)
  main.rs       # CLI entry, parse_args, run loop, interactive
  grid.rs       # Grid, wrap topology, ages, sequential + parallel step
  rule.rs       # Life-like Rule bitmasks + presets
  history.rs    # FNV grid hash + cycle detection
  patterns.rs   # built-in patterns + RLE load/save
  render.rs     # ANSI block / braille / dots + themes + PPM export
  theme.rs      # classic / neon / fire / ocean / mono
  transform.rs  # rotate90, flip_h, flip_v
  term.rs       # raw TTY + non-blocking keys (bin-only, extern C termios)
  rng.rs        # XorShift PRNG
```

## Languages used

- **Rust** — the simulator, library, and CLI
- **Makefile** — build / test / install targets
- **Shell** — POSIX install script
- **GitHub Actions** — CI

## License

MIT — see [LICENSE](LICENSE).
