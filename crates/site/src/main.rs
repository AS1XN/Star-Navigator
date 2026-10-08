//! Generates the static website: the map page, one map page per named star (for
//! deep links like `star/vega/`), the star catalog by constellation, credits, and a
//! sitemap. Usage: `site <out-dir>`.

use std::collections::HashMap;
use std::{env, fs, io, path::Path};

use catalog::{CONSTELLATIONS, Catalog, LY_PER_PC, Star};
use stucco::prelude::*;
use stucco::theme::{Fonts, Radius};
use stucco::{Delivery, Meta, Raw};

const MONO: &str = r#""IBM Plex Mono", ui-monospace, Consolas, "Courier New", monospace"#;
const SITE_URL: &str = "https://as1xn.github.io/Star-Navigator/";
/// Faintest star listed on a constellation page (named stars are always listed).
const LIST_MAG: f32 = 5.5;

fn main() -> io::Result<()> {
    let out = env::args().nth(1).unwrap_or_else(|| "dist".into());
    // Folder holding the wasm build; xtask names it after the build's contents.
    let pkg = env::args().nth(2).unwrap_or_else(|| "pkg".into());
    let out = Path::new(&out);
    fs::create_dir_all(out)?;

    let bundle = Bundle::new(theme());
    let catalog = load_catalog()?;
    let mut urls = vec![String::new(), "catalog/".into(), "credits.html".into()];

    fs::write(
        out.join("index.html"),
        viewer_page(
            &bundle,
            &pkg,
            "Star-Navigator",
            "Interactive map of the real night sky and our stellar neighbourhood, \
             in an analog hologram style.",
            "",
            None,
        ),
    )?;

    let pages: HashMap<usize, String> = catalog.named_slugs().into_iter().collect();
    for (&index, slug) in &pages {
        let star = &catalog.stars()[index];
        let dir = out.join("star").join(slug);
        fs::create_dir_all(&dir)?;
        let title = format!("{} - Star-Navigator", star.display_name());
        fs::write(
            dir.join("index.html"),
            viewer_page(&bundle, &pkg, &title, &star_summary(star), "../../", Some(slug)),
        )?;
        urls.push(format!("star/{slug}/"));
    }

    let catalog_dir = out.join("catalog");
    fs::create_dir_all(&catalog_dir)?;
    fs::write(catalog_dir.join("index.html"), catalog_index(&bundle, &catalog))?;
    for (i, con) in CONSTELLATIONS.iter().enumerate() {
        let file = format!("{}.html", con.abbr.to_lowercase());
        fs::write(catalog_dir.join(&file), constellation_page(&bundle, &catalog, &pages, i as u8))?;
        urls.push(format!("catalog/{file}"));
    }

    fs::write(out.join("credits.html"), credits(&bundle, &catalog))?;
    fs::write(out.join("sitemap.xml"), sitemap(&urls))?;
    fs::write(
        out.join("robots.txt"),
        format!("User-agent: *\nAllow: /\nSitemap: {SITE_URL}sitemap.xml\n"),
    )?;
    // GitHub Pages runs Jekyll unless told otherwise.
    fs::write(out.join(".nojekyll"), "")?;

    println!("site written to {} ({} pages)", out.display(), urls.len());
    Ok(())
}

fn theme() -> Theme {
    Theme::from_seed(225.0)
        .neutral_hue(225.0)
        .neutral_tint(0.02)
        .radius(Radius::Sharp)
        .fonts(Fonts::system().sans(MONO).mono(MONO))
}

fn page<'b, 'a>(bundle: &'b Bundle, title: &str) -> Page<'b, 'a> {
    Page::new(bundle, title)
        .lang("en")
        .delivery(Delivery::Inline)
        .body_attrs(Attrs::default().attr("data-theme", "dark"))
}

