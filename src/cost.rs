//! Costs, as observed in golden batch 17 (MSPDI stores money in cents).
//!
//! - A work resource costs its standard rate for regular work and its overtime rate for
//!   overtime work, plus its cost per use once per assignment. When the rate changes over time
//!   (cost rate table A), each stretch of work is charged at the rate in force then.
//! - A material resource costs its rate per unit times the quantity (MSPDI writes the quantity
//!   as work hours: 10 sacks = `PT10H`).
//! - A task costs its assignments plus its fixed cost; a summary sums its subtasks. Fixed-cost
//!   accrual (start, prorated, end) only shifts cost in time, not the total.

use crate::calendar::Calendars;
use crate::cpm::TaskSchedule;
use crate::model::{Project, ResourceKind};

#[derive(Debug, Clone, PartialEq)]
pub struct Costs {
    /// Per task (summaries included), in currency units.
    pub task: Vec<f64>,
    /// Per assignment.
    pub assignment: Vec<f64>,
    /// Per resource.
    pub resource: Vec<f64>,
}

pub fn costs(project: &Project, cals: &Calendars, schedule: &[TaskSchedule]) -> Costs {
    let mut assignment = Vec::with_capacity(project.assignments.len());
    for (k, a) in project.assignments.iter().enumerate() {
        let res = &project.resources[a.resource];
        let start = schedule[a.task].start;
        let rate_at = |at| res.rate_at(cals.clock.dt(at));
        let first = rate_at(start);
        let cost = match res.kind {
            ResourceKind::Material => a.work() / 60.0 * first.standard,
            ResourceKind::Work => {
                let tl = cals.assignment(k);
                let task_tl = cals.task(a.task);
                let begin = task_tl.start_at(task_tl.w(start) + a.offset);
                let total = a.work();
                let regular_share = if total > 0.0 { (total - a.overtime_work as f64) / total } else { 0.0 };
                // Walk the profile, splitting each stretch at rate changes.
                let mut cost = 0.0;
                let mut w = tl.w(begin);
                for piece in &a.profile {
                    let (mut from, to) = (w, w + piece.len);
                    while from < to {
                        let at = tl.start_at(from);
                        let next_change = res
                            .rates
                            .iter()
                            .map(|r| tl.w(cals.clock.at(r.from)))
                            .filter(|&c| c > from && c < to)
                            .min()
                            .unwrap_or(to);
                        let minutes = (next_change - from) as f64 * piece.rate * regular_share;
                        cost += minutes / 60.0 * rate_at(at).standard;
                        from = next_change;
                    }
                    w = to;
                }
                cost + a.overtime_work as f64 / 60.0 * first.overtime + first.per_use
            }
            ResourceKind::Cost => 0.0,
        };
        assignment.push(cost);
    }

    let n = project.tasks.len();
    let mut task = vec![0.0; n];
    for (k, a) in project.assignments.iter().enumerate() {
        task[a.task] += assignment[k];
    }
    for (i, t) in project.tasks.iter().enumerate() {
        task[i] += t.fixed_cost;
    }
    // Roll leaves up into every ancestor summary.
    let leaf_costs = task.clone();
    for (i, t) in project.tasks.iter().enumerate() {
        if t.summary {
            continue;
        }
        let mut up = t.parent;
        while let Some(s) = up {
            task[s] += leaf_costs[i];
            up = project.tasks[s].parent;
        }
    }

    let mut resource = vec![0.0; project.resources.len()];
    for (k, a) in project.assignments.iter().enumerate() {
        resource[a.resource] += assignment[k];
    }
    Costs { task, assignment, resource }
}
