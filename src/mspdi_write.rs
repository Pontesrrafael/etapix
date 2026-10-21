//! Writer for Microsoft Project XML (MSPDI).
//!
//! Writes the model (calendars, tasks, links, resources, assignments, progress) plus the dates
//! pjsched computed. Project recalculates on open, so the round-trip test (`tests/roundtrip.rs`)
//! checks that what Project computes from this file matches pjsched.
//!
//! The file says `ProjectExternallyEdited = 0`: otherwise Project treats it as edited outside
//! Project, rebuilds tasks from their assignments, drops their links and pins every task with
//! a start-no-earlier-than constraint (observed in the round trip).
//!
//! `HonorConstraints = 0` is what Project itself writes with "tasks always honor their
//! constraint dates" on (the behavior every golden shows); writing 1 made Project let links win.
//!
//! Every calendar is written as a base calendar with its full week and exceptions; a resource
//! gets its own derived calendar on top of its base, as Project expects.

use std::fmt::Write;

use chrono::NaiveDateTime;

use crate::calendar::{At, Calendar, Calendars, Shift};
use crate::cpm::TaskSchedule;
use crate::mspdi::NS;
use crate::model::{Constraint, LinkType, Project, TaskType, Wm};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn dt(d: NaiveDateTime) -> String {
    d.format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn duration(minutes: Wm) -> String {
    let (sign, m) = if minutes < 0 { ("-", -minutes) } else { ("", minutes) };
    format!("{sign}PT{}H{}M0S", m / 60, m % 60)
}

fn hm(minute_of_day: u32) -> String {
    // MSPDI writes the end of the day as 00:00:00.
    let m = minute_of_day % (24 * 60);
    format!("{:02}:{:02}:00", m / 60, m % 60)
}

fn working_times(out: &mut String, shifts: &[Shift]) {
    if shifts.is_empty() {
        return;
    }
    out.push_str("<WorkingTimes>");
    for &(a, b) in shifts {
        let _ = write!(out, "<WorkingTime><FromTime>{}</FromTime><ToTime>{}</ToTime></WorkingTime>", hm(a), hm(b));
    }
    out.push_str("</WorkingTimes>");
}

fn base_calendar(out: &mut String, uid: usize, cal: &Calendar) {
    let _ = write!(out, "<Calendar><UID>{uid}</UID><Name>{}</Name><IsBaseCalendar>1</IsBaseCalendar>", esc(&cal.name));
    out.push_str("<BaseCalendarUID>-1</BaseCalendarUID><WeekDays>");
    for (day, shifts) in cal.weekdays.iter().enumerate() {
        let working = u8::from(!shifts.is_empty());
        let _ = write!(out, "<WeekDay><DayType>{}</DayType><DayWorking>{working}</DayWorking>", day + 1);
        working_times(out, shifts);
        out.push_str("</WeekDay>");
    }
    out.push_str("</WeekDays>");
    if !cal.exceptions.is_empty() {
        out.push_str("<Exceptions>");
        for e in &cal.exceptions {
            let working = u8::from(!e.shifts.is_empty());
            let _ = write!(
                out,
                "<Exception><EnteredByOccurrences>0</EnteredByOccurrences><TimePeriod><FromDate>{}T00:00:00</FromDate>\
                 <ToDate>{}T23:59:00</ToDate></TimePeriod><Occurrences>1</Occurrences><Name/><Type>1</Type>\
                 <DayWorking>{working}</DayWorking>",
                e.from, e.to
            );
            working_times(out, &e.shifts);
            out.push_str("</Exception>");
        }
        out.push_str("</Exceptions>");
    }
    out.push_str("</Calendar>");
}

fn link_code(kind: LinkType) -> u8 {
    match kind {
        LinkType::FinishToFinish => 0,
        LinkType::FinishToStart => 1,
        LinkType::StartToFinish => 2,
        LinkType::StartToStart => 3,
    }
}

fn constraint(c: &Constraint) -> (u8, Option<NaiveDateTime>) {
    match *c {
        Constraint::AsSoonAsPossible => (0, None),
        Constraint::AsLateAsPossible => (1, None),
        Constraint::MustStartOn(d) => (2, Some(d)),
        Constraint::MustFinishOn(d) => (3, Some(d)),
        Constraint::StartNoEarlierThan(d) => (4, Some(d)),
        Constraint::StartNoLaterThan(d) => (5, Some(d)),
        Constraint::FinishNoEarlierThan(d) => (6, Some(d)),
        Constraint::FinishNoLaterThan(d) => (7, Some(d)),
    }
}

/// Per-day `(start, finish, working minutes)` stretches of `[a, b)` on task `i`'s timeline, the
/// way Project chunks timephased data: a full day runs to the next working day's start, the last
/// chunk ends at `b`.
fn days(cals: &Calendars, i: usize, a: At, b: At) -> Vec<(At, At, Wm)> {
    let tl = cals.task(i);
    let mut out = Vec::new();
    let mut at = tl.snap_start(a);
    while at < b {
        let midnight = cals.clock.at(cals.clock.dt(at).date().succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap());
        let next = tl.snap_start(midnight).min(b);
        let minutes = tl.w(next.min(midnight)) - tl.w(at);
        if minutes > 0 {
            out.push((at, next, minutes));
        }
        at = next;
    }
    out
}

/// Writes one `<TimephasedData>` record.
fn timephased(out: &mut String, cals: &Calendars, kind: u8, uid: usize, (a, b): (At, At), unit: u8, value: &str) {
    let at = |x| dt(cals.clock.dt(x));
    let _ = write!(
        out,
        "<TimephasedData><Type>{kind}</Type><UID>{uid}</UID><Start>{}</Start><Finish>{}</Finish>         <Unit>{unit}</Unit><Value>{value}</Value></TimephasedData>",
        at(a),
        at(b)
    );
}

/// End of the done part of a task in progress (`None` when nothing is done).
fn stop(cals: &Calendars, i: usize, dur: Wm, s: &TaskSchedule, pct: u32) -> Option<At> {
    let done = dur * pct as Wm / 100;
    (done > 0).then(|| cals.task(i).finish_after(s.start, done))
}

/// Task-level timephased progress: per day of done work, the percent of the duration done.
fn task_progress(out: &mut String, cals: &Calendars, i: usize, uid: usize, dur: Wm, s: &TaskSchedule, pct: u32) {
    if let Some(stop) = stop(cals, i, dur, s, pct) {
        for (a, b, m) in days(cals, i, s.start, stop) {
            timephased(out, cals, 11, uid, (a, b), 2, &(m * 100 / dur).to_string());
        }
    }
}

/// Stop/Resume and per-day actual (type 2) and remaining (type 1) work of a task in progress,
/// for one of its assignments working at `rate`.
fn progress_work(cals: &Calendars, uid: usize, i: usize, dur: Wm, s: &TaskSchedule, pct: u32, rate: f64) -> (String, String) {
    let at = |x| dt(cals.clock.dt(x));
    let split = s.segments.len() > 1;
    let dates = if split {
        format!("<Stop>{}</Stop><Resume>{}</Resume>", at(s.segments[0].1), at(s.segments[1].0))
    } else {
        String::new()
    };
    let mut tp = String::new();
    if let Some(stop) = stop(cals, i, dur, s, pct) {
        let work = |m: Wm| duration((m as f64 * rate).round() as Wm);
        for (a, b, m) in days(cals, i, s.start, stop) {
            timephased(&mut tp, cals, 2, uid, (a, b), 1, &work(m));
        }
        let rest = if split { s.segments[1].0 } else { stop };
        for (a, b, m) in days(cals, i, rest, s.finish) {
            timephased(&mut tp, cals, 1, uid, (a, b), 1, &work(m));
        }
    }
    (dates, tp)
}

/// The unassigned placeholder assignment (resource -65535) Project keeps for a task without
/// resources. It carries the actual and remaining work per day; without it Project re-spreads a
/// rescheduled split and finishes a day late (found by bisecting the round trip).
#[allow(clippy::too_many_arguments)]
fn placeholder_assignment(out: &mut String, cals: &Calendars, uid: usize, i: usize, task_uid: i64, dur: Wm, s: &TaskSchedule, pct: u32) {
    let at = |x| dt(cals.clock.dt(x));
    let done = dur * pct as Wm / 100;
    let (dates, tp) = progress_work(cals, uid, i, dur, s, pct, 1.0);
    let _ = write!(
        out,
        "<Assignment><UID>{uid}</UID><TaskUID>{task_uid}</TaskUID><ResourceUID>-65535</ResourceUID>         <PercentWorkComplete>{pct}</PercentWorkComplete><ActualStart>{}</ActualStart>         <ActualWork>{}</ActualWork><Finish>{}</Finish><RemainingWork>{}</RemainingWork>         <Start>{}</Start>{dates}<Units>1</Units><Work>{}</Work>{tp}</Assignment>",
        at(s.start),
        duration(done),
        at(s.finish),
        duration(dur - done),
        at(s.start),
        duration(dur),
    );
}

/// Serializes `project` with the dates in `schedule` (from [`crate::cpm::schedule`] or
/// [`crate::leveling::level`]).
pub fn write_project(project: &Project, cals: &Calendars, schedule: &[TaskSchedule], name: &str) -> String {
    let at = |a| dt(cals.clock.dt(a));
    let mut out = String::new();
    let _ = write!(
        out,
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Project xmlns=\"{NS}\">\
         <Name>{}</Name><ScheduleFromStart>1</ScheduleFromStart><StartDate>{}</StartDate>\
         <CriticalSlackLimit>{}</CriticalSlackLimit>\
         <CalendarUID>{}</CalendarUID><MinutesPerDay>480</MinutesPerDay><MinutesPerWeek>2400</MinutesPerWeek>\
         <DaysPerMonth>20</DaysPerMonth><HonorConstraints>0</HonorConstraints><MultipleCriticalPaths>{}</MultipleCriticalPaths>\
         <NewTasksEffortDriven>0</NewTasksEffortDriven><SplitsInProgressTasks>1</SplitsInProgressTasks>\
         <NewTasksAreManual>0</NewTasksAreManual>\
         <ProjectExternallyEdited>0</ProjectExternallyEdited>",
        esc(name),
        dt(project.start),
        project.critical_slack_limit,
        project.calendar + 1,
        u8::from(project.multiple_critical_paths)
    );

    // Calendars: base calendars get UID index + 1; resource calendars follow.
    out.push_str("<Calendars>");
    for (i, cal) in project.calendars.iter().enumerate() {
        base_calendar(&mut out, i + 1, cal);
    }
    let first_resource_cal = project.calendars.len() + 1;
    for (r, res) in project.resources.iter().enumerate() {
        let base = res.calendar.unwrap_or(project.calendar) + 1;
        let _ = write!(
            out,
            "<Calendar><UID>{}</UID><Name>{}</Name><IsBaseCalendar>0</IsBaseCalendar>\
             <BaseCalendarUID>{base}</BaseCalendarUID></Calendar>",
            first_resource_cal + r,
            esc(&res.name)
        );
    }
    out.push_str("</Calendars>");

    out.push_str("<Tasks>");
    for (i, t) in project.tasks.iter().enumerate() {
        let s = &schedule[i];
        let level = {
            let (mut l, mut up) = (1, t.parent);
            while let Some(p) = up {
                l += 1;
                up = project.tasks[p].parent;
            }
            l
        };
        let task_type = match t.task_type {
            TaskType::FixedUnits => 0,
            TaskType::FixedDuration => 1,
            TaskType::FixedWork => 2,
        };
        let (ctype, cdate) = constraint(&t.constraint);
        let p = &t.progress;
        let opt = |tag: &str, d: Option<NaiveDateTime>| d.map_or(String::new(), |d| format!("<{tag}>{}</{tag}>", dt(d)));
        // Element order follows the MSPDI schema: Project silently drops out-of-order elements
        // (and everything after them, such as links).
        let _ = write!(
            out,
            "<Task><UID>{}</UID><ID>{}</ID><Name>{}</Name><Manual>{}</Manual><Type>{task_type}</Type>\
             <OutlineLevel>{level}</OutlineLevel><Priority>{}</Priority>\
             <Start>{}</Start><Finish>{}</Finish><Duration>{}</Duration>{}<DurationFormat>{}</DurationFormat>\
             {}{}<EffortDriven>{}</EffortDriven><Summary>{}</Summary>\
             <PercentComplete>{}</PercentComplete>{}{}{}\
             <ConstraintType>{ctype}</ConstraintType><CalendarUID>{}</CalendarUID>{}{}\
             <LevelingCanSplit>{}</LevelingCanSplit><LevelingDelay>{}</LevelingDelay><LevelingDelayFormat>8</LevelingDelayFormat><IgnoreResourceCalendar>{}</IgnoreResourceCalendar>",
            t.uid,
            t.id,
            esc(&t.name),
            u8::from(t.manual.is_some()),
            t.priority,
            at(s.start),
            at(s.finish),
            duration(t.duration),
            t.manual.map_or(String::new(), |_| format!(
                "<ManualStart>{}</ManualStart><ManualFinish>{}</ManualFinish><ManualDuration>{}</ManualDuration>",
                at(s.start),
                at(s.finish),
                duration(t.duration)
            )),
            if t.elapsed { 8 } else { 7 },
            // A split from rescheduled progress needs Stop next to Resume.
            if s.segments.len() > 1 { format!("<Stop>{}</Stop>", at(s.segments[0].1)) } else { String::new() },
            // ResumeValid makes Project restart remaining work at Resume: only for a real split.
            p.resume.map_or(String::new(), |d| {
                let valid = if s.segments.len() > 1 { "<ResumeValid>1</ResumeValid>" } else { "" };
                format!("<Resume>{}</Resume>{valid}", dt(d))
            }),
            u8::from(t.effort_driven),
            u8::from(t.summary),
            p.percent_complete,
            opt("ActualStart", p.actual_start),
            opt("ActualFinish", p.actual_finish),
            if p.actual_start.is_some() {
                let done = t.duration * p.percent_complete as Wm / 100;
                format!(
                    "<ActualDuration>{}</ActualDuration><RemainingDuration>{}</RemainingDuration>",
                    duration(done),
                    duration(t.duration - done)
                )
            } else {
                String::new()
            },
            t.calendar.map_or(-1, |c| c as i64 + 1),
            opt("ConstraintDate", cdate),
            opt("Deadline", t.deadline),
            u8::from(t.leveling_can_split),
            s.leveling_delay * 10,
            u8::from(t.ignore_resource_calendar),
        );
        for link in &t.preds {
            let _ = write!(
                out,
                "<PredecessorLink><PredecessorUID>{}</PredecessorUID><Type>{}</Type><CrossProject>0</CrossProject>\
                 <LinkLag>{}</LinkLag><LagFormat>{}</LagFormat></PredecessorLink>",
                project.tasks[link.pred].uid,
                link_code(link.kind),
                link.lag * 10,
                if link.lag_elapsed { 8 } else { 7 }
            );
        }
        if p.actual_start.is_some() && !t.summary && t.duration > 0 {
            task_progress(&mut out, cals, i, t.uid as usize, t.duration, s, p.percent_complete);
        }
        out.push_str("</Task>");
    }
    out.push_str("</Tasks>");

    out.push_str("<Resources>");
    for (r, res) in project.resources.iter().enumerate() {
        let _ = write!(
            out,
            "<Resource><UID>{}</UID><ID>{}</ID><Name>{}</Name><Type>1</Type><MaxUnits>{:.2}</MaxUnits>\
             <CalendarUID>{}</CalendarUID></Resource>",
            res.uid,
            r + 1,
            esc(&res.name),
            res.max_units,
            first_resource_cal + r
        );
    }
    out.push_str("</Resources>");

    out.push_str("<Assignments>");
    for (k, a) in project.assignments.iter().enumerate() {
        // Dates let Project keep an assignment stretched past work / units (fixed work).
        let tl = cals.task(a.task);
        let first = tl.w(schedule[a.task].start) + a.offset;
        let total = a.work().round() as Wm;
        // Progress on a resourced task lives on its assignments: without the actual start and
        // actual work there, Project reopens the task as not started.
        let p = &project.tasks[a.task].progress;
        let actual = total * p.percent_complete as Wm / 100;
        let (dates, tp) = if p.actual_start.is_some() {
            let t = &project.tasks[a.task];
            progress_work(cals, k + 1, a.task, t.duration, &schedule[a.task], p.percent_complete, a.units)
        } else {
            (String::new(), String::new())
        };
        let started = if p.actual_start.is_some() {
            format!(
                "<PercentWorkComplete>{}</PercentWorkComplete><ActualStart>{}</ActualStart><ActualWork>{}</ActualWork>",
                p.percent_complete,
                at(schedule[a.task].start),
                duration(actual)
            )
        } else {
            String::new()
        };
        let _ = write!(
            out,
            "<Assignment><UID>{}</UID><TaskUID>{}</TaskUID><ResourceUID>{}</ResourceUID>{started}             <Finish>{}</Finish><RemainingWork>{}</RemainingWork><Start>{}</Start>{dates}             <Units>{}</Units><Work>{}</Work>{tp}</Assignment>",
            k + 1,
            project.tasks[a.task].uid,
            project.resources[a.resource].uid,
            at(cals.assignment(k).finish_after(tl.start_at(first), a.span())),
            duration(total - actual),
            at(tl.start_at(first)),
            a.units,
            duration(total),
        );
    }
    let mut uid = project.assignments.len();
    for (i, t) in project.tasks.iter().enumerate() {
        let has_resource = project.assignments.iter().any(|a| a.task == i);
        if t.progress.actual_start.is_some() && !t.summary && !has_resource && t.duration > 0 {
            uid += 1;
            placeholder_assignment(&mut out, cals, uid, i, t.uid, t.duration, &schedule[i], t.progress.percent_complete);
        }
    }
    out.push_str("</Assignments></Project>\n");
    out
}
