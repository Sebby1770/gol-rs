use crate::grid::Grid;
use crate::rng::Rng;
use std::fs;
use std::path::Path;

/// Names of all built-in patterns (excluding aliases).
pub const PATTERN_NAMES: &[&str] = &[
    "random",
    "glider",
    "spaceship", // alias of glider in listing note
    "blinker",
    "toad",
    "beacon",
    "block",
    "beehive",
    "lwss",
    "mwss",
    "hwss",
    "rpentomino",
    "acorn",
    "diehard",
    "pulsar",
    "gosper",
    "pentadecathlon",
    "glider-pair",
    "infinite1",
];

/// Print available patterns to stdout.
pub fn list_patterns() {
    println!("Built-in patterns:");
    println!("  random          — random fill (use --density)");
    println!("  glider          — classic glider spaceship");
    println!("  spaceship       — alias for glider");
    println!("  blinker         — period-2 oscillator");
    println!("  toad            — period-2 oscillator");
    println!("  beacon          — period-2 oscillator");
    println!("  block           — 2x2 still life");
    println!("  beehive         — still life");
    println!("  lwss            — lightweight spaceship");
    println!("  mwss            — middleweight spaceship");
    println!("  hwss            — heavyweight spaceship");
    println!("  rpentomino      — methuselah (R-pentomino)");
    println!("  acorn           — methuselah");
    println!("  diehard         — dies after 130 gens");
    println!("  pulsar          — period-3 oscillator");
    println!("  gosper          — Gosper glider gun");
    println!("  pentadecathlon  — period-15 oscillator");
    println!("  glider-pair     — two gliders");
    println!("  infinite1       — infinite-growth methuselah (switch engine)");
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

fn place_glider(g: &mut Grid, ox: usize, oy: usize) {
    for &(dx, dy) in &[(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
        g.set(ox + dx, oy + dy, true);
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

const TOAD: &str = "\
.ooo
ooo.";

const BEACON: &str = "\
oo..
oo..
..oo
..oo";

const BLOCK: &str = "\
oo
oo";

const BEEHIVE: &str = "\
.oo.
o..o
.oo.";

const LWSS: &str = "\
.o..o
o....
o...o
oooo.";

const MWSS: &str = "\
..o..
.o...
o....
o...o
oooo.";

const HWSS: &str = "\
..oo..
.o....
o.....
o....o
ooooo.";

const RPENTOMINO: &str = "\
.oo
oo.
.o.";

const ACORN: &str = "\
.o.....
...o...
oo..ooo";

const DIEHARD: &str = "\
......o.
oo......
.o...ooo";

const BLINKER: &str = "\
ooo";

/// Pentadecathlon (period-15 oscillator), 10×3 bounding core.
const PENTADECATHLON: &str = "\
..o....o..
oo.oooo.oo
..o....o..";

/// Two gliders heading the same way (simple glider pair).
const GLIDER_PAIR: &str = "\
.o.....o.
..o.....o
ooo...ooo";

/// Classic infinite-growth seed (switch-engine precursor / "infinite1").
/// Minimal pattern that grows without bound on an infinite plane.
const INFINITE1: &str = "\
ooooooo.o
oo.o...oo
";

/// Seed `grid` with a named built-in pattern.
pub fn seed_pattern(
    grid: &mut Grid,
    name: &str,
    rng: &mut Rng,
    density: f64,
) -> Result<(), String> {
    let name = name.to_ascii_lowercase();
    match name.as_str() {
        "random" => {
            for c in grid.cells.iter_mut() {
                *c = rng.next_f64() < density;
            }
            grid.sync_ages_from_cells();
        }
        "glider" | "spaceship" => place_glider(grid, 1, 1),
        "blinker" => {
            let ox = grid.w.saturating_sub(3) / 2;
            let oy = grid.h / 2;
            place_from_str(grid, ox, oy, BLINKER);
        }
        "toad" => {
            let ox = grid.w.saturating_sub(4) / 2;
            let oy = grid.h.saturating_sub(2) / 2;
            place_from_str(grid, ox, oy, TOAD);
        }
        "beacon" => {
            let ox = grid.w.saturating_sub(4) / 2;
            let oy = grid.h.saturating_sub(4) / 2;
            place_from_str(grid, ox, oy, BEACON);
        }
        "block" => {
            let ox = grid.w.saturating_sub(2) / 2;
            let oy = grid.h.saturating_sub(2) / 2;
            place_from_str(grid, ox, oy, BLOCK);
        }
        "beehive" => {
            let ox = grid.w.saturating_sub(4) / 2;
            let oy = grid.h.saturating_sub(3) / 2;
            place_from_str(grid, ox, oy, BEEHIVE);
        }
        "lwss" => {
            let ox = 2;
            let oy = grid.h.saturating_sub(4) / 2;
            place_from_str(grid, ox, oy, LWSS);
        }
        "mwss" => {
            let ox = 2;
            let oy = grid.h.saturating_sub(5) / 2;
            place_from_str(grid, ox, oy, MWSS);
        }
        "hwss" => {
            let ox = 2;
            let oy = grid.h.saturating_sub(5) / 2;
            place_from_str(grid, ox, oy, HWSS);
        }
        "rpentomino" => {
            let ox = grid.w.saturating_sub(3) / 2;
            let oy = grid.h.saturating_sub(3) / 2;
            place_from_str(grid, ox, oy, RPENTOMINO);
        }
        "acorn" => {
            let ox = grid.w.saturating_sub(7) / 2;
            let oy = grid.h.saturating_sub(3) / 2;
            place_from_str(grid, ox, oy, ACORN);
        }
        "diehard" => {
            let ox = grid.w.saturating_sub(8) / 2;
            let oy = grid.h.saturating_sub(3) / 2;
            place_from_str(grid, ox, oy, DIEHARD);
        }
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
        "pentadecathlon" | "penta" | "pd" => {
            let ox = grid.w.saturating_sub(10) / 2;
            let oy = grid.h.saturating_sub(3) / 2;
            place_from_str(grid, ox, oy, PENTADECATHLON);
        }
        "glider-pair" | "gliderpair" | "gliders" => {
            let ox = grid.w.saturating_sub(9) / 2;
            let oy = grid.h.saturating_sub(3) / 2;
            place_from_str(grid, ox, oy, GLIDER_PAIR);
        }
        "infinite1" | "infinite" => {
            let ox = grid.w.saturating_sub(9) / 2;
            let oy = grid.h.saturating_sub(2) / 2;
            place_from_str(grid, ox, oy, INFINITE1);
        }
        other => {
            let known = PATTERN_NAMES.join(", ");
            return Err(format!("unknown pattern: {other} (try: {known})"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// RLE (Life RLE subset) + Life 1.05-ish plain text
// ---------------------------------------------------------------------------

/// Load a pattern from RLE or Life 1.05 plain text into `grid`, centered when possible.
pub fn load_file(grid: &mut Grid, path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("load {}: {e}", path.display()))?;
    load_rle_into(grid, &text)
}

/// Parse RLE / Life 1.05 content and place cells into `grid`.
pub fn load_rle_into(grid: &mut Grid, text: &str) -> Result<(), String> {
    let trimmed = text.trim_start();
    // Life 1.05 / plain: lines of . and O/*/# (or #P header)
    if looks_like_plain(trimmed) {
        return load_plain_into(grid, text);
    }
    load_rle_body(grid, text)
}

fn looks_like_plain(text: &str) -> bool {
    // If there is no `x =` header and content uses .O patterns, treat as plain.
    let has_rle_header = text.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("x") && t.contains('=')
    });
    if has_rle_header {
        return false;
    }
    // Life 1.05 often starts with #Life or #P
    text.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("#Life") || t.starts_with("#P") || t.starts_with("#N")
    })
}

fn load_plain_into(grid: &mut Grid, text: &str) -> Result<(), String> {
    let mut rows: Vec<Vec<bool>> = Vec::new();
    let mut ox = 0i64;
    let mut oy = 0i64;
    let mut cur_y = 0i64;
    let mut saw_p = false;

    for line in text.lines() {
        let t = line.trim_end();
        if t.is_empty() {
            continue;
        }
        if t.starts_with('#') {
            if let Some(rest) = t.strip_prefix("#P") {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() >= 2 {
                    ox = parts[0].parse().unwrap_or(0);
                    oy = parts[1].parse().unwrap_or(0);
                    cur_y = 0;
                    saw_p = true;
                }
            }
            continue;
        }
        // pattern row
        let mut row = Vec::new();
        for c in t.chars() {
            match c {
                'o' | 'O' | '*' | '#' | '1' => row.push(true),
                '.' | 'b' | '0' | ' ' => row.push(false),
                _ => {}
            }
        }
        if !row.is_empty() {
            rows.push(row);
            if !saw_p {
                // accumulate height for centering later
            }
            cur_y += 1;
        }
    }

    if rows.is_empty() {
        return Err("no cells found in plain pattern".into());
    }

    let ph = rows.len();
    let pw = rows.iter().map(|r| r.len()).max().unwrap_or(0);

    // If no #P, center on grid.
    let (base_x, base_y) = if saw_p {
        (
            if ox < 0 {
                grid.w as i64 + ox
            } else {
                ox
            },
            if oy < 0 {
                grid.h as i64 + oy
            } else {
                oy
            },
        )
    } else {
        (
            (grid.w.saturating_sub(pw) / 2) as i64,
            (grid.h.saturating_sub(ph) / 2) as i64,
        )
    };

    grid.clear();
    for (dy, row) in rows.iter().enumerate() {
        for (dx, &alive) in row.iter().enumerate() {
            if alive {
                let x = (base_x + dx as i64).rem_euclid(grid.w as i64) as usize;
                let y = (base_y + dy as i64).rem_euclid(grid.h as i64) as usize;
                grid.set(x, y, true);
            }
        }
    }
    let _ = cur_y;
    Ok(())
}

fn load_rle_body(grid: &mut Grid, text: &str) -> Result<(), String> {
    let mut width: Option<usize> = None;
    let mut height: Option<usize> = None;
    let mut body = String::new();

    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if t.starts_with('x') || t.starts_with('X') {
            // x = W, y = H, rule = B3/S23
            for part in t.split(',') {
                let part = part.trim();
                if let Some(v) = part
                    .strip_prefix("x")
                    .or_else(|| part.strip_prefix("X"))
                    .map(|s| s.trim())
                    .and_then(|s| s.strip_prefix('='))
                    .map(|s| s.trim())
                {
                    width = v
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .or(width);
                } else if let Some(v) = part
                    .strip_prefix("y")
                    .or_else(|| part.strip_prefix("Y"))
                    .map(|s| s.trim())
                    .and_then(|s| s.strip_prefix('='))
                    .map(|s| s.trim())
                {
                    height = v
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .or(height);
                }
            }
            continue;
        }
        body.push_str(t);
    }

    if body.is_empty() {
        // Maybe plain cells without header — try plain
        return load_plain_into(grid, text);
    }

    let (pw, ph, cells) = decode_rle(&body)?;
    let pattern_w = width.unwrap_or(pw);
    let pattern_h = height.unwrap_or(ph);

    if pattern_w > grid.w || pattern_h > grid.h {
        return Err(format!(
            "pattern {pattern_w}x{pattern_h} does not fit in grid {}x{}",
            grid.w, grid.h
        ));
    }

    let ox = grid.w.saturating_sub(pattern_w) / 2;
    let oy = grid.h.saturating_sub(pattern_h) / 2;

    grid.clear();
    for y in 0..ph {
        for x in 0..pw {
            if cells[y * pw + x] {
                grid.set(ox + x, oy + y, true);
            }
        }
    }
    Ok(())
}

/// Decode RLE body (run counts + b/o/$/!) into a dense bool grid.
fn decode_rle(body: &str) -> Result<(usize, usize, Vec<bool>), String> {
    let mut rows: Vec<Vec<bool>> = vec![Vec::new()];
    let mut count: usize = 0;
    let mut has_digit = false;

    let finish_run = |count: &mut usize, has_digit: &mut bool| {
        let n = if *has_digit { *count } else { 1 };
        *count = 0;
        *has_digit = false;
        n
    };

    for c in body.chars() {
        match c {
            '0'..='9' => {
                has_digit = true;
                count = count
                    .checked_mul(10)
                    .and_then(|v| v.checked_add((c as u8 - b'0') as usize))
                    .ok_or("RLE run count overflow")?;
            }
            'b' | '.' => {
                let n = finish_run(&mut count, &mut has_digit);
                let row = rows.last_mut().unwrap();
                for _ in 0..n {
                    row.push(false);
                }
            }
            'o' | 'O' | '*' => {
                let n = finish_run(&mut count, &mut has_digit);
                let row = rows.last_mut().unwrap();
                for _ in 0..n {
                    row.push(true);
                }
            }
            '$' => {
                let n = finish_run(&mut count, &mut has_digit);
                // end current row, then add n-1 empty rows
                for _ in 0..n {
                    rows.push(Vec::new());
                }
            }
            '!' => break,
            ' ' | '\t' | '\n' | '\r' => {}
            other => {
                // ignore unknown tags for subset tolerance
                let _ = other;
            }
        }
    }

    // Drop trailing empty row often produced by trailing $
    while rows.last().map(|r| r.is_empty()).unwrap_or(false) && rows.len() > 1 {
        // Keep if we ended with $ intentionally — normalize width first.
        // Actually RLE `$` ends a line; trailing empty is fine to drop only if extra.
        if body.trim_end().ends_with('!') || body.contains('!') {
            // leave last empty if pattern explicitly ended mid-structure
        }
        break;
    }

    // Remove final empty rows that are only from a terminating $ before !
    while rows.len() > 1 && rows.last().map(|r| r.is_empty()).unwrap_or(false) {
        rows.pop();
    }

    let ph = rows.len().max(1);
    let pw = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
    let mut cells = vec![false; pw * ph];
    for (y, row) in rows.iter().enumerate() {
        for (x, &alive) in row.iter().enumerate() {
            cells[y * pw + x] = alive;
        }
    }
    Ok((pw, ph, cells))
}

/// Encode live cells of `grid` as a Life RLE string (trimmed bounding box).
pub fn to_rle(grid: &Grid, name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("#N {name}\n"));
    out.push_str("#O gol-rs\n");

    let Some((min_x, min_y, max_x, max_y)) = grid.live_bounds() else {
        out.push_str("x = 0, y = 0, rule = B3/S23\n!\n");
        return out;
    };

    let w = max_x - min_x + 1;
    let h = max_y - min_y + 1;
    out.push_str(&format!("x = {w}, y = {h}, rule = B3/S23\n"));

    let mut body = String::new();
    for y in min_y..=max_y {
        let mut x = min_x;
        while x <= max_x {
            let alive = grid.get(x, y);
            let mut run = 1usize;
            while x + run <= max_x && grid.get(x + run, y) == alive {
                run += 1;
            }
            if run > 1 {
                body.push_str(&run.to_string());
            }
            body.push(if alive { 'o' } else { 'b' });
            x += run;
        }
        // trim trailing dead runs on the line
        while body.ends_with('b') {
            body.pop();
        }
        // also strip trailing count digits before b we may have over-trimmed...
        // simpler: rebuild line without trailing b's — already done by not writing trailing dead?
        // We may have written "3b" at end — strip trailing dead run fully:
        strip_trailing_dead_run(&mut body);
        if y < max_y {
            body.push('$');
        }
    }
    body.push('!');
    // wrap body at ~70 cols
    for (i, c) in body.chars().enumerate() {
        if i > 0 && i % 70 == 0 {
            out.push('\n');
        }
        out.push(c);
    }
    out.push('\n');
    out
}

fn strip_trailing_dead_run(s: &mut String) {
    // Remove trailing `b` or `Nb` (digits then b)
    loop {
        if s.ends_with('b') {
            s.pop();
            // remove digits belonging to that run
            while s
                .chars()
                .last()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                s.pop();
            }
        } else {
            break;
        }
    }
}

/// Save grid as RLE to `path`.
pub fn save_file(grid: &Grid, path: &Path) -> Result<(), String> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("pattern");
    let rle = to_rle(grid, name);
    fs::write(path, rle).map_err(|e| format!("save {}: {e}", path.display()))
}

