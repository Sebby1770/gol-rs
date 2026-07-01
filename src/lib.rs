//! gol-rs engine — Conway's Game of Life on a toroidal grid.
//!
//! This crate is deliberately dependency-free. The core simulation lives here
//! as a library so it can be driven by three front-ends that share one engine:
//!
//! * the [`bin/gol`](../gol) terminal CLI (`gol`),
//! * the built-in HTTP + WebSocket server (`gol serve`, see [`server`]),
//! * the throughput benchmark (`gol bench`).
//!
//! Keeping the rules in one place means the browser you stream to and the
//! terminal you watch are running byte-for-byte identical physics.

pub mod server;
pub mod wire;

use std::io::{self, Write};

/// A tiny, fast, deterministic xorshift64 PRNG.
///
/// Not cryptographically secure — we only need reproducible soup for the
/// `random` pattern, and a fixed seed must always yield the same board.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0xDEAD_BEEF_CAFE_BABE
        } else {
            seed
        })
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

/// A double-buffered, toroidal (wrap-around) Life grid.
pub struct Grid {
    pub w: usize,
    pub h: usize,
    cells: Vec<bool>,
    next: Vec<bool>,
}

impl Grid {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![false; w * h],
            next: vec![false; w * h],
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.cells[self.idx(x, y)]
    }

    pub fn set(&mut self, x: usize, y: usize, v: bool) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            self.cells[i] = v;
        }
    }

    pub fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        let mut n = 0u8;
        let xoffsets = [self.w - 1, 0, 1];
        let yoffsets = [self.h - 1, 0, 1];
        for &dy in &yoffsets {
            for &dx in &xoffsets {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = (x + dx) % self.w;
                let ny = (y + dy) % self.h;
                if self.cells[self.idx(nx, ny)] {
                    n += 1;
                }
            }
        }
        n
    }

    /// Advance the board by one generation (B3/S23).
    pub fn step(&mut self) {
        for y in 0..self.h {
            for x in 0..self.w {
                let n = self.count_neighbors(x, y);
                let i = self.idx(x, y);
                let alive = self.cells[i];
                self.next[i] = matches!((alive, n), (true, 2) | (true, 3) | (false, 3));
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
    }

    pub fn population(&self) -> usize {
        self.cells.iter().filter(|c| **c).count()
    }

    /// Pack the board into a 1-bit-per-cell bitset (row-major, LSB-first).
    ///
    /// This is what the WebSocket/SSE server sends on the wire: a 60×30 board
    /// is 225 bytes instead of 1800, and the browser client unpacks it back
    /// onto a canvas. See [`wire`] for the base64 transport used by SSE.
    pub fn pack(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.cells.len().div_ceil(8)];
        for (i, &alive) in self.cells.iter().enumerate() {
            if alive {
                out[i / 8] |= 1 << (i % 8);
            }
        }
        out
    }

    /// Render the board to an ANSI terminal buffer at the home position.
    pub fn render<W: Write>(&self, generation: u64, out: &mut W) -> io::Result<()> {
        out.write_all(b"\x1b[H")?;
        writeln!(
            out,
            "\x1b[1;36mgol-rs\x1b[0m  gen \x1b[33m{:>5}\x1b[0m  pop \x1b[35m{:>5}\x1b[0m  grid \x1b[33m{}x{}\x1b[0m  (Ctrl-C to quit)\x1b[K",
            generation,
            self.population(),
            self.w,
            self.h
        )?;
        for y in 0..self.h {
            let mut run_alive = false;
            for x in 0..self.w {
                let alive = self.cells[self.idx(x, y)];
                if alive && !run_alive {
                    out.write_all(b"\x1b[32m")?;
                    run_alive = true;
                } else if !alive && run_alive {
                    out.write_all(b"\x1b[0m")?;
                    run_alive = false;
                }
                out.write_all(if alive { "██".as_bytes() } else { b"  " })?;
            }
            if run_alive {
                out.write_all(b"\x1b[0m")?;
            }
            out.write_all(b"\x1b[K\n")?;
        }
        out.flush()
    }
}

