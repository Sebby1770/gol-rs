mod grid;
mod history;
mod patterns;
mod render;
mod rng;
mod rule;
mod term;

use grid::{Grid, StepStats};
use history::{CycleCheck, History};
use patterns::{dump_ascii, dump_rle, list_patterns, load_file, save_file, seed_pattern};
use render::{render_frame, CursorGuard, RenderOpts, Style};
use rng::Rng;
use rule::{list_rules, Rule};
use term::{drain_keys, poll_key, stdin_is_tty, Key, RawMode};

use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const HELP: &str = "\
gol — Conway's Game of Life in your terminal (v0.3)

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
        --rule NAME|B#/S#  Life-like rule (default: conway / B3/S23)
        --list-rules       list built-in rules and exit
        --age              heat-map colour by cell age (green→yellow→red)
        --until-stable     stop when grid equals previous generation (period 1)
        --until-cycle      stop when any prior generation reappears; report period
    -i, --interactive      keyboard control (TTY): Space pause, . step, +/- speed, r reseed, q quit
        --quiet, --once    only print final stats (no animation)
        --no-color         disable ANSI colours
        --stats PATH       write gen,pop CSV history to PATH
    -h, --help             print this help

EXAMPLES:
    gol
    gol --pattern gosper --width 80 --height 30 --delay 60
    gol --pattern toad --until-stable --gens 100
    gol --pattern blinker --until-cycle --quiet
    gol --rule highlife --pattern random --density 0.2
    gol --rule B36/S23 --list-rules
    gol --age --pattern acorn --style block
    gol -i --pattern gosper --width 80 --height 25
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
    rule: Rule,
    age_heat: bool,
    until_stable: bool,
    until_cycle: bool,
    interactive: bool,
    quiet: bool,
    color: bool,
    stats_path: Option<PathBuf>,
    list_patterns: bool,
    list_rules: bool,
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
            rule: Rule::CONWAY,
            age_heat: false,
            until_stable: false,
            until_cycle: false,
            interactive: false,
            quiet: false,
            color: true,
            stats_path: None,
            list_patterns: false,
            list_rules: false,
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
            "--list-rules" => cfg.list_rules = true,
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
                let fmt = if i + 1 < argv.len() && !argv[i + 1].starts_with('-') {
                    i += 1;
                    argv[i].clone()
                } else {
                    "rle".into()
                };
                cfg.dump = Some(fmt);
            }
            "--style" => cfg.style = Style::parse(&take(&mut i)?)?,
            "--rule" => cfg.rule = Rule::parse(&take(&mut i)?)?,
            "--age" => cfg.age_heat = true,
            "--until-stable" => cfg.until_stable = true,
            "--until-cycle" => cfg.until_cycle = true,
            "-i" | "--interactive" => cfg.interactive = true,
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
    if cfg.list_patterns || cfg.list_rules {
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
    if cfg.interactive && cfg.quiet {
        return Err("--interactive cannot be combined with --quiet".into());
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

enum StopReason {
    MaxGens,
    Stable,
    Cycle { period: u64 },
    Quit,
}

impl StopReason {
    fn label(&self) -> String {
        match self {
            StopReason::MaxGens => "max-gens".into(),
            StopReason::Stable => "stable".into(),
            StopReason::Cycle { period } => format!("cycle-p{period}"),
            StopReason::Quit => "quit".into(),
        }
    }
}

fn reseed_grid(grid: &mut Grid, cfg: &Config, rng: &mut Rng) -> Result<(), String> {
    grid.clear();
    if let Some(ref path) = cfg.load {
        load_file(grid, path)?;
    } else {
        seed_pattern(grid, &cfg.pattern, rng, cfg.density)?;
    }
    Ok(())
}

fn run(cfg: Config) -> Result<(), String> {
    if cfg.list_patterns {
        list_patterns();
        return Ok(());
    }
    if cfg.list_rules {
        list_rules();
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

    let want_interactive = cfg.interactive && stdin_is_tty() && !cfg.quiet;
    let _raw = if want_interactive {
        Some(RawMode::enter().map_err(|e| format!("interactive mode: {e}"))?)
    } else {
        if cfg.interactive && !stdin_is_tty() {
            eprintln!("warning: --interactive ignored (stdin is not a TTY)");
        }
        None
    };

    let animate = !cfg.quiet;
    let _cursor = CursorGuard::new(animate);
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if animate {
        let _ = out.write_all(b"\x1b[?25l\x1b[2J\x1b[H");
    }

    let rule = cfg.rule;
    let mut ropts = RenderOpts {
        style: cfg.style,
        color: cfg.color && animate,
        show_stats: true,
        age_heat: cfg.age_heat,
        status_extra: String::new(),
    };

    let mut last_stats: Option<StepStats> = None;
    let mut generation: u64 = 0;
    let mut stop_reason = StopReason::MaxGens;
    let mut delay_ms = cfg.delay_ms.max(1);
    let mut paused = false;

    // History for cycle detection (also used for until-stable as period-1).
    let need_history = cfg.until_cycle || cfg.until_stable;
    let mut history = if need_history {
        Some(History::default_window())
    } else {
        None
    };

    // Record gen 0
    if let Some(ref mut f) = stats_file {
        append_stats(f, 0, grid.population()).map_err(|e| format!("stats: {e}"))?;
    }
    if let Some(ref mut hist) = history {
        // Observe gen 0; cannot be a cycle yet.
        let _ = hist.observe(&grid, 0);
    }

    loop {
        // Status line extras
        if animate {
            let mut extra = String::new();
            if want_interactive {
                extra.push_str("interactive");
                if paused {
                    extra.push_str(" PAUSED");
                }
                extra.push_str(&format!(" d={delay_ms}ms"));
            }
            if rule != Rule::CONWAY {
                if !extra.is_empty() {
                    extra.push(' ');
                }
                extra.push_str(&rule.to_string_bs());
            }
            ropts.status_extra = extra;
            if render_frame(&grid, generation, last_stats, &ropts, &mut out).is_err() {
                break;
            }
        }

        // Interactive / delay handling
        let mut do_step = true;
        if want_interactive {
            let timeout = if paused { 50 } else { delay_ms as i32 };
            // Wait for key or timeout; also drain any burst.
            if let Some(key) = poll_key(timeout) {
                match key {
                    Key::Quit => {
                        stop_reason = StopReason::Quit;
                        break;
                    }
                    Key::Space => {
                        paused = !paused;
                        do_step = false;
                    }
                    Key::Step => {
                        if paused {
                            do_step = true;
                        } else {
                            do_step = false;
                        }
                    }
                    Key::Faster => {
                        delay_ms = (delay_ms / 2).max(1);
                        do_step = !paused;
                    }
                    Key::Slower => {
                        delay_ms = delay_ms.saturating_mul(2).min(5000);
                        do_step = !paused;
                    }
                    Key::Reseed => {
                        // Advance seed a bit for variety
                        let _ = rng.next_u64();
                        reseed_grid(&mut grid, &cfg, &mut rng)?;
                        generation = 0;
                        last_stats = None;
                        if let Some(ref mut hist) = history {
                            *hist = History::default_window();
                            let _ = hist.observe(&grid, 0);
                        }
                        if let Some(ref mut f) = stats_file {
                            append_stats(f, 0, grid.population())
                                .map_err(|e| format!("stats: {e}"))?;
                        }
                        do_step = false;
                    }
                    Key::Other(_) => {
                        do_step = !paused;
                    }
                }
                // Drain remaining keys in buffer
                if let Some(k) = drain_keys() {
                    if matches!(k, Key::Quit) {
                        stop_reason = StopReason::Quit;
                        break;
                    }
                }
            } else {
                // timeout — step only if not paused
                do_step = !paused;
            }
        } else if animate {
            thread::sleep(Duration::from_millis(delay_ms));
        }

        if !do_step {
            continue;
        }

        // Max gens reached before stepping further?
        if generation >= cfg.gens {
            stop_reason = StopReason::MaxGens;
            break;
        }

        let stats = grid.step_with(&rule);
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

        if let Some(ref mut hist) = history {
            if let CycleCheck::Cycle { period } = hist.observe(&grid, generation) {
                // --until-cycle: any period; --until-stable: period 1 only
                let stop = if cfg.until_cycle {
                    true
                } else {
                    cfg.until_stable && period == 1
                };
                if stop {
                    stop_reason = if period == 1 {
                        StopReason::Stable
                    } else {
                        StopReason::Cycle { period }
                    };
                    break;
                }
            }
        }

        if generation >= cfg.gens {
            stop_reason = StopReason::MaxGens;
            break;
        }
    }

    // Final frame / quiet summary
    let reason = stop_reason.label();
    if animate {
        let mut extra = String::new();
        if rule != Rule::CONWAY {
            extra.push_str(&rule.to_string_bs());
            extra.push(' ');
        }
        extra.push_str(&format!("done:{reason}"));
        ropts.status_extra = extra;
        let _ = render_frame(&grid, generation, last_stats, &ropts, &mut out);
        let _ = out.flush();
        // Print cycle info below the grid
        if let StopReason::Cycle { period } = stop_reason {
            let _ = writeln!(
                out,
                "\x1b[Kcycle detected: period {period} at generation {generation}"
            );
            let _ = out.flush();
        } else if matches!(stop_reason, StopReason::Stable) {
            let _ = writeln!(
                out,
                "\x1b[Kstable (period 1) at generation {generation}"
            );
            let _ = out.flush();
        }
    } else {
        let pop = grid.population();
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
        if let StopReason::Cycle { period } = stop_reason {
            writeln!(out, "cycle period: {period}").map_err(|e| e.to_string())?;
        }
        if rule != Rule::CONWAY {
            writeln!(out, "rule: {}", rule.display_name()).map_err(|e| e.to_string())?;
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
    use history::{CycleCheck, History};
    use rule::Rule;

    #[test]
    fn until_stable_stops_on_block() {
        let mut g = Grid::new(6, 6);
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
        g.set(2, 2, true);
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
        assert_eq!(generation, 2);
    }

    #[test]
    fn until_cycle_blinker_period_2() {
        let mut g = Grid::new(5, 5);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g.set(3, 2, true);

        let mut hist = History::new(64);
        hist.observe(&g, 0);
        let mut generation = 0u64;
        let mut period_found = None;
        for _ in 0..10 {
            g.step_with(&Rule::CONWAY);
            generation += 1;
            if let CycleCheck::Cycle { period } = hist.observe(&g, generation) {
                period_found = Some(period);
                break;
            }
        }
        assert_eq!(period_found, Some(2));
        assert_eq!(generation, 2);
    }

    #[test]
    fn rule_parse_cli_style() {
        assert_eq!(Rule::parse("conway").unwrap(), Rule::CONWAY);
        assert_eq!(Rule::parse("B3/S23").unwrap(), Rule::CONWAY);
        assert_eq!(Rule::parse("23/3").unwrap(), Rule::CONWAY);
        assert!(Rule::parse("highlife").unwrap().births(6));
    }
}
