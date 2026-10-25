//! Removes a person's name from the metadata of `.mpp` files (author, last saved by, company),
//! in place: `cargo run --example mpp_anonymize -- "Nome Completo" arquivo.mpp...`.
//!
//! Project writes the user's name into the document summary and its own properties. The
//! template every new file starts from (`assets/modelo.mpp`) and the test corpus must not carry
//! it. Each character of the name becomes a NUL, so every field keeps its length and readers see
//! an empty value.

use pjsched::cfb::Compound;

/// The streams Project keeps the author in (the summaries' names start with an invisible \x05).
const METADATA: [&str; 3] = ["SummaryInformation", "DocumentSummaryInformation", "/Props14"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((name, files)) = args.split_first().filter(|(_, f)| !f.is_empty()) else {
        eprintln!("uso: mpp_anonymize \"Nome Completo\" arquivo.mpp...");
        std::process::exit(2);
    };
    for path in files {
        match anonymize(path, name) {
            Ok(0) => {}
            Ok(n) => println!("{path}: {n} ocorrência(s) removida(s)"),
            Err(e) => eprintln!("{path}: {e}"),
        }
    }
}

fn anonymize(path: &str, name: &str) -> Result<usize, String> {
    let mut file = Compound::parse(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.0)?;
    let ansi: Vec<u8> = name.bytes().collect();
    let wide: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut found = 0;
    let streams: Vec<String> = file.streams().into_keys().filter(|k| METADATA.iter().any(|m| k.ends_with(m))).collect();
    for stream in streams {
        let Some(mut data) = file.get(&stream).map(<[u8]>::to_vec) else { continue };
        let n = blank(&mut data, &ansi) + blank(&mut data, &wide);
        if n > 0 {
            file.set(&stream, data).map_err(|e| e.0)?;
            found += n;
        }
    }
    if found > 0 {
        std::fs::write(path, file.to_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(found)
}

/// Zeroes every occurrence of `needle` in `data`; returns how many there were.
fn blank(data: &mut [u8], needle: &[u8]) -> usize {
    let mut count = 0;
    let mut at = 0;
    while let Some(i) = data[at..].windows(needle.len()).position(|w| w == needle) {
        data[at + i..at + i + needle.len()].fill(0);
        at += i + needle.len();
        count += 1;
    }
    count
}
