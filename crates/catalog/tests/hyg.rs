//! Checks against the generated HYG catalog in assets/catalog.

use std::sync::LazyLock;

use catalog::{Catalog, Star};

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/catalog/stars.bin");
    let bytes = std::fs::read(path).expect("run `cargo xtask data` first");
    Catalog::from_bytes(&bytes).unwrap()
});

fn top(query: &str) -> &'static Star {
    let hit = CATALOG.search(query, 1).into_iter().next();
    let hit = hit.unwrap_or_else(|| panic!("no result for {query:?}"));
    CATALOG.get(hit.index).unwrap()
}

#[test]
fn size_and_order() {
    assert!(CATALOG.len() > 119_000);
    let stars = CATALOG.stars();
    assert!(stars[0].is_sun());
    assert!(stars.windows(2).all(|w| w[0].mag <= w[1].mag));
}

#[test]
fn vega_by_every_designation() {
    for q in [
        "Vega",
        "vega",
        "Alpha Lyrae",
        "alp lyr",
        "α Lyr",
        "3 Lyrae",
        "HIP 91262",
        "HD 172167",
        "HR 7001",
        "Gl 721",
        "GJ 721",
    ] {
        assert_eq!(top(q).proper.as_deref(), Some("Vega"), "{q}");
    }
}

#[test]
fn sirius_facts() {
    let s = top("Sirius");
    assert_eq!(s.hip, Some(32349));
    assert_eq!(s.constellation().unwrap().abbr, "CMa");
    assert!((s.dist_ly().unwrap() - 8.6).abs() < 0.1);
    assert_eq!(s.bayer_name().as_deref(), Some("Alpha Canis Majoris"));
}

#[test]
fn alpha_centauri_system() {
    let names: Vec<_> = CATALOG
        .search("Alpha Centauri", 5)
        .iter()
        .map(|h| CATALOG.get(h.index).unwrap().proper.clone().unwrap_or_default())
        .collect();
    assert!(names.contains(&"Rigil Kentaurus".to_string()), "{names:?}");
    assert!(names.contains(&"Toliman".to_string()), "{names:?}");
    assert_eq!(top("alpha-2 centauri").proper.as_deref(), Some("Toliman"));
    assert!(top("Proxima").dist_ly().unwrap() < 4.3);
}

#[test]
fn prefixes_and_partial_names() {
    assert_eq!(top("betel").proper.as_deref(), Some("Betelgeuse"));
    assert_eq!(top("kentaurus").proper.as_deref(), Some("Rigil Kentaurus"));
    let hits = CATALOG.search("al", 10);
    assert_eq!(hits.len(), 10);
    assert!(hits.iter().all(|h| !h.label.is_empty()));
}

#[test]
fn flamsteed_only_star() {
    let s = top("61 Cygni");
    assert_eq!(s.constellation().unwrap().abbr, "Cyg");
    assert!(s.dist_ly().unwrap() < 12.0);
}

#[test]
fn polaris_points_north() {
    let [_, _, z] = top("Polaris").direction();
    assert!(z > 0.999);
}

#[test]
fn unknown_queries() {
    assert!(CATALOG.search("zzzz not a star", 5).is_empty());
    assert!(CATALOG.search("HIP 999999999", 5).is_empty());
}

#[test]
fn alpha_centauri_neighbours() {
    let index = CATALOG.search("Rigil Kentaurus", 1)[0].index;
    let near: Vec<_> = CATALOG
        .nearest(index, 3)
        .into_iter()
        .map(|(i, d)| (CATALOG.get(i).unwrap().display_name(), d))
        .collect();
    let names: Vec<&str> = near.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names[0], "Toliman", "{near:?}");
    assert_eq!(names[1], "Proxima Centauri", "{near:?}");
    assert_eq!(names[2], "Sol", "{near:?}");
    assert!((near[2].1 - 1.32).abs() < 0.02);
    assert!(near.windows(2).all(|w| w[0].1 <= w[1].1));
}

#[test]
fn nearest_without_distance_is_empty() {
    let index = CATALOG.stars().iter().position(|s| s.dist_pc.is_none()).unwrap();
    assert!(CATALOG.nearest(index, 5).is_empty());
}

#[test]
fn physical_estimates() {
    let (l, r) = top("Sirius").luminosity_and_radius().unwrap();
    assert!((15.0..40.0).contains(&l), "Sirius L = {l}");
    assert!((1.4..2.5).contains(&r), "Sirius R = {r}");
    let (_, r) = top("Betelgeuse").luminosity_and_radius().unwrap();
    assert!(r > 200.0, "Betelgeuse R = {r}");
    assert_eq!(top("Betelgeuse").spectral_type().unwrap().description(), "RED SUPERGIANT");
}
