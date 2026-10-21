//! Critical path method: forward pass, backward pass, slack and criticality.
//!
//! Dates are absolute instants ([`At`]); each task counts its duration and slack in the working
//! time of its own [`Timeline`] (see [`Calendars`]). Observed rules this module encodes:
//! - A task without predecessors starts at the project start, snapped to its calendar's next
//!   working minute (an elapsed task starts at the exact project start time); a task with
//!   predecessors is placed purely by its links (a start-to-finish link can put it before the
//!   project start). Finish-to-finish and start-to-finish links pin the finish to the exact
//!   instant the link asks for.
//! - Working-time lags count on the project calendar; elapsed lags add real time.
//! - Constraints beat links (Project's default "honor constraints"): must-start/finish-on and
//!   start/finish-no-later-than can pull a task before its predecessors allow. Total slack is
//!   the working time from `max(start the links ask for, start)` to the late start, so such a
//!   conflict shows as negative slack on the task and its predecessors; free slack is never
//!   negative.
//! - Late dates come from a backward pass. Only tasks without successors are tied to the latest
//!   scheduled finish. A late finish snaps back to the end of the task's own working time
//!   (a seven-day task before a Monday successor may finish late on Sunday). Finish-no-later-than
//!   and deadlines cap the late finish; must-start/finish-on pin the late dates; a
//!   start-no-later-than date *becomes* the late start even past the project finish.
//! - Early start ignores the task's own leveling delay but honors leveled predecessors, so after
//!   leveling `early_start <= start` and the delay is `start - early_start` in elapsed time. A
//!   successor's own leveling delay is not slack for its predecessors.
//! - As-late-as-possible tasks run at their late dates.
//! - Summary tasks are not scheduled: links into a summary bind each of its subtasks' starts,
//!   links out of it bind on each subtask's finish, and the summary rolls up its subtasks
//!   (earliest start, latest finish, smallest slack).

use crate::calendar::{At, Calendars, Timeline};
use crate::model::{Constraint, Link, LinkType, Project, Wm};

/// Where leveling put a task: absolute `(start, finish)` stretches, more than one when split.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub segments: Vec<(At, At)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskSchedule {
    pub early_start: At,
    pub start: At,
    pub finish: At,
    pub late_start: At,
    pub late_finish: At,
    /// Working minutes on the task's calendar (elapsed minutes for elapsed tasks).
    pub total_slack: Wm,
    pub free_slack: Wm,
    pub critical: bool,
    /// Elapsed minutes between early start and start.
    pub leveling_delay: i64,
    pub segments: Vec<(At, At)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    DependencyCycle,
    Unsupported(String),
}

/// The links between schedulable (non-summary) tasks, after expanding summary links.
#[derive(Debug, Clone)]
pub struct Network {
    pub preds: Vec<Vec<Link>>,
    /// For each task, `(successor index, link index within the successor's preds)`.
    pub succs: Vec<Vec<(usize, usize)>>,
    /// Non-summary tasks, predecessors first.
    pub order: Vec<usize>,
    /// For each summary task, its non-summary descendants.
    pub leaves_of: Vec<Vec<usize>>,
}

