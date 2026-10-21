//! Baselines, variances and earned value (golden batch 18).
//!
//! - Saving a baseline snapshots each task's start, finish, duration, work and cost.
//! - Start and finish variances are the working time (task calendar) from the baseline date to
//!   the current one; work variance is current minus baseline work.
//! - Earned value at the status date: BCWS (planned value) is the baseline cost times the share
//!   of the baseline's working time already past; BCWP (earned value) is the baseline cost times
//!   percent complete; ACWP (actual cost) is the cost of the work done. CV = BCWP - ACWP,
//!   SV = BCWP - BCWS. Summaries sum their subtasks.

use crate::calendar::Calendars;
use crate::cost::Costs;
use crate::cpm::TaskSchedule;
use crate::model::{Baseline, Project, ResourceKind, Wm};

/// Snapshots every task as its baseline.
pub fn save_baseline(project: &mut Project, cals: &Calendars, schedule: &[TaskSchedule], costs: &Costs) {
    let work: Vec<f64> = (0..project.tasks.len()).map(|i| task_work(project, i)).collect();
    for (i, t) in project.tasks.iter_mut().enumerate() {
        let tl = cals.task(i);
        t.baseline = Some(Baseline {
            start: cals.clock.dt(schedule[i].start),
            finish: cals.clock.dt(schedule[i].finish),
            duration: tl.w(schedule[i].finish) - tl.w(schedule[i].start),
            work: work[i],
            cost: costs.task[i],
        });
    }
}

/// Minutes of work on a task's work-resource assignments (material quantities are not work).
fn task_work(project: &Project, task: usize) -> f64 {
    project
        .assignments
        .iter()
        .filter(|a| a.task == task && project.resources[a.resource].kind == ResourceKind::Work)
        .map(|a| a.work())
        .sum()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tracking {
    /// Working minutes; positive means later than the baseline.
    pub start_variance: Wm,
    pub finish_variance: Wm,
    /// Minutes.
    pub work_variance: f64,
    pub bcws: f64,
    pub bcwp: f64,
    pub acwp: f64,
}

impl Tracking {
    pub fn cv(&self) -> f64 {
        self.bcwp - self.acwp
    }
    pub fn sv(&self) -> f64 {
        self.bcwp - self.bcws
    }
}

pub fn tracking(project: &Project, cals: &Calendars, schedule: &[TaskSchedule], costs: &Costs) -> Vec<Tracking> {
    let n = project.tasks.len();
    let status = project.status_date.map(|d| cals.clock.at(d));
    let mut out: Vec<Tracking> = (0..n)
        .map(|i| {
            let t = &project.tasks[i];
            let tl = cals.task(i);
            let work: f64 = task_work(project, i);
            let Some(b) = &t.baseline else {
                // Without a baseline Project compares work against zero; dates show no variance.
                return Tracking { start_variance: 0, finish_variance: 0, work_variance: work, bcws: 0.0, bcwp: 0.0, acwp: 0.0 };
            };
            let (bs, bf) = (cals.clock.at(b.start), cals.clock.at(b.finish));
            let pct = t.progress.percent_complete as f64 / 100.0;
            let bcws = match status {
                Some(sd) if !t.summary => {
                    let total = (tl.w(bf) - tl.w(bs)).max(1) as f64;
                    let past = (tl.w(sd.min(bf)) - tl.w(bs)).max(0) as f64;
                    b.cost * (past / total).min(1.0)
                }
                _ => 0.0,
            };
            let (bcwp, acwp) = if status.is_some() && !t.summary { (b.cost * pct, costs.task[i] * pct) } else { (0.0, 0.0) };
            Tracking {
                start_variance: tl.w(schedule[i].start) - tl.w(bs),
                finish_variance: tl.w(schedule[i].finish) - tl.w(bf),
                work_variance: work - b.work,
                bcws,
                bcwp,
                acwp,
            }
        })
        .collect();
    // Summaries: earned value and work sum their subtasks.
    let leaves = out.clone();
    for (i, t) in project.tasks.iter().enumerate() {
        if t.summary {
            continue;
        }
        let mut up = t.parent;
        while let Some(s) = up {
            out[s].bcws += leaves[i].bcws;
            out[s].bcwp += leaves[i].bcwp;
            out[s].acwp += leaves[i].acwp;
            out[s].work_variance += leaves[i].work_variance;
            up = project.tasks[s].parent;
        }
    }
    out
}
