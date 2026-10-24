//! Calendars and the mapping between absolute time and working time.
//!
//! Scheduling uses absolute instants ([`At`], minutes on a fixed clock) so tasks on different
//! calendars meet at real moments: a standard-calendar successor starts at 10:00 when its
//! half-day predecessor finishes at 10:00. Durations are counted in each task's own working time
//! ([`Wm`]), through a [`Timeline`]: the working calendar a task runs on, unrolled over a fixed
//! window, or elapsed (24/7) time.
//!
//! A [`Timeline`] maps an instant to the working minutes before it (`w`), and back, either as a
//! *start* (the beginning of the next working minute, Mon 09:00) or as a *finish* (the end of
//! the previous one, Fri 18:00).

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::model::{Project, Wm};

/// Minutes since the clock's epoch.
pub type At = i64;

/// Minutes from midnight, `[from, to)`.
pub type Shift = (u32, u32);

/// Dated working time that replaces the weekly pattern; no shifts = non-working.
#[derive(Debug, Clone, PartialEq)]
pub struct Exception {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub shifts: Vec<Shift>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Calendar {
    pub name: String,
    /// Indexed by `chrono::Weekday::num_days_from_sunday()`.
    pub weekdays: [Vec<Shift>; 7],
    /// Later entries win when ranges overlap (a resource's own exceptions follow its base's).
    pub exceptions: Vec<Exception>,
    /// The calendar's UID in the file it was read from, when it came from one: saving into that
    /// file again writes over that calendar.
    pub source_uid: Option<i64>,
}

impl Calendar {
    /// Mon-Fri 09:00-12:00, 13:00-18:00: the default observed in the pt-BR installation.
    pub fn standard() -> Self {
        let day = vec![(9 * 60, 12 * 60), (13 * 60, 18 * 60)];
        Calendar {
            name: "Standard".into(),
            weekdays: [vec![], day.clone(), day.clone(), day.clone(), day.clone(), day, vec![]],
            exceptions: vec![],
            source_uid: None,
        }
    }

    pub fn shifts(&self, date: NaiveDate) -> &[Shift] {
        match self.exceptions.iter().rev().find(|e| e.from <= date && date <= e.to) {
            Some(e) => &e.shifts,
            None => &self.weekdays[date.weekday().num_days_from_sunday() as usize],
        }
    }
}

fn intersect(a: &[Shift], b: &[Shift]) -> Vec<Shift> {
    let mut out = Vec::new();
    for &(a0, a1) in a {
        for &(b0, b1) in b {
            let (s, f) = (a0.max(b0), a1.min(b1));
            if s < f {
                out.push((s, f));
            }
        }
    }
    out.sort_unstable();
    out
}

const DAYS_BEFORE: i64 = 400;
const DAYS_AFTER: i64 = 2000;

/// Converts between datetimes and [`At`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    epoch: NaiveDate,
}

impl Clock {
    pub fn new(project_start: NaiveDateTime) -> Self {
        Clock { epoch: project_start.date() - Duration::days(DAYS_BEFORE) }
    }

    pub fn at(&self, dt: NaiveDateTime) -> At {
        (dt - self.epoch.and_hms_opt(0, 0, 0).unwrap()).num_minutes()
    }

