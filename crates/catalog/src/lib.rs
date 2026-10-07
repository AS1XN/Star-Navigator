//! Star catalog shared by the viewer and the site generator.
//!
//! Data comes from the HYG database v4.1 (CC BY-SA 4.0), converted to a compact
//! binary file by `cargo xtask data`.

use std::collections::HashMap;

mod format;
mod lines;
pub mod names;
pub mod physics;

pub use format::{DecodeError, decode, encode};
pub use lines::{Polyline, decode_lines, encode_lines};
pub use names::{CONSTELLATIONS, Constellation, normalize};

/// Light years per parsec.
pub const LY_PER_PC: f32 = 3.261_564;

#[derive(Debug, Clone, PartialEq)]
pub struct Star {
    /// HYG database id. 0 is the Sun.
    pub hyg: u32,
    pub hip: Option<u32>,
    pub hd: Option<u32>,
    pub hr: Option<u32>,
    pub gliese: Option<String>,
    pub proper: Option<String>,
    /// Bayer letter as abbreviated in HYG, e.g. "Alp" or "Alp-1".
    pub bayer: Option<String>,
    pub flamsteed: Option<u8>,
    /// Index into [`CONSTELLATIONS`].
    pub con: Option<u8>,
    /// Right ascension in hours.
    pub ra: f32,
    /// Declination in degrees.
    pub dec: f32,
    /// Distance from the Sun in parsecs, if a usable parallax exists.
    pub dist_pc: Option<f32>,
    /// Apparent visual magnitude.
    pub mag: f32,
    pub absmag: f32,
    /// B-V color index.
    pub ci: Option<f32>,
    pub spectral: String,
}

impl Star {
    pub fn is_sun(&self) -> bool {
        self.hyg == 0
    }

    pub fn dist_ly(&self) -> Option<f32> {
        self.dist_pc.map(|d| d * LY_PER_PC)
    }

    pub fn spectral_type(&self) -> Option<physics::Spectral> {
        physics::Spectral::parse(&self.spectral)
    }

    /// Estimated bolometric luminosity and radius in solar units. Needs a distance
    /// (for the absolute magnitude) and a parseable spectral type.
    pub fn luminosity_and_radius(&self) -> Option<(f32, f32)> {
        self.dist_pc?;
        let t = self.spectral_type()?.temperature_k();
        let l = physics::luminosity_solar(self.absmag, t);
        Some((l, physics::radius_solar(l, t)))
    }

    pub fn constellation(&self) -> Option<&'static Constellation> {
        self.con.map(|c| &CONSTELLATIONS[c as usize])
    }

    /// Unit vector on the celestial sphere, equatorial frame: +X toward RA 0h,
    /// +Z toward the north celestial pole.
    pub fn direction(&self) -> [f32; 3] {
        let ra = (self.ra * 15.0).to_radians();
        let dec = self.dec.to_radians();
        [dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin()]
    }

    /// Position relative to the Sun in parsecs (same frame as `direction`), if
    /// the distance is known.
    pub fn position(&self) -> Option<[f32; 3]> {
        let d = self.dist_pc?;
        Some(self.direction().map(|v| v * d))
    }

    /// "Alpha-1 Centauri" style designation, if the star has one.
    pub fn bayer_name(&self) -> Option<String> {
        let con = self.constellation()?;
        Some(format!("{} {}", names::greek_name(self.bayer.as_deref()?)?, con.genitive))
    }

    /// "61 Cygni" style designation, if the star has one.
    pub fn flamsteed_name(&self) -> Option<String> {
        Some(format!("{} {}", self.flamsteed?, self.constellation()?.genitive))
    }

    /// Best human-readable label: proper name, then Bayer, Flamsteed, Gliese,
    /// HIP, HD, and finally the HYG id.
    pub fn display_name(&self) -> String {
        self.proper
            .clone()
            .or_else(|| self.bayer_name())
            .or_else(|| self.flamsteed_name())
            .or_else(|| self.gliese.clone())
            .or_else(|| self.hip.map(|n| format!("HIP {n}")))
            .or_else(|| self.hd.map(|n| format!("HD {n}")))
            .unwrap_or_else(|| format!("HYG {}", self.hyg))
    }

    /// Every designation this star is known by, for display.
    pub fn designations(&self) -> Vec<String> {
        let mut out: Vec<String> = [
            self.proper.clone(),
            self.bayer_name(),
            self.flamsteed_name(),
            self.gliese.clone(),
            self.hip.map(|n| format!("HIP {n}")),
            self.hd.map(|n| format!("HD {n}")),
            self.hr.map(|n| format!("HR {n}")),
        ]
        .into_iter()
        .flatten()
        .collect();
        out.dedup();
        out
    }

    /// URL-friendly identifier, e.g. "rigil-kentaurus" or "hip-71683".
    pub fn slug(&self) -> String {
        self.display_name()
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub index: usize,
    /// The designation that matched, in display form.
    pub label: String,
    pub exact: bool,
}

