use crate::{Pattern, PatternError, Rule, RuleParseError};
use std::error::Error;
use std::fmt;

/// Hard ceiling used when decoding untrusted RLE input.
pub const MAX_RLE_CELLS: usize = 16_000_000;
/// Maximum encoded input accepted by the strict parser.
pub const MAX_RLE_INPUT_BYTES: usize = 8 * 1024 * 1024;

/// Parse a Life 1.05-style run-length encoded pattern.
///
/// Comment lines beginning with `#` and arbitrary ASCII whitespace in the
/// body are accepted. The parser is deliberately strict about dimensions,
/// run lengths, the terminating `!`, and trailing data.
pub fn parse_rle(input: &str) -> Result<Pattern, RleError> {
    if input.len() > MAX_RLE_INPUT_BYTES {
        return Err(RleError::new(format!(
            "encoded input is {} bytes; limit is {MAX_RLE_INPUT_BYTES}",
            input.len()
        )));
    }
    let mut lines = input.lines();
    let header = loop {
        let line = lines
            .next()
            .ok_or_else(|| RleError::new("missing x/y header"))?
            .trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        break line;
    };

    let (width, height, rule) = parse_header(header)?;
    let cells = width
        .checked_mul(height)
        .ok_or_else(|| RleError::new("declared dimensions overflow address space"))?;
    if cells > MAX_RLE_CELLS {
        return Err(RleError::new(format!(
            "declared pattern has {cells} cells; limit is {MAX_RLE_CELLS}"
        )));
    }

    let mut pattern =
        Pattern::from_live_cells(width, height, std::iter::empty()).map_err(RleError::pattern)?;
    let mut body = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        body.extend(
            trimmed
                .chars()
                .filter(|character| !character.is_whitespace()),
        );
    }
    if body.is_empty() {
        return Err(RleError::new("missing pattern body and ! terminator"));
    }

    let mut characters = body.char_indices().peekable();
    let mut run: Option<usize> = None;
    let mut x = 0usize;
    let mut y = 0usize;
    let mut terminated = false;

    while let Some((position, character)) = characters.next() {
        if character.is_ascii_digit() {
            let digit = character.to_digit(10).expect("ASCII digit") as usize;
            run = Some(
                run.unwrap_or(0)
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(digit))
                    .ok_or_else(|| RleError::at(position, "run length overflows address space"))?,
            );
            continue;
        }

        match character.to_ascii_lowercase() {
            'b' | 'o' => {
                let count = take_run(&mut run, position)?;
                if y >= height {
                    return Err(RleError::at(position, "cell data exceeds declared height"));
                }
                let end = x
                    .checked_add(count)
                    .ok_or_else(|| RleError::at(position, "row run overflows address space"))?;
                if end > width {
                    return Err(RleError::at(
                        position,
                        format!("row reaches column {end}, beyond declared width {width}"),
                    ));
                }
                if character.eq_ignore_ascii_case(&'o') {
                    pattern.set_live_run(y, x, end);
                }
                x = end;
            }
            '$' => {
                let count = take_run(&mut run, position)?;
                y = y
                    .checked_add(count)
                    .ok_or_else(|| RleError::at(position, "row count overflows address space"))?;
                if y >= height {
                    return Err(RleError::at(
                        position,
                        format!("body advances past the final row of declared height {height}"),
                    ));
                }
                x = 0;
            }
            '!' => {
                if run.is_some() {
                    return Err(RleError::at(position, "a run length cannot prefix !"));
                }
                if let Some((trailing_position, _)) = characters.peek().copied() {
                    return Err(RleError::at(
                        trailing_position,
                        "trailing data after ! terminator",
                    ));
                }
                terminated = true;
                break;
            }
            other => {
                return Err(RleError::at(
                    position,
                    format!("unexpected token {other:?}; expected b, o, $, or !"),
                ));
            }
        }
    }

    if !terminated {
        return Err(RleError::new("missing ! terminator"));
    }

    Ok(match rule {
        Some(rule) => pattern.with_rule(rule),
        None => pattern,
    })
}

/// Export a pattern as canonical, wrapped RLE text.
#[must_use]
pub fn export_rle(pattern: &Pattern) -> String {
    let mut body = String::new();
    let last_live_row = pattern.live_cells().map(|(_, y)| y).max();
    if let Some(last_y) = last_live_row {
        for y in 0..=last_y {
            let last_x = (0..pattern.width())
                .rev()
                .find(|&x| pattern.get(x, y) == Some(true));
            if let Some(last_x) = last_x {
                let mut current = pattern.get(0, y).unwrap_or(false);
                let mut count = 0usize;
                for x in 0..=last_x {
                    let alive = pattern.get(x, y).unwrap_or(false);
                    if alive == current {
                        count += 1;
                    } else {
                        push_run(&mut body, count, current);
                        current = alive;
                        count = 1;
                    }
                }
                push_run(&mut body, count, current);
            }
            if y != last_y {
                body.push('$');
            }
        }
    }
    body.push('!');

    let rule = pattern
        .rule()
        .map(|rule| format!(", rule = {rule}"))
        .unwrap_or_default();
    let mut output = format!("x = {}, y = {}{rule}\n", pattern.width(), pattern.height());
    for chunk in body.as_bytes().chunks(70) {
        output.push_str(std::str::from_utf8(chunk).expect("RLE body is ASCII"));
        output.push('\n');
    }
    output
}

