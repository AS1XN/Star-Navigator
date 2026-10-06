//! Generates the static website: the page that hosts the wasm viewer plus
//! supporting pages. Usage: `site <out-dir>`.

use std::{env, fs, io, path::Path};

use catalog::Catalog;
use stucco::prelude::*;
use stucco::theme::{Fonts, Radius};
use stucco::{Delivery, Meta, Raw};

const MONO: &str = r#""IBM Plex Mono", ui-monospace, Consolas, "Courier New", monospace"#;

fn main() -> io::Result<()> {
    let out = env::args().nth(1).unwrap_or_else(|| "dist".into());
    let out = Path::new(&out);
    fs::create_dir_all(out)?;

    let bundle = Bundle::new(theme());
    fs::write(out.join("index.html"), index(&bundle))?;
    let catalog = load_catalog()?;
    fs::write(out.join("credits.html"), credits(&bundle, &catalog))?;
    // GitHub Pages runs Jekyll unless told otherwise.
    fs::write(out.join(".nojekyll"), "")?;

    println!("site written to {}", out.display());
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

fn index(bundle: &Bundle) -> String {
    page(bundle, "Star-Navigator")
        .meta(Meta::description(
            "Interactive map of the real night sky in an analog hologram style.",
        ))
        .head(Raw::trusted(VIEWER_CSS))
        .body(Raw::trusted(VIEWER_BODY))
        .render()
}

fn load_catalog() -> io::Result<Catalog> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/catalog/stars.bin");
    let bytes = fs::read(&path)?;
    Catalog::from_bytes(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn credits(bundle: &Bundle, catalog: &Catalog) -> String {
    let named = catalog.stars().iter().filter(|s| s.proper.is_some()).count();
    let summary = format!(
        "The map currently holds {} stars, {named} of them with proper names.",
        catalog.len()
    );

    page(bundle, "Credits - Star-Navigator")
        .main(
            Container::new().child(
                Stack::new()
                    .space(Space::S4)
                    .child(Heading::new(1, "Credits"))
                    .child(Heading::new(2, "Star data"))
                    .child(Text::new(
                        "HYG Database v4.1 by David Nash (astronexus), licensed CC BY-SA 4.0.                          Compiled from the Hipparcos, Yale Bright Star and Gliese catalogs.                          Star names follow the IAU Working Group on Star Names.",
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
                    .child(Link::new("Back to the map", "index.html")),
            ),
        )
        .render()
}

const VIEWER_CSS: &str = r#"<style>
  html, body { margin: 0; height: 100%; background: #000; overflow: hidden; }
  #viewer { display: block; width: 100vw; height: 100vh; outline: none; }
  #boot { position: fixed; inset: 0; display: grid; place-items: center;
          color: oklch(85% 0.06 225); letter-spacing: 0.2em; pointer-events: none; }
  .site-nav { position: fixed; top: 16px; right: 20px; font-size: 0.8rem; }
  .site-nav a { color: oklch(75% 0.06 225); }
</style>"#;

const VIEWER_BODY: &str = r#"<canvas id="viewer"></canvas>
<div id="boot">INITIALIZING STAR CHARTS...</div>
<nav class="site-nav"><a href="credits.html">CREDITS</a></nav>
<script type="module">
  import init from "./pkg/star-navigator.js";
  const boot = document.getElementById("boot");
  init().catch((e) => {
    // Bevy exits its init via an exception on the web; only real failures matter.
    if (!String(e).includes("Using exceptions for control flow")) {
      boot.textContent = "SIGNAL LOST: " + e;
      throw e;
    }
  });
  new MutationObserver(() => boot.remove()).observe(
    document.getElementById("viewer"), { attributes: true });
</script>"#;
