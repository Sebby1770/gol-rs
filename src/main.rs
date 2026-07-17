use gol_rs::{
    Boundary, Cycle, CycleDetector, GenerationStats, Grid, MAX_RLE_INPUT_BYTES, Pattern, Rule,
    SeededRng, Simulation, builtin_pattern, catalogue, export_rle, parse_rle,
};
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_GRID_CELLS: usize = 16_000_000;

const HELP: &str = "\
gol — a terminal laboratory for Life-like cellular automata

USAGE:
    gol [run] [OPTIONS]
    gol patterns

INPUT:
    -p, --pattern NAME       built-in pattern or random (default: random)
        --rle FILE           load an RLE file; use - to read stdin
        --origin X,Y         place a pattern at an exact origin (default: centred)
    -s, --seed N             deterministic random seed (default: time-based)
        --density F          random live-cell density, 0.0-1.0 (default: 0.25)

SIMULATION:
    -W, --width N            grid width (default: 60)
    -H, --height N           grid height (default: 30)
    -g, --gens N             transitions after generation zero (default: 500)
    -d, --delay MS           animation delay in milliseconds (default: 80)
        --rule RULE          Life-like rule, e.g. B3/S23 or B36/S23
        --boundary MODE      toroidal or finite (default: toroidal)
        --detect-cycles      detect exact repeated grid states
        --stop-on-cycle      stop at the first exact repeat
        --cycle-limit N      maximum retained states (default: 10000)

OUTPUT:
        --output MODE        terminal, plain, or jsonl (auto-detected by default)
        --headless           shorthand for --output jsonl
        --no-color           disable terminal colours
        --export-rle FILE    write the final live bounding box as RLE
    -h, --help               show this help
    -V, --version            show the version

NOTES:
    Generation zero is emitted before any transitions. RLE header rules are
    used unless --rule is supplied. Cycle detection compares complete states;
    translating spaceships are not treated as cycles until the board repeats.

EXAMPLES:
    gol --pattern gosper --width 80 --height 25 --stop-on-cycle
    gol --rle glider.rle --boundary finite --gens 100 --output plain
    gol --pattern blinker --width 9 --height 9 --headless --stop-on-cycle
    gol patterns
";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputMode {
    Terminal,
    Plain,
    JsonLines,
}

impl FromStr for OutputMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "terminal" | "tty" | "ansi" => Ok(Self::Terminal),
            "plain" | "text" => Ok(Self::Plain),
            "jsonl" | "json" | "headless" => Ok(Self::JsonLines),
            other => Err(format!(
                "unknown output mode {other:?}; expected terminal, plain, or jsonl"
            )),
        }
    }
}

#[derive(Debug)]
enum Command {
    Run(Config),
    Patterns,
    Help,
    Version,
}

#[derive(Debug)]
struct Config {
    width: usize,
    height: usize,
    width_explicit: bool,
    height_explicit: bool,
    generations: u64,
    delay_ms: u64,
    seed: u64,
    density: f64,
    density_explicit: bool,
    pattern: String,
    pattern_explicit: bool,
    rle: Option<String>,
    origin: Option<(usize, usize)>,
    rule: Option<Rule>,
    boundary: Boundary,
    detect_cycles: bool,
    stop_on_cycle: bool,
    cycle_limit: usize,
    output: Option<OutputMode>,
    no_color: bool,
    export_rle: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0xC0FFEE);
        Self {
            width: 60,
            height: 30,
            width_explicit: false,
            height_explicit: false,
            generations: 500,
            delay_ms: 80,
            seed,
            density: 0.25,
            density_explicit: false,
            pattern: "random".into(),
            pattern_explicit: false,
            rle: None,
            origin: None,
            rule: None,
            boundary: Boundary::Toroidal,
            detect_cycles: false,
            stop_on_cycle: false,
            cycle_limit: 10_000,
            output: None,
            no_color: false,
            export_rle: None,
        }
    }
}

#[derive(Debug)]
enum AppError {
    Usage(String),
    Runtime(String),
    Io(io::Error),
}

impl AppError {
    const fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => 2,
            Self::Runtime(_) | Self::Io(_) => 1,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::Runtime(message) => formatter.write_str(message),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            if matches!(error, AppError::Usage(_)) {
                eprintln!("Try 'gol --help' for usage.");
            }
            ExitCode::from(error.exit_code())
        }
    }
}

