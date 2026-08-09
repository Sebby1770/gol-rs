//! `gol` CLI binary — thin front-end over the `gol_rs` library.

mod term;

use gol_rs::grid::{Grid, StepStats};
use gol_rs::history::{CycleCheck, History};
use gol_rs::patterns::{dump_ascii, dump_rle, list_patterns, load_file, save_file, seed_pattern};
use gol_rs::render::{export_ppm, render_frame, CursorGuard, RenderOpts, Style};
use gol_rs::rng::Rng;
use gol_rs::rule::{list_rules, Rule};
use gol_rs::theme::Theme;
use gol_rs::transform::{flip_h_in_place, flip_v_in_place, rotate90_in_place};
use term::{drain_keys, poll_key, stdin_is_tty, Key, RawMode};

use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const HELP: &str = "\
gol — Life-like cellular automata in your terminal (v0.4)

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
        --age              heat-map colour by cell age (within theme)
        --theme NAME       classic|neon|fire|ocean|mono (default: classic)
        --finite, --no-wrap  hard edges (neighbors off-grid count as dead)
        --rotate90         rotate seed 90° clockwise after load/pattern
        --flip-h           flip seed horizontally
        --flip-v           flip seed vertically
        --until-stable     stop when grid equals previous generation (period 1)
        --until-cycle      stop when any prior generation reappears; report period
    -i, --interactive      keyboard control (TTY)
        --quiet, --once    only print final stats (no animation)
        --summary          always print final summary (even when animating)
        --no-color         disable ANSI colours
        --stats PATH       write gen,pop,births,deaths CSV history to PATH
        --export-ppm PATH  write final frame as P6 PPM (RGB from theme/age)
        --bench            run gens without render; print gens/sec & cells/sec
    -h, --help             print this help

INTERACTIVE KEYS (-i):
    Space  pause/resume    .  step    +/-  speed
    r      reseed          t  cycle theme    a  toggle age heat-map
    w      toggle wrap     q  quit

EXAMPLES:
    gol
    gol --pattern gosper --width 80 --height 30 --delay 60
    gol --pattern blinker --until-cycle --quiet
    gol --rule highlife --theme neon --age
    gol --finite --pattern glider --width 40 --height 20
    gol --bench --width 200 --height 100 --gens 200 --seed 1
    gol --pattern acorn --export-ppm out.ppm --quiet --gens 100
    gol -i --pattern gosper --theme fire
    gol --load gun.rle --rotate90 --flip-h --stats pop.csv
    gol --list-patterns
    gol --list-rules
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
    theme: Theme,
    wrap: bool,
    rotate90: bool,
    flip_h: bool,
    flip_v: bool,
    until_stable: bool,
    until_cycle: bool,
    interactive: bool,
    quiet: bool,
    summary: bool,
    color: bool,
    stats_path: Option<PathBuf>,
    export_ppm: Option<PathBuf>,
    bench: bool,
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
            theme: Theme::CLASSIC,
            wrap: true,
            rotate90: false,
            flip_h: false,
            flip_v: false,
            until_stable: false,
            until_cycle: false,
            interactive: false,
            quiet: false,
            summary: false,
            color: true,
            stats_path: None,
            export_ppm: None,
            bench: false,
            list_patterns: false,
            list_rules: false,
        }
    }
}

/// Aggregate run statistics.
#[derive(Debug, Clone, Default)]
struct RunStats {
    max_pop: usize,
    min_pop: usize,
    gen_at_max_pop: u64,
    total_births: u64,
    total_deaths: u64,
    /// True once gen ≥ 1 has been observed for min tracking.
    started: bool,
}

impl RunStats {
    fn observe_gen0(&mut self, pop: usize) {
        self.max_pop = pop;
        self.min_pop = pop;
        self.gen_at_max_pop = 0;
        self.started = false;
    }

