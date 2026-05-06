use std::env;
use std::io::{self, Write};
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const HELP: &str = "\
gol — Conway's Game of Life in your terminal

USAGE:
    gol [OPTIONS]

OPTIONS:
    -W, --width N        grid width  (default: 60)
    -H, --height N       grid height (default: 30)
    -g, --gens N         number of generations to run (default: 500)
    -d, --delay MS       delay between frames in ms (default: 80)
    -s, --seed N         RNG seed for the random pattern (default: time-based)
    -p, --pattern NAME   initial pattern: random | glider | pulsar | gosper
                         (default: random)
        --density F      density 0.0-1.0 for the random pattern (default: 0.25)
    -h, --help           print this help

EXAMPLES:
    gol
    gol --pattern gosper --width 80 --height 30 --delay 60
    gol --pattern pulsar --gens 200
    gol --seed 42 --density 0.35
";

struct Config {
    w: usize,
    h: usize,
    gens: u64,
    delay_ms: u64,
    seed: u64,
    pattern: String,
    density: f64,
}

impl Default for Config {
    fn default() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xC0FFEE);
        Self {
            w: 60,
            h: 30,
            gens: 500,
            delay_ms: 80,
            seed,
            pattern: "random".into(),
            density: 0.25,
        }
    }
}

fn parse_args() -> Result<Config, String> {
    let mut cfg = Config::default();
    let argv: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].clone();
        let take = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            argv.get(*i)
                .cloned()
                .ok_or_else(|| format!("{a} requires a value"))
        };
        match a.as_str() {
            "-W" | "--width" => {
                cfg.w = take(&mut i)?.parse().map_err(|e| format!("--width: {e}"))?
            }
            "-H" | "--height" => {
                cfg.h = take(&mut i)?
                    .parse()
                    .map_err(|e| format!("--height: {e}"))?
            }
            "-g" | "--gens" => {
                cfg.gens = take(&mut i)?.parse().map_err(|e| format!("--gens: {e}"))?
            }
            "-d" | "--delay" => {
                cfg.delay_ms = take(&mut i)?.parse().map_err(|e| format!("--delay: {e}"))?
            }
            "-s" | "--seed" => {
                cfg.seed = take(&mut i)?.parse().map_err(|e| format!("--seed: {e}"))?
            }
            "-p" | "--pattern" => cfg.pattern = take(&mut i)?,
            "--density" => {
                cfg.density = take(&mut i)?
                    .parse()
                    .map_err(|e| format!("--density: {e}"))?
            }
            "-h" | "--help" => {
                print!("{HELP}");
                process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    if cfg.w < 4 || cfg.h < 4 {
        return Err("grid too small (min 4x4)".into());
    }
    if !(0.0..=1.0).contains(&cfg.density) {
        return Err("density must be between 0.0 and 1.0".into());
    }
    Ok(cfg)
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0xDEAD_BEEF_CAFE_BABE
        } else {
            seed
        })
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

struct Grid {
    w: usize,
    h: usize,
    cells: Vec<bool>,
    next: Vec<bool>,
}

impl Grid {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![false; w * h],
            next: vec![false; w * h],
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    fn set(&mut self, x: usize, y: usize, v: bool) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            self.cells[i] = v;
        }
    }

    fn count_neighbors(&self, x: usize, y: usize) -> u8 {
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

    fn step(&mut self) {
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

    fn population(&self) -> usize {
        self.cells.iter().filter(|c| **c).count()
    }

    fn render<W: Write>(&self, generation: u64, out: &mut W) -> io::Result<()> {
        out.write_all(b"\x1b[H")?;
        write!(
            out,
            "\x1b[1;36mgol-rs\x1b[0m  gen \x1b[33m{:>5}\x1b[0m  pop \x1b[35m{:>5}\x1b[0m  grid \x1b[33m{}x{}\x1b[0m  (Ctrl-C to quit)\x1b[K\n",
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

struct CursorGuard;

impl Drop for CursorGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = stdout.write_all(b"\x1b[?25h\x1b[0m\n");
        let _ = stdout.flush();
    }
}

fn place_glider(g: &mut Grid, ox: usize, oy: usize) {
    for &(dx, dy) in &[(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
        g.set(ox + dx, oy + dy, true);
    }
}

fn place_from_str(g: &mut Grid, ox: usize, oy: usize, art: &str) {
    for (dy, line) in art.lines().enumerate() {
        for (dx, c) in line.chars().enumerate() {
            if matches!(c, 'o' | 'O' | '#' | '*') {
                g.set(ox + dx, oy + dy, true);
            }
        }
    }
}

const PULSAR: &str = "\
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

const GOSPER: &str = "\
........................o...........
......................o.o...........
............oo......oo............oo
...........o...o....oo............oo
oo........o.....o...oo..............
oo........o...o.oo....o.o...........
..........o.....o.......o...........
...........o...o....................
............oo......................";

fn seed_pattern(grid: &mut Grid, name: &str, rng: &mut Rng, density: f64) -> Result<(), String> {
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

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}\n\n{HELP}");
            process::exit(2);
        }
    };

    let mut grid = Grid::new(cfg.w, cfg.h);
    let mut rng = Rng::new(cfg.seed);
    if let Err(e) = seed_pattern(&mut grid, &cfg.pattern, &mut rng, cfg.density) {
        eprintln!("error: {e}");
        process::exit(2);
    }

    let _cursor = CursorGuard;
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let _ = out.write_all(b"\x1b[?25l\x1b[2J\x1b[H");

    for generation in 0..cfg.gens {
        if grid.render(generation, &mut out).is_err() {
            break;
        }
        thread::sleep(Duration::from_millis(cfg.delay_ms));
        grid.step();
    }
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
        assert!(!g.cells[g.idx(1, 2)]);
        assert!(!g.cells[g.idx(3, 2)]);
        assert!(g.cells[g.idx(2, 1)]);
        assert!(g.cells[g.idx(2, 2)]);
        assert!(g.cells[g.idx(2, 3)]);
        g.step();
        assert!(g.cells[g.idx(1, 2)]);
        assert!(g.cells[g.idx(2, 2)]);
        assert!(g.cells[g.idx(3, 2)]);
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
}