pub struct Catalog {
    stars: Vec<Star>,
    /// Sorted (normalized key, star index) pairs for name/designation lookups.
    keys: Vec<(String, u32)>,
    hip: HashMap<u32, u32>,
    hd: HashMap<u32, u32>,
    hr: HashMap<u32, u32>,
}

impl Catalog {
    pub fn from_bytes(bytes: &[u8]) -> Result<Catalog, DecodeError> {
        Ok(Catalog::new(decode(bytes)?))
    }

    pub fn new(stars: Vec<Star>) -> Catalog {
        let mut keys = Vec::new();
        let (mut hip, mut hd, mut hr) = (HashMap::new(), HashMap::new(), HashMap::new());

        for (i, s) in stars.iter().enumerate() {
            let i = i as u32;
            let con = s.constellation().map(|c| c.abbr.to_lowercase());
            if let Some(p) = &s.proper {
                keys.push((normalize(p), i));
            }
            if let (Some(b), Some(con)) = (&s.bayer, &con) {
                let full = normalize(&format!("{b} {con}"));
                // "Alp-1 Cen" is also reachable as plain "alp cen".
                if let Some((letter, _)) = b.split_once('-') {
                    keys.push((normalize(&format!("{letter} {con}")), i));
                }
                keys.push((full, i));
            }
            if let (Some(f), Some(con)) = (s.flamsteed, &con) {
                keys.push((format!("{f} {con}"), i));
            }
            if let Some(g) = &s.gliese {
                keys.push((normalize(g), i));
            }
            if let Some(n) = s.hip {
                hip.insert(n, i);
            }
            if let Some(n) = s.hd {
                hd.insert(n, i);
            }
            if let Some(n) = s.hr {
                hr.insert(n, i);
            }
        }
        keys.sort();
        keys.dedup();

        Catalog { stars, keys, hip, hd, hr }
    }

    pub fn stars(&self) -> &[Star] {
        &self.stars
    }

    pub fn len(&self) -> usize {
        self.stars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stars.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Star> {
        self.stars.get(index)
    }

    /// The `n` stars closest to `index` in space (the Sun included), nearest first,
    /// with distances in parsecs. Empty if the star has no distance.
    pub fn nearest(&self, index: usize, n: usize) -> Vec<(usize, f32)> {
        let Some(origin) = self.stars.get(index).and_then(Star::position) else {
            return Vec::new();
        };
        let mut found: Vec<(usize, f32)> = Vec::with_capacity(n + 1);
        for (i, star) in self.stars.iter().enumerate() {
            if i == index {
                continue;
            }
            let Some(p) = star.position() else { continue };
            let d = ((p[0] - origin[0]).powi(2)
                + (p[1] - origin[1]).powi(2)
                + (p[2] - origin[2]).powi(2))
            .sqrt();
            if found.len() < n || d < found[found.len() - 1].1 {
                let at = found.partition_point(|f| f.1 < d);
                found.insert(at, (i, d));
                found.truncate(n);
            }
        }
        found
    }

    pub fn by_hip(&self, hip: u32) -> Option<&Star> {
        self.hip.get(&hip).map(|&i| &self.stars[i as usize])
    }

    /// Finds stars by proper name, Bayer or Flamsteed designation, or HIP/HD/HR/Gliese
    /// number. Exact matches come first, then prefix matches by brightness, then
    /// proper names containing the query.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let q = normalize(query);
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }

        if let Some(hit) = self.numbered(&q) {
            return vec![hit];
        }

        let start = self.keys.partition_point(|(k, _)| k.as_str() < q.as_str());
        let mut exact = Vec::new();
        let mut prefix = Vec::new();
        for (key, i) in &self.keys[start..] {
            if !key.starts_with(&q) {
                break;
            }
            if *key == q { &mut exact } else { &mut prefix }.push(*i as usize);
        }

