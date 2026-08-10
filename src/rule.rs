/// Life-like cellular automaton rule with birth/survive bitmasks.
///
/// Bit `k` (0..=8) set means neighbor count `k` triggers birth or survival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Bitmask of neighbor counts that birth a dead cell (bits 0–8).
    pub birth: u16,
    /// Bitmask of neighbor counts that keep a live cell alive (bits 0–8).
    pub survive: u16,
}

impl Rule {
    /// Conway's Game of Life: B3/S23.
    pub const CONWAY: Rule = Rule {
        birth: 1 << 3,
        survive: (1 << 2) | (1 << 3),
    };

    /// HighLife: B36/S23.
    pub const HIGHLIFE: Rule = Rule {
        birth: (1 << 3) | (1 << 6),
        survive: (1 << 2) | (1 << 3),
    };

    /// Seeds: B2/S (no survival).
    pub const SEEDS: Rule = Rule {
        birth: 1 << 2,
        survive: 0,
    };

    /// Day & Night: B3678/S34678.
    pub const DAYNIGHT: Rule = Rule {
        birth: (1 << 3) | (1 << 6) | (1 << 7) | (1 << 8),
        survive: (1 << 3) | (1 << 4) | (1 << 6) | (1 << 7) | (1 << 8),
    };

    /// Life without Death: B3/S012345678.
    pub const LIFE_WITHOUT_DEATH: Rule = Rule {
        birth: 1 << 3,
        survive: 0b1_1111_1111, // bits 0–8
    };

    /// Maze: B3/S12345.
    pub const MAZE: Rule = Rule {
        birth: 1 << 3,
        survive: (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 5),
    };

    /// Replicator: B1357/S1357.
    pub const REPLICATOR: Rule = Rule {
        birth: (1 << 1) | (1 << 3) | (1 << 5) | (1 << 7),
        survive: (1 << 1) | (1 << 3) | (1 << 5) | (1 << 7),
    };

    /// Build a rule from birth/survive neighbor lists.
    pub fn from_lists(birth: &[u8], survive: &[u8]) -> Result<Self, String> {
        Ok(Self {
            birth: counts_to_mask(birth)?,
            survive: counts_to_mask(survive)?,
        })
    }

    /// Whether a dead cell with `n` neighbors is born.
    #[inline]
    pub fn births(&self, n: u8) -> bool {
        n <= 8 && (self.birth & (1u16 << n)) != 0
    }

    /// Whether a live cell with `n` neighbors survives.
    #[inline]
    pub fn survives(&self, n: u8) -> bool {
        n <= 8 && (self.survive & (1u16 << n)) != 0
    }

    /// Next state for a cell given current alive flag and neighbor count.
    #[inline]
    pub fn next_alive(&self, alive: bool, n: u8) -> bool {
        if alive {
            self.survives(n)
        } else {
            self.births(n)
        }
    }

    /// Parse Life-like notation or a named preset.
    ///
    /// Accepts:
    /// - Named: `conway`, `highlife`, `seeds`, `daynight`, `life-without-death`/`lwd`,
    ///   `maze`, `replicator`
    /// - B/S form: `B3/S23`, `b36/s23`
    /// - S/B form: `23/3` (survive/birth, classic)
    pub fn parse(s: &str) -> Result<Self, String> {
        let t = s.trim().to_ascii_lowercase();
        if t.is_empty() {
            return Err("empty rule".into());
        }

        // Named presets
        match t.as_str() {
            "conway" | "life" | "b3/s23" => return Ok(Self::CONWAY),
            "highlife" | "b36/s23" => return Ok(Self::HIGHLIFE),
            "seeds" | "b2/s" | "b2/s0" => return Ok(Self::SEEDS),
            "daynight" | "day-night" | "day&night" | "b3678/s34678" => return Ok(Self::DAYNIGHT),
            "life-without-death" | "lwd" | "b3/s012345678" => return Ok(Self::LIFE_WITHOUT_DEATH),
            "maze" | "b3/s12345" => return Ok(Self::MAZE),
            "replicator" | "b1357/s1357" => return Ok(Self::REPLICATOR),
            _ => {}
        }

        // B#/S# form (case already lowercased)
        if let Some(rest) = t.strip_prefix('b') {
            return parse_bs_form(rest);
        }

        // S/B form: digits/digits  e.g. 23/3
        if let Some((s_part, b_part)) = t.split_once('/') {
            if s_part.chars().all(|c| c.is_ascii_digit())
                && b_part.chars().all(|c| c.is_ascii_digit())
            {
                let survive = parse_digit_counts(s_part)?;
                let birth = parse_digit_counts(b_part)?;
                return Self::from_lists(&birth, &survive);
            }
        }

        Err(format!(
            "unknown rule: {s} (try conway, highlife, seeds, daynight, lwd, maze, \
             replicator, or B3/S23 / 23/3)"
        ))
    }

