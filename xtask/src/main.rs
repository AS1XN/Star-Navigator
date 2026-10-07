//! Project tasks. Run with `cargo xtask <task>`.
//!
//!   data     convert the HYG csv (downloaded if missing) into assets/catalog/stars.bin
//!   web      build the wasm viewer and the static site into dist/
//!   serve    build web, then serve dist/ on http://localhost:8080
//!            (`serve 8090` picks another port, `--no-build` serves the last build)

mod data;

use std::{
    env, fs,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, exit},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

const WASM_TARGET: &str = "wasm32-unknown-unknown";
const PROFILE: &str = "wasm-release";

fn main() {
    let task = env::args().nth(1).unwrap_or_default();
    let res = match task.as_str() {
        "data" => data::data(env::args().any(|a| a == "--download")),
        "web" => web(),
        "serve" => {
            let args: Vec<String> = env::args().skip(2).collect();
            let port = args.iter().find_map(|a| a.parse().ok()).unwrap_or(8080);
            let build = if args.iter().any(|a| a == "--no-build") { Ok(()) } else { web() };
            build.and_then(|_| serve(port))
        }
        _ => {
            eprintln!("usage: cargo xtask <data [--download] | web | serve [port] [--no-build]>");
            exit(2);
        }
    };
    if let Err(e) = res {
        eprintln!("error: {e}");
        exit(1);
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn run(cmd: &mut Command) -> Result {
    eprintln!("> {cmd:?}");
    let status = cmd.current_dir(root()).status()?;
    if !status.success() {
        return Err(format!("command failed with {status}").into());
    }
    Ok(())
}

fn web() -> Result {
    let root = root();
    let dist = root.join("dist");
    if dist.exists() {
        fs::remove_dir_all(&dist)?;
    }

    check_bindgen_version()?;

    run(Command::new("cargo").args([
        "build",
        "--package",
        "viewer",
        "--target",
        WASM_TARGET,
        "--profile",
        PROFILE,
    ]))?;

    let wasm = root.join("target").join(WASM_TARGET).join(PROFILE).join("star-navigator.wasm");
    run(Command::new("wasm-bindgen")
        .args(["--target", "web", "--no-typescript", "--out-dir"])
        .arg(dist.join("pkg"))
        .arg(&wasm))?;

    let bg = dist.join("pkg").join("star-navigator_bg.wasm");
    let before = fs::metadata(&bg)?.len();
    fs::write(&bg, strip_custom_sections(&fs::read(&bg)?)?)?;
    optimize_wasm(&bg);
    let wasm = fs::read(&bg)?;
    eprintln!("wasm: {} -> {} KB", before / 1024, wasm.len() / 1024);

    // GitHub Pages serves .wasm and binary assets uncompressed, so ship gzipped
    // copies ourselves; the page and the asset loader unpack them.
    let gz = gzip(&wasm)?;
    fs::write(bg.with_extension("wasm.gz"), &gz)?;
    eprintln!("wasm.gz: {} KB", gz.len() / 1024);

    // Name the folder after the build's contents. GitHub Pages lets browsers cache
    // files for 10 minutes, so a fixed `pkg/` path could pair a fresh page with
    // last release's wasm.
    let pkg = format!("pkg-{:08x}", fnv1a(&wasm) as u32);
    fs::rename(dist.join("pkg"), dist.join(&pkg))?;

    run(Command::new("cargo")
        .args(["run", "--package", "site", "--release", "--"])
        .arg(&dist)
        .arg(&pkg))?;

    let assets = root.join("assets");
    if assets.exists() {
        copy_dir(&assets, &dist.join("assets"))?;
    }
    // The catalog is the other big download. The viewer's loader recognises gzip,
    // so the web copy can be compressed in place under the same name.
    for name in ["stars.bin", "constellations.bin"] {
        let path = dist.join("assets/catalog").join(name);
        let raw = fs::read(&path)?;
        let gz = gzip(&raw)?;
        fs::write(&path, &gz)?;
        eprintln!("{name}: {} -> {} KB gzipped", raw.len() / 1024, gz.len() / 1024);
    }
    Ok(())
}

fn gzip(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(bytes)?;
    enc.finish()
}

/// Runs binaryen's `wasm-opt -Oz` if it's installed (CI installs it); the build
/// works without it, just larger.
fn optimize_wasm(path: &Path) {
    let features = [
        "--enable-bulk-memory",
        "--enable-nontrapping-float-to-int",
        "--enable-sign-ext",
        "--enable-mutable-globals",
        "--enable-reference-types",
        "--enable-multivalue",
    ];
    let status =
        Command::new("wasm-opt").arg("-Oz").args(features).arg(path).arg("-o").arg(path).status();
    match status {
        Ok(s) if s.success() => eprintln!("wasm-opt: done"),
        Ok(s) => eprintln!("wasm-opt failed ({s}); keeping the unoptimized build"),
        Err(_) => eprintln!("wasm-opt not found; skipping"),
    }
}

/// wasm-bindgen-cli must match the wasm-bindgen crate version exactly.
fn check_bindgen_version() -> Result {
    let lock = fs::read_to_string(root().join("Cargo.lock"))?;
    let want = lock
        .split("[[package]]")
        .find(|p| p.contains("name = \"wasm-bindgen\"\n"))
        .and_then(|p| p.lines().find_map(|l| l.strip_prefix("version = ")))
        .map(|v| v.trim_matches('"').to_string())
        .ok_or("wasm-bindgen not found in Cargo.lock")?;

    let out = Command::new("wasm-bindgen").arg("--version").output();
    let have = out.ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    if !have.contains(&want) {
        return Err(format!(
            "wasm-bindgen-cli {want} required (found: {}). Install with:\n  \
             cargo install wasm-bindgen-cli --version {want} --locked",
            if have.is_empty() { "none" } else { have.trim() }
        )
        .into());
    }
    Ok(())
}

/// 64-bit FNV-1a, enough to fingerprint a build.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// Drops debug info and the name section from a wasm module. Saves installing
/// binaryen just for `wasm-opt --strip-debug`.
fn strip_custom_sections(wasm: &[u8]) -> Result<Vec<u8>> {
    fn leb(bytes: &[u8], pos: &mut usize) -> Result<usize> {
        let (mut n, mut shift) = (0usize, 0);
        loop {
            let b = *bytes.get(*pos).ok_or("truncated wasm")?;
            *pos += 1;
            n |= ((b & 0x7f) as usize) << shift;
            if b & 0x80 == 0 {
                return Ok(n);
            }
            shift += 7;
        }
    }

    if wasm.get(..4) != Some(b"\0asm") {
        return Err("not a wasm module".into());
    }
    let mut out = wasm[..8].to_vec();
    let mut pos = 8;
    while pos < wasm.len() {
        let start = pos;
        let id = wasm[pos];
        pos += 1;
        let size = leb(wasm, &mut pos)?;
        let end = pos + size;
        let keep = id != 0 || {
            let mut p = pos;
            let len = leb(wasm, &mut p)?;
            let name = &wasm[p..p + len];
            !(name.starts_with(b".debug") || name == b"name" || name == b"producers")
        };
        if keep {
            out.extend_from_slice(&wasm[start..end]);
        }
        pos = end;
    }
    Ok(out)
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

/// Minimal static file server for local testing.
fn serve(port: u16) -> Result {
    let dist = root().join("dist");
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    eprintln!("serving {} at http://localhost:{port}", dist.display());
    for stream in listener.incoming().flatten() {
        // One thread per connection: browsers open idle speculative connections
        // that would otherwise block the accept loop.
        let dist = dist.clone();
        std::thread::spawn(move || {
            if let Err(e) = respond(stream, &dist) {
                eprintln!("request failed: {e}");
            }
        });
    }
    Ok(())
}

fn respond(mut stream: TcpStream, dist: &Path) -> io::Result<()> {
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("/");
    let path = path.split('?').next().unwrap_or("/").trim_start_matches('/');

    let mut file = dist.join(if path.is_empty() { "index.html" } else { path });
    if file.is_dir() {
        file = file.join("index.html");
    }
    let inside = file.canonicalize().is_ok_and(|f| f.starts_with(dist.canonicalize().unwrap()));

    match fs::read(&file) {
        Ok(body) if inside => {
            let mime = match file.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("js") => "text/javascript",
                Some("wasm") => "application/wasm",
                Some("css") => "text/css",
                Some("json") => "application/json",
                Some("png") => "image/png",
                _ => "application/octet-stream",
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\r\n",
                body.len()
            )?;
            stream.write_all(&body)
        }
        _ => stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nnot found"),
    }
}
