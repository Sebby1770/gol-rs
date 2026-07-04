use std::env;
use std::io::{self, Write};
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gol_rs::server::{ServerConfig, serve};
use gol_rs::{Grid, Rng, bench, seed_pattern};

const HELP: &str = "\
gol — Conway's Game of Life in your terminal (and now on the web)

USAGE:
    gol [OPTIONS]                 animate in the terminal (default)
    gol serve [SERVE OPTIONS]     stream the simulation over HTTP + WebSocket
    gol bench [BENCH OPTIONS]     measure raw generations/second (throughput)

OPTIONS (animate):
    -W, --width N        grid width  (default: 60)
    -H, --height N       grid height (default: 30)
    -g, --gens N         number of generations to run (default: 500)
    -d, --delay MS       delay between frames in ms (default: 80)
    -s, --seed N         RNG seed for the random pattern (default: time-based)
    -p, --pattern NAME   initial pattern: random | glider | pulsar | gosper
                         (default: random)
        --density F      density 0.0-1.0 for the random pattern (default: 0.25)
    -h, --help           print this help

SERVE OPTIONS:
    -a, --addr HOST:PORT bind address (default: 127.0.0.1:8080)
    -W, --width N        grid width  (default: 80)
    -H, --height N       grid height (default: 40)
    -d, --delay MS       ms per generation (default: 100)
    -p, --pattern NAME   initial pattern (default: random)
    -s, --seed N         RNG seed (default: time-based)
        --density F      random density (default: 0.28)

BENCH OPTIONS:
    -W, --width N        grid width  (default: 256)
    -H, --height N       grid height (default: 256)
    -g, --gens N         generations to run (default: 2000)
    -s, --seed N         RNG seed (default: 1)

EXAMPLES:
    gol
    gol --pattern gosper --width 80 --height 30 --delay 60
    gol serve --addr 0.0.0.0:8080 --pattern gosper
    gol bench --width 512 --height 512 --gens 1000
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

fn time_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xC0FFEE)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            w: 60,
            h: 30,
            gens: 500,
            delay_ms: 80,
            seed: time_seed(),
            pattern: "random".into(),
            density: 0.25,
        }
    }
}

/// Pull the next value for a flag, or bail with a helpful message.
fn take(argv: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    *i += 1;
    argv.get(*i)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_args(argv: &[String]) -> Result<Config, String> {
    let mut cfg = Config::default();
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].clone();
        match a.as_str() {
            "-W" | "--width" => {
                cfg.w = take(argv, &mut i, "--width")?
                    .parse()
                    .map_err(|e| format!("--width: {e}"))?
            }
            "-H" | "--height" => {
                cfg.h = take(argv, &mut i, "--height")?
                    .parse()
                    .map_err(|e| format!("--height: {e}"))?
            }
            "-g" | "--gens" => {
                cfg.gens = take(argv, &mut i, "--gens")?
                    .parse()
                    .map_err(|e| format!("--gens: {e}"))?
            }
            "-d" | "--delay" => {
                cfg.delay_ms = take(argv, &mut i, "--delay")?
                    .parse()
                    .map_err(|e| format!("--delay: {e}"))?
            }
            "-s" | "--seed" => {
                cfg.seed = take(argv, &mut i, "--seed")?
                    .parse()
                    .map_err(|e| format!("--seed: {e}"))?
            }
            "-p" | "--pattern" => cfg.pattern = take(argv, &mut i, "--pattern")?,
            "--density" => {
                cfg.density = take(argv, &mut i, "--density")?
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

struct CursorGuard;
impl Drop for CursorGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = stdout.write_all(b"\x1b[?25h\x1b[0m\n");
        let _ = stdout.flush();
    }
}

fn run_animation(cfg: Config) {
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

fn run_serve(argv: &[String]) -> Result<(), String> {
    let mut cfg = ServerConfig {
        addr: "127.0.0.1:8080".into(),
        w: 80,
        h: 40,
        delay_ms: 100,
        pattern: "random".into(),
        seed: time_seed(),
        density: 0.28,
        rate_burst: 60.0,
        rate_per_sec: 10.0,
    };
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "-a" | "--addr" => cfg.addr = take(argv, &mut i, "--addr")?,
            "-W" | "--width" => {
                cfg.w = take(argv, &mut i, "--width")?
                    .parse()
                    .map_err(|e| format!("--width: {e}"))?
            }
            "-H" | "--height" => {
                cfg.h = take(argv, &mut i, "--height")?
                    .parse()
                    .map_err(|e| format!("--height: {e}"))?
            }
            "-d" | "--delay" => {
                cfg.delay_ms = take(argv, &mut i, "--delay")?
                    .parse()
                    .map_err(|e| format!("--delay: {e}"))?
            }
            "-p" | "--pattern" => cfg.pattern = take(argv, &mut i, "--pattern")?,
            "-s" | "--seed" => {
                cfg.seed = take(argv, &mut i, "--seed")?
                    .parse()
                    .map_err(|e| format!("--seed: {e}"))?
            }
            "--density" => {
                cfg.density = take(argv, &mut i, "--density")?
                    .parse()
                    .map_err(|e| format!("--density: {e}"))?
            }
            "-h" | "--help" => {
                print!("{HELP}");
                process::exit(0);
            }
            other => return Err(format!("unknown serve argument: {other}")),
        }
        i += 1;
    }
    if cfg.w < 4 || cfg.h < 4 {
        return Err("grid too small (min 4x4)".into());
    }
    serve(cfg).map_err(|e| e.to_string())
}

fn run_bench(argv: &[String]) -> Result<(), String> {
    let (mut w, mut h, mut gens, mut seed) = (256usize, 256usize, 2000u64, 1u64);
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "-W" | "--width" => {
                w = take(argv, &mut i, "--width")?
                    .parse()
                    .map_err(|e| format!("--width: {e}"))?
            }
            "-H" | "--height" => {
                h = take(argv, &mut i, "--height")?
                    .parse()
                    .map_err(|e| format!("--height: {e}"))?
            }
            "-g" | "--gens" => {
                gens = take(argv, &mut i, "--gens")?
                    .parse()
                    .map_err(|e| format!("--gens: {e}"))?
            }
            "-s" | "--seed" => {
                seed = take(argv, &mut i, "--seed")?
                    .parse()
                    .map_err(|e| format!("--seed: {e}"))?
            }
            "-h" | "--help" => {
                print!("{HELP}");
                process::exit(0);
            }
            other => return Err(format!("unknown bench argument: {other}")),
        }
        i += 1;
    }
    let (ran, secs) = bench(w, h, gens, seed, 0.3);
    let cells = (w * h) as f64;
    let gps = ran as f64 / secs;
    println!("gol bench — {w}x{h} grid, {ran} generations");
    println!("  elapsed:     {secs:.3} s");
    println!("  throughput:  {gps:.0} generations/sec");
    println!("  cell-updates: {:.1} million/sec", gps * cells / 1e6);
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    let result = match args.first().map(String::as_str) {
        Some("serve") => run_serve(&args[1..]),
        Some("bench") => run_bench(&args[1..]),
        _ => match parse_args(&args) {
            Ok(cfg) => {
                run_animation(cfg);
                Ok(())
            }
            Err(e) => Err(e),
        },
    };

    if let Err(e) = result {
        eprintln!("error: {e}\n\n{HELP}");
        process::exit(2);
    }
}
