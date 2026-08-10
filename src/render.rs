use crate::grid::{state, Grid, StepStats};
use crate::theme::Theme;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// How to draw live/dead cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Full block █ (double-wide cell).
    Block,
    /// Unicode braille — 2×4 cells per character.
    Braille,
    /// Middle-dot · for live, space for dead.
    Dots,
}

impl Style {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "block" => Ok(Style::Block),
            "braille" => Ok(Style::Braille),
            "dots" | "dot" => Ok(Style::Dots),
            other => Err(format!(
                "unknown style: {other} (expected block|braille|dots)"
            )),
        }
    }
}

/// Restores cursor visibility and SGR on drop.
pub struct CursorGuard {
    active: bool,
}

impl CursorGuard {
    pub fn new(active: bool) -> Self {
        Self { active }
    }
}

impl Drop for CursorGuard {
    fn drop(&mut self) {
        if self.active {
            let mut stdout = io::stdout();
            let _ = stdout.write_all(b"\x1b[?25h\x1b[0m\n");
            let _ = stdout.flush();
        }
    }
}

/// Render options for the frame header / body.
pub struct RenderOpts {
    pub style: Style,
    pub color: bool,
    pub show_stats: bool,
    /// Colour live cells by age within the theme palette.
    pub age_heat: bool,
    /// Colour theme.
    pub theme: Theme,
    /// Extra status suffix (e.g. interactive pause / rule name).
    pub status_extra: String,
    /// Optional population sparkline shown in the header.
    pub sparkline: String,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            style: Style::Block,
            color: true,
            show_stats: true,
            age_heat: false,
            theme: Theme::CLASSIC,
            status_extra: String::new(),
            sparkline: String::new(),
        }
    }
}

pub fn render_frame<W: Write>(
    grid: &Grid,
    generation: u64,
    last_stats: Option<StepStats>,
    opts: &RenderOpts,
    out: &mut W,
) -> io::Result<()> {
    // Home cursor (animation mode assumes screen was cleared once).
    out.write_all(b"\x1b[H")?;

    if opts.show_stats {
        write_header(grid, generation, last_stats, opts, out)?;
    }

    match opts.style {
        Style::Block => render_block(grid, opts, out)?,
        Style::Dots => render_dots(grid, opts, out)?,
        Style::Braille => render_braille(grid, opts, out)?,
    }
    out.flush()
}

fn write_header<W: Write>(
    grid: &Grid,
    generation: u64,
    last_stats: Option<StepStats>,
    opts: &RenderOpts,
    out: &mut W,
) -> io::Result<()> {
    let pop = grid.population();
    let quit_hint = if opts.status_extra.contains("interactive") {
        "keys: spc . + - r t a w s p q"
    } else {
        "Ctrl-C to quit"
    };
    let theme = opts.theme;
    if opts.color {
        write!(
            out,
            "{}gol-rs\x1b[0m  gen {}{:>5}\x1b[0m  pop {}{:>5}\x1b[0m  grid {}{}x{}\x1b[0m",
            theme.title_ansi(),
            theme.accent_ansi(),
            generation,
            theme.pop_ansi(),
            pop,
            theme.accent_ansi(),
            grid.w,
            grid.h
        )?;
        if let Some(s) = last_stats {
            write!(
                out,
                "  +\x1b[32m{}\x1b[0m/-\x1b[31m{}\x1b[0m",
                s.births, s.deaths
            )?;
        }
        if opts.age_heat {
            write!(out, "  maxage \x1b[31m{}\x1b[0m", grid.max_age())?;
        }
        if !opts.sparkline.is_empty() {
            write!(out, "  {}", opts.sparkline)?;
        }
        if !opts.status_extra.is_empty() {
            write!(out, "  {}{}\x1b[0m", theme.title_ansi(), opts.status_extra)?;
        }
        write!(out, "  ({quit_hint})\x1b[K\n")?;
    } else {
        write!(
            out,
            "gol-rs  gen {:>5}  pop {:>5}  grid {}x{}",
            generation, pop, grid.w, grid.h
        )?;
        if let Some(s) = last_stats {
            write!(out, "  +{}/-{}", s.births, s.deaths)?;
        }
        if opts.age_heat {
            write!(out, "  maxage {}", grid.max_age())?;
        }
        if !opts.sparkline.is_empty() {
            write!(out, "  {}", opts.sparkline)?;
        }
        if !opts.status_extra.is_empty() {
            write!(out, "  {}", opts.status_extra)?;
        }
        write!(out, "  ({quit_hint})\x1b[K\n")?;
    }
    Ok(())
}

