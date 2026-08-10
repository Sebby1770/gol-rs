//! `gol` CLI binary — thin front-end over the `gol_rs` library.

mod term;

use gol_rs::grid::{Grid, StepStats};
use gol_rs::history::{CycleCheck, History};
use gol_rs::patterns::{
    dump_ascii, dump_rle_with_rule, list_patterns, load_file, save_file_with_rule, seed_pattern,
};
use gol_rs::render::{export_ppm, render_frame, CursorGuard, RenderOpts, Style};
use gol_rs::rng::Rng;
use gol_rs::rule::{list_rules, CaRule, Rule};
use gol_rs::sparkline::sparkline;
use gol_rs::theme::Theme;
use gol_rs::transform::{flip_h_in_place, flip_v_in_place, rotate90_in_place};
use term::{drain_keys, poll_key, stdin_is_tty, Key, RawMode};

use std::env;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const HELP: &str = "\
gol — Life-like cellular automata in your terminal (v0.5)

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
        --rule NAME|B#/S#  Life-like or multi-state rule (default: conway)
        --list-rules       list built-in rules and exit
        --age              heat-map colour by cell age (within theme)
        --theme NAME       classic|neon|fire|ocean|mono (default: classic)
        --finite, --no-wrap  hard edges (neighbors off-grid count as dead)
        --rotate90         rotate seed 90° clockwise after load/pattern
        --flip-h           flip seed horizontally
        --flip-v           flip seed vertically
        --until-stable     stop when grid equals previous generation (period 1)
        --until-cycle      stop when any prior generation reappears; report period
        --analyze          soup analysis: pop stats, density, period if cycled
        --record DIR       write frames as frame-NNNNNN.ppm into DIR
        --record-every N   record every N gens (default: 1; requires --record)
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
    w      toggle wrap     s  save snapshot.rle    p  print pop sparkline
    q      quit

EXAMPLES:
    gol
    gol --pattern gosper --width 80 --height 30 --delay 60
    gol --pattern blinker --until-cycle --quiet
    gol --rule highlife --theme neon --age
    gol --rule brian --pattern random --density 0.2 --theme fire
    gol --finite --pattern glider --width 40 --height 20
    gol --bench --width 200 --height 100 --gens 200 --seed 1
    gol --pattern acorn --export-ppm out.ppm --quiet --gens 100
    gol --pattern random --seed 1 --analyze --gens 1000 --quiet
    gol --record frames --record-every 5 --pattern gosper --gens 50 --quiet
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
    rule: CaRule,
    /// True if the user passed `--rule` on the CLI (overrides RLE header rule).
    rule_explicit: bool,
    age_heat: bool,
    theme: Theme,
    wrap: bool,
    rotate90: bool,
    flip_h: bool,
    flip_v: bool,
    until_stable: bool,
    until_cycle: bool,
    analyze: bool,
    record_dir: Option<PathBuf>,
    record_every: u64,
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
            rule: CaRule::default(),
            rule_explicit: false,
            age_heat: false,
            theme: Theme::CLASSIC,
            wrap: true,
            rotate90: false,
            flip_h: false,
            flip_v: false,
            until_stable: false,
            until_cycle: false,
            analyze: false,
            record_dir: None,
            record_every: 1,
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
    initial_pop: usize,
    /// True once gen ≥ 1 has been observed for min tracking.
    started: bool,
    /// Population samples for sparkline (gen 0 + each step).
    pop_samples: Vec<usize>,
}

impl RunStats {
    fn observe_gen0(&mut self, pop: usize) {
        self.max_pop = pop;
        self.min_pop = pop;
        self.gen_at_max_pop = 0;
        self.initial_pop = pop;
        self.started = false;
        self.pop_samples.clear();
        self.pop_samples.push(pop);
    }

