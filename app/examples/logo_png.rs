//! Writes the Etapix mark as a PNG: `cargo run -p etapix --example logo_png -- <path> [size]`.
//! Used for the Claude Desktop connector icon (tools\build-integrations.ps1).

#[path = "../src/logo.rs"]
#[allow(dead_code)]
mod logo;
#[path = "../src/snapshot.rs"]
#[allow(dead_code)]
mod snapshot;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("caminho do PNG");
    let size: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(256);
    std::fs::write(path, snapshot::png(size as usize, size as usize, &logo::rgba(size))).unwrap();
}
