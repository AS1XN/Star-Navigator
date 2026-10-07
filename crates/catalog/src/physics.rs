//! Rough physical properties from a star's spectral type and absolute magnitude.
//!
//! These are estimates for display: temperature from a spectral-class table,
//! bolometric luminosity via a temperature-based bolometric correction, and radius
//! from Stefan-Boltzmann. Good to tens of percent for ordinary stars.

const SUN_TEMP_K: f32 = 5772.0;
const SUN_MBOL: f32 = 4.74;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LumClass {
    Supergiant,
    BrightGiant,
    Giant,
    Subgiant,
    Dwarf,
    Subdwarf,
    WhiteDwarf,
}

impl LumClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::Supergiant => "SUPERGIANT",
            Self::BrightGiant => "BRIGHT GIANT",
            Self::Giant => "GIANT",
            Self::Subgiant => "SUBGIANT",
            Self::Dwarf => "DWARF",
            Self::Subdwarf => "SUBDWARF",
            Self::WhiteDwarf => "WHITE DWARF",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spectral {
    /// Harvard class letter: O B A F G K M, or W C S L T Y, or D for white dwarfs.
    pub class: char,
    pub subclass: Option<f32>,
    pub lum: Option<LumClass>,
}

impl Spectral {
    /// Parses HYG-style spectral strings such as "G2V", "K1.5III", "M1Ia",
    /// "F7:Ib-IIv SB", "DA", "sdM4" or old-style "dM5e".
    pub fn parse(text: &str) -> Option<Spectral> {
        let s = text.trim();
        let (prefix_lum, s) = if let Some(rest) = s.strip_prefix("sd") {
            (Some(LumClass::Subdwarf), rest)
        } else if let Some(rest) = s.strip_prefix('d').filter(|r| starts_with_class(r)) {
            (Some(LumClass::Dwarf), rest)
        } else if let Some(rest) = s.strip_prefix('g').filter(|r| starts_with_class(r)) {
            (Some(LumClass::Giant), rest)
        } else if let Some(rest) = s.strip_prefix('c').filter(|r| starts_with_class(r)) {
            (Some(LumClass::Supergiant), rest)
        } else {
            (None, s)
        };

        let mut chars = s.chars();
        let class = chars.next()?;
        if !"OBAFGKMWCSLTYD".contains(class) {
            return None;
        }
        let mut rest = chars.as_str();
        if class == 'D' {
            // White dwarfs: "DA2", "DQ6"... the second letter is the subtype.
            rest = rest.trim_start_matches(|c: char| c.is_ascii_uppercase());
        }

        let digits = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
        let subclass = rest[..digits].parse::<f32>().ok().filter(|v| (0.0..10.0).contains(v));
        let rest = rest[digits..].trim_start_matches([':', ' ', '/']);

        let lum = if class == 'D' {
            Some(LumClass::WhiteDwarf)
        } else {
            lum_from_roman(rest).or(prefix_lum)
        };
        Some(Spectral { class, subclass, lum })
    }

    pub fn temperature_k(&self) -> f32 {
        // (temperature at subclass 0, at subclass 10)
        let (hot, cool) = match self.class {
            'O' => (50_000.0, 31_000.0),
            'B' => (31_000.0, 9_800.0),
            'A' => (9_800.0, 7_300.0),
            'F' => (7_300.0, 6_000.0),
            'G' => (6_000.0, 5_300.0),
            'K' => (5_300.0, 3_900.0),
            'M' => (3_900.0, 2_300.0),
            'W' => return 70_000.0,
            'C' | 'S' => return 3_000.0,
            'L' => return 1_800.0,
            'T' => return 1_000.0,
            'Y' => return 500.0,
            // White dwarf subclass is 50400 / T.
            'D' => return self.subclass.filter(|s| *s > 0.5).map_or(10_000.0, |s| 50_400.0 / s),
            _ => return SUN_TEMP_K,
        };
        let t = self.subclass.unwrap_or(5.0) / 10.0;
        hot + (cool - hot) * t
    }

    pub fn color_word(&self) -> &'static str {
        match self.class {
            'O' | 'W' => "BLUE",
            'B' => "BLUE-WHITE",
            'A' | 'D' => "WHITE",
            'F' => "YELLOW-WHITE",
            'G' => "YELLOW",
            'K' => "ORANGE",
            'M' | 'C' | 'S' => "RED",
            _ => "BROWN",
        }
    }

    /// e.g. "YELLOW DWARF", "RED SUPERGIANT", "WHITE DWARF", "ORANGE STAR".
    pub fn description(&self) -> String {
        match self.lum {
            Some(LumClass::WhiteDwarf) => "WHITE DWARF".into(),
            Some(lum) => format!("{} {}", self.color_word(), lum.label()),
            None if matches!(self.class, 'L' | 'T' | 'Y') => "BROWN DWARF".into(),
            None => format!("{} STAR", self.color_word()),
        }
    }
}

fn starts_with_class(s: &str) -> bool {
    s.starts_with(|c: char| "OBAFGKM".contains(c))
}

