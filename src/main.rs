mod grid;
mod patterns;
mod render;
mod rng;

use grid::{Grid, StepStats};
use patterns::{dump_ascii, dump_rle, list_patterns, load_file, save_file, seed_pattern};
use render::{render_frame, CursorGuard, RenderOpts, Style};
use rng::Rng;

use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const HELP: &str = "\
gol — Conway's Game of Life in your terminal (v0.2)

USAGE:
    gol [OPTIONS]

OPTIONS:
    -W, --width N          grid width  (default: 60)
    -H, --height N         grid height (default: 30)
    -g, --gens N           max generations (default: 500)
    -d, --delay MS         delay between frames in ms (default: 80)
    -s, --seed N           RNG seed for random pattern (default: time-based)
    -p, --pattern NAME     initial pattern (default: random)
        --density F        density 0.0-1.0 for random (default: 0.25)
        --list-patterns    list built-in patterns and exit
        --load PATH        seed grid from RLE / Life 1.05 file
        --save PATH        save final grid as RLE
        --save-every N     also write RLE every N generations (with --save)
        --dump [FMT]       print final pattern: rle (default) | ascii
        --style MODE       block | braille | dots (default: block)
        --until-stable     stop when grid equals previous generation
        --quiet, --once    only print final stats (no animation)
        --no-color         disable ANSI colours
        --stats PATH       write gen,pop CSV history to PATH
    -h, --help             print this help

EXAMPLES:
    gol
    gol --pattern gosper --width 80 --height 30 --delay 60
    gol --pattern toad --until-stable --gens 100
    gol --load gun.rle --style braille --gens 200
    gol --pattern acorn --quiet --dump ascii --stats pop.csv
    gol --list-patterns
";

struct Config {
    w: usize,
    h: usize,
    gens: u64,
    delay_ms: u64,
    seed: u64,
    pattern: String,
    density: f64,
    load: Option<PathBuf>,
    save: Option<PathBuf>,
    save_every: Option<u64>,
    dump: Option<String>,
    style: Style,
    until_stable: bool,
    quiet: bool,
    color: bool,
    stats_path: Option<PathBuf>,
    list_patterns: bool,
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
            load: None,
            save: None,
            save_every: None,
            dump: None,
            style: Style::Block,
            until_stable: false,
            quiet: false,
            color: true,
            stats_path: None,
            list_patterns: false,
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
            "--list-patterns" => cfg.list_patterns = true,
            "--load" => cfg.load = Some(PathBuf::from(take(&mut i)?)),
            "--save" => cfg.save = Some(PathBuf::from(take(&mut i)?)),
            "--save-every" => {
                cfg.save_every = Some(
                    take(&mut i)?
                        .parse()
                        .map_err(|e| format!("--save-every: {e}"))?,
                );
            }
            "--dump" => {
                // optional format
                let fmt = if i + 1 < argv.len() && !argv[i + 1].starts_with('-') {
                    i += 1;
                    argv[i].clone()
                } else {
                    "rle".into()
                };
                cfg.dump = Some(fmt);
            }
            "--style" => cfg.style = Style::parse(&take(&mut i)?)?,
            "--until-stable" => cfg.until_stable = true,
            "--quiet" | "--once" => cfg.quiet = true,
            "--no-color" => cfg.color = false,
            "--stats" => cfg.stats_path = Some(PathBuf::from(take(&mut i)?)),
            "-h" | "--help" => {
                print!("{HELP}");
                process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    if cfg.list_patterns {
        return Ok(cfg);
    }
    if cfg.w < 4 || cfg.h < 4 {
        return Err("grid too small (min 4x4)".into());
    }
    if !(0.0..=1.0).contains(&cfg.density) {
        return Err("density must be between 0.0 and 1.0".into());
    }
    if let Some(n) = cfg.save_every {
        if n == 0 {
            return Err("--save-every must be >= 1".into());
        }
        if cfg.save.is_none() {
            return Err("--save-every requires --save PATH".into());
        }
    }
    if let Some(ref fmt) = cfg.dump {
        let f = fmt.to_ascii_lowercase();
        if f != "rle" && f != "ascii" {
            return Err("--dump format must be rle or ascii".into());
        }
    }
    Ok(cfg)
}

fn write_stats_header(f: &mut File) -> io::Result<()> {
    writeln!(f, "gen,pop")
}

fn append_stats(f: &mut File, gen: u64, pop: usize) -> io::Result<()> {
    writeln!(f, "{gen},{pop}")
}

fn save_snapshot(grid: &Grid, base: &PathBuf, gen: u64) -> Result<(), String> {
    let path = if gen == u64::MAX {
        base.clone()
    } else {
        // insert gen before extension: name-genN.rle
        let stem = base
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("pattern");
        let ext = base
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("rle");
        let parent = base.parent().unwrap_or_else(|| std::path::Path::new("."));
        parent.join(format!("{stem}-gen{gen}.{ext}"))
    };
    save_file(grid, &path)
}

fn run(cfg: Config) -> Result<(), String> {
    if cfg.list_patterns {
        list_patterns();
        return Ok(());
    }

    let mut grid = Grid::new(cfg.w, cfg.h);
    let mut rng = Rng::new(cfg.seed);

    if let Some(ref path) = cfg.load {
        load_file(&mut grid, path)?;
    } else {
        seed_pattern(&mut grid, &cfg.pattern, &mut rng, cfg.density)?;
    }

    let mut stats_file = if let Some(ref p) = cfg.stats_path {
        let mut f = File::create(p).map_err(|e| format!("stats file: {e}"))?;
        write_stats_header(&mut f).map_err(|e| format!("stats file: {e}"))?;
        Some(f)
    } else {
        None
    };

    let animate = !cfg.quiet;
    let _cursor = CursorGuard::new(animate);
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if animate {
        let _ = out.write_all(b"\x1b[?25l\x1b[2J\x1b[H");
    }

    let ropts = RenderOpts {
        style: cfg.style,
        color: cfg.color && animate,
        show_stats: true,
    };

    let mut last_stats: Option<StepStats> = None;
    let mut generation: u64 = 0;
    let mut stopped_stable = false;

    // Record gen 0
    if let Some(ref mut f) = stats_file {
        append_stats(f, 0, grid.population()).map_err(|e| format!("stats: {e}"))?;
    }

    while generation < cfg.gens {
        if animate {
            if render_frame(&grid, generation, last_stats, &ropts, &mut out).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(cfg.delay_ms));
        }

        // Check stable before stepping past last requested gen: we step after display.
        if generation + 1 >= cfg.gens && !cfg.until_stable {
            // Will exit after this display on next loop check — but we still want to step?
            // Original: for gen in 0..gens { render; sleep; step } — so gens steps after gens frames.
            // Keep same: render current, then step, until we've shown `gens` frames... 
            // Actually original runs gens iterations of render+step, ending after last step
            // without showing final. We keep: show generation, step, increment.
        }

        let prev = if cfg.until_stable {
            Some(grid.clone())
        } else {
            None
        };

        let stats = grid.step();
        last_stats = Some(stats);
        generation += 1;

        if let Some(ref mut f) = stats_file {
            append_stats(f, generation, grid.population()).map_err(|e| format!("stats: {e}"))?;
        }

        if let (Some(ref base), Some(every)) = (&cfg.save, cfg.save_every) {
            if generation % every == 0 {
                save_snapshot(&grid, base, generation)?;
            }
        }

        if let Some(ref p) = prev {
            if grid.equals(p) {
                stopped_stable = true;
                break;
            }
        }

        if generation >= cfg.gens {
            break;
        }
    }