    fn observe_step(&mut self, gen: u64, pop: usize, stats: StepStats) {
        self.total_births += stats.births as u64;
        self.total_deaths += stats.deaths as u64;
        if !self.started {
            self.min_pop = pop;
            self.started = true;
        } else {
            self.min_pop = self.min_pop.min(pop);
        }
        if pop > self.max_pop {
            self.max_pop = pop;
            self.gen_at_max_pop = gen;
        }
        self.pop_samples.push(pop);
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
            "--rule" => {
                cfg.rule = CaRule::parse(&take(&mut i)?)?;
                cfg.rule_explicit = true;
            }
            "--age" => cfg.age_heat = true,
            "--theme" => cfg.theme = Theme::parse(&take(&mut i)?)?,
            "--finite" | "--no-wrap" => cfg.wrap = false,
            "--rotate90" => cfg.rotate90 = true,
            "--flip-h" => cfg.flip_h = true,
            "--flip-v" => cfg.flip_v = true,
            "--until-stable" => cfg.until_stable = true,
            "--until-cycle" => cfg.until_cycle = true,
            "--analyze" => cfg.analyze = true,
            "--record" => cfg.record_dir = Some(PathBuf::from(take(&mut i)?)),
            "--record-every" => {
                cfg.record_every = take(&mut i)?
                    .parse()
                    .map_err(|e| format!("--record-every: {e}"))?;
            }
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
    if cfg.record_every == 0 {
        return Err("--record-every must be >= 1".into());
    }
    if cfg.record_dir.is_none() && cfg.record_every != 1 {
        // allow default 1 without --record; only error if user set every without dir
        // (we can't distinguish default; only check if record_every was meaningful with dir)
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

fn append_stats(
    f: &mut File,
    gen: u64,
    pop: usize,
    births: usize,
    deaths: usize,
) -> io::Result<()> {
    writeln!(f, "{gen},{pop},{births},{deaths}")
}

fn save_snapshot(grid: &Grid, base: &PathBuf, gen: u64, rule: &CaRule) -> Result<(), String> {
    let path = if gen == u64::MAX {
        base.clone()
    } else {
        let stem = base
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("pattern");
        let ext = base.extension().and_then(|s| s.to_str()).unwrap_or("rle");
        let parent = base.parent().unwrap_or_else(|| Path::new("."));
        parent.join(format!("{stem}-gen{gen}.{ext}"))
    };
    save_file_with_rule(grid, &path, &rule.to_string_bs())
}

fn record_frame(
    grid: &Grid,
    dir: &Path,
    frame_idx: u64,
    theme: Theme,
    age_heat: bool,
) -> Result<(), String> {
    let path = dir.join(format!("frame-{frame_idx:06}.ppm"));
    export_ppm(grid, &path, theme, age_heat)
}

fn step_grid(grid: &mut Grid, rule: &CaRule) -> StepStats {
    match rule {
        CaRule::LifeLike(r) => grid.step_with(r),
        CaRule::BriansBrain => grid.step_brians_brain(),
    }
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
    let wrap = grid.wrap;
    let states = cfg.rule.states();
    *grid = Grid::with_states(cfg.w, cfg.h, wrap, states);
    if let Some(ref path) = cfg.load {
        let _ = load_file(grid, path)?;
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
    rule: &CaRule,
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
    let spark = sparkline(&run.pop_samples, 40);
    if !spark.is_empty() {
        writeln!(out, "  pop: {spark}")?;
    }
    if let Some(s) = last_stats {
        writeln!(out, "  last step: +{}/-{}", s.births, s.deaths)?;
    }
    if *rule != CaRule::LifeLike(Rule::CONWAY) {
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

fn print_analyze(
    out: &mut impl Write,
    generation: u64,
    run: &RunStats,
    period: Option<u64>,
    grid_cells: usize,
) -> io::Result<()> {
    let final_pop = run.pop_samples.last().copied().unwrap_or(0);
    let density = if grid_cells == 0 {
        0.0
    } else {
        100.0 * final_pop as f64 / grid_cells as f64
    };
    writeln!(out, "analyze:")?;
    writeln!(out, "  initial_pop: {}", run.initial_pop)?;
    writeln!(out, "  final_pop:   {final_pop}")?;
    writeln!(
        out,
        "  max_pop:     {} (gen {})",
        run.max_pop, run.gen_at_max_pop
    )?;
    writeln!(out, "  min_pop:     {}", run.min_pop)?;
    writeln!(out, "  gens:        {generation}")?;
    writeln!(out, "  density:     {density:.2}%")?;
    match period {
        Some(p) => writeln!(out, "  period:      {p}")?,
        None => writeln!(out, "  period:      (none detected)")?,
    }
    let spark = sparkline(&run.pop_samples, 40);
    if !spark.is_empty() {
        writeln!(out, "  pop:         {spark}")?;
    }
    Ok(())
}

fn run_bench(cfg: &Config, grid: &mut Grid) -> Result<(), String> {
    let rule = cfg.rule;
    let cells = (grid.w * grid.h) as u64;
    let gens = cfg.gens;
    let _ = step_grid(grid, &rule);

    let start = Instant::now();
    for _ in 0..gens {
        step_grid(grid, &rule);
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

fn run(mut cfg: Config) -> Result<(), String> {
    if cfg.list_patterns {
        list_patterns();
        return Ok(());
    }
    if cfg.list_rules {
        list_rules();
        return Ok(());
    }

    let mut grid = Grid::with_states(cfg.w, cfg.h, cfg.wrap, cfg.rule.states());
    let mut rng = Rng::new(cfg.seed);

    if let Some(ref path) = cfg.load {
        let info = load_file(&mut grid, path)?;
        if !cfg.rule_explicit {
            if let Some(ref r) = info.rule {
                if let Ok(parsed) = CaRule::parse(r) {
                    cfg.rule = parsed;
                    grid.states = cfg.rule.states();
                }
            }
        }
    } else {
        seed_pattern(&mut grid, &cfg.pattern, &mut rng, cfg.density)?;
    }
    // Ensure states matches rule (e.g. after pattern seed on binary default)
    grid.states = cfg.rule.states().max(grid.states);
    apply_transforms(&mut grid, &cfg);

    if cfg.bench {
        return run_bench(&cfg, &mut grid);
    }

    // Frame recording setup
    if let Some(ref dir) = cfg.record_dir {
        fs::create_dir_all(dir).map_err(|e| format!("--record {}: {e}", dir.display()))?;
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

    // Quiet still records frames if --record set; animation is terminal render.
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
        sparkline: String::new(),
    };

    let mut last_stats: Option<StepStats> = None;
    let mut generation: u64 = 0;
    let mut stop_reason = StopReason::MaxGens;
    let mut delay_ms = cfg.delay_ms.max(1);
    let mut paused = false;
    let mut run_stats = RunStats::default();
    let mut detected_period: Option<u64> = None;

    // History for cycle detection; also enabled for --analyze.
    let need_history = cfg.until_cycle || cfg.until_stable || cfg.analyze;
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
    if let Some(ref dir) = cfg.record_dir {
        // frame index is 1-based: frame-000001 is gen 0
        record_frame(&grid, dir, 1, theme, age_heat)?;
    }

    let mut frame_counter: u64 = 1; // frames written so far

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
            if rule != CaRule::LifeLike(Rule::CONWAY) {
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
            ropts.sparkline = sparkline(&run_stats.pop_samples, 24);
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
                        detected_period = None;
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
                    Key::Save => {
                        let path = PathBuf::from("snapshot.rle");
                        if let Err(e) = save_file_with_rule(&grid, &path, &rule.to_string_bs()) {
                            eprintln!("save error: {e}");
                        } else {
                            eprintln!("saved snapshot.rle");
                        }
                        do_step = !paused;
                    }
                    Key::Sparkline => {
                        let s = sparkline(&run_stats.pop_samples, 40);
                        eprintln!("pop: {s}");
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

        let stats = step_grid(&mut grid, &rule);
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
                save_snapshot(&grid, base, generation, &rule)?;
            }
        }

        // Frame recording
        if let Some(ref dir) = cfg.record_dir {
            if generation % cfg.record_every == 0 {
                frame_counter += 1;
                record_frame(&grid, dir, frame_counter, theme, age_heat)?;
            }
        }

        if let Some(ref mut hist) = history {
            if let CycleCheck::Cycle { period } = hist.observe(&grid, generation) {
                if detected_period.is_none() {
                    detected_period = Some(period);
                }
                let stop = if cfg.until_cycle {
                    true
                } else if cfg.until_stable && period == 1 {
                    true
                } else {
                    false
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
        if rule != CaRule::LifeLike(Rule::CONWAY) {
            extra.push_str(&rule.to_string_bs());
            extra.push(' ');
        }
        extra.push_str(&format!("done:{reason}"));
        ropts.status_extra = extra;
        ropts.theme = theme;
        ropts.age_heat = age_heat;
        ropts.sparkline = sparkline(&run_stats.pop_samples, 24);
        let _ = render_frame(&grid, generation, last_stats, &ropts, &mut out);
        let _ = out.flush();
        if let StopReason::Cycle { period } = stop_reason {
            let _ = writeln!(
                out,
                "\x1b[Kcycle detected: period {period} at generation {generation}"
            );
            let _ = out.flush();
        } else if matches!(stop_reason, StopReason::Stable) {
            let _ = writeln!(out, "\x1b[Kstable (period 1) at generation {generation}");
            let _ = out.flush();
        }
        if cfg.summary || cfg.analyze {
            print_summary(
                &mut out, generation, pop, &reason, &run_stats, last_stats, &rule, theme,
                grid.wrap, cfg.color,
            )
            .map_err(|e| e.to_string())?;
            if cfg.analyze {
                print_analyze(
                    &mut out,
                    generation,
                    &run_stats,
                    detected_period.or_else(|| match stop_reason {
                        StopReason::Cycle { period } => Some(period),
                        StopReason::Stable => Some(1),
                        _ => None,
                    }),
                    grid.w * grid.h,
                )
                .map_err(|e| e.to_string())?;
            }
            let _ = out.flush();
        }
    } else {
        // quiet: always print summary with rich stats
        print_summary(
            &mut out, generation, pop, &reason, &run_stats, last_stats, &rule, theme, grid.wrap,
            cfg.color,
        )
        .map_err(|e| e.to_string())?;
        if let StopReason::Cycle { period } = stop_reason {
            writeln!(out, "  cycle period: {period}").map_err(|e| e.to_string())?;
        }
        if cfg.analyze {
            print_analyze(
                &mut out,
                generation,
                &run_stats,
                detected_period.or_else(|| match stop_reason {
                    StopReason::Cycle { period } => Some(period),
                    StopReason::Stable => Some(1),
                    _ => None,
                }),
                grid.w * grid.h,
            )
            .map_err(|e| e.to_string())?;
        }
        let _ = out.flush();
    }

    if let Some(ref path) = cfg.save {
        save_file_with_rule(&grid, path, &rule.to_string_bs())?;
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
            _ => print!("{}", dump_rle_with_rule(&grid, &rule.to_string_bs())),
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
    use gol_rs::rule::{CaRule, Rule};
    use gol_rs::sparkline::sparkline;

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
        assert_eq!(
            CaRule::parse("conway").unwrap(),
            CaRule::LifeLike(Rule::CONWAY)
        );
        assert_eq!(
            CaRule::parse("B3/S23").unwrap(),
            CaRule::LifeLike(Rule::CONWAY)
        );
        assert_eq!(
            CaRule::parse("23/3").unwrap(),
            CaRule::LifeLike(Rule::CONWAY)
        );
        assert!(CaRule::parse("highlife")
            .unwrap()
            .as_life_like()
            .unwrap()
            .births(6));
        assert_eq!(CaRule::parse("brian").unwrap(), CaRule::BriansBrain);
    }

    #[test]
    fn run_stats_tracks_max_min() {
        let mut rs = RunStats::default();
        rs.observe_gen0(10);
        assert_eq!(rs.max_pop, 10);
        assert_eq!(rs.min_pop, 10);
        assert_eq!(rs.initial_pop, 10);
        rs.observe_step(
            1,
            15,
            StepStats {
                births: 5,
                deaths: 0,
            },
        );
        assert_eq!(rs.max_pop, 15);
        assert_eq!(rs.gen_at_max_pop, 1);
        assert_eq!(rs.min_pop, 15);
        rs.observe_step(
            2,
            8,
            StepStats {
                births: 0,
                deaths: 7,
            },
        );
        assert_eq!(rs.max_pop, 15);
        assert_eq!(rs.min_pop, 8);
        assert_eq!(rs.total_births, 5);
        assert_eq!(rs.total_deaths, 7);
        assert_eq!(rs.pop_samples.len(), 3);
        let s = sparkline(&rs.pop_samples, 3);
        assert_eq!(s.chars().count(), 3);
    }

    #[test]
    fn step_grid_brians_brain() {
        let mut g = Grid::with_states(5, 5, true, 3);
        g.set_state(1, 2, 1);
        g.set_state(3, 2, 1);
        let stats = step_grid(&mut g, &CaRule::BriansBrain);
        assert!(stats.births >= 1);
        assert_eq!(g.get_state(2, 2), 1);
    }
}
