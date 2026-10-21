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

/// Baseline variances and earned value against Project, for every golden input.
#[test]
fn variances_and_earned_value_match_project_everywhere() {
    use pjsched::tracking::tracking;
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut problems = Vec::new();
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
            let got = tracking(&project, &cals, &schedule, &costs(&project, &cals, &schedule));
            for (i, w) in read_results(&xml).unwrap().iter().enumerate() {
                let g = &got[i];
                let mut check = |field: &str, a: f64, b: f64| {
                    if (a - b).abs() > 0.005 {
                        problems.push(format!("{name} / {}: {field} = {a:.2}, Project = {b:.2}", w.name));
                    }
                };
                check("variacao_inicio", g.start_variance as f64, w.start_variance as f64);
                check("variacao_termino", g.finish_variance as f64, w.finish_variance as f64);
                check("variacao_trabalho", g.work_variance, w.work_variance);
                check("BCWS", g.bcws, w.bcws);
                check("BCWP", g.bcwp, w.bcwp);
                check("ACWP", g.acwp, w.acwp);
            }
        }
    }
    assert!(problems.is_empty(), "{} divergences:\n{}", problems.len(), problems.join("\n"));
}

/// Saving a baseline on B18-B2's plan (nothing changed since Project saved it there) reproduces
/// Project's baseline: dates, duration, work and cost.
#[test]
fn save_baseline_reproduces_project() {
    use pjsched::tracking::save_baseline;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/b18/B18-B2-valor-agregado-antes.xml");
    let mut project = read_project(&std::fs::read_to_string(path).unwrap()).unwrap();
    let want: Vec<_> = project.tasks.iter().map(|t| t.baseline.clone().unwrap()).collect();
    let cals = Calendars::new(&project).unwrap();
    let schedule = cpm::schedule(&project, &cals, None).unwrap();
    let c = costs(&project, &cals, &schedule);
    save_baseline(&mut project, &cals, &schedule, &c);
    for (t, w) in project.tasks.iter().zip(&want) {
        let b = t.baseline.as_ref().unwrap();
        assert_eq!((b.start, b.finish, b.duration), (w.start, w.finish, w.duration), "{}", t.name);
        assert!((b.work - w.work).abs() < 0.5 && (b.cost - w.cost).abs() < 0.005, "{}: {b:?} vs {w:?}", t.name);
    }
}