    /// Canonical B#/S# string.
    pub fn to_string_bs(&self) -> String {
        format!(
            "B{}/S{}",
            mask_to_digits(self.birth),
            mask_to_digits(self.survive)
        )
    }

    /// Human-readable name if this is a known preset, else B/S string.
    pub fn display_name(&self) -> String {
        if *self == Self::CONWAY {
            "conway (B3/S23)".into()
        } else if *self == Self::HIGHLIFE {
            "highlife (B36/S23)".into()
        } else if *self == Self::SEEDS {
            "seeds (B2/S)".into()
        } else if *self == Self::DAYNIGHT {
            "daynight (B3678/S34678)".into()
        } else if *self == Self::LIFE_WITHOUT_DEATH {
            "life-without-death (B3/S012345678)".into()
        } else if *self == Self::MAZE {
            "maze (B3/S12345)".into()
        } else if *self == Self::REPLICATOR {
            "replicator (B1357/S1357)".into()
        } else {
            self.to_string_bs()
        }
    }
}

impl Default for Rule {
    fn default() -> Self {
        Self::CONWAY
    }
}

/// Top-level automaton selection: binary Life-like or multi-state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaRule {
    /// Binary birth/survive Life-like rule.
    LifeLike(Rule),
    /// Brian's Brain: 0=dead, 1=firing, 2=refractory.
    BriansBrain,
}

impl CaRule {
    /// Parse Life-like notation, named preset, or multi-state name.
    ///
    /// Multi-state: `brian`, `brains`, `brians-brain`, `bb`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let t = s.trim().to_ascii_lowercase();
        match t.as_str() {
            "brian" | "brains" | "brians-brain" | "brian's-brain" | "briansbrain" | "bb" => {
                Ok(Self::BriansBrain)
            }
            _ => Ok(Self::LifeLike(Rule::parse(s)?)),
        }
    }

    pub fn is_multistate(self) -> bool {
        matches!(self, Self::BriansBrain)
    }

    /// Max cell states used by this rule (2 binary, 3 Brian's Brain).
    pub fn states(self) -> u8 {
        match self {
            Self::LifeLike(_) => 2,
            Self::BriansBrain => 3,
        }
    }

    pub fn as_life_like(self) -> Option<Rule> {
        match self {
            Self::LifeLike(r) => Some(r),
            Self::BriansBrain => None,
        }
    }

    /// Canonical string for RLE `rule =` headers and status lines.
    pub fn to_string_bs(self) -> String {
        match self {
            Self::LifeLike(r) => r.to_string_bs(),
            Self::BriansBrain => "BB".into(),
        }
    }

    pub fn display_name(self) -> String {
        match self {
            Self::LifeLike(r) => r.display_name(),
            Self::BriansBrain => "brian (Brian's Brain)".into(),
        }
    }
}

impl Default for CaRule {
    fn default() -> Self {
        Self::LifeLike(Rule::CONWAY)
    }
}

/// Print built-in rule presets.
pub fn list_rules() {
    println!("Built-in rules (Life-like):");
    println!("  conway              — B3/S23          (Conway's Game of Life, default)");
    println!("  highlife            — B36/S23         (HighLife; has replicators)");
    println!("  seeds               — B2/S            (explosive, no survival)");
    println!("  daynight            — B3678/S34678    (Day & Night)");
    println!("  life-without-death  — B3/S012345678   (alias: lwd)");
    println!("  maze                — B3/S12345");
    println!("  replicator          — B1357/S1357");
    println!();
    println!("Multi-state:");
    println!("  brian / brains      — Brian's Brain (0=dead, 1=firing, 2=refractory)");
    println!();
    println!("Custom notation:");
    println!("  B3/S23              — birth/survive form");
    println!("  23/3                — survive/birth form (classic)");
}

fn counts_to_mask(counts: &[u8]) -> Result<u16, String> {
    let mut mask = 0u16;
    for &c in counts {
        if c > 8 {
            return Err(format!("neighbor count {c} out of range 0..=8"));
        }
        mask |= 1u16 << c;
    }
    Ok(mask)
}

fn parse_digit_counts(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            let d = (c as u8 - b'0') as u8;
            if d > 8 {
                return Err(format!("neighbor digit {c} out of range 0..=8"));
            }
            out.push(d);
        } else {
            return Err(format!("invalid character in rule counts: {c}"));
        }
    }
    Ok(out)
}

