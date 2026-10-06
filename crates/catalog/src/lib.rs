//! Star catalog types shared by the viewer and the site generator.
//!
//! The real catalog (HYG v4) is wired in during phase 1. For now this holds the
//! core types and a handful of bright stars so the other crates have data to show.

/// Light years per parsec.
pub const LY_PER_PC: f64 = 3.261_563_777;

#[derive(Debug, Clone, PartialEq)]
pub struct Star {
    pub name: &'static str,
    pub hip: Option<u32>,
    /// Right ascension in hours.
    pub ra: f64,
    /// Declination in degrees.
    pub dec: f64,
    /// Distance from Sol in parsecs.
    pub dist_pc: f64,
    /// Apparent visual magnitude.
    pub mag: f32,
    pub spectral: &'static str,
}

impl Star {
    pub fn dist_ly(&self) -> f64 {
        self.dist_pc * LY_PER_PC
    }

    /// Unit vector on the celestial sphere (equatorial frame, +Z toward the north pole).
    pub fn direction(&self) -> [f64; 3] {
        let ra = (self.ra * 15.0).to_radians();
        let dec = self.dec.to_radians();
        [dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin()]
    }

    /// URL-friendly identifier, e.g. "alpha-centauri".
    pub fn slug(&self) -> String {
        self.name
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }
}

/// Placeholder sample until the HYG pipeline lands.
pub fn sample() -> &'static [Star] {
    const STARS: &[Star] = &[
        Star {
            name: "Sirius",
            hip: Some(32349),
            ra: 6.7525,
            dec: -16.7161,
            dist_pc: 2.64,
            mag: -1.46,
            spectral: "A1V",
        },
        Star {
            name: "Canopus",
            hip: Some(30438),
            ra: 6.3992,
            dec: -52.6957,
            dist_pc: 94.79,
            mag: -0.74,
            spectral: "A9II",
        },
        Star {
            name: "Alpha Centauri",
            hip: Some(71683),
            ra: 14.6601,
            dec: -60.8339,
            dist_pc: 1.34,
            mag: -0.27,
            spectral: "G2V",
        },
        Star {
            name: "Arcturus",
            hip: Some(69673),
            ra: 14.2610,
            dec: 19.1825,
            dist_pc: 11.26,
            mag: -0.05,
            spectral: "K1.5III",
        },
        Star {
            name: "Vega",
            hip: Some(91262),
            ra: 18.6156,
            dec: 38.7837,
            dist_pc: 7.68,
            mag: 0.03,
            spectral: "A0V",
        },
        Star {
            name: "Capella",
            hip: Some(24608),
            ra: 5.2782,
            dec: 45.9980,
            dist_pc: 13.12,
            mag: 0.08,
            spectral: "G8III",
        },
        Star {
            name: "Rigel",
            hip: Some(24436),
            ra: 5.2423,
            dec: -8.2016,
            dist_pc: 264.6,
            mag: 0.13,
            spectral: "B8Ia",
        },
        Star {
            name: "Procyon",
            hip: Some(37279),
            ra: 7.6550,
            dec: 5.2250,
            dist_pc: 3.51,
            mag: 0.37,
            spectral: "F5IV",
        },
        Star {
            name: "Betelgeuse",
            hip: Some(27989),
            ra: 5.9195,
            dec: 7.4071,
            dist_pc: 168.1,
            mag: 0.42,
            spectral: "M1Ia",
        },
        Star {
            name: "Altair",
            hip: Some(97649),
            ra: 19.8464,
            dec: 8.8683,
            dist_pc: 5.13,
            mag: 0.76,
            spectral: "A7V",
        },
        Star {
            name: "Aldebaran",
            hip: Some(21421),
            ra: 4.5987,
            dec: 16.5093,
            dist_pc: 20.43,
            mag: 0.86,
            spectral: "K5III",
        },
        Star {
            name: "Antares",
            hip: Some(80763),
            ra: 16.4901,
            dec: -26.4320,
            dist_pc: 170.0,
            mag: 0.91,
            spectral: "M1.5Iab",
        },
        Star {
            name: "Polaris",
            hip: Some(11767),
            ra: 2.5302,
            dec: 89.2641,
            dist_pc: 132.6,
            mag: 1.98,
            spectral: "F7Ib",
        },
        Star {
            name: "Deneb",
            hip: Some(102098),
            ra: 20.6905,
            dec: 45.2803,
            dist_pc: 802.0,
            mag: 1.25,
            spectral: "A2Ia",
        },
    ];
    STARS
}

pub fn find(query: &str) -> Option<&'static Star> {
    let q = query.trim();
    sample().iter().find(|s| s.name.eq_ignore_ascii_case(q))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_is_case_insensitive() {
        assert_eq!(find("  vega ").map(|s| s.hip), Some(Some(91262)));
        assert!(find("not a star").is_none());
    }

    #[test]
    fn polaris_points_north() {
        let [_, _, z] = find("Polaris").unwrap().direction();
        assert!(z > 0.999);
    }

    #[test]
    fn slugs() {
        assert_eq!(find("Alpha Centauri").unwrap().slug(), "alpha-centauri");
    }

    #[test]
    fn distance_conversion() {
        let ly = find("Sirius").unwrap().dist_ly();
        assert!((ly - 8.61).abs() < 0.01);
    }
}