fn execute() -> Result<(), AppError> {
    match parse_cli(env::args().skip(1).collect())? {
        Command::Help => {
            print!("{HELP}");
            Ok(())
        }
        Command::Version => {
            println!("gol {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Patterns => print_patterns(),
        Command::Run(config) => run(config),
    }
}

fn parse_cli(arguments: Vec<String>) -> Result<Command, AppError> {
    if arguments
        .first()
        .is_some_and(|argument| argument == "patterns")
    {
        if arguments.len() == 1 {
            return Ok(Command::Patterns);
        }
        return Err(AppError::Usage(
            "the patterns command does not accept options".into(),
        ));
    }

    let mut config = Config::default();
    let mut index = usize::from(arguments.first().is_some_and(|argument| argument == "run"));
    while index < arguments.len() {
        let argument = &arguments[index];
        let take = |index: &mut usize| -> Result<String, AppError> {
            *index += 1;
            arguments
                .get(*index)
                .cloned()
                .ok_or_else(|| AppError::Usage(format!("{argument} requires a value")))
        };
        match argument.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "-W" | "--width" => {
                config.width = parse_number(&take(&mut index)?, "--width")?;
                config.width_explicit = true;
            }
            "-H" | "--height" => {
                config.height = parse_number(&take(&mut index)?, "--height")?;
                config.height_explicit = true;
            }
            "-g" | "--gens" => {
                config.generations = parse_number(&take(&mut index)?, "--gens")?;
            }
            "-d" | "--delay" => {
                config.delay_ms = parse_number(&take(&mut index)?, "--delay")?;
            }
            "-s" | "--seed" => {
                config.seed = parse_number(&take(&mut index)?, "--seed")?;
            }
            "-p" | "--pattern" => {
                if config.pattern_explicit {
                    return Err(AppError::Usage(
                        "--pattern may only be supplied once".into(),
                    ));
                }
                config.pattern = take(&mut index)?;
                config.pattern_explicit = true;
            }
            "--rle" => {
                if config.rle.is_some() {
                    return Err(AppError::Usage("--rle may only be supplied once".into()));
                }
                config.rle = Some(take(&mut index)?);
            }
            "--density" => {
                config.density = take(&mut index)?
                    .parse::<f64>()
                    .map_err(|error| AppError::Usage(format!("--density: {error}")))?;
                config.density_explicit = true;
            }
            "--origin" => config.origin = Some(parse_origin(&take(&mut index)?)?),
            "--rule" => {
                config.rule = Some(
                    take(&mut index)?
                        .parse()
                        .map_err(|error| AppError::Usage(format!("--rule: {error}")))?,
                );
            }
            "--boundary" => {
                config.boundary = take(&mut index)?
                    .parse()
                    .map_err(|error: String| AppError::Usage(format!("--boundary: {error}")))?;
            }
            "--detect-cycles" => config.detect_cycles = true,
            "--stop-on-cycle" => {
                config.detect_cycles = true;
                config.stop_on_cycle = true;
            }
            "--cycle-limit" => {
                config.cycle_limit = parse_number(&take(&mut index)?, "--cycle-limit")?;
                config.detect_cycles = true;
            }
            "--output" => {
                let output = take(&mut index)?
                    .parse()
                    .map_err(|error: String| AppError::Usage(format!("--output: {error}")))?;
                if config.output.replace(output).is_some() {
                    return Err(AppError::Usage("--output may only be supplied once".into()));
                }
            }
            "--headless" => {
                if config
                    .output
                    .replace(OutputMode::JsonLines)
                    .is_some_and(|mode| mode != OutputMode::JsonLines)
                {
                    return Err(AppError::Usage(
                        "--headless conflicts with another --output mode".into(),
                    ));
                }
            }
            "--no-color" => config.no_color = true,
            "--export-rle" => {
                let value = take(&mut index)?;
                if value == "-" {
                    return Err(AppError::Usage(
                        "--export-rle requires a file path, not stdout".into(),
                    ));
                }
                config.export_rle = Some(PathBuf::from(value));
            }
            unknown => return Err(AppError::Usage(format!("unknown argument: {unknown}"))),
        }
        index += 1;
    }

    validate_config(&config)?;
    Ok(Command::Run(config))
}

fn parse_number<T>(value: &str, option: &str) -> Result<T, AppError>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    value
        .parse()
        .map_err(|error| AppError::Usage(format!("{option}: {error}")))
}

fn parse_origin(value: &str) -> Result<(usize, usize), AppError> {
    let (x, y) = value
        .split_once(',')
        .ok_or_else(|| AppError::Usage("--origin expects X,Y".into()))?;
    Ok((
        parse_number(x, "--origin X")?,
        parse_number(y, "--origin Y")?,
    ))
}

