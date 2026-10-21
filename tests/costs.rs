//! Costs against Microsoft Project: every golden input's task and assignment costs, as
//! computed by pjsched from the schedule, must match what Project wrote (to the cent).

use std::path::Path;

use pjsched::calendar::Calendars;
use pjsched::cost::costs;
use pjsched::cpm;
use pjsched::mspdi::{read_assignment_results, read_project, read_results};

#[test]
fn costs_match_project_everywhere() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut problems = Vec::new();
    let mut files = 0;
    for dir in std::fs::read_dir(&golden).unwrap().map(|e| e.unwrap().path()) {
        if dir.file_name().unwrap() == "rt" {
            continue;
        }
        for file in std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()) {
            let name = file.file_name().unwrap().to_string_lossy().to_string();
            let xml = std::fs::read_to_string(&file).unwrap();
            let project = read_project(&xml).unwrap();
            let cals = Calendars::new(&project).unwrap();
            let schedule = cpm::schedule(&project, &cals, None).unwrap();
            let got = costs(&project, &cals, &schedule);
            for (i, want) in read_results(&xml).unwrap().iter().enumerate() {
                if (got.task[i] - want.cost).abs() > 0.005 {
                    problems.push(format!("{name} / {}: custo = {:.2}, Project = {:.2}", want.name, got.task[i], want.cost));
                }
            }
            for want in read_assignment_results(&xml).unwrap() {
                let k = project.assignments.iter().position(|a| {
                    project.tasks[a.task].uid == want.task_uid && project.resources[a.resource].name == want.resource
                });
                if let Some(k) = k {
                    if (got.assignment[k] - want.cost).abs() > 0.005 {
                        problems.push(format!("{name} / atribuicao {}: custo = {:.2}, Project = {:.2}", want.resource, got.assignment[k], want.cost));
                    }
                }
            }
            files += 1;
        }
    }
    assert!(problems.is_empty(), "{} divergences in {files} files:\n{}", problems.len(), problems.join("\n"));
}
