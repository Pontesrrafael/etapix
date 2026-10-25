//! New `.mpp` files from the template, without Project: every corpus project, saved as a new
//! file, must read back (natively) as the project Etapix saved. That Project reads these files
//! the same way is checked by `examples/mpp_write_check.rs prepare-new`.

use std::path::{Path, PathBuf};

use pjsched::calendar::Calendars;
use pjsched::cpm;
use pjsched::mpp::{create_mpp, mpp_to_mspdi};
use pjsched::mspdi::read_project;
use pjsched::mspdi_write::write_project;

#[path = "support/describe.rs"]
mod describe;

fn corpus(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            corpus(&p, out);
        } else if p.extension().is_some_and(|x| x == "mpp") {
            out.push(p);
        }
    }
}

#[test]
fn new_files_read_back_as_saved() {
    let mut files = Vec::new();
    corpus(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mpp-corpus"), &mut files);
    files.sort();
    let (mut created, mut failures) = (0, Vec::new());
    for f in &files {
        let project = read_project(&mpp_to_mspdi(&std::fs::read(f).unwrap()).unwrap()).unwrap();
        let cals = Calendars::new(&project).unwrap();
        let schedule = cpm::schedule(&project, &cals, None).unwrap();
        let xml = write_project(&project, &cals, &schedule, "teste");
        let Ok(bytes) = create_mpp(&xml) else { continue }; // a project the template cannot hold yet
        created += 1;
        let (saved, read) = (read_project(&xml).unwrap(), read_project(&mpp_to_mspdi(&bytes).unwrap()).unwrap());
        let (expected, got) = (describe::describe(&saved), describe::describe(&read));
        // Summary durations are Project's to roll up when it opens the file.
        if let Some((e, g)) = expected.iter().zip(&got).find(|(e, g)| e != g && !e.contains(" sum true ")) {
            failures.push(format!("{}:\n   saved {e}\n   read  {g}", f.display()));
        } else if expected.len() != got.len() {
            failures.push(format!("{}: {} vs {} items", f.display(), expected.len(), got.len()));
        } else if describe::schedule(&saved) != describe::schedule(&read) {
            failures.push(format!("{}: same data, different schedule", f.display()));
        }
    }
    assert!(created > 120, "only {created} of {} projects created", files.len());
    assert!(failures.is_empty(), "{} files differ:\n{}", failures.len(), failures.join("\n"));
}

/// A string property (author, last saved by) of the document summary, or `None` if absent.
fn summary_text(summary: &[u8], pid: u32) -> Option<String> {
    let u32_at = |o: usize| summary.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let section = u32_at(44)? as usize;
    let count = u32_at(section + 4)? as usize;
    (0..count).find_map(|k| {
        let entry = section + 8 + 8 * k;
        (u32_at(entry)? == pid).then_some(())?;
        let at = section + u32_at(entry + 4)? as usize;
        let len = u32_at(at + 4)? as usize;
        let text = summary.get(at + 8..at + 8 + len)?;
        Some(String::from_utf8_lossy(text).trim_end_matches('\0').to_string())
    })
}

/// New files carry no one's name: the template's author and last-saved-by are empty, so a file
/// a person creates is not attributed to whoever made the template.
#[test]
fn new_files_have_no_author() {
    const AUTHOR: u32 = 4;
    const LAST_SAVED_BY: u32 = 8;
    let xml = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/b1").read_dir().unwrap().map(|e| e.unwrap().path()).find(|p| p.extension().is_some_and(|x| x == "xml")).unwrap()).unwrap();
    let file = pjsched::cfb::read_streams(&create_mpp(&xml).unwrap()).unwrap();
    let summary = file.iter().find(|(k, _)| k.ends_with("\u{5}SummaryInformation")).map(|(_, v)| v).unwrap();
    for pid in [AUTHOR, LAST_SAVED_BY] {
        assert_eq!(summary_text(summary, pid).unwrap_or_default(), "", "propriedade {pid} do resumo");
    }
}