    // Final frame / quiet summary
    if animate {
        let _ = render_frame(&grid, generation, last_stats, &ropts, &mut out);
        let _ = out.flush();
    } else {
        let pop = grid.population();
        let reason = if stopped_stable {
            "stable"
        } else {
            "max-gens"
        };
        if cfg.color {
            writeln!(
                out,
                "\x1b[1;36mgol-rs\x1b[0m  gen \x1b[33m{generation}\x1b[0m  pop \x1b[35m{pop}\x1b[0m  ({reason})"
            )
            .map_err(|e| e.to_string())?;
        } else {
            writeln!(out, "gol-rs  gen {generation}  pop {pop}  ({reason})")
                .map_err(|e| e.to_string())?;
        }
        if let Some(s) = last_stats {
            writeln!(out, "last step: +{}/-{}", s.births, s.deaths).map_err(|e| e.to_string())?;
        }
        let _ = out.flush();
    }

    if let Some(ref path) = cfg.save {
        save_file(&grid, path)?;
        if !cfg.quiet {
            eprintln!("saved RLE → {}", path.display());
        }
    }

    if let Some(ref fmt) = cfg.dump {
        match fmt.to_ascii_lowercase().as_str() {
            "ascii" => print!("{}", dump_ascii(&grid)),
            _ => print!("{}", dump_rle(&grid)),
        }
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

    if let Err(e) = run(cfg) {
        eprintln!("error: {e}");
        process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid::Grid;

    #[test]
    fn until_stable_stops_on_block() {
        let mut g = Grid::new(6, 6);
        // block still life
        g.set(2, 2, true);
        g.set(3, 2, true);
        g.set(2, 3, true);
        g.set(3, 3, true);

        let mut generation = 0u64;
        let max = 100u64;
        let mut stopped_stable = false;
        while generation < max {
            let prev = g.clone();
            g.step();
            generation += 1;
            if g.equals(&prev) {
                stopped_stable = true;
                break;
            }
        }
        assert!(stopped_stable);
        assert_eq!(generation, 1);
        assert_eq!(g.population(), 4);
    }

    #[test]
    fn until_stable_dies_out() {
        let mut g = Grid::new(5, 5);
        g.set(2, 2, true); // lonely cell
        let mut generation = 0u64;
        let mut stopped = false;
        while generation < 50 {
            let prev = g.clone();
            g.step();
            generation += 1;
            if g.equals(&prev) {
                stopped = true;
                break;
            }
        }
        assert!(stopped);
        assert_eq!(g.population(), 0);
        assert_eq!(generation, 2); // dies at gen1 empty, gen2 empty==prev
    }
}
