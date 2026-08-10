//! Unicode block sparkline for population samples.

/// Block characters from empty to full (U+2581..U+2588).
const BLOCKS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Build a mini unicode sparkline of the last `width` samples (or fewer).
///
/// Empty samples → empty string. All-zero → all lowest bars. Values are
/// scaled linearly across the block set relative to min/max in the window.
pub fn sparkline(samples: &[usize], width: usize) -> String {
    if samples.is_empty() || width == 0 {
        return String::new();
    }
    let n = samples.len().min(width);
    let window = &samples[samples.len() - n..];
    let min = *window.iter().min().unwrap_or(&0);
    let max = *window.iter().max().unwrap_or(&0);
    let mut out = String::with_capacity(n * 3); // UTF-8 block chars
    for &v in window {
        let idx = if max == min {
            if max == 0 {
                0
            } else {
                BLOCKS.len() / 2
            }
        } else {
            let t = (v - min) as f64 / (max - min) as f64;
            let i = (t * (BLOCKS.len() - 1) as f64).round() as usize;
            i.min(BLOCKS.len() - 1)
        };
        out.push(BLOCKS[idx]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_zero_width() {
        assert_eq!(sparkline(&[], 40), "");
        assert_eq!(sparkline(&[1, 2, 3], 0), "");
    }

    #[test]
    fn all_zero() {
        let s = sparkline(&[0, 0, 0, 0], 4);
        assert_eq!(s.chars().count(), 4);
        assert!(s.chars().all(|c| c == '▁'));
    }

    #[test]
    fn flat_nonzero() {
        let s = sparkline(&[10, 10, 10], 3);
        assert_eq!(s.chars().count(), 3);
        // mid block for flat non-zero
        assert!(s.chars().all(|c| c == '▄' || c == '▅'));
    }

    #[test]
    fn rising_uses_higher_blocks() {
        let s = sparkline(&[0, 1, 2, 3, 4, 5, 6, 7], 8);
        assert_eq!(s.chars().count(), 8);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars[0], '▁');
        assert_eq!(chars[7], '█');
    }

    #[test]
    fn truncates_to_width() {
        let samples: Vec<usize> = (0..100).collect();
        let s = sparkline(&samples, 10);
        assert_eq!(s.chars().count(), 10);
        // last sample is 99 (max) → █
        assert_eq!(s.chars().last(), Some('█'));
    }

    #[test]
    fn single_sample() {
        let s = sparkline(&[42], 1);
        assert_eq!(s.chars().count(), 1);
    }
}