fn validate_config(config: &Config) -> Result<(), AppError> {
    if config.width == 0 || config.height == 0 {
        return Err(AppError::Usage(
            "grid width and height must both be at least 1".into(),
        ));
    }
    let cells = config
        .width
        .checked_mul(config.height)
        .ok_or_else(|| AppError::Usage("grid dimensions overflow address space".into()))?;
    if cells > MAX_GRID_CELLS {
        return Err(AppError::Usage(format!(
            "grid has {cells} cells; limit is {MAX_GRID_CELLS}"
        )));
    }
    if !config.density.is_finite() || !(0.0..=1.0).contains(&config.density) {
        return Err(AppError::Usage(
            "--density must be a finite number in 0.0..=1.0".into(),
        ));
    }
    if config.pattern_explicit && config.rle.is_some() {
        return Err(AppError::Usage("--pattern conflicts with --rle".into()));
    }
    let is_random = config.rle.is_none() && config.pattern.eq_ignore_ascii_case("random");
    if config.density_explicit && !is_random {
        return Err(AppError::Usage(
            "--density is only valid with the random pattern".into(),
        ));
    }
    if config.origin.is_some() && is_random {
        return Err(AppError::Usage(
            "--origin is only valid with a built-in or RLE pattern".into(),
        ));
    }
    if config.detect_cycles && config.cycle_limit == 0 {
        return Err(AppError::Usage(
            "--cycle-limit must be at least 1 when cycle detection is enabled".into(),
        ));
    }
    Ok(())
}

fn print_patterns() -> Result<(), AppError> {
    let mut stdout = io::BufWriter::new(io::stdout());
    writeln!(stdout, "NAME                 SIZE      POP  DESCRIPTION")?;
    writeln!(
        stdout,
        "random               grid      —   seeded random field"
    )?;
    for pattern in catalogue() {
        writeln!(
            stdout,
            "{:<20} {:>2}x{:<6} {:>3}  {}",
            pattern.name, pattern.width, pattern.height, pattern.population, pattern.description
        )?;
    }
    stdout.flush()?;
    Ok(())
}