/// True when the grid uses multi-state rendering (firing bright / refractory dim).
#[inline]
fn multistate(grid: &Grid) -> bool {
    grid.states > 2
}

/// Choose ANSI open for a multi-state cell (firing bright, refractory dim).
fn multistate_ansi(theme: Theme, cell: u8) -> String {
    match cell {
        state::LIVE => theme.live_ansi(),
        state::REFRACTORY => format!("\x1b[2;{}m", theme.live_sgr), // dim
        _ => String::new(),
    }
}

/// RGB for multi-state PPM.
fn multistate_rgb(theme: Theme, cell: u8) -> (u8, u8, u8) {
    match cell {
        state::LIVE => theme.live_rgb,
        state::REFRACTORY => {
            let (r, g, b) = theme.live_rgb;
            (r / 3, g / 3, b / 3)
        }
        _ => theme.dead_rgb,
    }
}

fn write_age_fg<W: Write>(out: &mut W, theme: Theme, age: u16) -> io::Result<()> {
    let c = theme.age_color_256(age);
    write!(out, "\x1b[38;5;{c}m")
}

fn render_block<W: Write>(grid: &Grid, opts: &RenderOpts, out: &mut W) -> io::Result<()> {
    let live_open = opts.theme.live_ansi();
    let ms = multistate(grid);
    for y in 0..grid.h {
        if ms && opts.color {
            for x in 0..grid.w {
                let i = grid.idx(x, y);
                let c = grid.cells[i];
                if c != 0 {
                    let open = multistate_ansi(opts.theme, c);
                    out.write_all(open.as_bytes())?;
                    out.write_all("██".as_bytes())?;
                    out.write_all(b"\x1b[0m")?;
                } else {
                    out.write_all(b"  ")?;
                }
            }
        } else if opts.age_heat && opts.color {
            for x in 0..grid.w {
                let i = grid.idx(x, y);
                if grid.cells[i] != 0 {
                    write_age_fg(out, opts.theme, grid.ages[i])?;
                    out.write_all("██".as_bytes())?;
                    out.write_all(b"\x1b[0m")?;
                } else {
                    out.write_all(b"  ")?;
                }
            }
        } else {
            let mut run_alive = false;
            for x in 0..grid.w {
                let alive = grid.cells[grid.idx(x, y)] != 0;
                if opts.color {
                    if alive && !run_alive {
                        out.write_all(live_open.as_bytes())?;
                        run_alive = true;
                    } else if !alive && run_alive {
                        out.write_all(b"\x1b[0m")?;
                        run_alive = false;
                    }
                }
                out.write_all(if alive { "██".as_bytes() } else { b"  " })?;
            }
            if opts.color && run_alive {
                out.write_all(b"\x1b[0m")?;
            }
        }
        out.write_all(b"\x1b[K\n")?;
    }
    Ok(())
}

fn render_dots<W: Write>(grid: &Grid, opts: &RenderOpts, out: &mut W) -> io::Result<()> {
    let live_open = opts.theme.live_ansi();
    let ms = multistate(grid);
    for y in 0..grid.h {
        if ms && opts.color {
            for x in 0..grid.w {
                let i = grid.idx(x, y);
                let c = grid.cells[i];
                if c != 0 {
                    let open = multistate_ansi(opts.theme, c);
                    out.write_all(open.as_bytes())?;
                    // firing = · , refractory = °
                    let glyph = if c == state::REFRACTORY { "°" } else { "·" };
                    out.write_all(glyph.as_bytes())?;
                    out.write_all(b"\x1b[0m")?;
                } else {
                    out.write_all(b" ")?;
                }
            }
        } else if opts.age_heat && opts.color {
            for x in 0..grid.w {
                let i = grid.idx(x, y);
                if grid.cells[i] != 0 {
                    write_age_fg(out, opts.theme, grid.ages[i])?;
                    out.write_all("·".as_bytes())?;
                    out.write_all(b"\x1b[0m")?;
                } else {
                    out.write_all(b" ")?;
                }
            }
        } else {
            let mut run_alive = false;
            for x in 0..grid.w {
                let alive = grid.cells[grid.idx(x, y)] != 0;
                if opts.color {
                    if alive && !run_alive {
                        out.write_all(live_open.as_bytes())?;
                        run_alive = true;
                    } else if !alive && run_alive {
                        out.write_all(b"\x1b[0m")?;
                        run_alive = false;
                    }
                }
                out.write_all(if alive { "·".as_bytes() } else { b" " })?;
            }
            if opts.color && run_alive {
                out.write_all(b"\x1b[0m")?;
            }
        }
        out.write_all(b"\x1b[K\n")?;
    }
    Ok(())
}

