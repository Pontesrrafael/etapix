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

use crate::calendar::{Calendar, Calendars, Shift};
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

/// Serializes `project` with the dates in `schedule` (from [`crate::cpm::schedule`] or
/// [`crate::leveling::level`]).
pub fn write_project(project: &Project, cals: &Calendars, schedule: &[TaskSchedule], name: &str) -> String {
    let at = |a| dt(cals.clock.dt(a));
    let mut out = String::new();
    let _ = write!(
        out,
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Project xmlns=\"{NS}\">\
         <Name>{}</Name><ScheduleFromStart>1</ScheduleFromStart><StartDate>{}</StartDate>\
         <CalendarUID>{}</CalendarUID><MinutesPerDay>480</MinutesPerDay><MinutesPerWeek>2400</MinutesPerWeek>\
         <DaysPerMonth>20</DaysPerMonth><HonorConstraints>0</HonorConstraints>\
         <NewTasksEffortDriven>0</NewTasksEffortDriven><SplitsInProgressTasks>1</SplitsInProgressTasks>\
         <NewTasksAreManual>0</NewTasksAreManual>\
         <ProjectExternallyEdited>0</ProjectExternallyEdited>",
        esc(name),
        dt(project.start),
        project.calendar + 1
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
            "<Task><UID>{}</UID><ID>{}</ID><Name>{}</Name><Manual>0</Manual><Type>{task_type}</Type>\
             <OutlineLevel>{level}</OutlineLevel><Priority>{}</Priority>\
             <Start>{}</Start><Finish>{}</Finish><Duration>{}</Duration><DurationFormat>{}</DurationFormat>\
             {}{}<EffortDriven>{}</EffortDriven><Summary>{}</Summary>\
             <PercentComplete>{}</PercentComplete>{}{}{}\
             <ConstraintType>{ctype}</ConstraintType><CalendarUID>{}</CalendarUID>{}{}\
             <LevelingCanSplit>{}</LevelingCanSplit><IgnoreResourceCalendar>{}</IgnoreResourceCalendar>",
            t.uid,
            t.id,
            esc(&t.name),
            t.priority,
            at(s.start),
            at(s.finish),
            duration(t.duration),
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
        let work = duration(a.work().round() as Wm);
        let _ = write!(
            out,
            "<Assignment><UID>{}</UID><TaskUID>{}</TaskUID><ResourceUID>{}</ResourceUID>\
             <Finish>{}</Finish><RemainingWork>{work}</RemainingWork><Start>{}</Start>\
             <Units>{}</Units><Work>{work}</Work></Assignment>",
            k + 1,
            project.tasks[a.task].uid,
            project.resources[a.resource].uid,
            at(tl.finish_at(first + a.span())),
            at(tl.start_at(first)),
            a.units,
        );
    }
    out.push_str("</Assignments></Project>\n");
    out
}
