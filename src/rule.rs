use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// A Life-like cellular automaton rule, represented as birth and survival
/// bitsets for neighbour counts zero through eight.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rule {
    birth: u16,
    survival: u16,
}

impl Rule {
    /// Conway's original `B3/S23` rule.
    pub const CONWAY: Self = Self {
        birth: 1 << 3,
        survival: (1 << 2) | (1 << 3),
    };

    /// Construct a rule from neighbour counts.
    pub fn new(birth: &[u8], survival: &[u8]) -> Result<Self, RuleParseError> {
        Ok(Self {
            birth: counts_to_bits(birth)?,
            survival: counts_to_bits(survival)?,
        })
    }

    /// Whether a dead cell is born with `neighbours` live neighbours.
    #[must_use]
    pub const fn births(&self, neighbours: u8) -> bool {
        neighbours <= 8 && self.birth & (1 << neighbours) != 0
    }

    /// Whether a live cell survives with `neighbours` live neighbours.
    #[must_use]
    pub const fn survives(&self, neighbours: u8) -> bool {
        neighbours <= 8 && self.survival & (1 << neighbours) != 0
    }

    /// Compute the next state for one cell.
    #[must_use]
    pub const fn next_state(&self, alive: bool, neighbours: u8) -> bool {
        if alive {
            self.survives(neighbours)
        } else {
            self.births(neighbours)
        }
    }

    /// Birth neighbour counts in ascending order.
    #[must_use]
    pub fn birth_counts(&self) -> Vec<u8> {
        bits_to_counts(self.birth)
    }

    /// Survival neighbour counts in ascending order.
    #[must_use]
    pub fn survival_counts(&self) -> Vec<u8> {
        bits_to_counts(self.survival)
    }
}

impl Default for Rule {
    fn default() -> Self {
        Self::CONWAY
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("B")?;
        write_counts(f, self.birth)?;
        f.write_str("/S")?;
        write_counts(f, self.survival)
    }
}

impl FromStr for Rule {
    type Err = RuleParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let upper = input.trim().to_ascii_uppercase();
        let (left, right) = upper
            .split_once('/')
            .ok_or_else(|| RuleParseError::new("expected B<digits>/S<digits>"))?;

        let (birth, survival) =
            if let (Some(b), Some(s)) = (left.strip_prefix('B'), right.strip_prefix('S')) {
                (b, s)
            } else if left.bytes().all(|b| b.is_ascii_digit())
                && right.bytes().all(|b| b.is_ascii_digit())
            {
                // The older survival/birth notation, e.g. 23/3.
                (right, left)
            } else {
                return Err(RuleParseError::new(
                    "expected B<digits>/S<digits> (or legacy S/B notation)",
                ));
            };

        Ok(Self {
            birth: parse_counts(birth)?,
            survival: parse_counts(survival)?,
        })
    }
}

/// Error returned when parsing an invalid Life-like rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleParseError {
    message: String,
}

impl RuleParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RuleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid Life-like rule: {}", self.message)
    }
}

impl Error for RuleParseError {}

fn counts_to_bits(counts: &[u8]) -> Result<u16, RuleParseError> {
    let mut bits = 0;
    for &count in counts {
        if count > 8 {
            return Err(RuleParseError::new(format!(
                "neighbour count {count} is outside 0..=8"
            )));
        }
        let bit = 1 << count;
        if bits & bit != 0 {
            return Err(RuleParseError::new(format!(
                "duplicate neighbour count {count}"
            )));
        }
        bits |= bit;
    }
    Ok(bits)
}

fn parse_counts(input: &str) -> Result<u16, RuleParseError> {
    let counts: Result<Vec<u8>, _> = input
        .bytes()
        .map(|byte| {
            if (b'0'..=b'8').contains(&byte) {
                Ok(byte - b'0')
            } else {
                Err(RuleParseError::new(format!(
                    "invalid neighbour count {:?}",
                    char::from(byte)
                )))
            }
        })
        .collect();
    counts_to_bits(&counts?)
}

fn bits_to_counts(bits: u16) -> Vec<u8> {
    (0..=8).filter(|count| bits & (1 << count) != 0).collect()
}

fn write_counts(f: &mut fmt::Formatter<'_>, bits: u16) -> fmt::Result {
    for count in bits_to_counts(bits) {
        write!(f, "{count}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_rule_case_insensitively() {
        let rule: Rule = " b36/s23 ".parse().unwrap();
        assert!(rule.births(3));
        assert!(rule.births(6));
        assert!(rule.survives(2));
        assert_eq!(rule.to_string(), "B36/S23");
    }

    #[test]
    fn parses_legacy_survival_birth_notation() {
        assert_eq!("23/3".parse::<Rule>().unwrap(), Rule::CONWAY);
    }

    #[test]
    fn accepts_empty_birth_or_survival_sets() {
        assert_eq!("B/S".parse::<Rule>().unwrap().to_string(), "B/S");
    }

    #[test]
    fn rejects_duplicates_and_out_of_range_counts() {
        assert!("B33/S23".parse::<Rule>().is_err());
        assert!("B9/S23".parse::<Rule>().is_err());
        assert!(Rule::new(&[3], &[2, 9]).is_err());
    }

    #[test]
    fn computes_next_state() {
        assert!(Rule::CONWAY.next_state(false, 3));
        assert!(Rule::CONWAY.next_state(true, 2));
        assert!(!Rule::CONWAY.next_state(true, 4));
    }
}
