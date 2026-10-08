# Star-Navigator

Retro star map. An interactive chart of the real night sky and our stellar
neighbourhood, drawn like an analog hologram from old film sci-fi.

**Live:** https://as1xn.github.io/Star-Navigator/

Browse the [star catalog](https://as1xn.github.io/Star-Navigator/catalog/) by
constellation, or link straight to a star:
[star/vega/](https://as1xn.github.io/Star-Navigator/star/vega/) for named stars, or
`?star=<name or catalog number>` for anything else (e.g. `?star=hip-71683`).
Stars that aren't in the catalog are looked up live in SIMBAD (search offers
"QUERY SIMBAD ONLINE", and links fall back to it, e.g. `?star=trappist-1`).
Locating a star updates the address bar, so the current view can be shared. On
desktop, `star-navigator --star vega` does the same.

See [docs/DESIGN.md](docs/DESIGN.md) for the plan and roadmap.

## Controls

Drag to rotate, scroll to zoom, click a star for its details. Press `/` (or Enter) to
search by name or catalog number; Enter on a result flies to the star in 3D.

| Key | Action |
|---|---|
| `[` `]` | Show fewer / more (fainter) stars |
| G / C | Toggle grid / constellation figures |
| Space | Toggle idle spin |
| P | Cycle palette (holo blue, tactical red, targeting amber, wireframe green) |
| T | Open the display calibration panel (arrows adjust, S saves a preset) |
| H | Show the raw view without the display effects |
| `/` or Enter | Find a star (Up/Down to pick, Enter to locate) |
| L | Locate the selected star in 3D |
| Esc | Return from the 3D view to the sky chart |

## Building

Requires Rust (stable) and, on Windows, the MSVC build tools.

```sh
cargo run -p viewer          # desktop
cargo test --workspace
cargo xtask data             # rebuild assets/catalog/stars.bin from the HYG database
```

Web build:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version <version in Cargo.lock> --locked
cargo xtask serve            # builds into dist/ and serves http://localhost:8080
cargo xtask serve 8090 --no-build   # another port, reuse the last build
```

## Layout

- `crates/catalog` - star catalog format, loading and search
- `crates/viewer` - the Bevy app (desktop + wasm)
- `crates/site` - static site generator (stucco) that hosts the web build
- `xtask` - build tasks

## License

Code is MIT licensed (see LICENSE). Star data credits are listed on the site.