fn parse_header(header: &str) -> Result<(usize, usize, Option<Rule>), RleError> {
    let mut width = None;
    let mut height = None;
    let mut rule = None;
    for field in header.split(',') {
        let (raw_key, raw_value) = field
            .split_once('=')
            .ok_or_else(|| RleError::new(format!("invalid header field {field:?}")))?;
        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value.trim();
        match key.as_str() {
            "x" => {
                if width.is_some() {
                    return Err(RleError::new("duplicate x field in header"));
                }
                width = Some(parse_dimension(value, "x")?);
            }
            "y" => {
                if height.is_some() {
                    return Err(RleError::new("duplicate y field in header"));
                }
                height = Some(parse_dimension(value, "y")?);
            }
            "rule" => {
                if rule.is_some() {
                    return Err(RleError::new("duplicate rule field in header"));
                }
                rule = Some(value.parse().map_err(RleError::rule)?);
            }
            _ => return Err(RleError::new(format!("unknown header field {key:?}"))),
        }
    }
    Ok((
        width.ok_or_else(|| RleError::new("header is missing x"))?,
        height.ok_or_else(|| RleError::new("header is missing y"))?,
        rule,
    ))
}

fn parse_dimension(value: &str, name: &str) -> Result<usize, RleError> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| RleError::new(format!("{name} must be a positive integer")))?;
    if parsed == 0 {
        return Err(RleError::new(format!("{name} must be at least 1")));
    }
    Ok(parsed)
}

fn take_run(run: &mut Option<usize>, position: usize) -> Result<usize, RleError> {
    let count = run.take().unwrap_or(1);
    if count == 0 {
        return Err(RleError::at(position, "run lengths must be at least 1"));
    }
    Ok(count)
}

fn push_run(body: &mut String, count: usize, alive: bool) {
    if count > 1 {
        body.push_str(&count.to_string());
    }
    body.push(if alive { 'o' } else { 'b' });
}

/// Error returned for malformed or unsafe RLE input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RleError {
    message: String,
}

impl RleError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn at(position: usize, message: impl Into<String>) -> Self {
        Self::new(format!("body byte {}: {}", position + 1, message.into()))
    }

    fn pattern(error: PatternError) -> Self {
        Self::new(error.to_string())
    }

    fn rule(error: RuleParseError) -> Self {
        Self::new(error.to_string())
    }
}

impl fmt::Display for RleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid RLE: {}", self.message)
    }
}

impl Error for RleError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_whitespace_runs_and_rule() {
        let input = "#N Glider\r\nx = 3, y = 3, rule = B3/S23\r\nbob$2bo$3o!\r\n";
        let pattern = parse_rle(input).unwrap();
        assert_eq!((pattern.width(), pattern.height()), (3, 3));
        assert_eq!(pattern.population(), 5);
        assert_eq!(pattern.rule(), Some(Rule::CONWAY));
    }

    #[test]
    fn accepts_empty_pattern() {
        let pattern = parse_rle("x = 2, y = 2\n!").unwrap();
        assert_eq!(pattern.population(), 0);
    }

    #[test]
    fn parses_dense_runs_directly_into_pattern_storage() {
        let pattern = parse_rle("x = 100000, y = 1\n100000o!").unwrap();
        assert_eq!(pattern.population(), 100_000);
    }

    #[test]
    fn round_trip_preserves_shape_and_rule() {
        let original = parse_rle("x = 5, y = 4, rule = B36/S23\n2bo$3o2$bo!").unwrap();
        let decoded = parse_rle(&export_rle(&original)).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn rejects_malformed_or_out_of_bounds_bodies() {
        for bad in [
            "x = 3, y = 3\n3o",
            "x = 3, y = 3\n0o!",
            "x = 3, y = 3\n4o!",
            "x = 3, y = 3\n4$o!",
            "x = 3, y = 3\n3o!junk",
            "x = 3, y = 3\n999999999999999999999999o!",
        ] {
            assert!(parse_rle(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn rejects_bad_headers_and_large_allocations() {
        assert!(parse_rle("y = 3\n!").is_err());
        assert!(parse_rle("x = 3, y = 3, x = 2\n!").is_err());
        assert!(parse_rle("x = 5000, y = 5000\n!").is_err());
    }

    #[test]
    fn rejects_encoded_input_over_byte_limit() {
        let input = " ".repeat(MAX_RLE_INPUT_BYTES + 1);
        assert!(parse_rle(&input).is_err());
    }

    #[test]
    fn rejects_row_advance_past_declared_final_row() {
        assert!(parse_rle("x = 3, y = 3\n3o3$!").is_err());
    }
}
