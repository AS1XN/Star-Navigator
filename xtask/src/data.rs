//! `cargo xtask data`: downloads the HYG database and writes the binary catalog.

use std::{collections::HashMap, fs, path::Path, process::Command};

use catalog::{Catalog, Polyline, Star, encode_lines, names::constellation_index};

use crate::{Result, root, run};

const HYG_URL: &str =
    "https://raw.githubusercontent.com/astronexus/HYG-Database/main/hyg/CURRENT/hygdata_v41.csv";
const HYG_FILE: &str = "hygdata_v41.csv";
const LINES_URL: &str =
    "https://raw.githubusercontent.com/ofrohn/d3-celestial/master/data/constellations.lines.json";
const LINES_FILE: &str = "constellations.lines.json";
/// HYG's placeholder distance for stars without a usable parallax.
const NO_DISTANCE_PC: f32 = 100_000.0;

pub fn data(force_download: bool) -> Result {
    let root = root();
    let raw = root.join("data/raw").join(HYG_FILE);
    if force_download || !raw.exists() {
        fs::create_dir_all(raw.parent().unwrap())?;
        run(Command::new("curl").args(["-fL", "--retry", "3", "-o"]).arg(&raw).arg(HYG_URL))?;
    }

    let mut stars = parse_hyg(&raw)?;
    // Brightest first, so renderers can take a prefix as a magnitude cut.
    stars.sort_by(|a, b| a.mag.total_cmp(&b.mag).then(a.hyg.cmp(&b.hyg)));

    let bytes = catalog::encode(&stars);
    let catalog = Catalog::from_bytes(&bytes)?;
    validate(&catalog)?;

    let out = root.join("assets/catalog/stars.bin");
    fs::create_dir_all(out.parent().unwrap())?;
    fs::write(&out, &bytes)?;

    let no_dist = stars.iter().filter(|s| s.dist_pc.is_none()).count();
    let named = stars.iter().filter(|s| s.proper.is_some()).count();
    eprintln!(
        "wrote {} ({} KB): {} stars, {named} named, {no_dist} without distance",
        out.strip_prefix(&root).unwrap_or(&out).display(),
        bytes.len() / 1024,
        stars.len(),
    );

    constellation_lines(&root, force_download, &catalog)
}

/// Converts d3-celestial's constellation figures (GeoJSON, RA as -180..180 degrees)
/// into `assets/catalog/constellations.bin`.
fn constellation_lines(root: &Path, force_download: bool, catalog: &Catalog) -> Result {
    let raw = root.join("data/raw").join(LINES_FILE);
    if force_download || !raw.exists() {
        run(Command::new("curl").args(["-fL", "--retry", "3", "-o"]).arg(&raw).arg(LINES_URL))?;
    }
    let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(&raw)?)?;
    let features = json["features"].as_array().ok_or("constellation lines: no features")?;

    let mut lines = Vec::new();
    for f in features {
        let id = f["id"].as_str().ok_or("constellation lines: feature without id")?;
        let con = constellation_index(id).ok_or_else(|| format!("unknown constellation {id}"))?;
        let strips = f["geometry"]["coordinates"].as_array().ok_or("bad geometry")?;
        for strip in strips {
            let points = strip
                .as_array()
                .ok_or("bad line")?
                .iter()
                .map(|p| {
                    let lon = p[0].as_f64().ok_or("bad point")?;
                    let dec = p[1].as_f64().ok_or("bad point")?;
                    Ok(((lon.rem_euclid(360.0) / 15.0) as f32, dec as f32))
                })
                .collect::<Result<Vec<_>>>()?;
            lines.push(Polyline { con, points });
        }
    }

    // Every figure vertex should sit on a catalogued star; this catches an RA/Dec
    // convention mix-up.
    let stars: Vec<[f32; 3]> =
        catalog.stars().iter().filter(|s| s.mag < 6.5).map(|s| s.direction()).collect();
    let miss = lines
        .iter()
        .flat_map(|l| &l.points)
        .filter(|&&(ra, dec)| {
            let probe = Star { ra, dec, ..catalog.stars()[0].clone() }.direction();
            !stars.iter().any(|d| d.iter().zip(probe).map(|(a, b)| a * b).sum::<f32>() > 0.99999)
        })
        .count();
    let total: usize = lines.iter().map(|l| l.points.len()).sum();
    if miss * 20 > total {
        return Err(format!("constellation lines: {miss}/{total} vertices match no star").into());
    }

    let out = root.join("assets/catalog/constellations.bin");
    let bytes = encode_lines(&lines);
    fs::write(&out, &bytes)?;
    eprintln!(
        "wrote {} ({} KB): {} polylines, {total} vertices, {miss} off-star",
        out.strip_prefix(root).unwrap_or(&out).display(),
        bytes.len() / 1024,
        lines.len()
    );
    Ok(())
}

