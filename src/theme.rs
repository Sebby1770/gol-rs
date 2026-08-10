//! Named colour themes for terminal rendering and PPM export.

/// A named colour theme: solid live colour, header accents, and age ramps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    /// Classic 16-colour SGR code for solid live cells (e.g. 32 = green).
    pub live_sgr: u8,
    /// Header title SGR (bold-friendly 16-colour code).
    pub title_sgr: u8,
    /// Accent SGR for gen / extras.
    pub accent_sgr: u8,
    /// Pop / secondary accent.
    pub pop_sgr: u8,
    /// RGB for solid live cells (PPM / non-age).
    pub live_rgb: (u8, u8, u8),
    /// RGB for dead cells (PPM background).
    pub dead_rgb: (u8, u8, u8),
}

/// Ordered list of themes for cycling (`t` key / docs).
pub const THEME_NAMES: &[&str] = &["classic", "neon", "fire", "ocean", "mono"];

impl Theme {
    pub const CLASSIC: Theme = Theme {
        name: "classic",
        live_sgr: 32,   // green
        title_sgr: 36,  // cyan
        accent_sgr: 33, // yellow
        pop_sgr: 35,    // magenta
        live_rgb: (0, 220, 80),
        dead_rgb: (0, 0, 0),
    };

    pub const NEON: Theme = Theme {
        name: "neon",
        live_sgr: 96,  // bright cyan
        title_sgr: 95, // bright magenta
        accent_sgr: 93,
        pop_sgr: 96,
        live_rgb: (0, 255, 220),
        dead_rgb: (8, 0, 20),
    };

    pub const FIRE: Theme = Theme {
        name: "fire",
        live_sgr: 91, // bright red
        title_sgr: 91,
        accent_sgr: 93,
        pop_sgr: 33,
        live_rgb: (255, 90, 20),
        dead_rgb: (12, 4, 0),
    };

    pub const OCEAN: Theme = Theme {
        name: "ocean",
        live_sgr: 34, // blue
        title_sgr: 36,
        accent_sgr: 34,
        pop_sgr: 36,
        live_rgb: (40, 160, 255),
        dead_rgb: (0, 8, 24),
    };

    pub const MONO: Theme = Theme {
        name: "mono",
        live_sgr: 37, // white
        title_sgr: 37,
        accent_sgr: 90, // bright black / grey
        pop_sgr: 37,
        live_rgb: (230, 230, 230),
        dead_rgb: (0, 0, 0),
    };

    pub fn all() -> &'static [Theme] {
        &[
            Self::CLASSIC,
            Self::NEON,
            Self::FIRE,
            Self::OCEAN,
            Self::MONO,
        ]
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "classic" | "default" | "green" => Ok(Self::CLASSIC),
            "neon" | "cyber" => Ok(Self::NEON),
            "fire" | "hot" => Ok(Self::FIRE),
            "ocean" | "blue" | "sea" => Ok(Self::OCEAN),
            "mono" | "mono chrome" | "monochrome" | "bw" | "gray" | "grey" => Ok(Self::MONO),
            other => Err(format!(
                "unknown theme: {other} (try: {})",
                THEME_NAMES.join(", ")
            )),
        }
    }

    /// Next theme in the cycle (for interactive `t`).
    pub fn next(self) -> Self {
        let all = Self::all();
        let i = all.iter().position(|t| t.name == self.name).unwrap_or(0);
        all[(i + 1) % all.len()]
    }

    /// ANSI 256 colour index for a cell of the given age within this theme.
    pub fn age_color_256(self, age: u16) -> u8 {
        match self.name {
            "neon" => neon_age(age),
            "fire" => fire_age(age),
            "ocean" => ocean_age(age),
            "mono" => mono_age(age),
            _ => classic_age(age),
        }
    }

    /// RGB for a live cell of the given age (PPM heat-map).
    pub fn age_rgb(self, age: u16) -> (u8, u8, u8) {
        match self.name {
            "neon" => age_rgb_neon(age),
            "fire" => age_rgb_fire(age),
            "ocean" => age_rgb_ocean(age),
            "mono" => age_rgb_mono(age),
            _ => age_rgb_classic(age),
        }
    }

    /// SGR open sequence for solid live cells.
    pub fn live_ansi(self) -> String {
        format!("\x1b[{}m", self.live_sgr)
    }

    pub fn title_ansi(self) -> String {
        format!("\x1b[1;{}m", self.title_sgr)
    }

    pub fn accent_ansi(self) -> String {
        format!("\x1b[{}m", self.accent_sgr)
    }

    pub fn pop_ansi(self) -> String {
        format!("\x1b[{}m", self.pop_sgr)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::CLASSIC
    }
}

// --- age ramps (ANSI 256) ---------------------------------------------------

