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

#[test]
fn unsupported_projects_are_refused_with_a_reason() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mpp-corpus/b17/B17-C4-material.mpp");
    let project = read_project(&mpp_to_mspdi(&std::fs::read(path).unwrap()).unwrap()).unwrap();
    let cals = Calendars::new(&project).unwrap();
    let schedule = cpm::schedule(&project, &cals, None).unwrap();
    let err = create_mpp(&write_project(&project, &cals, &schedule, "teste")).unwrap_err();
    assert!(err.0.contains("material") && err.0.contains("Microsoft Project"), "{}", err.0);
}
