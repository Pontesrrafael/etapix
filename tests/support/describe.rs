//! A project as comparable text: what two readings of the same file must agree on.

use pjsched::calendar::Calendars;
use pjsched::cpm;
use pjsched::model::Project;

/// What must agree, as text, so a mismatch shows exactly where.
pub fn describe(p: &Project) -> Vec<String> {
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
        out.push(format!("assignment task {} res {} units {} offset {} work {:.1} cost {}", a.task, a.resource, a.units, a.offset, a.work(), a.cost));
    }
    out
}

pub fn schedule(p: &Project) -> Vec<(i64, i64)> {
    let cals = Calendars::new(p).unwrap();
    cpm::schedule(p, &cals, None).unwrap().iter().map(|s| (s.start, s.finish)).collect()
}

