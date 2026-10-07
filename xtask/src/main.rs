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
    eprintln!("stripped wasm: {} -> {} KB", before / 1024, fs::metadata(&bg)?.len() / 1024);

    run(Command::new("cargo").args(["run", "--package", "site", "--release", "--"]).arg(&dist))?;

    let assets = root.join("assets");
    if assets.exists() {
        copy_dir(&assets, &dist.join("assets"))?;
    }
    Ok(())
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
