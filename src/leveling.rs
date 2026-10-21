//! Resource leveling as serial list scheduling.
//!
//! Tasks are placed one at a time. Among the tasks whose predecessors are already placed, the
//! leveling order picks the next one; it goes to the earliest point at or after its early start
//! where every assigned resource has spare units for its whole duration (or, when it may split,
//! its work fills the earliest free gaps).
//!
//! Observed orderings (golden batch 2):
//! - [`LevelOrder::Id`]: lower ID wins; links, slack and priority do not matter.
//! - [`LevelOrder::Standard`]: less total slack wins; priority only breaks ties after slack and
//!   early start (a priority-900 task with slack still lost to a priority-500 critical one).
//! - [`LevelOrder::PriorityStandard`]: higher priority wins, then the Standard rules.
//! - Priority 1000 never moves, under every order.
//! - Leveling within slack caps the delay at the task's late start and leaves any remaining
//!   overallocation unresolved.

use chrono::Datelike;

use crate::calendar::{At, Calendars};
use crate::cpm::{self, Network, Placement, ScheduleError, TaskSchedule};
use crate::model::{Assignment, Project, Wm, PRIORITY_DO_NOT_LEVEL};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelOrder {
    Id,
    Standard,
    PriorityStandard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelingOptions {
    pub order: LevelOrder,
    pub within_slack: bool,
    pub basis: PeriodBasis,
}

/// How overallocation is judged. Day and week compare each period's booked work with the
/// resource's capacity in that period, so two half-day tasks may overlap within a day
/// (observed, golden batch 11); minute and hour require spare units at every moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodBasis {
    Minute,
    Hour,
    Day,
    Week,
}

impl Default for LevelingOptions {
    fn default() -> Self {
        LevelingOptions { order: LevelOrder::Standard, within_slack: false, basis: PeriodBasis::Day }
    }
}

/// Period boundaries (midnights, or Sunday midnights) in `tl`'s working minutes.
fn period_bounds(cals: &Calendars, tl: &crate::calendar::Timeline, basis: PeriodBasis) -> Vec<Wm> {
    let step = match basis {
        PeriodBasis::Week => 7,
        _ => 1,
    };
    let start = cals.clock.dt(cals.project_start).date();
    let first = match basis {
        PeriodBasis::Week => start - chrono::Duration::days(start.weekday().num_days_from_sunday() as i64),
        _ => start,
    } - chrono::Duration::days(364);
    let mut out: Vec<Wm> = (0..(2000 / step))
        .map(|k| tl.w(cals.clock.at((first + chrono::Duration::days(k * step)).and_hms_opt(0, 0, 0).unwrap())))
        .collect();
    out.dedup();
    out
}

const EPS: f64 = 1e-9;

/// Booked units on one resource, as `(start, finish, units)` intervals (absolute instants, or
/// one task's working minutes while that task is being placed).
#[derive(Debug, Default, Clone)]
struct Usage(Vec<(Wm, Wm, f64)>);

impl Usage {
    fn load_at(&self, t: Wm) -> f64 {
        self.0.iter().filter(|&&(s, f, _)| s <= t && t < f).map(|&(_, _, u)| u).sum()
    }

    /// Highest load anywhere in `[s, f)`.
    fn peak(&self, s: Wm, f: Wm) -> f64 {
        let points = std::iter::once(s).chain(self.0.iter().map(|&(a, _, _)| a).filter(|&a| s < a && a < f));
        points.map(|t| self.load_at(t)).fold(0.0, f64::max)
    }

    /// Booked work (minutes x units) inside `[lo, hi)`.
    fn load(&self, lo: Wm, hi: Wm) -> f64 {
        self.0.iter().map(|&(s, f, u)| (f.min(hi) - s.max(lo)).max(0) as f64 * u).sum()
    }

    /// First interval boundary strictly after `t`.
    fn next_event(&self, t: Wm) -> Option<Wm> {
        self.0.iter().flat_map(|&(a, b, _)| [a, b]).filter(|&x| x > t).min()
    }
}