impl Network {
    pub fn build(project: &Project) -> Result<Network, ScheduleError> {
        let n = project.tasks.len();
        let mut leaves_of = vec![Vec::new(); n];
        for i in (0..n).filter(|&i| !project.tasks[i].summary) {
            let mut up = project.tasks[i].parent;
            while let Some(s) = up {
                leaves_of[s].push(i);
                up = project.tasks[s].parent;
            }
        }
        let expand = |i: usize| if project.tasks[i].summary { leaves_of[i].clone() } else { vec![i] };

        let mut preds = vec![Vec::new(); n];
        for (i, task) in project.tasks.iter().enumerate() {
            for link in &task.preds {
                let into_summary = task.summary;
                let from_summary = project.tasks[link.pred].summary;
                let unsupported = (into_summary
                    && matches!(link.kind, LinkType::FinishToFinish | LinkType::StartToFinish))
                    || (from_summary && matches!(link.kind, LinkType::StartToStart | LinkType::StartToFinish));
                if unsupported {
                    return Err(ScheduleError::Unsupported(format!(
                        "{:?} link involving a summary task ({})",
                        link.kind, task.name
                    )));
                }
                for target in expand(i) {
                    for source in expand(link.pred) {
                        preds[target].push(Link { pred: source, ..link.clone() });
                    }
                }
            }
        }

        let mut succs = vec![Vec::new(); n];
        for (s, links) in preds.iter().enumerate() {
            for (k, link) in links.iter().enumerate() {
                succs[link.pred].push((s, k));
            }
        }

        let mut indegree: Vec<usize> = preds.iter().map(Vec::len).collect();
        let mut ready: Vec<usize> =
            (0..n).filter(|&i| !project.tasks[i].summary && indegree[i] == 0).rev().collect();
        let mut order = Vec::new();
        while let Some(i) = ready.pop() {
            order.push(i);
            for &(s, _) in succs[i].iter().rev() {
                indegree[s] -= 1;
                if indegree[s] == 0 {
                    ready.push(s);
                }
            }
        }
        if order.len() != project.tasks.iter().filter(|t| !t.summary).count() {
            return Err(ScheduleError::DependencyCycle);
        }
        Ok(Network { preds, succs, order, leaves_of })
    }
}

/// Where the forward pass puts a task.
#[derive(Debug, Clone, Copy)]
pub struct EarlyStart {
    /// Start honoring links and constraints.
    pub start: At,
    /// Finish implied by `start`, or the exact instant a finish-driven link or constraint asks for.
    pub finish: At,
    /// Start the links alone ask for (the task's own start when it has no predecessors).
    pub link_bound: At,
}

/// Start (and the finish implied by it) for a task that must start at or after `at`.
fn from_start(tl: &Timeline, at: At, duration: Wm) -> (At, At) {
    let start = tl.snap_start(at);
    (start, tl.finish_after(start, duration))
}

/// Start (and exact finish) for a task that must finish at `at`.
fn from_finish(tl: &Timeline, at: At, duration: Wm) -> (At, At) {
    (tl.start_at(tl.w(at) - duration), at)
}

/// Earliest start allowed by links and constraints, given the predecessors' scheduled dates.
pub fn earliest_start(
    project: &Project,
    cals: &Calendars,
    net: &Network,
    task: usize,
    starts: &[At],
    finishes: &[At],
) -> EarlyStart {
    let t = &project.tasks[task];
    let tl = cals.task(task);
    if let Some(actual) = t.progress.actual_start {
        let start = cals.clock.at(actual);
        let finish = match t.progress.actual_finish {
            Some(f) => cals.clock.at(f),
            None => tl.finish_after(start, t.duration),
        };
        return EarlyStart { start, finish, link_bound: start };
    }
    let mut bound: Option<(At, At)> = None;
    for link in &net.preds[task] {
        let (p_start, p_finish) = (starts[link.pred], finishes[link.pred]);
        let lag = |at| cals.lag(at, link.lag, link.lag_elapsed);
        let candidate = match link.kind {
            LinkType::FinishToStart => from_start(tl, lag(p_finish), t.duration),
            LinkType::StartToStart => from_start(tl, lag(p_start), t.duration),
            LinkType::FinishToFinish => from_finish(tl, lag(p_finish), t.duration),
            LinkType::StartToFinish => from_finish(tl, lag(p_start), t.duration),
        };
        if bound.is_none_or(|b| candidate.0 > b.0) {
            bound = Some(candidate);
        }
    }
    let base = bound.unwrap_or_else(|| from_start(tl, cals.project_start, t.duration));
    let at = |d| cals.clock.at(d);
    let later = |a: (At, At), b: (At, At)| if b.0 > a.0 { b } else { a };
    let earlier = |a: (At, At), b: (At, At)| if b.0 < a.0 { b } else { a };
    let (start, finish) = match &t.constraint {
        Constraint::AsSoonAsPossible | Constraint::AsLateAsPossible => base,
        Constraint::StartNoEarlierThan(d) => later(base, from_start(tl, at(*d), t.duration)),
        Constraint::FinishNoEarlierThan(d) => later(base, from_finish(tl, at(*d), t.duration)),
        Constraint::StartNoLaterThan(d) => earlier(base, from_start(tl, at(*d), t.duration)),
        Constraint::FinishNoLaterThan(d) => earlier(base, from_finish(tl, at(*d), t.duration)),
        Constraint::MustStartOn(d) => from_start(tl, at(*d), t.duration),
        Constraint::MustFinishOn(d) => from_finish(tl, at(*d), t.duration),
    };
    EarlyStart { start, finish, link_bound: bound.map_or(start, |b| b.0) }
}