/// Parse after leading `b`: e.g. `3/s23` or `36/s23`.
fn parse_bs_form(rest: &str) -> Result<Rule, String> {
    let (b_part, s_part) = if let Some((b, s)) = rest.split_once('/') {
        let s = s
            .strip_prefix('s')
            .ok_or_else(|| format!("expected S after / in B…/S… rule, got: {rest}"))?;
        (b, s)
    } else {
        return Err(format!("expected B#/S# form, got: b{rest}"));
    };
    let birth = parse_digit_counts(b_part)?;
    let survive = parse_digit_counts(s_part)?;
    Rule::from_lists(&birth, &survive)
}

fn mask_to_digits(mask: u16) -> String {
    let mut s = String::new();
    for i in 0..=8u8 {
        if (mask & (1u16 << i)) != 0 {
            s.push(char::from(b'0' + i));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_conway_named() {
        assert_eq!(Rule::parse("conway").unwrap(), Rule::CONWAY);
        assert_eq!(Rule::parse("life").unwrap(), Rule::CONWAY);
        assert_eq!(Rule::parse("B3/S23").unwrap(), Rule::CONWAY);
        assert_eq!(Rule::parse("b3/s23").unwrap(), Rule::CONWAY);
        assert_eq!(Rule::parse("23/3").unwrap(), Rule::CONWAY);
    }

    #[test]
    fn parse_highlife() {
        let r = Rule::parse("highlife").unwrap();
        assert_eq!(r, Rule::HIGHLIFE);
        assert!(r.births(3));
        assert!(r.births(6));
        assert!(!r.births(2));
        assert!(r.survives(2));
        assert!(r.survives(3));
        assert!(!r.survives(4));
    }

    #[test]
    fn parse_seeds() {
        let r = Rule::parse("seeds").unwrap();
        assert!(r.births(2));
        assert!(!r.survives(2));
        assert!(!r.survives(3));
        assert_eq!(r.survive, 0);
    }

    #[test]
    fn parse_daynight_maze_replicator_lwd() {
        assert_eq!(Rule::parse("daynight").unwrap(), Rule::DAYNIGHT);
        assert_eq!(Rule::parse("maze").unwrap(), Rule::MAZE);
        assert_eq!(Rule::parse("replicator").unwrap(), Rule::REPLICATOR);
        assert_eq!(Rule::parse("lwd").unwrap(), Rule::LIFE_WITHOUT_DEATH);
        assert_eq!(
            Rule::parse("life-without-death").unwrap(),
            Rule::LIFE_WITHOUT_DEATH
        );
    }

    #[test]
    fn parse_sb_form() {
        // 23/3 = S23/B3
        let r = Rule::parse("23/3").unwrap();
        assert_eq!(r, Rule::CONWAY);
        // 23/36 = HighLife in S/B form
        let r = Rule::parse("23/36").unwrap();
        assert_eq!(r, Rule::HIGHLIFE);
    }

    #[test]
    fn parse_custom_bs() {
        let r = Rule::parse("B1357/S1357").unwrap();
        assert_eq!(r, Rule::REPLICATOR);
        assert_eq!(r.to_string_bs(), "B1357/S1357");
    }

    #[test]
    fn parse_empty_survive() {
        let r = Rule::parse("B2/S").unwrap();
        assert_eq!(r, Rule::SEEDS);
        assert_eq!(r.to_string_bs(), "B2/S");
    }

    #[test]
    fn next_alive_conway() {
        let r = Rule::CONWAY;
        assert!(!r.next_alive(false, 2));
        assert!(r.next_alive(false, 3));
        assert!(r.next_alive(true, 2));
        assert!(r.next_alive(true, 3));
        assert!(!r.next_alive(true, 1));
        assert!(!r.next_alive(true, 4));
    }

    #[test]
    fn highlife_births_on_6() {
        let r = Rule::HIGHLIFE;
        assert!(r.births(6));
        assert!(!Rule::CONWAY.births(6));
    }

    #[test]
    fn unknown_rule_errors() {
        assert!(Rule::parse("foobar").is_err());
        assert!(Rule::parse("").is_err());
    }

    #[test]
    fn default_is_conway() {
        assert_eq!(Rule::default(), Rule::CONWAY);
    }

    #[test]
    fn parse_brians_brain() {
        assert_eq!(CaRule::parse("brian").unwrap(), CaRule::BriansBrain);
        assert_eq!(CaRule::parse("brains").unwrap(), CaRule::BriansBrain);
        assert_eq!(CaRule::parse("BB").unwrap(), CaRule::BriansBrain);
        assert!(CaRule::parse("brian").unwrap().is_multistate());
        assert_eq!(CaRule::parse("brian").unwrap().states(), 3);
        assert_eq!(
            CaRule::parse("conway").unwrap(),
            CaRule::LifeLike(Rule::CONWAY)
        );
        assert!(!CaRule::parse("conway").unwrap().is_multistate());
    }
}
