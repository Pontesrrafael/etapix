//! Writes every golden input back out as MSPDI through pjsched, for Project to reopen:
//! `cargo run --example roundtrip_write -- <out dir>`, then `tools/roundtrip.ps1`.

use std::path::Path;

use pjsched::calendar::Calendars;
use pjsched::cpm;
use pjsched::mspdi::read_project;
use pjsched::mspdi_write::write_project;

fn main() {
    let out = std::env::args().nth(1).expect("usage: roundtrip_write <out dir>");
    std::fs::create_dir_all(&out).unwrap();
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut written = 0;
    for dir in std::fs::read_dir(&golden).unwrap().map(|e| e.unwrap().path()) {
        for file in std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()) {
            let name = file.file_stem().unwrap().to_string_lossy().to_string();
            if name.ends_with("-depois") || dir.file_name().unwrap() == "rt" {
                continue;
            }
            let project = read_project(&std::fs::read_to_string(&file).unwrap()).unwrap();
            let cals = Calendars::new(&project).unwrap();
            let schedule = cpm::schedule(&project, &cals, None).unwrap();
            let xml = write_project(&project, &cals, &schedule, &name);
            std::fs::write(Path::new(&out).join(format!("{name}.xml")), xml).unwrap();
            written += 1;
        }
    }
    println!("{written} projetos gravados em {out}");
}