        let by_mag = |a: &usize, b: &usize| self.stars[*a].mag.total_cmp(&self.stars[*b].mag);
        exact.sort_by(by_mag);
        prefix.sort_by(by_mag);

        let mut hits: Vec<Hit> = Vec::new();
        let push = |hits: &mut Vec<Hit>, index: usize, exact: bool| {
            if hits.len() < limit && !hits.iter().any(|h| h.index == index) {
                hits.push(Hit { index, label: self.label_for(index, &q), exact });
            }
        };
        for i in exact {
            push(&mut hits, i, true);
        }
        for i in prefix {
            push(&mut hits, i, false);
        }
        if hits.len() < limit && q.len() >= 3 {
            let mut inner: Vec<usize> = self
                .stars
                .iter()
                .enumerate()
                .filter(|(_, s)| s.proper.as_deref().is_some_and(|p| normalize(p).contains(&q)))
                .map(|(i, _)| i)
                .collect();
            inner.sort_by(by_mag);
            for i in inner {
                push(&mut hits, i, false);
            }
        }
        hits
    }

    fn numbered(&self, q: &str) -> Option<Hit> {
        let (prefix, num) = q.split_once(' ')?;
        let num: u32 = num.parse().ok()?;
        let (map, label) = match prefix {
            "hip" => (&self.hip, "HIP"),
            "hd" => (&self.hd, "HD"),
            "hr" => (&self.hr, "HR"),
            _ => return None,
        };
        let index = *map.get(&num)? as usize;
        Some(Hit { index, label: format!("{label} {num}"), exact: true })
    }

    /// Picks the designation the user was most likely typing.
    fn label_for(&self, index: usize, q: &str) -> String {
        let s = &self.stars[index];
        let starts =
            |name: &Option<String>| name.as_deref().is_some_and(|n| normalize(n).starts_with(q));
        if starts(&s.proper) {
            return s.proper.clone().unwrap();
        }
        if s.proper.is_none() {
            return s.display_name();
        }
        for name in [s.bayer_name(), s.flamsteed_name(), s.gliese.clone()] {
            if name.as_deref().is_some_and(|n| normalize(n).starts_with(q)) {
                return format!("{} ({})", name.unwrap(), s.proper.as_deref().unwrap());
            }
        }
        s.display_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn star(hyg: u32, proper: Option<&str>, bayer: Option<&str>, con: &str, mag: f32) -> Star {
        Star {
            hyg,
            hip: Some(hyg * 10),
            hd: None,
            hr: None,
            gliese: None,
            proper: proper.map(Into::into),
            bayer: bayer.map(Into::into),
            flamsteed: None,
            con: names::constellation_index(con),
            ra: 1.0,
            dec: 2.0,
            dist_pc: Some(10.0),
            mag,
            absmag: 1.0,
            ci: None,
            spectral: "G2V".into(),
        }
    }

    #[test]
    fn roundtrip() {
        let stars = vec![
            star(1, Some("Vega"), Some("Alp"), "Lyr", 0.03),
            Star { dist_pc: None, ci: Some(0.5), con: None, ..star(2, None, None, "Lyr", 9.0) },
        ];
        assert_eq!(decode(&encode(&stars)).unwrap(), stars);
        assert!(matches!(decode(b"nope"), Err(DecodeError::BadMagic)));
        assert!(matches!(decode(&encode(&stars)[..30]), Err(DecodeError::Truncated)));
    }

    #[test]
    fn display_names() {
        let s = star(1, None, Some("Alp-1"), "Cen", 0.0);
        assert_eq!(s.display_name(), "Alpha-1 Centauri");
        assert_eq!(s.slug(), "alpha-1-centauri");
        assert_eq!(star(3, None, None, "Xxx", 0.0).display_name(), "HIP 30");
    }

    #[test]
    fn search_orders_exact_then_brightness() {
        let cat = Catalog::new(vec![
            star(1, Some("Vegas"), None, "Lyr", 5.0),
            star(2, Some("Vega"), Some("Alp"), "Lyr", 0.03),
            star(3, Some("Vegaz"), None, "Lyr", 2.0),
        ]);
        let hits: Vec<_> = cat.search("vega", 10).into_iter().map(|h| h.index).collect();
        assert_eq!(hits, [1, 2, 0]);
        assert_eq!(cat.search("HIP 20", 5)[0].index, 1);
        assert_eq!(cat.search("alpha lyrae", 5)[0].label, "Alpha Lyrae (Vega)");
        assert!(cat.search("   ", 5).is_empty());
    }
}