fn run(mut config: Config) -> Result<(), AppError> {
    let output = config.output.unwrap_or_else(|| {
        if io::stdout().is_terminal() {
            OutputMode::Terminal
        } else {
            OutputMode::Plain
        }
    });
    let (grid, rule, source) = build_grid(&mut config)?;
    let mut simulation = Simulation::new(grid, rule, config.boundary);
    let mut detector = config
        .detect_cycles
        .then(|| CycleDetector::new(config.cycle_limit));
    if let Some(detector) = detector.as_mut() {
        detector.observe(0, simulation.grid());
    }

    let mut stdout = io::BufWriter::new(io::stdout());
    if output == OutputMode::Terminal {
        // Keep the cursor visible so abrupt SIGINT termination cannot leave the
        // user's terminal in a broken state.
        stdout.write_all(b"\x1b[2J\x1b[H")?;
        stdout.flush()?;
    }
    let mut stats = simulation.current_stats();
    let mut peak_population = stats.population;
    let mut cycle = None;
    write_generation(
        output,
        !config.no_color,
        simulation.grid(),
        &stats,
        simulation.rule(),
        simulation.boundary(),
        None,
        &mut stdout,
    )?;

    for _ in 0..config.generations {
        if output == OutputMode::Terminal && config.delay_ms > 0 {
            thread::sleep(Duration::from_millis(config.delay_ms));
        }
        stats = simulation.step();
        peak_population = peak_population.max(stats.population);
        let repeated = detector
            .as_mut()
            .and_then(|detector| detector.observe(stats.generation, simulation.grid()));
        if cycle.is_none() {
            cycle = repeated;
        }
        write_generation(
            output,
            !config.no_color,
            simulation.grid(),
            &stats,
            simulation.rule(),
            simulation.boundary(),
            repeated,
            &mut stdout,
        )?;
        if repeated.is_some() && config.stop_on_cycle {
            break;
        }
    }

    write_summary(
        output,
        &source,
        &simulation,
        peak_population,
        cycle,
        detector.as_ref().is_some_and(CycleDetector::is_saturated),
        &mut stdout,
    )?;
    stdout.flush()?;

    if let Some(path) = config.export_rle {
        let final_pattern = cropped_pattern(simulation.grid(), simulation.rule())?;
        fs::write(&path, export_rle(&final_pattern)).map_err(|error| {
            AppError::Runtime(format!(
                "could not write RLE export {}: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn build_grid(config: &mut Config) -> Result<(Grid, Rule, String), AppError> {
    if let Some(rle_source) = config.rle.as_deref() {
        let input = read_rle_source(rle_source)?;
        let pattern = parse_rle(&input).map_err(|error| AppError::Usage(error.to_string()))?;
        if !config.width_explicit {
            config.width = config.width.max(pattern.width());
        }
        if !config.height_explicit {
            config.height = config.height.max(pattern.height());
        }
        validate_config(config)?;
        let rule = config.rule.or(pattern.rule()).unwrap_or_default();
        let mut grid = Grid::new(config.width, config.height)
            .map_err(|error| AppError::Usage(error.to_string()))?;
        place(&mut grid, &pattern, config.origin)?;
        return Ok((grid, rule, format!("rle:{rle_source}")));
    }

    if config.pattern.eq_ignore_ascii_case("random") {
        let mut grid = Grid::new(config.width, config.height)
            .map_err(|error| AppError::Usage(error.to_string()))?;
        grid.randomize(&mut SeededRng::new(config.seed), config.density)
            .map_err(|error| AppError::Usage(error.to_string()))?;
        return Ok((grid, config.rule.unwrap_or_default(), "random".into()));
    }

    let pattern = builtin_pattern(&config.pattern).ok_or_else(|| {
        AppError::Usage(format!(
            "unknown pattern {:?}; run 'gol patterns' to list choices",
            config.pattern
        ))
    })?;
    let mut grid = Grid::new(config.width, config.height)
        .map_err(|error| AppError::Usage(error.to_string()))?;
    place(&mut grid, &pattern, config.origin)?;
    Ok((
        grid,
        config.rule.unwrap_or_default(),
        format!("builtin:{}", config.pattern),
    ))
}

fn place(
    grid: &mut Grid,
    pattern: &Pattern,
    origin: Option<(usize, usize)>,
) -> Result<(), AppError> {
    let result = match origin {
        Some((x, y)) => grid.place_pattern(pattern, x, y),
        None => grid.place_pattern_centered(pattern),
    };
    result.map_err(|error| AppError::Usage(error.to_string()))
}

fn read_rle_source(source: &str) -> Result<String, AppError> {
    if source == "-" {
        return read_bounded_rle(io::stdin(), "stdin");
    }
    let file = fs::File::open(source)
        .map_err(|error| AppError::Runtime(format!("could not read RLE file {source}: {error}")))?;
    read_bounded_rle(file, source)
}

fn read_bounded_rle(reader: impl Read, label: &str) -> Result<String, AppError> {
    let mut input = String::new();
    reader
        .take((MAX_RLE_INPUT_BYTES + 1) as u64)
        .read_to_string(&mut input)
        .map_err(|error| AppError::Runtime(format!("could not read RLE {label}: {error}")))?;
    if input.len() > MAX_RLE_INPUT_BYTES {
        return Err(AppError::Usage(format!(
            "RLE {label} exceeds the {MAX_RLE_INPUT_BYTES}-byte input limit"
        )));
    }
    Ok(input)
}

#[allow(clippy::too_many_arguments)]
fn write_generation(
    mode: OutputMode,
    color: bool,
    grid: &Grid,
    stats: &GenerationStats,
    rule: Rule,
    boundary: Boundary,
    cycle: Option<Cycle>,
    output: &mut impl Write,
) -> io::Result<()> {
    match mode {
        OutputMode::Terminal => render_terminal(color, grid, stats, rule, boundary, cycle, output),
        OutputMode::Plain => render_plain(grid, stats, rule, boundary, cycle, output),
        OutputMode::JsonLines => render_json(stats, rule, boundary, cycle, output),
    }
}

fn render_terminal(
    color: bool,
    grid: &Grid,
    stats: &GenerationStats,
    rule: Rule,
    boundary: Boundary,
    cycle: Option<Cycle>,
    output: &mut impl Write,
) -> io::Result<()> {
    output.write_all(b"\x1b[H")?;
    if color {
        writeln!(
            output,
            "\x1b[1;36mgol-rs\x1b[0m  gen \x1b[33m{}\x1b[0m  pop \x1b[35m{}\x1b[0m  +{} -{}  {} {}\x1b[K",
            stats.generation, stats.population, stats.births, stats.deaths, rule, boundary
        )?;
    } else {
        writeln!(
            output,
            "gol-rs  gen {}  pop {}  +{} -{}  {} {}\x1b[K",
            stats.generation, stats.population, stats.births, stats.deaths, rule, boundary
        )?;
    }
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let alive = grid.get(x, y).unwrap_or(false);
            if alive && color {
                output.write_all("\x1b[32m██\x1b[0m".as_bytes())?;
            } else {
                output.write_all(if alive { "██".as_bytes() } else { b"  " })?;
            }
        }
        output.write_all(b"\x1b[K\n")?;
    }
    if let Some(cycle) = cycle {
        writeln!(
            output,
            "cycle: {} period {} (first {}, repeated {})\x1b[K",
            cycle.kind, cycle.period, cycle.first_generation, cycle.repeated_generation
        )?;
    } else {
        output.write_all(b"Ctrl-C to quit\x1b[K\n")?;
    }
    output.flush()
}

fn render_plain(
    grid: &Grid,
    stats: &GenerationStats,
    rule: Rule,
    boundary: Boundary,
    cycle: Option<Cycle>,
    output: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        output,
        "generation={} population={} births={} deaths={} changed={} rule={} boundary={}",
        stats.generation,
        stats.population,
        stats.births,
        stats.deaths,
        stats.changed(),
        rule,
        boundary
    )?;
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            output.write_all(if grid.get(x, y) == Some(true) {
                b"#"
            } else {
                b"."
            })?;
        }
        output.write_all(b"\n")?;
    }
    if let Some(cycle) = cycle {
        writeln!(
            output,
            "cycle={} period={} first={} repeated={}",
            cycle.kind, cycle.period, cycle.first_generation, cycle.repeated_generation
        )?;
    }
    output.write_all(b"\n")
}

