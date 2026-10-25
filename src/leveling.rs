//! Resource leveling as serial list scheduling.
//!
//! Units are placed one at a time: each assignment when leveling individual assignments
//! (Project's default), each whole task otherwise. Among the units whose task's predecessors are
//! placed, the next one is the one on the lowest resource, then by the leveling order: Project
//! levels resource by resource (golden batch 15: with whole tasks, a two-resource task fixed by
//! the R1 pass pushed a critical R2-only task; with individual assignments only the R2 part of
//! the two-resource task moved). A unit goes to the earliest point at or after its early start
//! where its resources have room (or, when it may split, its work fills the earliest gaps).
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
    /// Level each assignment on its own (Project's default) instead of whole tasks.
    pub individual_assignments: bool,
    /// Only tasks whose early start falls in this range move; the rest stay as booked.
    pub range: Option<(chrono::NaiveDateTime, chrono::NaiveDateTime)>,
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
        LevelingOptions {
            order: LevelOrder::Standard,
            within_slack: false,
            basis: PeriodBasis::Day,
            individual_assignments: true,
            range: None,
        }
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
    let net = Network::build(project)?;
    // Bookings per resource, in absolute time.
    let mut booked = vec![Usage::default(); project.resources.len()];
    let mut placed: Vec<Option<Placement>> = vec![None; n];
    let mut starts = vec![0; n];
    let mut finishes = vec![0; n];

    // Units: (task, assignment indices).
    let mut units: Vec<(usize, Vec<usize>)> = Vec::new();
    for &i in &net.order {
        let asgs: Vec<usize> = (0..project.assignments.len()).filter(|&k| project.assignments[k].task == i).collect();
        if options.individual_assignments && asgs.len() > 1 {
            units.extend(asgs.into_iter().map(|k| (i, vec![k])));
        } else {
            units.push((i, asgs));
        }
    }
    let mut units_of = vec![0usize; n];
    for (i, _) in &units {
        units_of[*i] += 1;
    }
    let mut left_units = units_of.clone();

    let key = |u: usize| {
        let (i, asgs) = &units[u];
        let t = &project.tasks[*i];
        let s = &unleveled[*i];
        let fixed = t.priority >= PRIORITY_DO_NOT_LEVEL;
        let resource = asgs.iter().map(|&k| project.assignments[k].resource).min().unwrap_or(0);
        let standard = (s.total_slack, s.early_start);
        match options.order {
            LevelOrder::Id => (!fixed, resource, 0, (0, 0), 0, t.id),
            LevelOrder::Standard => (!fixed, resource, 0, standard, -(t.priority as i64), t.id),
            LevelOrder::PriorityStandard => (!fixed, resource, -(t.priority as i64), standard, 0, t.id),
        }
    };
    let pred_placed = |placed: &[Option<Placement>], l: &crate::model::Link| {
        if project.tasks[l.pred].summary {
            net.leaves_of[l.pred].iter().all(|&x| placed[x].is_some())
        } else {
            placed[l.pred].is_some()
        }
    };

    let mut early: Vec<Option<cpm::EarlyStart>> = vec![None; n];
    // Placed stretches per task, in the task's working minutes.
    let mut extent: Vec<Vec<(Wm, Wm)>> = vec![Vec::new(); n];
    let mut done = vec![false; units.len()];
    for _ in 0..units.len() {
        let u = (0..units.len())
            .filter(|&u| !done[u])
            .filter(|&u| net.preds[units[u].0].iter().all(|l| pred_placed(&placed, l)))
            .min_by_key(|&u| key(u))
            .ok_or(ScheduleError::DependencyCycle)?;
        done[u] = true;
        let i = units[u].0;
        let task = &project.tasks[i];
        let early_i = *early[i].get_or_insert_with(|| cpm::earliest_start(project, cals, &net, i, &starts, &finishes));
        let assignments: Vec<&Assignment> = units[u].1.iter().map(|&k| &project.assignments[k]).collect();
        // Search in the task's own working time: bookings made on other calendars are seen
        // through this task's timeline.
        let tl = cals.task(i);
        let es = tl.w(early_i.start);
        let usage: Vec<Usage> = booked
            .iter()
            .map(|b| Usage(b.0.iter().map(|&(s, f, r)| (tl.w(s), tl.w(f), r)).filter(|b| b.1 > b.0).collect()))
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
        // A one-assignment unit splits on its own; a whole task only when every assignment runs
        // the whole task evenly. `base` is where the unit's work starts relative to the task.
        let single = assignments.len() == 1;
        let span = if single { assignments[0].span() } else { task.duration };
        let base = if single { assignments[0].offset } else { 0 };
        let uniform = assignments
            .iter()
            .all(|a| a.profile.len() == 1 && (single || (a.offset == 0 && a.span() == task.duration)));

        let in_range = options.range.is_none_or(|(a, b)| {
            let es = cals.clock.dt(unleveled[i].early_start);
            a <= es && es <= b
        });
        let unmoved = task.priority >= PRIORITY_DO_NOT_LEVEL
            || assignments.is_empty()
            || task.progress.actual_start.is_some()
            || !in_range;
        let mut split = false;
        let segments: Vec<(Wm, Wm)> = if unmoved {
            vec![(es + base, es + base + span)]
        } else if task.leveling_can_split && uniform {
            split = true;
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
            fill_gaps(es + base, span, &usage, &fits, &next_event)
        } else {
            // `t` is the task start the unit's assignments hang from (their offsets apply).
            let mut t = es;
            let fits = |t: Wm, usage: &[Usage]| if bounds.is_empty() { fits_at(t, usage) } else { fits_periods(t, usage) };
            while !fits(t, &usage) {
                // No later booking to wait for: the unit alone exceeds max units; leave it.
                let Some(e) = next_event(t + base, &usage) else { break };
                t = e - base;
            }
            vec![(t + base, t + base + span)]
        };
        let late_start = tl.w(unleveled[i].late_start);
        let segments = if !unmoved && options.within_slack && segments[0].0 - base > late_start {
            split = false;
            let t = es.max(late_start) + base;
            vec![(t, t + span)]
        } else {
            segments
        };

        for a in &assignments {
            let stretches = if split {
                segments.iter().map(|&(s, f)| (s, f, a.profile[0].rate)).collect()
            } else {
                bookings(a, segments[0].0 - base)
            };
            booked[a.resource].0.extend(stretches.into_iter().map(|(s, f, r)| (tl.start_at(s), tl.finish_at(f), r)));
        }
        extent[i].extend(segments.iter().copied());
        left_units[i] -= 1;
        if left_units[i] == 0 {
            // A task whose assignments place no work (a milestone, say) keeps its own dates.
            let segments: Vec<(At, At)> = if assignments.is_empty() || extent[i].is_empty() || (unmoved && units_of[i] == 1) {
                vec![(early_i.start, early_i.finish)]
            } else if units_of[i] == 1 && base == 0 {
                extent[i].iter().map(|&(s, f)| (tl.start_at(s), tl.finish_at(f))).collect()
            } else {
                // Assignments placed separately: the task runs from its earliest assignment
                // start to its latest assignment finish.
                let s = extent[i].iter().map(|e| e.0).min().unwrap();
                let f = extent[i].iter().map(|e| e.1).max().unwrap();
                vec![(tl.start_at(s), tl.finish_at(f))]
            };
            starts[i] = segments[0].0;
            finishes[i] = segments[segments.len() - 1].1;
            placed[i] = Some(Placement { segments });
        }
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