    pub fn dt(&self, at: At) -> NaiveDateTime {
        self.epoch.and_hms_opt(0, 0, 0).unwrap() + Duration::minutes(at)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Interval {
    start: At,
    end: At,
    /// Working minutes in all earlier intervals.
    before: Wm,
}

impl Interval {
    fn after(&self) -> Wm {
        self.before + (self.end - self.start)
    }
}

#[derive(Debug, Clone)]
pub enum Timeline {
    /// Working shifts over the window, in order.
    Working(Vec<Interval>),
    /// Every minute counts (elapsed durations).
    Elapsed,
}

impl Timeline {
    /// The working time common to all `calendars`.
    pub fn working(calendars: &[&Calendar], clock: &Clock) -> Self {
        let mut intervals = Vec::new();
        let mut before = 0;
        for day in 0..DAYS_BEFORE + DAYS_AFTER {
            let date = clock.epoch + Duration::days(day);
            let mut shifts = calendars[0].shifts(date).to_vec();
            for c in &calendars[1..] {
                shifts = intersect(&shifts, c.shifts(date));
            }
            for (a, b) in shifts {
                let start = day * 1440 + a as At;
                let end = day * 1440 + b as At;
                intervals.push(Interval { start, end, before });
                before += end - start;
            }
        }
        Timeline::Working(intervals)
    }

    /// Working minutes before `at`.
    pub fn w(&self, at: At) -> Wm {
        match self {
            Timeline::Elapsed => at,
            Timeline::Working(iv) => match iv.partition_point(|i| i.start <= at) {
                0 => 0,
                k => {
                    let i = &iv[k - 1];
                    i.before + (at.min(i.end) - i.start)
                }
            },
        }
    }

    /// The instant where working minute `w` begins.
    pub fn start_at(&self, w: Wm) -> At {
        match self {
            Timeline::Elapsed => w,
            Timeline::Working(iv) => {
                let i = &iv[iv.partition_point(|i| i.after() <= w).min(iv.len() - 1)];
                i.start + (w - i.before)
            }
        }
    }

    /// The instant where working minute `w - 1` ends.
    pub fn finish_at(&self, w: Wm) -> At {
        match self {
            Timeline::Elapsed => w,
            Timeline::Working(iv) => {
                let i = &iv[iv.partition_point(|i| i.after() < w).min(iv.len() - 1)];
                i.start + (w - i.before)
            }
        }
    }

    /// The earliest start at or after `at`.
    pub fn snap_start(&self, at: At) -> At {
        self.start_at(self.w(at))
    }

    /// The instant `minutes` of work after the start `at` (a finish).
    pub fn finish_after(&self, at: At, minutes: Wm) -> At {
        if minutes == 0 {
            return self.snap_start(at);
        }
        self.finish_at(self.w(at) + minutes)
    }
}

/// Every timeline a project needs: the project calendar, elapsed time, and the calendar each
/// task runs on.
#[derive(Debug, Clone)]
pub struct Calendars {
    pub clock: Clock,
    pub project_start: At,
    timelines: Vec<Timeline>,
    project: usize,
    task: Vec<usize>,
    assignment: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarError(pub String);

impl Calendars {
    /// A task runs on elapsed time when its duration is elapsed; otherwise on its resources'
    /// calendar (intersected with its own calendar when it has one), unless it ignores resource
    /// calendars or has no resources; otherwise on its own calendar or the project's.
    pub fn new(project: &Project) -> Result<Self, CalendarError> {
        let clock = Clock::new(project.start);
        let mut timelines = vec![Timeline::Elapsed];
        let mut index: HashMap<Vec<usize>, usize> = HashMap::new();
        let mut get = |cals: Vec<usize>, timelines: &mut Vec<Timeline>| {
            *index.entry(cals.clone()).or_insert_with(|| {
                let refs: Vec<&Calendar> = cals.iter().map(|&c| &project.calendars[c]).collect();
                timelines.push(Timeline::working(&refs, &clock));
                timelines.len() - 1
            })
        };
        // Every resource has its own calendar in MSPDI; identical working time is one calendar.
        let canonical = |c: usize| {
            let same = |a: &Calendar, b: &Calendar| a.weekdays == b.weekdays && a.exceptions == b.exceptions;
            (0..project.calendars.len()).find(|&j| same(&project.calendars[j], &project.calendars[c])).unwrap()
        };
        let project_tl = get(vec![canonical(project.calendar)], &mut timelines);
        let mut task = Vec::with_capacity(project.tasks.len());
        for (i, t) in project.tasks.iter().enumerate() {
            if t.elapsed {
                task.push(0);
                continue;
            }
            let mut cals: Vec<usize> = t.calendar.map(canonical).into_iter().collect();
            if !t.ignore_resource_calendar {
                let mut res: Vec<usize> = project
                    .assignments
                    .iter()
                    .filter(|a| a.task == i)
                    .map(|a| canonical(project.resources[a.resource].calendar.unwrap_or(project.calendar)))
                    .collect();
                res.sort_unstable();
                res.dedup();
                // Resources on different calendars each work their own hours (see
                // `assignment`); the task itself then counts on its own or the project calendar.
                if res.len() == 1 {
                    cals.extend(res);
                }
            }
            if cals.is_empty() {
                cals.push(canonical(project.calendar));
            }
            cals.sort_unstable();
            cals.dedup();
            task.push(get(cals, &mut timelines));
        }
        let mut assignment = Vec::with_capacity(project.assignments.len());
        for a in &project.assignments {
            let t = &project.tasks[a.task];
            let i = if t.elapsed || t.ignore_resource_calendar {
                task[a.task]
            } else {
                let mut cals: Vec<usize> = t.calendar.map(canonical).into_iter().collect();
                cals.push(canonical(project.resources[a.resource].calendar.unwrap_or(project.calendar)));
                cals.sort_unstable();
                cals.dedup();
                get(cals, &mut timelines)
            };
            assignment.push(i);
        }
        Ok(Calendars { clock, project_start: clock.at(project.start), timelines, project: project_tl, task, assignment })
    }

    pub fn project(&self) -> &Timeline {
        &self.timelines[self.project]
    }

    pub fn elapsed(&self) -> &Timeline {
        &self.timelines[0]
    }

    pub fn task(&self, i: usize) -> &Timeline {
        &self.timelines[self.task[i]]
    }

    /// The timeline assignment `k` works on: its resource's calendar (with the task's own), or
    /// the task's timeline for elapsed tasks and tasks that ignore resource calendars.
    pub fn assignment(&self, k: usize) -> &Timeline {
        &self.timelines[self.assignment[k]]
    }

    /// Applies a link lag to `at`: elapsed lags add real time, working lags count on the project
    /// calendar and land on the end of the last lag minute.
    pub fn lag(&self, at: At, lag: Wm, elapsed: bool) -> At {
        match (lag, elapsed) {
            (0, _) => at,
            (_, true) => at + lag,
            (_, false) => self.project().finish_at(self.project().w(at) + lag),
        }
    }

    /// Takes a link lag back off a successor's late date `at` (backward pass): the result is
    /// the start of the working minute the lag reaches, so a predecessor on another calendar
    /// can finish late right up to it (a seven-day task before a 1-day lag ending Monday 09:00
    /// may finish late on Sunday).
    pub fn lag_back(&self, at: At, lag: Wm, elapsed: bool) -> At {
        match (lag, elapsed) {
            (0, _) => at,
            (_, true) => at - lag,
            (_, false) => self.project().start_at(self.project().w(at) - lag),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").unwrap()
    }

    fn standard() -> (Clock, Timeline) {
        let clock = Clock::new(dt("2026-11-02T08:00"));
        (clock, Timeline::working(&[&Calendar::standard()], &clock))
    }

    #[test]
    fn start_snaps_forward_and_finish_lands_on_shift_end() {
        let (clock, tl) = standard();
        let monday = clock.at(dt("2026-11-02T08:00"));
        assert_eq!(clock.dt(tl.snap_start(monday)), dt("2026-11-02T09:00"));
        assert_eq!(clock.dt(tl.finish_after(monday, 180)), dt("2026-11-02T12:00"));
        assert_eq!(clock.dt(tl.finish_after(monday, 5 * 480)), dt("2026-11-06T18:00"));
        let friday_end = clock.at(dt("2026-11-06T18:00"));
        assert_eq!(clock.dt(tl.snap_start(friday_end)), dt("2026-11-09T09:00"));
    }

    #[test]
    fn intersection_keeps_common_hours() {
        let clock = Clock::new(dt("2026-11-02T08:00"));
        let mut mornings = Calendar::standard();
        mornings.weekdays.iter_mut().for_each(|d| d.retain(|&(a, _)| a < 12 * 60));
        let tl = Timeline::working(&[&Calendar::standard(), &mornings], &clock);
        let monday = clock.at(dt("2026-11-02T09:00"));
        assert_eq!(clock.dt(tl.finish_after(monday, 4 * 60)), dt("2026-11-03T10:00"));
    }
}
