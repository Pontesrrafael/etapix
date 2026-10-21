//! Replays the edit series of `tools/run_batch3.ps1` and compares every step with Project.
//!
//! Each series starts from its step-0 export and applies the same edits the script made through
//! COM; after each edit the task's duration, work and finish and every assignment's units, work
//! and finish must match the next export.

use std::path::Path;

use pjsched::calendar::Calendars;
use pjsched::cpm;
use pjsched::edit;
use pjsched::model::{Project, Wm};
use pjsched::mspdi::{read_assignment_results, read_project, read_results};

const DAY: Wm = 480;

#[derive(Clone, Copy)]
enum Op {
    Add(&'static str, f64),
    Remove(&'static str),
    Duration(Wm),
    Units(&'static str, f64),
    TaskWork(f64),
    AssignmentWork(&'static str, f64),
}

const SERIES_STEPS: [(&str, Op); 5] = [
    ("1-add-R2", Op::Add("R2", 1.0)),
    ("2-dur-6d", Op::Duration(6 * DAY)),
    ("3-R1-50pct", Op::Units("R1", 0.5)),
    ("4-work-80h", Op::TaskWork(80.0 * 60.0)),
    ("5-R2-work-40h", Op::AssignmentWork("R2", 40.0 * 60.0)),
];

const HALF_TIME_STEPS: [(&str, Op); 2] = [("1-add-R2-50pct", Op::Add("R2", 0.5)), ("2-del-R1", Op::Remove("R1"))];

fn golden(file: &str) -> String {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let path = ["b3", "b12"].iter().map(|d| base.join(d).join(file)).find(|p| p.exists()).unwrap_or(base.join("b3").join(file));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn resource(p: &Project, name: &str) -> usize {
    p.resources.iter().position(|r| r.name == name).unwrap_or_else(|| panic!("no resource {name}"))
}

fn apply(p: &mut Project, op: Op) {
    let t = 0;
    match op {
        Op::Add(r, u) => edit::add_assignment(p, t, resource(p, r), u).unwrap(),
        Op::Remove(r) => edit::remove_assignment(p, t, resource(p, r)).unwrap(),
        Op::Duration(d) => edit::set_duration(p, t, d),
        Op::Units(r, u) => edit::set_units(p, t, resource(p, r), u).unwrap(),
        Op::TaskWork(w) => edit::set_task_work(p, t, w),
        Op::AssignmentWork(r, w) => edit::set_assignment_work(p, t, resource(p, r), w).unwrap(),
    }
}

fn compare(step: &str, p: &Project, xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let cals = Calendars::new(p).unwrap();
    let (tl, clock) = (cals.task(0), &cals.clock);
    let sched = cpm::schedule(p, &cals, None).unwrap();
    let want = &read_results(xml).unwrap()[0];
    let task = &p.tasks[0];
    let work: f64 = p.assignments.iter().map(|a| a.work()).sum();
    let mut check = |field: String, got: String, want: String| {
        if got != want {
            out.push(format!("{step}: {field} = {got}, Project = {want}"));
        }
    };
    check("duracao".into(), task.duration.to_string(), want.duration.to_string());
    check("trabalho".into(), (work.round() as Wm).to_string(), want.work.to_string());
    check("termino".into(), clock.dt(sched[0].finish).to_string(), want.finish.to_string());

    let want_assignments = read_assignment_results(xml).unwrap();
    check("atribuicoes".into(), p.assignments.len().to_string(), want_assignments.len().to_string());
    for w in &want_assignments {
        let Some(a) = p.assignments.iter().find(|a| p.resources[a.resource].name == w.resource) else {
            check(format!("{}.existe", w.resource), "nao".into(), "sim".into());
            continue;
        };
        let r = &w.resource;
        check(format!("{r}.unidades"), format!("{:.3}", a.units), format!("{:.3}", w.units));
        check(format!("{r}.trabalho"), (a.work().round() as Wm).to_string(), w.work.to_string());
        let finish = tl.finish_at(tl.w(sched[0].start) + a.offset + a.span());
        check(format!("{r}.termino"), clock.dt(finish).to_string(), w.finish.to_string());
    }
    out
}

fn replay(series: &str, base: &str, steps: &[(&str, Op)]) -> Vec<String> {
    let mut p = read_project(&golden(&format!("{series}-0-{base}.xml"))).unwrap();
    let mut problems = Vec::new();
    for &(step, op) in steps {
        apply(&mut p, op);
        problems.extend(compare(&format!("{series}-{step}"), &p, &golden(&format!("{series}-{step}.xml"))));
    }
    problems
}

/// `(series prefix, step-0 suffix, steps)`.
type Series<'a> = (&'a str, &'a str, &'a [(&'a str, Op)]);

fn assert_series(cases: &[Series]) {
    let problems: Vec<String> = cases.iter().flat_map(|&(s, b, steps)| replay(s, b, steps)).collect();
    assert!(problems.is_empty(), "{} divergences:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn fixed_units() {
    assert_series(&[("B3-FU", "base-4d-R1", &SERIES_STEPS), ("B3-FU-ED", "base-4d-R1", &SERIES_STEPS)]);
}

#[test]
fn fixed_duration() {
    assert_series(&[("B3-FD", "base-4d-R1", &SERIES_STEPS), ("B3-FD-ED", "base-4d-R1", &SERIES_STEPS)]);
}

#[test]
fn fixed_work() {
    assert_series(&[("B3-FW", "base-4d-R1", &SERIES_STEPS)]);
}

#[test]
fn effort_driven_add_and_remove() {
    assert_series(&[
        ("B3-H-FU", "base-3d-R1", &HALF_TIME_STEPS),
        ("B3-H-FU-ED", "base-3d-R1", &HALF_TIME_STEPS),
        ("B3-H-FW", "base-3d-R1", &HALF_TIME_STEPS),
    ]);
}

#[test]
fn batch12_effort_driven_removal_and_less_work() {
    const FD_ED_REMOVE: [(&str, Op); 1] = [("1-del-R1", Op::Remove("R1"))];
    const FW_LESS: [(&str, Op); 2] = [("1-dur-6d", Op::Duration(6 * DAY)), ("2-work-16h", Op::TaskWork(16.0 * 60.0))];
    assert_series(&[("B12-FD-ED-remover", "base-4d-R1-R2", &FD_ED_REMOVE), ("B12-FW-reduzir", "base-4d-R1-R2", &FW_LESS)]);
}
