//! The native `.mpp` writer, without Project: every corpus file, edited the way a user would and
//! saved into a copy of itself, must read back (natively) as the project Etapix saved. That
//! Project reads these files the same way is checked by `examples/mpp_write_check.rs`.

use std::path::{Path, PathBuf};

use pjsched::calendar::Calendars;
use pjsched::cpm;
use pjsched::mpp::{mpp_to_mspdi, save_mpp, update_mpp};
use pjsched::mspdi::read_project;
use pjsched::mspdi_write::write_project;

#[path = "support/describe.rs"]
mod describe;
#[path = "support/edits.rs"]
mod edits;

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
fn edits_read_back_from_the_saved_file() {
    let mut files = Vec::new();
    corpus(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mpp-corpus"), &mut files);
    files.sort();
    let (mut saved, mut failures) = (0, Vec::new());
    for f in &files {
        let original = std::fs::read(f).unwrap();
        let mut project = read_project(&mpp_to_mspdi(&original).unwrap()).unwrap();
        edits::edit(&mut project);
        let cals = Calendars::new(&project).unwrap();
        let schedule = cpm::schedule(&project, &cals, None).unwrap();
        let xml = write_project(&project, &cals, &schedule, "teste");
        let Ok(bytes) = update_mpp(&original, &xml) else { continue }; // a change the writer refuses
        saved += 1;
        let (saved_project, read) = (read_project(&xml).unwrap(), read_project(&mpp_to_mspdi(&bytes).unwrap()).unwrap());
        let (expected, got) = (describe::describe(&saved_project), describe::describe(&read));
        // Summary durations are Project's to roll up when it opens the file.
        let differs = expected.iter().zip(&got).find(|(e, g)| e != g && !e.contains(" sum true "));
        if let Some((e, g)) = differs {
            failures.push(format!("{}:\n   saved {e}\n   read  {g}", f.display()));
        } else if describe::schedule(&saved_project) != describe::schedule(&read) {
            failures.push(format!("{}: same data, different schedule", f.display()));
        }
    }
    assert!(saved > 180, "only {saved} of {} files saved", files.len());
    assert!(failures.is_empty(), "{} files differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn structural_changes_are_refused_with_a_reason() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mpp-corpus/b1/S1-caminho-critico.mpp");
    let original = std::fs::read(path).unwrap();
    let mut project = read_project(&mpp_to_mspdi(&original).unwrap()).unwrap();
    project.tasks.pop();
    let cals = Calendars::new(&project).unwrap();
    let schedule = cpm::schedule(&project, &cals, None).unwrap();
    let err = update_mpp(&original, &write_project(&project, &cals, &schedule, "teste")).unwrap_err();
    assert!(err.0.contains("tarefas incluídas, excluídas"), "{}", err.0);
}

/// Tasks, links and assignments added and removed: saved into the file's own tables, the file
/// reads back as the project Etapix saved.
#[test]
fn structural_changes_read_back_from_the_saved_file() {
    let mut files = Vec::new();
    corpus(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mpp-corpus"), &mut files);
    files.sort();
    let (mut saved, mut failures) = (0, Vec::new());
    for f in &files {
        let original = std::fs::read(f).unwrap();
        let mut project = read_project(&mpp_to_mspdi(&original).unwrap()).unwrap();
        edits::restructure(&mut project);
        let cals = Calendars::new(&project).unwrap();
        let schedule = cpm::schedule(&project, &cals, None).unwrap();
        let xml = write_project(&project, &cals, &schedule, "teste");
        let Ok(bytes) = save_mpp(&original, &xml) else { continue };
        saved += 1;
        let (saved_project, read) = (read_project(&xml).unwrap(), read_project(&mpp_to_mspdi(&bytes).unwrap()).unwrap());
        let (expected, got) = (describe::describe(&saved_project), describe::describe(&read));
        if let Some((e, g)) = expected.iter().zip(&got).find(|(e, g)| e != g && !e.contains(" sum true ")) {
            failures.push(format!("{}:\n   saved {e}\n   read  {g}", f.display()));
        } else if expected.len() != got.len() {
            failures.push(format!("{}: {} vs {} items", f.display(), expected.len(), got.len()));
        }
    }
    assert!(saved > 190, "only {saved} of {} files saved", files.len());
    assert!(failures.is_empty(), "{} files differ:\n{}", failures.len(), failures.join("\n"));
}
