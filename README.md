# gol-rs

Conway's Game of Life in your terminal — pure Rust, **zero external crates**.

```
gol-rs  gen     0  pop   384  grid 60x30  +12/-8  (Ctrl-C to quit)
  ████  ██  ██████      ██  ████      ██████  ██  ████
  ██  ████      ██████  ██████      ██  ██████  ████
  ...
```

## Features

- Toroidal (wrap-around) grid, configurable size
- Built-in patterns: random, glider/spaceship, blinker, toad, beacon, block, beehive,
  lwss/mwss/hwss, r-pentomino, acorn, diehard, pulsar, gosper
- **RLE** load/save (Life RLE subset + Life 1.05 plain text)
- Render styles: `block` (█), `braille` (2×4 denser Unicode), `dots` (·)
- `--until-stable`, `--quiet` / `--once`, `--no-color`, births/deaths stats
- Population CSV history via `--stats`
- Deterministic RNG seeding for reproducible runs
- Pure stdlib — no crates.io dependencies

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
gol                                                    # 60x30 random grid
gol --pattern gosper --width 80 --height 25 --delay 60
gol --pattern pulsar --gens 200
gol --pattern toad --until-stable --gens 20
gol --style braille --pattern acorn --width 80 --height 40
gol --load gun.rle --save out.rle --gens 100
gol --pattern block --quiet --dump ascii
gol --seed 42 --density 0.35 --stats pop.csv
gol --list-patterns
gol --help
```

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
| `--until-stable` | | stop when grid equals previous gen |
| `--quiet`, `--once` | | final stats only (no animation) |
| `--no-color` | | disable ANSI colours |
| `--stats PATH` | | CSV history: `gen,pop` |

## Layout

```
src/
  main.rs       # CLI entry, parse_args, run loop
  grid.rs       # Grid, step, neighbors, population
  patterns.rs   # built-in patterns + RLE load/save
  render.rs     # ANSI block / braille / dots + CursorGuard
  rng.rs        # XorShift PRNG
```

## Languages used

- **Rust** — the simulator and CLI
- **Makefile** — build / test / install targets
- **Shell** — POSIX install script
- **GitHub Actions** — CI

## License

MIT — see [LICENSE](LICENSE).
