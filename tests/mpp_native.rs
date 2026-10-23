//! The native `.mpp` reader against Project itself: every file in `tests/mpp-corpus` was saved
//! by Project as `.mpp` and, in the same session, as Project XML. Reading the `.mpp` natively
//! must give the same project, and the same schedule, as reading Project's XML.

use std::path::{Path, PathBuf};

use pjsched::calendar::Calendars;
use pjsched::cpm;
use pjsched::model::Project;
use pjsched::mpp::mpp_to_mspdi;
use pjsched::mspdi::read_project;

fn pairs(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            pairs(&p, out);
        } else if p.extension().is_some_and(|x| x == "mpp") && p.with_extension("xml").exists() {
            out.push(p);
        }
    }
}

/// What must agree, as text, so a mismatch shows exactly where.
fn describe(p: &Project) -> Vec<String> {
    // Calendars compare by their working time; names of resource calendars vary between exports.
    let cal = |i: Option<usize>| i.map(|i| format!("{:?} {:?}", p.calendars[i].weekdays, p.calendars[i].exceptions)).unwrap_or_default();
    let mut out = vec![format!("start {} calendar {} honor {} multiple {} slack {} status {:?}", p.start, cal(Some(p.calendar)), p.honor_constraints, p.multiple_critical_paths, p.critical_slack_limit, p.status_date)];
    for t in &p.tasks {
        out.push(format!(
            "task {} {} '{}' dur {} el {} {:?} ed {} pr {} {:?} dl {:?} split {} preds {:?} sum {} parent {:?} cal '{}' ign {} manual {:?} fc {} acc {} base {:?} lev {} prog {:?}",
            t.uid, t.id, t.name, t.duration, t.elapsed, t.task_type, t.effort_driven, t.priority, t.constraint, t.deadline, t.leveling_can_split,
            t.preds.iter().map(|l| (l.pred, l.kind, l.lag, l.lag_elapsed)).collect::<Vec<_>>(), t.summary, t.parent, cal(t.calendar),
            t.ignore_resource_calendar, t.manual, t.fixed_cost, t.fixed_cost_accrual, t.baseline, t.leveling_delay, t.progress
        ));
    }
    for r in &p.resources {
        out.push(format!("resource {} '{}' {:?} rates {:?} max {} cal '{}'", r.uid, r.name, r.kind, r.rates, r.max_units, cal(r.calendar.or(Some(p.calendar)))));
    }
    for a in &p.assignments {
        out.push(format!("assignment task {} res {} units {} offset {} work {:.1}", a.task, a.resource, a.units, a.offset, a.work()));
    }
    out
}

fn schedule(p: &Project) -> Vec<(i64, i64)> {
    let cals = Calendars::new(p).unwrap();
    cpm::schedule(p, &cals, None).unwrap().iter().map(|s| (s.start, s.finish)).collect()
}

/// The first difference between reading `mpp` natively and reading Project's `xml` of it.
fn difference(mpp: &Path, xml: &Path) -> Option<String> {
    let truth = read_project(&std::fs::read_to_string(xml).unwrap()).ok()?; // features the engine does not read yet
    let native = match mpp_to_mspdi(&std::fs::read(mpp).unwrap()).map_err(|e| e.0).and_then(|x| read_project(&x).map_err(|e| e.0)) {
        Ok(p) => p,
        Err(e) => return Some(e),
    };
    let (a, b) = (describe(&truth), describe(&native));
    if let Some((x, y)) = a.iter().zip(&b).find(|(x, y)| x != y) {
        Some(format!("
   xml    {x}
   native {y}"))
    } else if a.len() != b.len() {
        Some(format!("{} vs {} items", a.len(), b.len()))
    } else if schedule(&truth) != schedule(&native) {
        Some("same data, different schedule".into())
    } else {
        None
    }
}

#[test]
fn native_reader_matches_projects_own_xml() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mpp-corpus");
    let mut files = Vec::new();
    pairs(&root, &mut files);
    files.sort();
    assert!(files.len() > 200, "corpus missing: {}", files.len());
    let failures: Vec<String> = files
        .iter()
        .filter_map(|f| difference(f, &f.with_extension("xml")).map(|d| format!("{}: {d}", f.strip_prefix(&root).unwrap().display())))
        .collect();
    assert!(failures.is_empty(), "{} of {} files differ:
{}", failures.len(), files.len(), failures.join("
"));
}

/// Other people's files, which stay out of the repository: set ETAPIX_MPP_EXTRA to a folder of
/// `.mpp` files, each next to the `.xml` Project saved of it, and run with `--ignored`.
#[test]
#[ignore]
fn native_reader_matches_extra_files() {
    let dir = PathBuf::from(std::env::var("ETAPIX_MPP_EXTRA").expect("set ETAPIX_MPP_EXTRA"));
    let mut files = Vec::new();
    pairs(&dir, &mut files);
    let failures: Vec<String> = files.iter().filter_map(|f| difference(f, &f.with_extension("xml")).map(|d| format!("{}: {d}", f.display()))).collect();
    assert!(failures.is_empty(), "{} of {} files differ:
{}", failures.len(), files.len(), failures.join("
"));
}