/// Braille packing: each character covers 2 columns × 4 rows of cells.
/// Dot bit layout (Unicode Braille):
///  0 3
///  1 4
///  2 5
///  6 7
fn render_braille<W: Write>(grid: &Grid, opts: &RenderOpts, out: &mut W) -> io::Result<()> {
    const DOT_MAP: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];
    let live_open = opts.theme.live_ansi();

    let rows = (grid.h + 3) / 4;
    let cols = (grid.w + 1) / 2;

    for br in 0..rows {
        for bc in 0..cols {
            let mut bits: u32 = 0;
            let mut max_age = 0u16;
            let mut any = false;
            let mut max_state = 0u8;
            for dy in 0..4 {
                for dx in 0..2 {
                    let x = bc * 2 + dx;
                    let y = br * 4 + dy;
                    if x < grid.w && y < grid.h {
                        let i = grid.idx(x, y);
                        if grid.cells[i] != 0 {
                            bits |= 1u32 << DOT_MAP[dy][dx];
                            any = true;
                            max_age = max_age.max(grid.ages[i]);
                            // prefer firing (1) over refractory for colour
                            if grid.cells[i] == state::LIVE || max_state == 0 {
                                max_state = grid.cells[i];
                            }
                        }
                    }
                }
            }
            if opts.color {
                if any && multistate(grid) {
                    let open = multistate_ansi(opts.theme, max_state);
                    out.write_all(open.as_bytes())?;
                } else if any && opts.age_heat {
                    write_age_fg(out, opts.theme, max_age)?;
                } else if any {
                    out.write_all(live_open.as_bytes())?;
                }
            }
            let ch = char::from_u32(0x2800 + bits).unwrap_or(' ');
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            out.write_all(s.as_bytes())?;
            if opts.color && any {
                out.write_all(b"\x1b[0m")?;
            }
        }
        out.write_all(b"\x1b[K\n")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// PPM export (P6 binary) — pure stdlib
// ---------------------------------------------------------------------------

/// Write the grid as a binary PPM (P6) image using theme / age colours.
///
/// Each cell is a single pixel. Live cells use age heat-map RGB when `age_heat`
/// is true, otherwise the theme's solid live colour.
pub fn export_ppm(grid: &Grid, path: &Path, theme: Theme, age_heat: bool) -> Result<(), String> {
    let f = File::create(path).map_err(|e| format!("export-ppm {}: {e}", path.display()))?;
    let mut w = BufWriter::new(f);
    write_ppm_p6(grid, theme, age_heat, &mut w)
        .map_err(|e| format!("export-ppm {}: {e}", path.display()))
}

/// Write P6 PPM to any writer.
pub fn write_ppm_p6<W: Write>(
    grid: &Grid,
    theme: Theme,
    age_heat: bool,
    out: &mut W,
) -> io::Result<()> {
    writeln!(out, "P6")?;
    writeln!(out, "{} {}", grid.w, grid.h)?;
    writeln!(out, "255")?;
    for y in 0..grid.h {
        for x in 0..grid.w {
            let i = grid.idx(x, y);
            let cell = grid.cells[i];
            let (r, g, b) = if multistate(grid) && cell != 0 {
                multistate_rgb(theme, cell)
            } else if cell != 0 {
                if age_heat {
                    theme.age_rgb(grid.ages[i])
                } else {
                    theme.live_rgb
                }
            } else {
                theme.dead_rgb
            };
            out.write_all(&[r, g, b])?;
        }
    }
    out.flush()
}

/// Write P3 (ASCII) PPM — handy for tests / debugging.
#[allow(dead_code)]
pub fn write_ppm_p3<W: Write>(
    grid: &Grid,
    theme: Theme,
    age_heat: bool,
    out: &mut W,
) -> io::Result<()> {
    writeln!(out, "P3")?;
    writeln!(out, "{} {}", grid.w, grid.h)?;
    writeln!(out, "255")?;
    for y in 0..grid.h {
        for x in 0..grid.w {
            let i = grid.idx(x, y);
            let cell = grid.cells[i];
            let (r, g, b) = if multistate(grid) && cell != 0 {
                multistate_rgb(theme, cell)
            } else if cell != 0 {
                if age_heat {
                    theme.age_rgb(grid.ages[i])
                } else {
                    theme.live_rgb
                }
            } else {
                theme.dead_rgb
            };
            if x > 0 {
                write!(out, " ")?;
            }
            write!(out, "{r} {g} {b}")?;
        }
        writeln!(out)?;
    }
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn style_parse() {
        assert_eq!(Style::parse("block").unwrap(), Style::Block);
        assert_eq!(Style::parse("BRAILLE").unwrap(), Style::Braille);
        assert_eq!(Style::parse("dots").unwrap(), Style::Dots);
        assert!(Style::parse("emoji").is_err());
    }

    #[test]
    fn braille_renders_something() {
        let mut g = Grid::new(4, 4);
        g.set(0, 0, true);
        g.set(1, 0, true);
        let mut buf = Vec::new();
        let opts = RenderOpts {
            color: false,
            age_heat: false,
            ..Default::default()
        };
        render_braille(&g, &opts, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            s.contains('\u{2800}') || s.chars().any(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
        );
    }

    #[test]
    fn age_color_ramps() {
        let t = Theme::CLASSIC;
        assert_eq!(t.age_color_256(1), 46);
        assert!(t.age_color_256(1) != t.age_color_256(50));
        assert!(t.age_color_256(200) != t.age_color_256(1));
    }

    #[test]
    fn age_heat_block_emits_256_color() {
        let mut g = Grid::new(3, 1);
        g.set(0, 0, true);
        g.set(1, 0, true);
        g.ages[0] = 1;
        g.ages[1] = 100;
        let mut buf = Vec::new();
        let opts = RenderOpts {
            color: true,
            age_heat: true,
            style: Style::Block,
            theme: Theme::CLASSIC,
            ..Default::default()
        };
        render_block(&g, &opts, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\x1b[38;5;"), "expected 256-color SGR: {s:?}");
    }

    #[test]
    fn theme_live_color_in_block() {
        let mut g = Grid::new(2, 1);
        g.set(0, 0, true);
        let mut buf = Vec::new();
        let opts = RenderOpts {
            color: true,
            age_heat: false,
            theme: Theme::FIRE,
            style: Style::Block,
            ..Default::default()
        };
        render_block(&g, &opts, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\x1b[91m"), "expected fire live SGR: {s:?}");
    }

    #[test]
    fn ppm_p6_header_and_size() {
        let mut g = Grid::new(2, 2);
        g.set(0, 0, true);
        let mut buf = Vec::new();
        write_ppm_p6(&g, Theme::CLASSIC, false, &mut buf).unwrap();
        // P6\n2 2\n255\n + 2*2*3 bytes
        assert!(buf.starts_with(b"P6\n"));
        let body_start = {
            // find third newline
            let mut count = 0;
            let mut idx = 0;
            for (i, &b) in buf.iter().enumerate() {
                if b == b'\n' {
                    count += 1;
                    if count == 3 {
                        idx = i + 1;
                        break;
                    }
                }
            }
            idx
        };
        assert_eq!(buf.len() - body_start, 2 * 2 * 3);
        // live pixel at (0,0) is classic green-ish
        assert_eq!(
            &buf[body_start..body_start + 3],
            &[
                Theme::CLASSIC.live_rgb.0,
                Theme::CLASSIC.live_rgb.1,
                Theme::CLASSIC.live_rgb.2
            ]
        );
    }

    #[test]
    fn ppm_p3_ascii() {
        let mut g = Grid::new(1, 1);
        g.set(0, 0, true);
        let mut buf = Vec::new();
        write_ppm_p3(&g, Theme::MONO, false, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with("P3\n"));
        assert!(s.contains("255"));
        assert!(s.contains("230 230 230"));
    }
}