fn classic_age(age: u16) -> u8 {
    match age {
        0 => 0,
        1 => 46,
        2 => 82,
        3..=4 => 118,
        5..=7 => 154,
        8..=12 => 190,
        13..=20 => 226,
        21..=35 => 220,
        36..=55 => 214,
        56..=80 => 208,
        81..=120 => 202,
        121..=200 => 196,
        _ => 160,
    }
}

fn neon_age(age: u16) -> u8 {
    match age {
        0 => 0,
        1 => 51, // cyan
        2 => 45,
        3..=4 => 39,
        5..=7 => 33,
        8..=12 => 201, // magenta
        13..=20 => 200,
        21..=35 => 165,
        36..=55 => 129,
        56..=80 => 93,
        81..=120 => 57,
        _ => 21,
    }
}

fn fire_age(age: u16) -> u8 {
    match age {
        0 => 0,
        1 => 226, // yellow
        2 => 220,
        3..=4 => 214,
        5..=7 => 208,
        8..=12 => 202,
        13..=20 => 196,
        21..=35 => 160,
        36..=55 => 124,
        56..=80 => 88,
        _ => 52,
    }
}

fn ocean_age(age: u16) -> u8 {
    match age {
        0 => 0,
        1 => 159, // pale cyan
        2 => 123,
        3..=4 => 87,
        5..=7 => 51,
        8..=12 => 45,
        13..=20 => 39,
        21..=35 => 33,
        36..=55 => 27,
        56..=80 => 21,
        _ => 17,
    }
}

fn mono_age(age: u16) -> u8 {
    match age {
        0 => 0,
        1 => 255,
        2 => 252,
        3..=4 => 250,
        5..=7 => 248,
        8..=12 => 245,
        13..=20 => 242,
        21..=35 => 238,
        36..=55 => 234,
        _ => 232,
    }
}

// --- age ramps (RGB for PPM) ------------------------------------------------

fn age_rgb_classic(age: u16) -> (u8, u8, u8) {
    match age {
        0 => (0, 0, 0),
        1 => (0, 255, 80),
        2 => (80, 255, 40),
        3..=4 => (160, 255, 0),
        5..=7 => (220, 255, 0),
        8..=12 => (255, 240, 0),
        13..=20 => (255, 200, 0),
        21..=35 => (255, 140, 0),
        36..=55 => (255, 80, 0),
        56..=80 => (255, 40, 0),
        81..=120 => (220, 0, 0),
        _ => (160, 0, 0),
    }
}

fn age_rgb_neon(age: u16) -> (u8, u8, u8) {
    match age {
        0 => (0, 0, 0),
        1 => (0, 255, 255),
        2 => (0, 200, 255),
        3..=7 => (80, 120, 255),
        8..=20 => (255, 0, 255),
        21..=55 => (200, 0, 200),
        _ => (120, 0, 180),
    }
}

fn age_rgb_fire(age: u16) -> (u8, u8, u8) {
    match age {
        0 => (0, 0, 0),
        1 => (255, 255, 100),
        2 => (255, 200, 40),
        3..=7 => (255, 140, 0),
        8..=20 => (255, 60, 0),
        21..=55 => (200, 0, 0),
        _ => (100, 0, 0),
    }
}

fn age_rgb_ocean(age: u16) -> (u8, u8, u8) {
    match age {
        0 => (0, 0, 0),
        1 => (180, 240, 255),
        2 => (100, 200, 255),
        3..=7 => (40, 160, 255),
        8..=20 => (0, 100, 220),
        21..=55 => (0, 60, 160),
        _ => (0, 20, 80),
    }
}

fn age_rgb_mono(age: u16) -> (u8, u8, u8) {
    let v = match age {
        0 => 0u8,
        1 => 255,
        2 => 230,
        3..=7 => 200,
        8..=20 => 160,
        21..=55 => 120,
        _ => 80,
    };
    (v, v, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_themes() {
        assert_eq!(Theme::parse("classic").unwrap().name, "classic");
        assert_eq!(Theme::parse("NEON").unwrap().name, "neon");
        assert_eq!(Theme::parse("fire").unwrap().name, "fire");
        assert_eq!(Theme::parse("ocean").unwrap().name, "ocean");
        assert_eq!(Theme::parse("mono").unwrap().name, "mono");
        assert!(Theme::parse("rainbow").is_err());
    }

    #[test]
    fn cycle_all_themes() {
        let mut t = Theme::CLASSIC;
        let mut names = vec![t.name];
        for _ in 0..THEME_NAMES.len() {
            t = t.next();
            names.push(t.name);
        }
        // after full cycle back to classic
        assert_eq!(t.name, "classic");
        assert_eq!(names.len(), THEME_NAMES.len() + 1);
    }

    #[test]
    fn age_colors_differ_by_theme() {
        assert_ne!(
            Theme::CLASSIC.age_color_256(1),
            Theme::FIRE.age_color_256(1)
        );
        assert_ne!(Theme::CLASSIC.age_rgb(10), Theme::OCEAN.age_rgb(10));
    }
}