    fn observe_step(&mut self, gen: u64, pop: usize, stats: StepStats) {
        self.total_births += stats.births as u64;
        self.total_deaths += stats.deaths as u64;
        if !self.started {
            // min_pop is tracked after gen0
            self.min_pop = pop;
            self.started = true;
        } else {
            self.min_pop = self.min_pop.min(pop);
        }
        if pop > self.max_pop {
            self.max_pop = pop;
            self.gen_at_max_pop = gen;
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
            "--theme" => cfg.theme = Theme::parse(&take(&mut i)?)?,
            "--finite" | "--no-wrap" => cfg.wrap = false,
            "--rotate90" => cfg.rotate90 = true,
            "--flip-h" => cfg.flip_h = true,
            "--flip-v" => cfg.flip_v = true,
            "--until-stable" => cfg.until_stable = true,
            "--until-cycle" => cfg.until_cycle = true,
            "-i" | "--interactive" => cfg.interactive = true,
            "--quiet" | "--once" => cfg.quiet = true,
            "--summary" => cfg.summary = true,
            "--no-color" => cfg.color = false,
            "--stats" => cfg.stats_path = Some(PathBuf::from(take(&mut i)?)),
            "--export-ppm" => cfg.export_ppm = Some(PathBuf::from(take(&mut i)?)),
            "--bench" => cfg.bench = true,
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
    if cfg.bench && cfg.interactive {
        return Err("--bench cannot be combined with --interactive".into());
    }
    Ok(cfg)
}

fn write_stats_header(f: &mut File) -> io::Result<()> {
    writeln!(f, "gen,pop,births,deaths")
}

fn append_stats(f: &mut File, gen: u64, pop: usize, births: usize, deaths: usize) -> io::Result<()> {
    writeln!(f, "{gen},{pop},{births},{deaths}")
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

fn apply_transforms(grid: &mut Grid, cfg: &Config) {
    if cfg.rotate90 {
        rotate90_in_place(grid);
    }
    if cfg.flip_h {
        flip_h_in_place(grid);
    }
    if cfg.flip_v {
        flip_v_in_place(grid);
    }
}

fn reseed_grid(grid: &mut Grid, cfg: &Config, rng: &mut Rng) -> Result<(), String> {
    // Preserve wrap; rebuild size if rotate swapped dims — reseed uses cfg w/h.
    let wrap = grid.wrap;
    *grid = Grid::with_wrap(cfg.w, cfg.h, wrap);
    if let Some(ref path) = cfg.load {
        load_file(grid, path)?;
    } else {
        seed_pattern(grid, &cfg.pattern, rng, cfg.density)?;
    }
    apply_transforms(grid, cfg);
    Ok(())
}

fn print_summary(
    out: &mut impl Write,
    generation: u64,
    pop: usize,
    reason: &str,
    run: &RunStats,
    last_stats: Option<StepStats>,
    rule: &Rule,
    theme: Theme,
    wrap: bool,
    color: bool,
) -> io::Result<()> {
    if color {
        writeln!(
            out,
            "{}gol-rs\x1b[0m  gen {}{generation}\x1b[0m  pop {}{pop}\x1b[0m  ({reason})",
            theme.title_ansi(),
            theme.accent_ansi(),
            theme.pop_ansi(),
        )?;
    } else {
        writeln!(out, "gol-rs  gen {generation}  pop {pop}  ({reason})")?;
    }
    writeln!(
        out,
        "  max_pop {} (gen {})  min_pop {}  births {}  deaths {}",
        run.max_pop, run.gen_at_max_pop, run.min_pop, run.total_births, run.total_deaths
    )?;
    if let Some(s) = last_stats {
        writeln!(out, "  last step: +{}/-{}", s.births, s.deaths)?;
    }
    if rule != &Rule::CONWAY {
        writeln!(out, "  rule: {}", rule.display_name())?;
    }
    if theme.name != "classic" {
        writeln!(out, "  theme: {}", theme.name)?;
    }
    if !wrap {
        writeln!(out, "  topology: finite")?;
    }
    Ok(())
}

fn run_bench(cfg: &Config, grid: &mut Grid) -> Result<(), String> {
    let rule = cfg.rule;
    let cells = (grid.w * grid.h) as u64;
    let gens = cfg.gens;
    // Warmup one step so first allocs don't skew (buffers already sized).
    let _ = grid.step_with(&rule);

    let start = Instant::now();
    for _ in 0..gens {
        grid.step_with(&rule);
    }
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64().max(1e-12);
    let gps = gens as f64 / secs;
    let cps = (gens as f64) * (cells as f64) / secs;
    println!(
        "bench  gens={gens}  grid={}x{}  cells={cells}  wrap={}  rule={}",
        grid.w,
        grid.h,
        grid.wrap,
        rule.to_string_bs()
    );
    println!(
        "  time={:.3}s  gens/sec={:.1}  cells/sec={:.0}",
        secs, gps, cps
    );
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

    let mut grid = Grid::with_wrap(cfg.w, cfg.h, cfg.wrap);
    let mut rng = Rng::new(cfg.seed);

    if let Some(ref path) = cfg.load {
        load_file(&mut grid, path)?;
    } else {
        seed_pattern(&mut grid, &cfg.pattern, &mut rng, cfg.density)?;
    }
    apply_transforms(&mut grid, &cfg);

    if cfg.bench {
        return run_bench(&cfg, &mut grid);
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
    let mut theme = cfg.theme;
    let mut age_heat = cfg.age_heat;
    let mut ropts = RenderOpts {
        style: cfg.style,
        color: cfg.color && animate,
        show_stats: true,
        age_heat,
        theme,
        status_extra: String::new(),
    };

    let mut last_stats: Option<StepStats> = None;
    let mut generation: u64 = 0;
    let mut stop_reason = StopReason::MaxGens;
    let mut delay_ms = cfg.delay_ms.max(1);
    let mut paused = false;
    let mut run_stats = RunStats::default();

    // History for cycle detection (also used for until-stable as period-1).
    let need_history = cfg.until_cycle || cfg.until_stable;
    let mut history = if need_history {
        Some(History::default_window())
    } else {
        None
    };

    // Record gen 0
    let pop0 = grid.population();
    run_stats.observe_gen0(pop0);
    if let Some(ref mut f) = stats_file {
        append_stats(f, 0, pop0, 0, 0).map_err(|e| format!("stats: {e}"))?;
    }
    if let Some(ref mut hist) = history {
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
            if theme.name != "classic" {
                if !extra.is_empty() {
                    extra.push(' ');
                }
                extra.push_str(theme.name);
            }
            if !grid.wrap {
                if !extra.is_empty() {
                    extra.push(' ');
                }
                extra.push_str("finite");
            }
            ropts.status_extra = extra;
            ropts.theme = theme;
            ropts.age_heat = age_heat;
            if render_frame(&grid, generation, last_stats, &ropts, &mut out).is_err() {
                break;
            }
        }

        // Interactive / delay handling
        let mut do_step = true;
        if want_interactive {
            let timeout = if paused { 50 } else { delay_ms as i32 };
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
                        let _ = rng.next_u64();
                        reseed_grid(&mut grid, &cfg, &mut rng)?;
                        generation = 0;
                        last_stats = None;
                        run_stats = RunStats::default();
                        run_stats.observe_gen0(grid.population());
                        if let Some(ref mut hist) = history {
                            *hist = History::default_window();
                            let _ = hist.observe(&grid, 0);
                        }
                        if let Some(ref mut f) = stats_file {
                            append_stats(f, 0, grid.population(), 0, 0)
                                .map_err(|e| format!("stats: {e}"))?;
                        }
                        do_step = false;
                    }
                    Key::Theme => {
                        theme = theme.next();
                        ropts.theme = theme;
                        do_step = !paused;
                    }
                    Key::Age => {
                        age_heat = !age_heat;
                        ropts.age_heat = age_heat;
                        do_step = !paused;
                    }
                    Key::Wrap => {
                        grid.wrap = !grid.wrap;
                        do_step = !paused;
                    }
                    Key::Other(_) => {
                        do_step = !paused;
                    }
                }
                if let Some(k) = drain_keys() {
                    if matches!(k, Key::Quit) {
                        stop_reason = StopReason::Quit;
                        break;
                    }
                }
            } else {
                do_step = !paused;
            }
        } else if animate {
            thread::sleep(Duration::from_millis(delay_ms));
        }

        if !do_step {
            continue;
        }

        if generation >= cfg.gens {
            stop_reason = StopReason::MaxGens;
            break;
        }

        let stats = grid.step_with(&rule);
        last_stats = Some(stats);
        generation += 1;
        let pop = grid.population();
        run_stats.observe_step(generation, pop, stats);

        if let Some(ref mut f) = stats_file {
            append_stats(f, generation, pop, stats.births, stats.deaths)
                .map_err(|e| format!("stats: {e}"))?;
        }

        if let (Some(ref base), Some(every)) = (&cfg.save, cfg.save_every) {
            if generation % every == 0 {
                save_snapshot(&grid, base, generation)?;
            }
        }

        if let Some(ref mut hist) = history {
            if let CycleCheck::Cycle { period } = hist.observe(&grid, generation) {
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
    let pop = grid.population();

    if animate {
        let mut extra = String::new();
        if rule != Rule::CONWAY {
            extra.push_str(&rule.to_string_bs());
            extra.push(' ');
        }
        extra.push_str(&format!("done:{reason}"));
        ropts.status_extra = extra;
        ropts.theme = theme;
        ropts.age_heat = age_heat;
        let _ = render_frame(&grid, generation, last_stats, &ropts, &mut out);
        let _ = out.flush();
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
        if cfg.summary {
            print_summary(
                &mut out,
                generation,
                pop,
                &reason,
                &run_stats,
                last_stats,
                &rule,
                theme,
                grid.wrap,
                cfg.color,
            )
            .map_err(|e| e.to_string())?;
            let _ = out.flush();
        }
    } else {
        // quiet: always print summary with rich stats
        print_summary(
            &mut out,
            generation,
            pop,
            &reason,
            &run_stats,
            last_stats,
            &rule,
            theme,
            grid.wrap,
            cfg.color,
        )
        .map_err(|e| e.to_string())?;
        if let StopReason::Cycle { period } = stop_reason {
            writeln!(out, "  cycle period: {period}").map_err(|e| e.to_string())?;
        }
        let _ = out.flush();
    }

    if let Some(ref path) = cfg.save {
        save_file(&grid, path)?;
        if !cfg.quiet {
            eprintln!("saved RLE → {}", path.display());
        }
    }

    if let Some(ref path) = cfg.export_ppm {
        export_ppm(&grid, path, theme, age_heat)?;
        if !cfg.quiet {
            eprintln!("exported PPM → {}", path.display());
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
    use gol_rs::grid::Grid;
    use gol_rs::history::{CycleCheck, History};
    use gol_rs::rule::Rule;

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

    #[test]
    fn run_stats_tracks_max_min() {
        let mut rs = RunStats::default();
        rs.observe_gen0(10);
        assert_eq!(rs.max_pop, 10);
        assert_eq!(rs.min_pop, 10);
        rs.observe_step(1, 15, StepStats { births: 5, deaths: 0 });
        assert_eq!(rs.max_pop, 15);
        assert_eq!(rs.gen_at_max_pop, 1);
        assert_eq!(rs.min_pop, 15);
        rs.observe_step(2, 8, StepStats { births: 0, deaths: 7 });
        assert_eq!(rs.max_pop, 15);
        assert_eq!(rs.min_pop, 8);
        assert_eq!(rs.total_births, 5);
        assert_eq!(rs.total_deaths, 7);
    }
}
