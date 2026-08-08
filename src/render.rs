use crate::grid::{Grid, StepStats};
use std::io::{self, Write};

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
    /// Colour live cells by age: young=green → yellow → red (ANSI 256).
    pub age_heat: bool,
    /// Extra status suffix (e.g. interactive pause / rule name).
    pub status_extra: String,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            style: Style::Block,
            color: true,
            show_stats: true,
            age_heat: false,
            status_extra: String::new(),
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
        "keys: spc . + - r q"
    } else {
        "Ctrl-C to quit"
    };
    if opts.color {
        write!(
            out,
            "\x1b[1;36mgol-rs\x1b[0m  gen \x1b[33m{:>5}\x1b[0m  pop \x1b[35m{:>5}\x1b[0m  grid \x1b[33m{}x{}\x1b[0m",
            generation, pop, grid.w, grid.h
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
        if !opts.status_extra.is_empty() {
            write!(out, "  \x1b[36m{}\x1b[0m", opts.status_extra)?;
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
        if !opts.status_extra.is_empty() {
            write!(out, "  {}", opts.status_extra)?;
        }
        write!(out, "  ({quit_hint})\x1b[K\n")?;
    }
    Ok(())
}

/// Map cell age → ANSI 256 colour index (green → yellow → red).
///
/// age 1: bright green; mid ages: yellow/orange; high: red.
fn age_color_256(age: u16) -> u8 {
    // Use a soft log-ish ramp so both young and old are visible.
    // 1 → 46 (green), then through 226 yellow, 208 orange, 196 red.
    match age {
        0 => 0,
        1 => 46,   // bright green
        2 => 82,   // green-yellow
        3..=4 => 118,
        5..=7 => 154,
        8..=12 => 190,
        13..=20 => 226,  // yellow
        21..=35 => 220,
        36..=55 => 214,
        56..=80 => 208,  // orange
        81..=120 => 202,
        121..=200 => 196, // red
        _ => 160,         // dark red
    }
}

fn write_age_fg<W: Write>(out: &mut W, age: u16) -> io::Result<()> {
    let c = age_color_256(age);
    write!(out, "\x1b[38;5;{c}m")
}

fn render_block<W: Write>(grid: &Grid, opts: &RenderOpts, out: &mut W) -> io::Result<()> {
    for y in 0..grid.h {
        if opts.age_heat && opts.color {
            for x in 0..grid.w {
                let i = grid.idx(x, y);
                if grid.cells[i] {
                    write_age_fg(out, grid.ages[i])?;
                    out.write_all("██".as_bytes())?;
                    out.write_all(b"\x1b[0m")?;
                } else {
                    out.write_all(b"  ")?;
                }
            }
        } else {
            let mut run_alive = false;
            for x in 0..grid.w {
                let alive = grid.cells[grid.idx(x, y)];
                if opts.color {
                    if alive && !run_alive {
                        out.write_all(b"\x1b[32m")?;
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
    for y in 0..grid.h {
        if opts.age_heat && opts.color {
            for x in 0..grid.w {
                let i = grid.idx(x, y);
                if grid.cells[i] {
                    write_age_fg(out, grid.ages[i])?;
                    out.write_all("·".as_bytes())?;
                    out.write_all(b"\x1b[0m")?;
                } else {
                    out.write_all(b" ")?;
                }
            }
        } else {
            let mut run_alive = false;
            for x in 0..grid.w {
                let alive = grid.cells[grid.idx(x, y)];
                if opts.color {
                    if alive && !run_alive {
                        out.write_all(b"\x1b[32m")?;
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

    let rows = (grid.h + 3) / 4;
    let cols = (grid.w + 1) / 2;

    for br in 0..rows {
        for bc in 0..cols {
            let mut bits: u32 = 0;
            let mut max_age = 0u16;
            let mut any = false;
            for dy in 0..4 {
                for dx in 0..2 {
                    let x = bc * 2 + dx;
                    let y = br * 4 + dy;
                    if x < grid.w && y < grid.h {
                        let i = grid.idx(x, y);
                        if grid.cells[i] {
                            bits |= 1u32 << DOT_MAP[dy][dx];
                            any = true;
                            max_age = max_age.max(grid.ages[i]);
                        }
                    }
                }
            }
            if opts.color {
                if any && opts.age_heat {
                    write_age_fg(out, max_age)?;
                } else if any {
                    out.write_all(b"\x1b[32m")?;
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
            s.contains('\u{2800}')
                || s.chars()
                    .any(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
        );
    }

    #[test]
    fn age_color_ramps() {
        assert_eq!(age_color_256(1), 46);
        assert!(age_color_256(1) != age_color_256(50));
        assert!(age_color_256(200) != age_color_256(1));
    }

    #[test]
    fn age_heat_block_emits_256_color() {
        let mut g = Grid::new(3, 1);
        g.set(0, 0, true);
        g.set(1, 0, true);
        // age them differently via direct write
        g.ages[0] = 1;
        g.ages[1] = 100;
        let mut buf = Vec::new();
        let opts = RenderOpts {
            color: true,
            age_heat: true,
            style: Style::Block,
            ..Default::default()
        };
        render_block(&g, &opts, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\x1b[38;5;"), "expected 256-color SGR: {s:?}");
    }
}