pub fn level(
    project: &Project,
    cals: &Calendars,
    options: LevelingOptions,
) -> Result<Vec<TaskSchedule>, ScheduleError> {
    let n = project.tasks.len();
    let unleveled = cpm::schedule(project, cals, None)?;
    // Bookings per resource, in absolute time.
    let mut booked = vec![Usage::default(); project.resources.len()];
    let mut placed: Vec<Option<Placement>> = vec![None; n];
    let mut starts = vec![0; n];
    let mut finishes = vec![0; n];

    let key = |i: usize| {
        let t = &project.tasks[i];
        let s = &unleveled[i];
        let fixed = t.priority >= PRIORITY_DO_NOT_LEVEL;
        let standard = (s.total_slack, s.early_start);
        match options.order {
            LevelOrder::Id => (!fixed, 0, (0, 0), 0, t.id),
            LevelOrder::Standard => (!fixed, 0, standard, -(t.priority as i64), t.id),
            LevelOrder::PriorityStandard => (!fixed, -(t.priority as i64), standard, 0, t.id),
        }
    };

    let net = Network::build(project)?;
    for _ in 0..net.order.len() {
        let next = net
            .order
            .iter()
            .copied()
            .filter(|&i| placed[i].is_none())
            .filter(|&i| net.preds[i].iter().all(|l| placed[l.pred].is_some()))
            .min_by_key(|&i| key(i))
            .ok_or(ScheduleError::DependencyCycle)?;
        let task = &project.tasks[next];
        let early = cpm::earliest_start(project, cals, &net, next, &starts, &finishes);
        let assignments: Vec<_> = project.assignments.iter().filter(|a| a.task == next).collect();
        // Search in the task's own working time: bookings made on other calendars are seen
        // through this task's timeline.
        let tl = cals.task(next);
        let es = tl.w(early.start);
        let usage: Vec<Usage> = booked
            .iter()
            .map(|u| Usage(u.0.iter().map(|&(s, f, r)| (tl.w(s), tl.w(f), r)).filter(|b| b.1 > b.0).collect()))
            .collect();

        let free = |a: &Assignment, s: Wm, f: Wm, rate: f64, usage: &[Usage]| {
            usage[a.resource].peak(s, f) + rate <= project.resources[a.resource].max_units + EPS
        };
        // Each assignment occupies its resource only while it works (observed: a resource that
        // finishes early on a task is free for others before the task ends).
        let fits_at = |t: Wm, usage: &[Usage]| {
            assignments.iter().all(|a| bookings(a, t).iter().all(|&(s, f, r)| free(a, s, f, r, usage)))
        };
        let bounds = match options.basis {
            PeriodBasis::Day | PeriodBasis::Week => period_bounds(cals, tl, options.basis),
            PeriodBasis::Minute | PeriodBasis::Hour => Vec::new(),
        };
        // Day/week basis: each period's booked work must fit the period's capacity.
        let fits_periods = |t: Wm, usage: &[Usage]| {
            assignments.iter().all(|a| {
                let max = project.resources[a.resource].max_units;
                bookings(a, t).iter().all(|&(s, f, r)| {
                    let k0 = bounds.partition_point(|&b| b <= s).saturating_sub(1);
                    bounds[k0..].windows(2).take_while(|w| w[0] < f).all(|w| {
                        let mine = (f.min(w[1]) - s.max(w[0])).max(0) as f64 * r;
                        mine == 0.0 || usage[a.resource].load(w[0], w[1]) + mine <= (w[1] - w[0]) as f64 * max + EPS
                    })
                })
            })
        };
        let next_event = |t: Wm, usage: &[Usage]| {
            let booked = assignments.iter().filter_map(|a| usage[a.resource].next_event(t)).min();
            let period = bounds.iter().copied().find(|&b| b > t);
            match (booked, period) {
                (Some(x), Some(y)) => Some(x.min(y)),
                (x, y) => x.or(y),
            }
        };
        // Splitting is modeled only for tasks whose assignments all run the whole task evenly.
        let uniform = assignments
            .iter()
            .all(|a| a.offset == 0 && a.profile.len() == 1 && a.span() == task.duration);

        let mut split = false;
        let unmoved = task.priority >= PRIORITY_DO_NOT_LEVEL
            || assignments.is_empty()
            || task.progress.actual_start.is_some();
        let segments = if unmoved {
            vec![(es, es + task.duration)]
        } else if task.leveling_can_split && uniform {
            split = true;
            // Stretches never cross a period bound (`next_event` stops there), so on a day/week
            // basis each stretch is checked against its own period's capacity.
            let fits = |s: Wm, f: Wm, usage: &[Usage]| {
                assignments.iter().all(|a| {
                    let rate = a.profile[0].rate;
                    if bounds.is_empty() {
                        return free(a, s, f, rate, usage);
                    }
                    let k = bounds.partition_point(|&b| b <= s).saturating_sub(1);
                    let (lo, hi) = (bounds[k], bounds.get(k + 1).copied().unwrap_or(f));
                    let cap = (hi - lo) as f64 * project.resources[a.resource].max_units;
                    usage[a.resource].load(lo, hi) + (f - s) as f64 * rate <= cap + EPS
                })
            };
            fill_gaps(es, task.duration, &usage, &fits, &next_event)
        } else {
            let mut t = es;
            let fits = |t: Wm, usage: &[Usage]| if bounds.is_empty() { fits_at(t, usage) } else { fits_periods(t, usage) };
            while !fits(t, &usage) {
                // No later booking to wait for: the task alone exceeds max units; leave it.
                let Some(e) = next_event(t, &usage) else { break };
                t = e;
            }
            vec![(t, t + task.duration)]
        };
        let late_start = tl.w(unleveled[next].late_start);
        let segments = if options.within_slack && segments[0].0 > late_start {
            split = false;
            let t = es.max(late_start);
            vec![(t, t + task.duration)]
        } else {
            segments
        };

        for a in &assignments {
            let stretches = if split {
                segments.iter().map(|&(s, f)| (s, f, a.profile[0].rate)).collect()
            } else {
                bookings(a, segments[0].0)
            };
            booked[a.resource].0.extend(stretches.into_iter().map(|(s, f, r)| (tl.start_at(s), tl.finish_at(f), r)));
        }
        let segments: Vec<(At, At)> = if unmoved {
            vec![(early.start, early.finish)]
        } else {
            segments.iter().map(|&(s, f)| (tl.start_at(s), tl.finish_at(f))).collect()
        };
        starts[next] = segments[0].0;
        finishes[next] = segments[segments.len() - 1].1;
        placed[next] = Some(Placement { segments });
    }

    cpm::schedule(project, cals, Some(&placed))
}

