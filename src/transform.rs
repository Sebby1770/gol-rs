//! Grid geometric transforms (rotate / flip).

use crate::grid::Grid;

/// Rotate the grid 90° clockwise. Dimensions swap (`w×h` → `h×w`).
///
/// New cell at `(h-1-y, x)` comes from old `(x, y)`.
pub fn rotate90(grid: &Grid) -> Grid {
    let mut out = Grid::with_states(grid.h, grid.w, grid.wrap, grid.states);
    for y in 0..grid.h {
        for x in 0..grid.w {
            let i = grid.idx(x, y);
            let v = grid.cells[i];
            if v != 0 {
                // (x,y) → (h-1-y, x)
                let nx = grid.h - 1 - y;
                let ny = x;
                let j = out.idx(nx, ny);
                out.cells[j] = v;
                out.ages[j] = grid.ages[i];
            }
        }
    }
    out
}

/// Flip horizontally (mirror left ↔ right).
pub fn flip_h(grid: &Grid) -> Grid {
    let mut out = Grid::with_states(grid.w, grid.h, grid.wrap, grid.states);
    for y in 0..grid.h {
        for x in 0..grid.w {
            let i = grid.idx(x, y);
            let v = grid.cells[i];
            if v != 0 {
                let nx = grid.w - 1 - x;
                let j = out.idx(nx, y);
                out.cells[j] = v;
                out.ages[j] = grid.ages[i];
            }
        }
    }
    out
}

/// Flip vertically (mirror top ↔ bottom).
pub fn flip_v(grid: &Grid) -> Grid {
    let mut out = Grid::with_states(grid.w, grid.h, grid.wrap, grid.states);
    for y in 0..grid.h {
        for x in 0..grid.w {
            let i = grid.idx(x, y);
            let v = grid.cells[i];
            if v != 0 {
                let ny = grid.h - 1 - y;
                let j = out.idx(x, ny);
                out.cells[j] = v;
                out.ages[j] = grid.ages[i];
            }
        }
    }
    out
}

/// In-place convenience: replace `grid` with a 90° clockwise rotation.
pub fn rotate90_in_place(grid: &mut Grid) {
    *grid = rotate90(grid);
}

/// In-place horizontal flip.
pub fn flip_h_in_place(grid: &mut Grid) {
    *grid = flip_h(grid);
}

/// In-place vertical flip.
pub fn flip_v_in_place(grid: &mut Grid) {
    *grid = flip_v(grid);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glider() -> Grid {
        let mut g = Grid::new(5, 5);
        // classic glider
        g.set(1, 0, true);
        g.set(2, 1, true);
        g.set(0, 2, true);
        g.set(1, 2, true);
        g.set(2, 2, true);
        g
    }

    #[test]
    fn flip_h_mirrors() {
        let mut g = Grid::new(3, 2);
        g.set(0, 0, true);
        g.set(2, 1, true);
        let f = flip_h(&g);
        assert!(f.get(2, 0));
        assert!(f.get(0, 1));
        assert!(!f.get(0, 0));
        assert_eq!(f.population(), 2);
    }

    #[test]
    fn flip_v_mirrors() {
        let mut g = Grid::new(2, 3);
        g.set(0, 0, true);
        g.set(1, 2, true);
        let f = flip_v(&g);
        assert!(f.get(0, 2));
        assert!(f.get(1, 0));
        assert_eq!(f.population(), 2);
    }

    #[test]
    fn rotate90_swaps_dims() {
        let mut g = Grid::new(4, 2);
        g.set(0, 0, true);
        g.set(3, 1, true);
        let r = rotate90(&g);
        assert_eq!(r.w, 2);
        assert_eq!(r.h, 4);
        // (0,0) → (1, 0)  because h-1-y = 2-1-0 = 1, ny = x = 0
        assert!(r.get(1, 0));
        // (3,1) → (2-1-1, 3) = (0, 3)
        assert!(r.get(0, 3));
        assert_eq!(r.population(), 2);
    }

    #[test]
    fn rotate90_four_times_identity() {
        let g = glider();
        let r = rotate90(&rotate90(&rotate90(&rotate90(&g))));
        assert_eq!(r.w, g.w);
        assert_eq!(r.h, g.h);
        assert_eq!(r.cells, g.cells);
    }

    #[test]
    fn flip_h_twice_identity() {
        let g = glider();
        let f = flip_h(&flip_h(&g));
        assert_eq!(f.cells, g.cells);
    }

    #[test]
    fn preserves_wrap() {
        let mut g = Grid::with_wrap(4, 4, false);
        g.set(1, 1, true);
        assert!(!rotate90(&g).wrap);
        assert!(!flip_h(&g).wrap);
        assert!(!flip_v(&g).wrap);
    }

    #[test]
    fn preserves_multistate() {
        let mut g = Grid::with_states(4, 4, true, 3);
        g.set_state(1, 1, 2);
        let r = rotate90(&g);
        assert_eq!(r.states, 3);
        assert_eq!(r.get_state(2, 1), 2); // (1,1) → (4-1-1, 1) = (2,1)
    }
}