fn load_catalog() -> io::Result<Catalog> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/catalog/stars.bin");
    let bytes = fs::read(&path)?;
    Catalog::from_bytes(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The map. `base` makes every relative URL (wasm, assets, links) resolve from the
/// site root, so star pages two levels deep load the same build.
fn viewer_page(
    bundle: &Bundle,
    pkg: &str,
    title: &str,
    description: &str,
    base: &str,
    focus: Option<&str>,
) -> String {
    let mut head = String::new();
    if !base.is_empty() {
        head += &format!(r#"<base href="{base}">"#);
    }
    head += VIEWER_CSS;
    let focus_attr =
        focus.map(|f| format!(r#" data-focus="{}""#, escape_attr(f))).unwrap_or_default();
    page(bundle, title)
        .meta(Meta::description(description))
        .head(Raw::trusted(head))
        .body(Raw::trusted(VIEWER_BODY.replace("{focus}", &focus_attr).replace("{pkg}", pkg)))
        .render()
}

fn star_summary(s: &Star) -> String {
    let kind = s
        .spectral_type()
        .map(|sp| sp.description().to_lowercase())
        .unwrap_or_else(|| "star".into());
    let place = s.constellation().map(|c| format!(" in {}", c.name)).unwrap_or_default();
    let dist =
        s.dist_ly().map(|ly| format!(", {ly:.1} light years from the Sun")).unwrap_or_default();
    format!(
        "{}: {kind}{place}{dist}. Locate it in 3D on an interactive map of the real night sky.",
        s.display_name()
    )
}

/// Site pages (not the map) share a header with links back to the map and catalog.
/// `root` is the relative path to the site root.
fn site_page(
    bundle: &Bundle,
    title: &str,
    description: &str,
    root: &str,
    body: impl Render,
) -> String {
    let nav = Cluster::new()
        .space(Space::S4)
        .child(Link::new("MAP", format!("{root}index.html")))
        .child(Link::new("CATALOG", format!("{root}catalog/index.html")))
        .child(Link::new("CREDITS", format!("{root}credits.html")));
    page(bundle, &format!("{title} - Star-Navigator"))
        .meta(Meta::description(description))
        .main(Container::new().child(Stack::new().space(Space::S4).child(nav).child(body)))
        .render()
}

fn ly_cell(s: &Star) -> String {
    s.dist_pc.map(|pc| format!("{:.1}", pc * LY_PER_PC)).unwrap_or_else(|| "--".into())
}

/// Where a star links to from the catalog: its own page if it has one, otherwise
/// the map with a query.
fn star_href(index: usize, s: &Star, pages: &HashMap<usize, String>, root: &str) -> String {
    match pages.get(&index) {
        Some(slug) => format!("{root}star/{slug}/"),
        None => format!("{root}index.html?star={}", s.slug()),
    }
}

fn catalog_index(bundle: &Bundle, catalog: &Catalog) -> String {
    let mut cons: Vec<(usize, &catalog::Constellation)> =
        CONSTELLATIONS.iter().enumerate().collect();
    cons.sort_by_key(|(_, c)| c.name);

    let mut table = Table::new("The 88 constellations").header(
        Row::new()
            .header("Constellation")
            .header("Abbr.")
            .header("Named stars")
            .header("Brightest"),
    );
    for (i, con) in cons {
        let members = catalog.stars().iter().filter(|s| s.con == Some(i as u8));
        let named = members.clone().filter(|s| s.proper.is_some()).count();
        // Stars are sorted brightest first, so the first member is the brightest.
        let brightest = members
            .clone()
            .next()
            .map(|s| format!("{} ({:+.2})", s.display_name(), s.mag))
            .unwrap_or_default();
        table = table.row(
            Row::new()
                .cell(Link::new(con.name, format!("{}.html", con.abbr.to_lowercase())))
                .cell(con.abbr)
                .cell(named.to_string())
                .cell(brightest),
        );
    }
    let body = Stack::new()
        .space(Space::S4)
        .child(Heading::new(1, "Star catalog"))
        .child(Text::new(format!(
            "{} stars from the HYG database, grouped by constellation. Each constellation page \
             lists its named stars and everything brighter than magnitude {LIST_MAG}.",
            catalog.len()
        )))
        .child(table);
    site_page(bundle, "Star catalog", "Browse the stars of all 88 constellations.", "../", body)
}

fn constellation_page(
    bundle: &Bundle,
    catalog: &Catalog,
    pages: &HashMap<usize, String>,
    con: u8,
) -> String {
    let c = &CONSTELLATIONS[con as usize];
    let stars: Vec<(usize, &Star)> = catalog
        .stars()
        .iter()
        .enumerate()
        .filter(|(_, s)| s.con == Some(con) && (s.proper.is_some() || s.mag <= LIST_MAG))
        .collect();

    let mut table = Table::new(format!("Stars of {}", c.name)).header(
        Row::new()
            .header("Star")
            .header("Designation")
            .header("Mag.")
            .header("Distance (ly)")
            .header("Class"),
    );
    for &(index, s) in &stars {
        let designation = s.bayer_name().or_else(|| s.flamsteed_name()).unwrap_or_default();
        let class = match s.spectral_type() {
            Some(sp) => format!("{} - {}", s.spectral, sp.description().to_lowercase()),
            None => s.spectral.clone(),
        };
        table = table.row(
            Row::new()
                .cell(Link::new(s.display_name(), star_href(index, s, pages, "../")))
                .cell(designation)
                .cell(format!("{:+.2}", s.mag))
                .cell(ly_cell(s))
                .cell(class),
        );
    }
    let body = Stack::new()
        .space(Space::S4)
        .child(Heading::new(1, format!("{} ({})", c.name, c.abbr)))
        .child(Text::new(format!(
            "{} stars listed, brightest first. Select a star to locate it on the map.",
            stars.len()
        )))
        .child(table)
        .child(Link::new("All constellations", "index.html"));
    site_page(
        bundle,
        c.name,
        &format!("Named and bright stars in the constellation {}.", c.name),
        "../",
        body,
    )
}

fn credits(bundle: &Bundle, catalog: &Catalog) -> String {
    let named = catalog.stars().iter().filter(|s| s.proper.is_some()).count();
    let summary = format!(
        "The map currently holds {} stars, {named} of them with proper names.",
        catalog.len()
    );
    let body = Stack::new()
        .space(Space::S4)
        .child(Heading::new(1, "Credits"))
        .child(Heading::new(2, "Star data"))
        .child(Text::new(
            "HYG Database v4.1 by David Nash (astronexus), licensed CC BY-SA 4.0. \
             Compiled from the Hipparcos, Yale Bright Star and Gliese catalogs. \
             Star names follow the IAU Working Group on Star Names.",
        ))
        .child(Link::new(
            "github.com/astronexus/HYG-Database",
            "https://github.com/astronexus/HYG-Database",
        ))
        .child(Text::new(summary))
        .child(Heading::new(2, "Constellation figures"))
        .child(Text::new(
            "Stick figures from d3-celestial by Olaf Frohn, BSD 3-Clause license. \
             Copyright (c) 2015, Olaf Frohn.",
        ))
        .child(Link::new(
            "github.com/ofrohn/d3-celestial",
            "https://github.com/ofrohn/d3-celestial",
        ))
        .child(Heading::new(2, "Online lookups"))
        .child(Text::new(
            "Stars not in the catalog are looked up live. This research has made use of \
             the SIMBAD database and the Sesame name resolver, operated at CDS, \
             Strasbourg, France.",
        ))
        .child(Link::new("simbad.cds.unistra.fr", "https://simbad.cds.unistra.fr/simbad/"))
        .child(Heading::new(2, "Sound effects"))
        .child(Text::new(
            "Trimmed from CC0 recordings on Freesound: \"Onboard Targeting Computer\" by \
             harrisonlace, \"Blip 7\" by deleted_user_2906614, \"Scanner Sci-Fi\" by \
             smokinghotdog and \"Sci-fi_short_error\" by melissapons. The projector hum is \
             synthesised in the viewer.",
        ))
        .child(Link::new("freesound.org", "https://freesound.org/"));
    site_page(bundle, "Credits", "Data sources and licenses for Star-Navigator.", "", body)
}

fn sitemap(urls: &[String]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for url in urls {
        s += &format!("  <url><loc>{SITE_URL}{url}</loc></url>\n");
    }
    s += "</urlset>\n";
    s
}

const VIEWER_CSS: &str = r#"<style>
  html, body { margin: 0; height: 100%; background: #000; overflow: hidden; }
  #viewer { display: block; width: 100vw; height: 100vh; outline: none; touch-action: none; }
  /* Touch screens get a real text field for FIND, since only a focused input can
     raise the on-screen keyboard. */
  /* Styled like the viewer's own touch buttons; --holo is set by the viewer to the
     current palette colour. */
  #find-btn { position: fixed; left: 12px; bottom: 36px; display: none;
              font: 13px/1.2 ui-monospace, Consolas, monospace; color: var(--holo, #b8dbff);
              background: rgba(0, 0, 0, 0.35); padding: 7px 10px; border-radius: 0;
              border: 1px solid color-mix(in srgb, var(--holo, #b8dbff) 50%, transparent); }
  #find-btn:active { background: color-mix(in srgb, var(--holo, #b8dbff) 30%, transparent); }
  @media (pointer: coarse) { #find-btn { display: block; } }
  #find { position: fixed; top: 12px; left: 50%; transform: translateX(-50%);
          width: min(90vw, 460px); box-sizing: border-box; display: none;
          font: 16px ui-monospace, Consolas, monospace; text-transform: uppercase;
          color: var(--holo, #b8dbff); background: rgba(0, 0, 0, 0.8);
          border: 1px solid color-mix(in srgb, var(--holo, #b8dbff) 60%, transparent);
          padding: 10px 12px; outline: none; border-radius: 0; }
  #find.open { display: block; }
  #boot { position: fixed; inset: 0; display: grid; place-items: center;
          color: oklch(85% 0.06 225); letter-spacing: 0.2em; pointer-events: none; }
  .site-nav { position: fixed; top: 16px; right: 20px; font-size: 0.8rem; }
  .site-nav a { color: oklch(75% 0.06 225); margin-left: 1.2em; }
</style>"#;

const VIEWER_BODY: &str = r#"<canvas id="viewer"{focus}></canvas>
<div id="boot">INITIALIZING STAR CHARTS...</div>
<nav class="site-nav"><a href="catalog/index.html">CATALOG</a><a href="credits.html">CREDITS</a></nav>
<button id="find-btn" type="button">FIND</button>
<input id="find" type="search" enterkeyhint="go" autocomplete="off" autocapitalize="off"
       spellcheck="false" placeholder="STAR NAME OR CATALOG NO." aria-label="Find a star">
<script>
  // The viewer reads data-query / data-submit on the canvas (see search.rs).
  const canvas = document.getElementById("viewer");
  const find = document.getElementById("find");
  document.getElementById("find-btn").addEventListener("click", () => {
    find.value = "";
    canvas.dataset.query = "";
    find.classList.add("open");
    find.focus();
  });
  find.addEventListener("input", () => { canvas.dataset.query = find.value; });
  find.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      canvas.dataset.submit = String(Date.now());
      delete canvas.dataset.query;
      find.classList.remove("open");
      find.blur();
    } else if (e.key === "Escape") {
      find.blur();
    }
  });
  // Leave the results up briefly so a tap on one still lands.
  find.addEventListener("blur", () => {
    find.classList.remove("open");
    setTimeout(() => { delete canvas.dataset.query; }, 400);
  });
</script>
<script>
  // Browsers keep audio suspended until the page is interacted with. Track the
  // audio contexts the viewer creates and wake them on the first tap or key.
  (() => {
    const contexts = [];
    for (const name of ["AudioContext", "webkitAudioContext"]) {
      const Base = window[name];
      if (!Base) continue;
      window[name] = class extends Base {
        constructor(...args) { super(...args); contexts.push(this); }
      };
    }
    const wake = () => contexts.forEach((c) => c.state === "suspended" && c.resume());
    for (const ev of ["pointerdown", "keydown", "touchend"]) {
      window.addEventListener(ev, wake, { capture: true });
    }
  })();
</script>
<script type="module">
  import init from "./{pkg}/star-navigator.js";
  const boot = document.getElementById("boot");
  // GitHub Pages serves .wasm uncompressed, so fetch the gzipped copy (about a
  // quarter of the size) and unpack it here. Falls back to the plain file.
  async function wasmBytes() {
    try {
      if (!("DecompressionStream" in window)) return undefined;
      const res = await fetch("./{pkg}/star-navigator_bg.wasm.gz");
      if (!res.ok) return undefined;
      const bytes = new Uint8Array(await res.arrayBuffer());
      // Already decoded by the server (Content-Encoding)? Then it's plain wasm.
      if (bytes[0] !== 0x1f || bytes[1] !== 0x8b) return bytes;
      const plain = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
      return new Uint8Array(await new Response(plain).arrayBuffer());
    } catch (e) {
      console.warn("compressed wasm unavailable, using the plain file", e);
      return undefined;
    }
  }
  init({ module_or_path: await wasmBytes() }).catch((e) => {
    // Bevy exits its init via an exception on the web; only real failures matter.
    if (!String(e).includes("Using exceptions for control flow")) {
      boot.textContent = "SIGNAL LOST: " + e;
      throw e;
    }
  });
  new MutationObserver(() => boot.remove()).observe(
    document.getElementById("viewer"), { attributes: true });
</script>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_page_paths() {
        let bundle = Bundle::new(theme());
        let root = viewer_page(&bundle, "pkg-1234abcd", "T", "D", "", None);
        assert!(!root.contains("<base"));
        assert!(root.contains(r#"<canvas id="viewer">"#));
        assert!(root.contains(r#"import init from "./pkg-1234abcd/star-navigator.js""#));

        let star = viewer_page(&bundle, "pkg-1234abcd", "Vega", "D", "../../", Some("vega"));
        assert!(star.contains(r#"<base href="../../">"#));
        assert!(star.contains(r#"<canvas id="viewer" data-focus="vega">"#));
    }

    #[test]
    fn summaries_and_links() {
        let catalog = load_catalog().unwrap();
        let pages: HashMap<usize, String> = catalog.named_slugs().into_iter().collect();
        let (index, vega) = catalog
            .stars()
            .iter()
            .enumerate()
            .find(|(_, s)| s.proper.as_deref() == Some("Vega"))
            .unwrap();
        let text = star_summary(vega);
        assert!(
            text.starts_with("Vega: white main-sequence star in Lyra, 25.0 light years"),
            "{text}"
        );
        assert_eq!(star_href(index, vega, &pages, "../"), "../star/vega/");

        let unnamed = catalog.stars().iter().position(|s| s.proper.is_none()).unwrap();
        let href = star_href(unnamed, &catalog.stars()[unnamed], &pages, "../");
        assert!(href.starts_with("../index.html?star="), "{href}");
    }
}
