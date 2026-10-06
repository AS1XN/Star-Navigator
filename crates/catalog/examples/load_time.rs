//! Times catalog decoding and index building: `cargo run --release -p catalog --example load_time`
use std::time::Instant;

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/catalog/stars.bin");
    let bytes = std::fs::read(path).unwrap();
    let t = Instant::now();
    let stars = catalog::decode(&bytes).unwrap();
    let decoded = t.elapsed();
    let t = Instant::now();
    let cat = catalog::Catalog::new(stars);
    println!("decode {decoded:?}, index {:?} ({} stars)", t.elapsed(), cat.len());
    let t = Instant::now();
    let hits = cat.search("kentaurus", 5).len();
    println!("substring search {:?} ({hits} hits)", t.elapsed());
}
