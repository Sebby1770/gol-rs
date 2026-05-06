# gol-rs

Conway's Game of Life in your terminal — written in Rust, with zero runtime dependencies.

```
gol-rs  gen     0  pop   384  grid 60x30  (Ctrl-C to quit)
  ████  ██  ██████      ██  ████      ██████  ██  ████
  ██  ████      ██████  ██████      ██  ██████  ████
  ...
```

## Features

- Toroidal (wrap-around) grid, configurable size
- Built-in patterns: `random`, `glider`, `pulsar`, `gosper` (Gosper glider gun)
- Deterministic seeding for reproducible runs
- Pure-stdlib: no external crates, builds in seconds
- ANSI-coloured renderer with hidden cursor and a clean exit guard

## Build

Requires a recent Rust toolchain (see [rustup.rs](https://rustup.rs)).

```sh
make release        # builds target/release/gol
make test           # runs the unit tests
make demo           # builds and runs the Gosper glider gun
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
gol --seed 42 --density 0.35
gol --help
```

| Flag | Default | Description |
| --- | --- | --- |
| `-W`, `--width N` | `60` | grid width |
| `-H`, `--height N` | `30` | grid height |
| `-g`, `--gens N` | `500` | generations to run |
| `-d`, `--delay MS` | `80` | per-frame delay |
| `-s`, `--seed N` | time-based | RNG seed (random pattern) |
| `-p`, `--pattern NAME` | `random` | `random` \| `glider` \| `pulsar` \| `gosper` |
| `--density F` | `0.25` | 0–1, for `random` |

## Languages used

- **Rust** — the simulator and CLI
- **Makefile** — build / test / install targets
- **Shell** — POSIX install script

## License

MIT — see [LICENSE](LICENSE).
