# Star-Navigator: Design & Roadmap

An interactive map of the real night sky and our stellar neighbourhood, presented
as an analog hologram display from late-70s / early-2000s film sci-fi. Search
for any catalogued star, then watch the map pull back, rotate and zoom in on it,
and bring up its readout.

This is a living document. The look will be adjusted as we go.

---

## 1. Goals

- **Real data.** Every point is a real star at its real position, and at its
  real distance where a parallax exists.
- **Find anything.** Search by proper name (Vega), Bayer/Flamsteed designation
  (Alpha Lyrae, 61 Cyg), or catalog number (HIP 91262, HD 172167, GJ 699).
  Stars that aren't in the bundled catalog are looked up online.
- **The look.** Monochrome dotted holograms, scanlines, flicker, glow and
  vector-line readouts. It should feel like analog film effects and practical
  props, not a modern glossy UI.
- **Runs everywhere.** One Rust codebase that builds as a native desktop app
  and as a web page hosted on GitHub Pages.

### Non-goals (for now)

- Planetarium-grade accuracy (proper motion, precession, atmospheric effects).
- Every star in Gaia (1.8 billion). We bundle a solid core catalog and fetch
  the rest on demand.

---

## 2. Visual direction

| Reference | What we take from it |
|---|---|
| Dotted white/blue hologram globe on an emitter base | Main **Globe view**: stars as clustered light dots on a sphere, a bright equator band, slight flicker and grain, glow bleeding into the dark |
| Briefing-room tactical holograms (red/green wireframe) | Wireframe spheres, lat/long grids, and the target-lock circle with a cone of lines connecting it to the map |
| Round briefing table / tactical display | Flat **Chart view**: circular projection with tick-marked rim, sector wedges |
| Vector targeting computer | Amber/yellow vector readouts, perspective grid, numeric counters |
| Archive star-map room (Ep. II) | **Locate sequence**: the globe expands into a room-scale 3D field of stars; the camera flies to the target, and the target ring locks onto it |

Rules:
- All art, fonts and sounds are original or openly licensed. Nothing is ripped
  from films.
- Nothing from the 2015+ sequel-era designs.
- Limited palettes: **holo blue-white** (default), **tactical red**,
  **targeting amber** and **wireframe green**, switchable at runtime.
- Text is monospace or vector-style and in uppercase where it helps the
  retro feel. Numbers count up instead of appearing instantly.

### Hologram post-processing stack

Each effect is applied as a full-screen pass and has its own adjustable parameters.

1. Bloom (built into Bevy)
2. Horizontal scanlines with slow vertical roll
3. Line jitter / occasional horizontal tear
4. Brightness flicker (low-frequency noise and rare dropouts)
5. Slight chromatic fringe at the edges
6. Film grain / static noise
7. Vignette

A developer panel (debug builds only) exposes every parameter as a slider so
the look can be tuned live, with presets saved to RON files.

---

## 3. Views & interaction

### Globe view (home)
Earth-centred celestial sphere seen from outside, slowly rotating. RA/Dec
grid, celestial equator as a bright band, faint constellation lines. Drag to
rotate, scroll to zoom, hover to show a name tag, click to select.

### Locate sequence
Triggered by search or by double-clicking a star:
1. The globe "unfolds". Stars move from sphere positions to true 3D positions
   (parsecs, from parallax), so the sphere becomes a 3D field around Sol.
2. The camera flies to the target. Neighbouring stars get labels and the
   target ring locks on with a short readout of coordinates.
3. The camera settles into an orbit around the target.

### Star dossier
A panel alongside the selected star: wireframe sphere sized and tinted by
spectral class, plus a data plate (designations, RA/Dec, distance in ly/pc,
apparent/absolute magnitude, spectral type, luminosity, nearest neighbours).

### Neighbourhood view
Free-fly 3D view of stars within N parsecs of Sol, with distance rings
around the Sun.

### Chart view (later)
Flat circular sky chart in the briefing-table style.

---

## 4. Data

| Source | Use | License |
|---|---|---|
| [HYG Database v4](https://github.com/astronexus/HYG-Database) (~120k stars; Hipparcos, Yale BSC, Gliese) | Core catalog: positions, distances, magnitudes, spectral types, designations | CC BY-SA 4.0 (attribution on credits page) |
| IAU WGSN star names (via HYG) | Proper names | - |
| Constellation line figures (d3-celestial or Stellarium "modern") | Constellation overlay | BSD-3 / to verify |
| SIMBAD (CDS) name resolver / TAP | Online fallback for stars not in the bundle | Free with acknowledgement |
| Gaia DR3 subset (optional, later) | Deeper star field | Free with acknowledgement |

**Pipeline:** an `xtask data` command downloads the raw CSVs into `data/raw/`
(gitignored), cleans and converts them, and writes a compact binary catalog
plus a name/designation search index to `assets/catalog/`. Target size is about
4-6 MB uncompressed, much less with gzip/brotli on the web.

---

## 5. Architecture

Cargo workspace:

```
crates/
  catalog/   star types, binary format, loading, search index (no Bevy dependency, unit-tested)
  viewer/    Bevy app: rendering, cameras, post-processing, HUD, input (native + wasm32)
  site/      static website generator built with stucco: landing page hosting the
             viewer canvas, catalog browser pages, credits/about
xtask/       data pipeline, web build, dist packaging
assets/      shaders, fonts, catalog binaries, palette/effect presets
docs/        this file
```

- **Viewer (Bevy):** stars rendered as one instanced draw (custom material,
  point size from magnitude). Custom render-graph node for the hologram post
  stack. HUD built in Bevy UI so desktop and web look identical. WebGL2 is
  the baseline for browser compatibility; WebGPU is used where available.
- **Site (stucco):** generates the HTML shell that loads the wasm viewer, plus
  pre-rendered pages: a star index by constellation, one page per named star
  (deep-linkable as `/star/vega/`, which opens the viewer focused on that
  star), and a credits page for the data licenses. Uses a custom OKLCH theme
  in the holo palette. Fully static, so it deploys straight to GitHub Pages.
- **Desktop:** the same viewer binary, packaged for Windows (and macOS/Linux
  via CI).

### Search
Prefix/fuzzy match over normalized names and designations ("alf Lyr",
"Alpha Lyrae" and "Vega" all resolve to the same star). Results show as an
autocomplete list in the HUD. On a miss, query SIMBAD, place the result in
the 3D field, and mark it as "external".

---

## 6. Roadmap

| Phase | Deliverable |
|---|---|
| 0. Setup | Toolchain, workspace skeleton, CI (fmt, clippy, test), empty Bevy window on desktop + web, GitHub Pages deploy |
| 1. Catalog | `xtask data` pipeline, `catalog` crate with binary format + search, tests against known stars |
| 2. Globe | All stars rendered on the celestial sphere, orbit camera, grid, equator band, constellations, hover/select |
| 3. Hologram look | Post-processing stack, palettes, live tuning panel |
| 4. Search & Locate | HUD search with autocomplete, sphere-to-3D transition, fly-to camera, target lock |
| 5. Dossier | Wireframe star, data plate, neighbours |
| 6. Website | stucco site: landing, catalog pages, deep links, credits, Pages deploy |
| 7. Polish | Online SIMBAD fallback, Chart view, original sound effects, emitter-base model, touch controls, performance passes |

---

## 7. Open questions

- Include deep-sky objects (nebulae, galaxies, clusters), e.g. the Messier list?
- Highlight exoplanet host stars (NASA Exoplanet Archive)?
- Sound: subtle analog beeps and hums, on or off by default?
- Final name / logo treatment.
