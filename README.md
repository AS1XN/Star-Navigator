# Star-Navigator

Retro star map. An interactive chart of the real night sky and our stellar
neighbourhood, drawn like an analog hologram from old film sci-fi.

**Live:** https://as1xn.github.io/Star-Navigator/

Early days: see [docs/DESIGN.md](docs/DESIGN.md) for the plan and roadmap.

## Building

Requires Rust (stable) and, on Windows, the MSVC build tools.

```sh
cargo run -p viewer          # desktop
cargo test --workspace
```

Web build:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version <version in Cargo.lock> --locked
cargo xtask serve            # builds into dist/ and serves http://localhost:8080
```

## Layout

- `crates/catalog` - star data types and lookup
- `crates/viewer` - the Bevy app (desktop + wasm)
- `crates/site` - static site generator (stucco) that hosts the web build
- `xtask` - build tasks

## License

Code is MIT licensed (see LICENSE). Star data credits are listed on the site.