/// Where an unleveled task runs: one stretch, or the done part plus the remaining work from
/// `resume` when progress was rescheduled past a gap.
fn progress_segments(project: &Project, cals: &Calendars, i: usize, es: EarlyStart) -> Vec<(At, At)> {
    let t = &project.tasks[i];
    let tl = cals.task(i);
    let (Some(resume), None) = (t.progress.resume, t.progress.actual_finish) else {
        return vec![(es.start, es.finish)];
    };
    let done = t.duration * t.progress.percent_complete as Wm / 100;
    let stop = tl.finish_after(es.start, done);
    let resume = cals.clock.at(resume);
    if done == 0 || resume <= tl.snap_start(stop) {
        return vec![(es.start, es.finish)];
    }
    vec![(es.start, stop), (resume, tl.finish_after(resume, t.duration - done))]
}

/// Schedules every task. `placements[i]`, when present, fixes where task `i` runs (leveling
/// output); otherwise the task runs at its early start.
pub fn schedule(
    project: &Project,
    cals: &Calendars,
    placements: Option<&[Option<Placement>]>,
) -> Result<Vec<TaskSchedule>, ScheduleError> {
    let net = Network::build(project)?;
    let n = project.tasks.len();
    let mut early = vec![0; n];
    let mut link_bound = vec![0; n];
    let mut segments: Vec<Vec<(At, At)>> = vec![Vec::new(); n];
    let mut starts = vec![0; n];
    let mut finishes = vec![0; n];

    for &i in &net.order {
        if project.tasks[i].constraint == Constraint::AsLateAsPossible && !net.succs[i].is_empty() {
            return Err(ScheduleError::Unsupported(format!(
                "as-late-as-possible task with successors ({})",
                project.tasks[i].name
            )));
        }
        let es = earliest_start(project, cals, &net, i, &starts, &finishes);
        early[i] = es.start;
        link_bound[i] = es.link_bound;
        segments[i] = match placements.and_then(|p| p[i].as_ref()) {
            Some(p) => p.segments.clone(),
            None => progress_segments(project, cals, i, es),
        };
        starts[i] = segments[i][0].0;
        finishes[i] = segments[i][segments[i].len() - 1].1;
    }

    // Backward pass, in each task's working time. A split task keeps its whole span, and a
    // successor's own leveling delay stays with it: predecessors see it as if undelayed.
    let project_finish = net.order.iter().map(|&i| finishes[i]).max().unwrap_or(0);
    let span: Vec<Wm> = (0..n).map(|i| cals.task(i).w(finishes[i]) - cals.task(i).w(starts[i])).collect();
    let delay: Vec<Wm> = (0..n).map(|i| cals.task(i).w(starts[i]) - cals.task(i).w(early[i])).collect();
    // Late dates of a successor as its predecessors see them (without its leveling delay).
    let undelayed = |s: usize, late_start: At, late_finish: At| {
        let tl = cals.task(s);
        (tl.start_at(tl.w(late_start) - delay[s]), tl.finish_at(tl.w(late_finish) - delay[s]))
    };
    let mut late_start = vec![0; n];
    let mut late_finish = vec![0; n];
    for &i in net.order.iter().rev() {
        let task = &project.tasks[i];
        let tl = cals.task(i);
        let w = |at: At| tl.w(at);
        let mut from_succs: Option<Wm> = None;
        for &(s, k) in &net.succs[i] {
            let link = &net.preds[s][k];
            let (s_ls, s_lf) = undelayed(s, late_start[s], late_finish[s]);
            let back = |at| cals.lag(at, -link.lag, link.lag_elapsed);
            let bound = match link.kind {
                LinkType::FinishToStart => w(back(s_ls)),
                LinkType::StartToStart => w(back(s_ls)) + span[i],
                LinkType::FinishToFinish => w(back(s_lf)),
                LinkType::StartToFinish => w(back(s_lf)) + span[i],
            };
            from_succs = Some(from_succs.map_or(bound, |b| b.min(bound)));
        }
        let base = from_succs.unwrap_or_else(|| w(project_finish));
        let at = |d| cals.clock.at(d);
        let mut lf = match &task.constraint {
            Constraint::MustStartOn(d) => w(tl.snap_start(at(*d))) + span[i],
            Constraint::MustFinishOn(d) => w(at(*d)),
            Constraint::StartNoLaterThan(d) => {
                let date = w(at(*d)) + span[i];
                from_succs.map_or(date, |b| b.min(date))
            }
            Constraint::FinishNoLaterThan(d) => base.min(w(at(*d))),
            _ => base,
        };
        if let Some(deadline) = task.deadline {
            lf = lf.min(w(at(deadline)));
        }
        late_finish[i] = tl.finish_at(lf);
        late_start[i] = tl.start_at(lf - span[i]);
        if task.progress.percent_complete >= 100 {
            // Completed work has no float: its late dates are its actual dates.
            late_start[i] = starts[i];
            late_finish[i] = finishes[i];
        }
        if task.constraint == Constraint::AsLateAsPossible && placements.is_none() {
            starts[i] = late_start[i];
            finishes[i] = late_finish[i];
            segments[i] = vec![(starts[i], finishes[i])];
        }
    }

    let mut out: Vec<Option<TaskSchedule>> = vec![None; n];
    for &i in &net.order {
        let tl = cals.task(i);
        let total_slack = tl.w(late_start[i]) - tl.w(link_bound[i].max(starts[i]));
        let mut free_slack = total_slack;
        for &(s, k) in &net.succs[i] {
            let link = &net.preds[s][k];
            let (s_start, s_finish) = undelayed(s, starts[s], finishes[s]);
            let back = |at| cals.lag(at, -link.lag, link.lag_elapsed);
            let gap = match link.kind {
                LinkType::FinishToStart => tl.w(back(s_start)) - tl.w(finishes[i]),
                LinkType::StartToStart => tl.w(back(s_start)) - tl.w(starts[i]),
                LinkType::FinishToFinish => tl.w(back(s_finish)) - tl.w(finishes[i]),
                LinkType::StartToFinish => tl.w(back(s_finish)) - tl.w(starts[i]),
            };
            free_slack = free_slack.min(gap);
        }
        let alap = project.tasks[i].constraint == Constraint::AsLateAsPossible;
        out[i] = Some(TaskSchedule {
            early_start: early[i],
            start: starts[i],
            finish: finishes[i],
            late_start: late_start[i],
            late_finish: late_finish[i],
            total_slack,
            free_slack: free_slack.max(0),
            // Finished work cannot slip: completed tasks are never critical.
            critical: total_slack <= 0 && project.tasks[i].progress.percent_complete < 100,
            leveling_delay: if alap { 0 } else { (starts[i] - early[i]).max(0) },
            segments: segments[i].clone(),
        });
    }
    for s in (0..n).filter(|&s| project.tasks[s].summary) {
        let leaves: Vec<&TaskSchedule> = net.leaves_of[s].iter().map(|&i| out[i].as_ref().unwrap()).collect();
        if leaves.is_empty() {
            return Err(ScheduleError::Unsupported(format!("empty summary task ({})", project.tasks[s].name)));
        }
        let min = |f: fn(&TaskSchedule) -> i64| leaves.iter().map(|t| f(t)).min().unwrap();
        let max = |f: fn(&TaskSchedule) -> i64| leaves.iter().map(|t| f(t)).max().unwrap();
        let (start, finish) = (min(|t| t.start), max(|t| t.finish));
        let total_slack = min(|t| t.total_slack);
        out[s] = Some(TaskSchedule {
            early_start: min(|t| t.early_start),
            start,
            finish,
            late_start: min(|t| t.late_start),
            late_finish: max(|t| t.late_finish),
            total_slack,
            free_slack: min(|t| t.free_slack),
            critical: total_slack <= 0,
            leveling_delay: 0,
            segments: vec![(start, finish)],
        });
    }
    Ok(out.into_iter().map(Option::unwrap).collect())
}