/// The `(start, finish, rate)` stretches an assignment occupies when its task starts at `t`.
fn bookings(a: &Assignment, t: Wm) -> Vec<(Wm, Wm, f64)> {
    let mut at = t + a.offset;
    a.profile
        .iter()
        .map(|p| {
            let b = (at, at + p.len, p.rate);
            at += p.len;
            b
        })
        .collect()
}

/// Puts `work` minutes into the earliest stretches from `from` on where the task fits,
/// merging adjacent stretches.
fn fill_gaps(
    from: Wm,
    work: Wm,
    usage: &[Usage],
    fits: &dyn Fn(Wm, Wm, &[Usage]) -> bool,
    next_event: &dyn Fn(Wm, &[Usage]) -> Option<Wm>,
) -> Vec<(Wm, Wm)> {
    let mut segments: Vec<(Wm, Wm)> = Vec::new();
    let (mut t, mut left) = (from, work);
    while left > 0 {
        let event = next_event(t, usage);
        let until = event.map_or(t + left, |e| e.min(t + left));
        // With no later booking the remaining work can never fit better: book it here.
        if fits(t, until, usage) || event.is_none() {
            match segments.last_mut() {
                Some(last) if last.1 == t => last.1 = until,
                _ => segments.push((t, until)),
            }
            left -= until - t;
        }
        t = until;
    }
    segments
}
