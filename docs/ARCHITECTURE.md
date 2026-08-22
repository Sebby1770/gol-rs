# gol-rs — Structural plan (v0.6)

gol-rs is a **zero-crate Rust cellular-automaton engine** with a terminal frontend. v0.5 already has Life-like rules, Brian's Brain, RLE, themes, cycle detection, and a library split. What it does not have is a way for anyone to *see* it without compiling a CLI.

## Decision

Keep the Rust crate as the oracle (CLI, tests, PPM, interactive TTY). Add a **browser lab** under `web/` that ports the same engine (Grid, Rule, CaRule, patterns, RLE, FNV cycle hash, themes) to JavaScript and renders on a canvas.

GitHub Pages hosts `web/`. The CLI remains `gol`.

Why not WASM for v0.6: the crate is `no crates.io` by design. wasm-bindgen would break that contract. A tested JS port keeps the CLI pure and still ships a real website.

## Product (the website)

A lab, not a toy:

- Canvas simulation of Conway, HighLife, Seeds, Day & Night, Maze, Replicator, Life without Death, Brian's Brain
- Every built-in pattern from the Rust crate
- Draw / erase, play / pause / step, speed, wrap, age heat-map, theme cycle
- RLE import / export
- Population sparkline
- Cycle detection (same FNV-1a idea as `history.rs`)
- Shareable URL fragment (`#p=gosper&r=conway&t=neon`)
- Keyboard parity with `-i` (Space, `.`, `+`/`-`, `r`, `t`, `a`, `w`, `s`, `q`)

## Verification

- `cargo test` still green
- `node --test web/engine.test.js` — blinker period 2, block still-life, Conway births, RLE round-trip
- Pages: `https://Sebby1770.github.io/gol-rs/`