fn lum_from_roman(s: &str) -> Option<LumClass> {
    // Longest first so "III" isn't read as "I".
    const TABLE: [(&str, LumClass); 10] = [
        ("Iab", LumClass::Supergiant),
        ("Ia0", LumClass::Supergiant),
        ("Ia", LumClass::Supergiant),
        ("Ib", LumClass::Supergiant),
        ("III", LumClass::Giant),
        ("II", LumClass::BrightGiant),
        ("IV", LumClass::Subgiant),
        ("VI", LumClass::Subdwarf),
        ("V", LumClass::Dwarf),
        ("I", LumClass::Supergiant),
    ];
    TABLE.iter().find(|(p, _)| s.starts_with(p)).map(|(_, l)| *l)
}

/// Visual-to-bolometric correction for a given effective temperature, interpolated
/// in log T from a coarse table.
pub fn bolometric_correction(temp_k: f32) -> f32 {
    const TABLE: [(f32, f32); 11] = [
        (2_500.0, -4.5),
        (3_000.0, -3.0),
        (3_500.0, -1.6),
        (4_000.0, -0.8),
        (5_000.0, -0.25),
        (6_000.0, -0.05),
        (7_500.0, 0.0),
        (10_000.0, -0.4),
        (20_000.0, -2.0),
        (30_000.0, -3.0),
        (40_000.0, -3.8),
    ];
    let t = temp_k.clamp(TABLE[0].0, TABLE[TABLE.len() - 1].0);
    let i = TABLE.iter().rposition(|(tt, _)| *tt <= t).unwrap().min(TABLE.len() - 2);
    let ((t0, b0), (t1, b1)) = (TABLE[i], TABLE[i + 1]);
    let f = (t.ln() - t0.ln()) / (t1.ln() - t0.ln());
    b0 + (b1 - b0) * f
}

/// Bolometric luminosity in solar units.
pub fn luminosity_solar(absmag: f32, temp_k: f32) -> f32 {
    let mbol = absmag + bolometric_correction(temp_k);
    10f32.powf((SUN_MBOL - mbol) / 2.5)
}

/// Radius in solar units from luminosity and temperature (Stefan-Boltzmann).
pub fn radius_solar(luminosity: f32, temp_k: f32) -> f32 {
    luminosity.sqrt() * (SUN_TEMP_K / temp_k).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Spectral {
        Spectral::parse(s).unwrap_or_else(|| panic!("{s} did not parse"))
    }

    #[test]
    fn parses_common_forms() {
        assert_eq!(
            parse("G2V"),
            Spectral { class: 'G', subclass: Some(2.0), lum: Some(LumClass::Dwarf) }
        );
        assert_eq!(parse("K1.5III").lum, Some(LumClass::Giant));
        assert_eq!(parse("K1.5III").subclass, Some(1.5));
        assert_eq!(parse("M1Ia").lum, Some(LumClass::Supergiant));
        assert_eq!(parse("M1.5Iab").lum, Some(LumClass::Supergiant));
        assert_eq!(parse("F7:Ib-IIv SB").lum, Some(LumClass::Supergiant));
        assert_eq!(parse("F5IV").lum, Some(LumClass::Subgiant));
        assert_eq!(parse("A0m...").lum, None);
        assert_eq!(parse("DA2").lum, Some(LumClass::WhiteDwarf));
        assert_eq!(parse("DA2").subclass, Some(2.0));
        assert_eq!(parse("sdM4").lum, Some(LumClass::Subdwarf));
        assert_eq!(parse("dM5e").class, 'M');
        assert_eq!(parse("dM5e").lum, Some(LumClass::Dwarf));
        assert!(Spectral::parse("").is_none());
        assert!(Spectral::parse("?").is_none());
    }

    #[test]
    fn descriptions() {
        assert_eq!(parse("G2V").description(), "YELLOW DWARF");
        assert_eq!(parse("M1Ia").description(), "RED SUPERGIANT");
        assert_eq!(parse("K5III").description(), "ORANGE GIANT");
        assert_eq!(parse("DA").description(), "WHITE DWARF");
        assert_eq!(parse("A0").description(), "WHITE STAR");
    }

    #[test]
    fn sun_comes_out_solar() {
        let g2v = parse("G2V");
        let t = g2v.temperature_k();
        assert!((t - 5860.0).abs() < 50.0);
        let l = luminosity_solar(4.83, t);
        assert!((l - 1.0).abs() < 0.1, "L = {l}");
        let r = radius_solar(l, t);
        assert!((r - 1.0).abs() < 0.1, "R = {r}");
    }

    #[test]
    fn sirius_and_betelgeuse_are_plausible() {
        let sirius = parse("A1V");
        let r =
            radius_solar(luminosity_solar(1.45, sirius.temperature_k()), sirius.temperature_k());
        assert!((1.4..2.5).contains(&r), "Sirius R = {r}");

        let betelgeuse = parse("M1Ia");
        let t = betelgeuse.temperature_k();
        let r = radius_solar(luminosity_solar(-5.85, t), t);
        assert!((300.0..1200.0).contains(&r), "Betelgeuse R = {r}");
    }

    #[test]
    fn bolometric_correction_is_continuous() {
        let mut prev = bolometric_correction(2_000.0);
        let mut t = 2_000.0;
        while t < 50_000.0 {
            t *= 1.05;
            let bc = bolometric_correction(t);
            assert!((bc - prev).abs() < 0.5, "jump at {t}: {prev} -> {bc}");
            prev = bc;
        }
    }
}