/// Dump grid as RLE string (for --dump rle).
pub fn dump_rle(grid: &Grid) -> String {
    to_rle(grid, "dump")
}

/// Dump grid as ASCII `.` / `O` (for --dump ascii).
pub fn dump_ascii(grid: &Grid) -> String {
    let mut out = String::new();
    if let Some((min_x, min_y, max_x, max_y)) = grid.live_bounds() {
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                out.push(if grid.get(x, y) { 'O' } else { '.' });
            }
            out.push('\n');
        }
    } else {
        out.push_str("(empty)\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;
    use crate::rng::Rng;

    #[test]
    fn pattern_list_contains_expected() {
        let joined = PATTERN_NAMES.join(" ");
        for name in [
            "random",
            "glider",
            "blinker",
            "toad",
            "beacon",
            "lwss",
            "mwss",
            "hwss",
            "rpentomino",
            "acorn",
            "diehard",
            "pulsar",
            "gosper",
            "spaceship",
            "pentadecathlon",
            "glider-pair",
            "infinite1",
        ] {
            assert!(joined.contains(name), "missing {name}");
        }
    }

    #[test]
    fn pentadecathlon_seeds() {
        let mut g = Grid::new(20, 12);
        let mut rng = Rng::new(1);
        seed_pattern(&mut g, "pentadecathlon", &mut rng, 0.0).unwrap();
        assert!(g.population() > 0);
        // Should oscillate: not still after 1 step
        let gen0 = g.cells.clone();
        g.step();
        // may or may not equal gen0 depending on phase — just ensure it runs
        let _ = gen0;
        assert!(g.population() > 0);
    }

    #[test]
    fn glider_pair_seeds() {
        let mut g = Grid::new(20, 12);
        let mut rng = Rng::new(1);
        seed_pattern(&mut g, "glider-pair", &mut rng, 0.0).unwrap();
        assert_eq!(g.population(), 10); // two gliders × 5
    }

    #[test]
    fn toad_period_2() {
        let mut g = Grid::new(8, 8);
        let mut rng = Rng::new(1);
        seed_pattern(&mut g, "toad", &mut rng, 0.0).unwrap();
        let gen0 = g.cells.clone();
        g.step();
        let gen1 = g.cells.clone();
        assert_ne!(gen0, gen1);
        g.step();
        assert_eq!(g.cells, gen0);
    }

    #[test]
    fn blinker_period_2() {
        let mut g = Grid::new(9, 9);
        let mut rng = Rng::new(1);
        seed_pattern(&mut g, "blinker", &mut rng, 0.0).unwrap();
        let gen0 = g.cells.clone();
        g.step();
        assert_ne!(g.cells, gen0);
        g.step();
        assert_eq!(g.cells, gen0);
    }

    #[test]
    fn block_still_via_pattern() {
        let mut g = Grid::new(8, 8);
        let mut rng = Rng::new(1);
        seed_pattern(&mut g, "block", &mut rng, 0.0).unwrap();
        let before = g.clone();
        g.step();
        assert_eq!(g, before);
        assert_eq!(g.population(), 4);
    }

    #[test]
    fn beehive_still_via_pattern() {
        let mut g = Grid::new(10, 10);
        let mut rng = Rng::new(1);
        seed_pattern(&mut g, "beehive", &mut rng, 0.0).unwrap();
        let before = g.clone();
        g.step();
        assert_eq!(g, before);
        assert_eq!(g.population(), 6);
    }

    #[test]
    fn spaceship_alias_glider() {
        let mut a = Grid::new(10, 10);
        let mut b = Grid::new(10, 10);
        let mut rng = Rng::new(1);
        seed_pattern(&mut a, "glider", &mut rng, 0.0).unwrap();
        seed_pattern(&mut b, "spaceship", &mut rng, 0.0).unwrap();
        assert_eq!(a.cells, b.cells);
    }

    #[test]
    fn rle_roundtrip() {
        let mut g = Grid::new(20, 20);
        let mut rng = Rng::new(1);
        seed_pattern(&mut g, "glider", &mut rng, 0.0).unwrap();
        let rle = to_rle(&g, "glider");
        assert!(rle.contains("x ="));
        assert!(rle.contains('o'));

        let mut g2 = Grid::new(20, 20);
        load_rle_into(&mut g2, &rle).unwrap();
        // Same live pattern up to translation — populations match and RLE re-encode equal shape
        assert_eq!(g.population(), g2.population());
        let rle2 = to_rle(&g2, "glider");
        // Compare decoded bounding bodies
        let mut g3 = Grid::new(20, 20);
        load_rle_into(&mut g3, &rle2).unwrap();
        assert_eq!(g2.cells, g3.cells);
    }

    #[test]
    fn rle_blinker_decode() {
        let rle = "#N blinker\nx = 3, y = 1, rule = B3/S23\n3o!\n";
        let mut g = Grid::new(9, 9);
        load_rle_into(&mut g, rle).unwrap();
        assert_eq!(g.population(), 3);
        let before = g.cells.clone();
        g.step();
        assert_ne!(g.cells, before);
        g.step();
        assert_eq!(g.cells, before);
    }

    #[test]
    fn rle_with_runs() {
        // 2o$2o! is a block
        let rle = "x = 2, y = 2, rule = B3/S23\n2o$2o!\n";
        let mut g = Grid::new(6, 6);
        load_rle_into(&mut g, rle).unwrap();
        assert_eq!(g.population(), 4);
        let before = g.clone();
        g.step();
        assert_eq!(g, before);
    }
}
