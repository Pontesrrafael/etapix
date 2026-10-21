//! Round trip through Microsoft Project: pjsched wrote every golden input as MSPDI
//! (`examples/roundtrip_write.rs`), Project reopened and recalculated each file
//! (`tools/roundtrip.ps1`, saved under `tests/golden/rt`). What Project computed from pjsched's
//! file must equal what pjsched computes from the original input.

use std::path::{Path, PathBuf};

use pjsched::calendar::Calendars;
use pjsched::cpm;
use pjsched::mspdi::{read_project, read_results};

fn original(golden: &Path, name: &str) -> PathBuf {
    std::fs::read_dir(golden)
        .unwrap()
        .map(|e| e.unwrap().path().join(name))
        .find(|p| p.exists() && !p.starts_with(golden.join("rt")))
        .unwrap_or_else(|| panic!("no original for {name}"))
}

/// Known gaps, each with the reason it is not yet reproduced.
const KNOWN: &[(&str, &str)] = &[];

#[test]
fn project_recalculates_pjsched_files_identically() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut problems = Vec::new();
    let mut cases = 0;
    for rt in std::fs::read_dir(golden.join("rt")).unwrap().map(|e| e.unwrap().path()) {
        let name = rt.file_name().unwrap().to_string_lossy().to_string();
        if KNOWN.iter().any(|(k, _)| *k == name) {
            continue;
        }
        let project = read_project(&std::fs::read_to_string(original(&golden, &name)).unwrap()).unwrap();
        let cals = Calendars::new(&project).unwrap();
        let ours = cpm::schedule(&project, &cals, None).unwrap();
        let theirs = read_results(&std::fs::read_to_string(&rt).unwrap()).unwrap();
        if ours.len() != theirs.len() {
            problems.push(format!("{name}: {} tasks, Project has {}", ours.len(), theirs.len()));
            continue;
        }
        for (o, t) in ours.iter().zip(&theirs) {
            let dt = |at| cals.clock.dt(at);
            for (field, got, want) in [
                ("start", dt(o.start), t.start),
                ("finish", dt(o.finish), t.finish),
                ("late_start", dt(o.late_start), t.late_start),
            ] {
                if got != want {
                    problems.push(format!("{name} / {}: {field} = {got}, Project = {want}", t.name));
                }
            }
            if o.total_slack != t.total_slack {
                problems.push(format!("{name} / {}: total_slack = {}, Project = {}", t.name, o.total_slack, t.total_slack));
            }
        }
        cases += 1;
    }
    assert!(cases > 0, "no round-trip files; run tools/roundtrip.ps1");
    assert!(problems.is_empty(), "{} divergences in {cases} files:\n{}", problems.len(), problems.join("\n"));
}