fn render_json(
    stats: &GenerationStats,
    rule: Rule,
    boundary: Boundary,
    cycle: Option<Cycle>,
    output: &mut impl Write,
) -> io::Result<()> {
    write!(
        output,
        "{{\"type\":\"generation\",\"generation\":{},\"population\":{},\"births\":{},\"deaths\":{},\"survivors\":{},\"changed\":{},\"rule\":\"{}\",\"boundary\":\"{}\"",
        stats.generation,
        stats.population,
        stats.births,
        stats.deaths,
        stats.survivors,
        stats.changed(),
        rule,
        boundary
    )?;
    if let Some(cycle) = cycle {
        write!(
            output,
            ",\"cycle\":{{\"kind\":\"{}\",\"period\":{},\"first_generation\":{},\"repeated_generation\":{}}}",
            cycle.kind, cycle.period, cycle.first_generation, cycle.repeated_generation
        )?;
    }
    output.write_all(b"}\n")
}

fn write_summary(
    mode: OutputMode,
    source: &str,
    simulation: &Simulation,
    peak_population: usize,
    cycle: Option<Cycle>,
    detector_saturated: bool,
    output: &mut impl Write,
) -> io::Result<()> {
    match mode {
        OutputMode::JsonLines => {
            write!(
                output,
                "{{\"type\":\"summary\",\"source\":\"{}\",\"final_generation\":{},\"final_population\":{},\"peak_population\":{},\"cycle_tracking_saturated\":{}",
                json_escape(source),
                simulation.generation(),
                simulation.grid().population(),
                peak_population,
                detector_saturated
            )?;
            if let Some(cycle) = cycle {
                write!(
                    output,
                    ",\"cycle\":{{\"kind\":\"{}\",\"period\":{},\"first_generation\":{},\"repeated_generation\":{}}}",
                    cycle.kind, cycle.period, cycle.first_generation, cycle.repeated_generation
                )?;
            }
            output.write_all(b"}\n")
        }
        OutputMode::Terminal | OutputMode::Plain => {
            write!(
                output,
                "complete: generation={} population={} peak={} source={}",
                simulation.generation(),
                simulation.grid().population(),
                peak_population,
                source
            )?;
            if let Some(cycle) = cycle {
                write!(output, " cycle={} period={}", cycle.kind, cycle.period)?;
            }
            if detector_saturated {
                write!(output, " cycle_tracking=saturated")?;
            }
            output.write_all(b"\n")
        }
    }
}

fn cropped_pattern(grid: &Grid, rule: Rule) -> Result<Pattern, AppError> {
    let pattern = if let Some(bounds) = grid.bounding_box() {
        Pattern::from_live_cells(
            bounds.width(),
            bounds.height(),
            grid.live_cells()
                .map(|(x, y)| (x - bounds.min_x, y - bounds.min_y)),
        )
    } else {
        Pattern::from_live_cells(1, 1, std::iter::empty())
    };
    pattern
        .map(|pattern| pattern.with_rule(rule))
        .map_err(|error| AppError::Runtime(error.to_string()))
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", u32::from(control));
            }
            other => escaped.push(other),
        }
    }
    escaped
}
