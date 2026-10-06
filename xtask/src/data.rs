//! `cargo xtask data`: downloads the HYG database and writes the binary catalog.

use std::{collections::HashMap, fs, path::Path, process::Command};

use catalog::{Catalog, Star, names::constellation_index};

use crate::{Result, root, run};

const HYG_URL: &str =
    "https://raw.githubusercontent.com/astronexus/HYG-Database/main/hyg/CURRENT/hygdata_v41.csv";
const HYG_FILE: &str = "hygdata_v41.csv";
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