pub fn place_glider(g: &mut Grid, ox: usize, oy: usize) {
    for &(dx, dy) in &[(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
        g.set(ox + dx, oy + dy, true);
    }
}

pub fn place_from_str(g: &mut Grid, ox: usize, oy: usize, art: &str) {
    for (dy, line) in art.lines().enumerate() {
        for (dx, c) in line.chars().enumerate() {
            if matches!(c, 'o' | 'O' | '#' | '*') {
                g.set(ox + dx, oy + dy, true);
            }
        }
    }
}

pub const PULSAR: &str = "\
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

pub const GOSPER: &str = "\
........................o...........
......................o.o...........
............oo......oo............oo
...........o...o....oo............oo
oo........o.....o...oo..............
oo........o...o.oo....o.o...........
..........o.....o.......o...........
...........o...o....................
............oo......................";

/// Seed `grid` with a named starting pattern.
pub fn seed_pattern(
    grid: &mut Grid,
    name: &str,
    rng: &mut Rng,
    density: f64,
) -> Result<(), String> {
    match name {
        "random" => {
            for c in grid.cells.iter_mut() {
                *c = rng.next_f64() < density;
            }
        }
        "glider" => place_glider(grid, 1, 1),
        "pulsar" => {
            let ox = grid.w.saturating_sub(13) / 2;
            let oy = grid.h.saturating_sub(13) / 2;
            place_from_str(grid, ox, oy, PULSAR);
        }
        "gosper" => {
            if grid.w < 40 || grid.h < 12 {
                return Err("gosper needs at least a 40x12 grid".into());
            }
            place_from_str(grid, 1, 1, GOSPER);
        }
        other => return Err(format!("unknown pattern: {other}")),
    }
    Ok(())
}

/// Run `gens` generations headless and return `(generations, elapsed_seconds)`.
///
/// Powers `gol bench` — the throughput number it prints is generations/sec,
/// which is the single-core ceiling the streaming server draws from.
pub fn bench(w: usize, h: usize, gens: u64, seed: u64, density: f64) -> (u64, f64) {
    use std::time::Instant;
    let mut grid = Grid::new(w, h);
    let mut rng = Rng::new(seed);
    for c in grid.cells.iter_mut() {
        *c = rng.next_f64() < density;
    }
    let start = Instant::now();
    for _ in 0..gens {
        grid.step();
    }
    // Touch the population so the optimiser cannot elide the loop.
    std::hint::black_box(grid.population());
    (gens, start.elapsed().as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blinker_oscillates() {
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);
        g.step();
        assert!(!g.get(1, 2));
        assert!(!g.get(3, 2));
        assert!(g.get(2, 1));
        assert!(g.get(2, 2));
        assert!(g.get(2, 3));
        g.step();
        assert!(g.get(1, 2));
        assert!(g.get(2, 2));
        assert!(g.get(3, 2));
    }

    #[test]
    fn block_is_still_life() {
        let mut g = Grid::new(4, 4);
        g.set(1, 1, true);
        g.set(2, 1, true);
        g.set(1, 2, true);
        g.set(2, 2, true);
        let snapshot = g.cells.clone();
        g.step();
        assert_eq!(g.cells, snapshot);
    }

    #[test]
    fn toroidal_wrap_counts_corner_neighbor() {
        let mut g = Grid::new(5, 5);
        g.set(0, 0, true);
        assert_eq!(g.count_neighbors(4, 4), 1);
    }

    #[test]
    fn lonely_cell_dies() {
        let mut g = Grid::new(5, 5);
        g.set(2, 2, true);
        g.step();
        assert_eq!(g.population(), 0);
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn pack_roundtrips_population() {
        let mut g = Grid::new(10, 10);
        g.set(0, 0, true);
        g.set(9, 9, true);
        g.set(3, 4, true);
        let packed = g.pack();
        // Exactly three bits set across the whole bitset.
        let ones: u32 = packed.iter().map(|b| b.count_ones()).sum();
        assert_eq!(ones, 3);
        // First cell (index 0) is bit 0 of byte 0.
        assert_eq!(packed[0] & 1, 1);
    }

    #[test]
    fn seed_pattern_rejects_unknown() {
        let mut g = Grid::new(20, 20);
        let mut rng = Rng::new(1);
        assert!(seed_pattern(&mut g, "nope", &mut rng, 0.3).is_err());
    }
}
