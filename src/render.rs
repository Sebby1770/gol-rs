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
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            style: Style::Block,
            color: true,
            show_stats: true,
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
        write_header(grid, generation, last_stats, opts.color, out)?;
    }

    match opts.style {
        Style::Block => render_block(grid, opts.color, out)?,
        Style::Dots => render_dots(grid, opts.color, out)?,
        Style::Braille => render_braille(grid, opts.color, out)?,
    }
    out.flush()
}

fn write_header<W: Write>(
    grid: &Grid,
    generation: u64,
    last_stats: Option<StepStats>,
    color: bool,
    out: &mut W,
) -> io::Result<()> {
    let pop = grid.population();
    if color {
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
        write!(out, "  (Ctrl-C to quit)\x1b[K\n")?;
    } else {
        write!(
            out,
            "gol-rs  gen {:>5}  pop {:>5}  grid {}x{}",
            generation, pop, grid.w, grid.h
        )?;
        if let Some(s) = last_stats {
            write!(out, "  +{}/-{}", s.births, s.deaths)?;
        }
        write!(out, "  (Ctrl-C to quit)\x1b[K\n")?;
    }
    Ok(())
}

fn render_block<W: Write>(grid: &Grid, color: bool, out: &mut W) -> io::Result<()> {
    for y in 0..grid.h {
        let mut run_alive = false;
        for x in 0..grid.w {
            let alive = grid.cells[grid.idx(x, y)];
            if color {
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
        if color && run_alive {
            out.write_all(b"\x1b[0m")?;
        }
        out.write_all(b"\x1b[K\n")?;
    }
    Ok(())
}

fn render_dots<W: Write>(grid: &Grid, color: bool, out: &mut W) -> io::Result<()> {
    for y in 0..grid.h {
        let mut run_alive = false;
        for x in 0..grid.w {
            let alive = grid.cells[grid.idx(x, y)];
            if color {
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
        if color && run_alive {
            out.write_all(b"\x1b[0m")?;
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
fn render_braille<W: Write>(grid: &Grid, color: bool, out: &mut W) -> io::Result<()> {
    const DOT_MAP: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

    let rows = (grid.h + 3) / 4;
    let cols = (grid.w + 1) / 2;

    for br in 0..rows {
        if color {
            out.write_all(b"\x1b[32m")?;
        }
        for bc in 0..cols {
            let mut bits: u32 = 0;
            for dy in 0..4 {
                for dx in 0..2 {
                    let x = bc * 2 + dx;
                    let y = br * 4 + dy;
                    if x < grid.w && y < grid.h && grid.cells[grid.idx(x, y)] {
                        bits |= 1u32 << DOT_MAP[dy][dx];
                    }
                }
            }
            let ch = char::from_u32(0x2800 + bits).unwrap_or(' ');
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            out.write_all(s.as_bytes())?;
        }
        if color {
            out.write_all(b"\x1b[0m")?;
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
        render_braille(&g, false, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains('\u{2800}') || s.chars().any(|c| ('\u{2801}'..='\u{28FF}').contains(&c)));
    }
}