fn parse_hyg(path: &Path) -> Result<Vec<Star>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let col: HashMap<&str, usize> = headers.iter().enumerate().map(|(i, h)| (h, i)).collect();

    let mut stars = Vec::new();
    let mut drift = 0;
    for (line, record) in reader.records().enumerate() {
        let record = record?;
        let get = |name: &str| record.get(col[name]).unwrap_or("").trim();
        let text = |name: &str| Some(get(name)).filter(|s| !s.is_empty()).map(str::to_owned);
        let num = |name: &str| get(name).parse::<u32>().ok().filter(|&n| n != 0);
        let float = |name: &str| -> Result<f32> {
            get(name).parse::<f32>().map_err(|e| format!("line {}: {name}: {e}", line + 2).into())
        };

        let dist = float("dist")?;
        let con = text("con");
        stars.push(Star {
            hyg: get("id").parse()?,
            hip: num("hip"),
            hd: num("hd"),
            hr: num("hr"),
            gliese: text("gl"),
            proper: text("proper"),
            bayer: text("bayer"),
            flamsteed: get("flam").parse().ok(),
            con: con.as_deref().and_then(constellation_index),
            ra: float("ra")?,
            dec: float("dec")?,
            dist_pc: (dist < NO_DISTANCE_PC).then_some(dist),
            mag: float("mag")?,
            absmag: float("absmag")?,
            ci: get("ci").parse().ok(),
            spectral: get("spect").to_owned(),
        });
        let star = stars.last().unwrap();
        if let Some(c) = con.filter(|_| star.con.is_none()) {
            return Err(format!("line {}: unknown constellation {c:?}", line + 2).into());
        }
        // Positions are derived from ra/dec/dist; make sure that agrees with HYG.
        // A handful of HYG rows disagree with themselves by ~0.1%, so only a
        // gross error (wrong frame, swapped columns) is fatal.
        if let Some(pos) = star.position() {
            let hyg = [float("x")?, float("y")?, float("z")?];
            let err = pos.iter().zip(hyg).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
            let rel = err / dist.max(1.0);
            if rel > 0.02 {
                return Err(format!("line {}: position {pos:?} vs HYG {hyg:?}", line + 2).into());
            }
            if rel > 1e-3 {
                drift += 1;
            }
        }
    }
    if drift > 0 {
        eprintln!("note: {drift} HYG rows have x/y/z off from ra/dec/dist by >0.1%");
    }
    Ok(stars)
}

/// Spot checks that catch a broken download or a column mix-up.
fn validate(catalog: &Catalog) -> Result {
    let expect = |query: &str, hip: u32| -> Result {
        let hit = catalog.search(query, 1).into_iter().next();
        let found = hit.and_then(|h| catalog.get(h.index)).and_then(|s| s.hip);
        if found != Some(hip) {
            return Err(format!("validation: {query:?} -> {found:?}, expected HIP {hip}").into());
        }
        Ok(())
    };
    expect("Sirius", 32349)?;
    expect("Vega", 91262)?;
    expect("Alpha Lyrae", 91262)?;
    expect("Rigil Kentaurus", 71683)?;
    expect("Polaris", 11767)?;
    if catalog.len() < 100_000 {
        return Err(format!("validation: only {} stars", catalog.len()).into());
    }
    Ok(())
}
