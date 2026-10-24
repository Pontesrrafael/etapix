//! Saving into an existing `.mpp` without Project: the edits Etapix made to a project opened from
//! a `.mpp` are written into that file's own tables, and everything Etapix does not model
//! (views, formats, custom fields, Project's caches) stays as Project left it.
//!
//! The original file is read natively, both versions are compared task by task, and only the
//! fields that changed are written, plus the dates Etapix scheduled. Changes to the structure
//! (tasks, links or assignments added or removed, outline, resources, calendars, project
//! options) are not written yet: [`update_mpp`] refuses them, so the caller can save another way.

use std::collections::HashMap;

use chrono::{NaiveDateTime, Timelike};
use roxmltree::Document;

use super::*;
use crate::calendar::Calendar;
use crate::model::{Project, Task};
use crate::mspdi::{parse_datetime, parse_duration, read_project};
use crate::mspdi_write::{constraint, link_code};

// Keys of the per-table counters in the project's `Props`, plus the table's number.
const ROW_COUNT: u32 = 0x0100_0000;
const VAR_SIZE: u32 = 0x0001_0000;
/// Where a FixedMeta item holds the number of variable values of its record.
pub(super) const ITEM_VAR_COUNT: usize = 2;

/// Where each table's records hold the UID their variable values are filed under.
fn uid_offset(table: &str) -> Option<usize> {
    match table {
        "TBkndTask" => Some(T_UID),
        "TBkndRsc" => Some(R_UID),
        "TBkndAssn" => Some(A_UID),
        "TBkndCal" => Some(8),
        _ => None,
    }
}

/// Bytes Project writes for an empty ("NA") date.
const NA: [u8; 4] = [0xFF; 4];
/// Remaining duration (tenths of a minute): Project takes the duration as done + remaining.
const T_REMAINING_DURATION: usize = 88;
/// Actual duration (tenths of a minute).
const T_ACTUAL_DURATION: usize = 80;
/// Where the done work stops (the resume date is `T_RESUME`), and the actual start of a task in
/// progress.
const T_STOP: usize = 136;
const T_STARTED_AT: usize = 174;
/// Progress flags in the FixedMeta item: some work done; not in progress (clear while started
/// and unfinished).
const F_SOME_WORK_DONE: (usize, u8) = (44, 3);
const F_NOT_IN_PROGRESS: (usize, u8) = (18, 5);
// Flags in a task's FixedMeta item: (byte, bit).
pub(super) const F_SUMMARY: (usize, u8) = (12, 3);
/// Manually scheduled, in the Fixed2Meta item.
const F2_MANUAL: (usize, u8) = (8, 7);
const F_MILESTONE: (usize, u8) = (10, 1);
const F_EFFORT_DRIVEN: (usize, u8) = (13, 3);
const F_LEVELING_CAN_SPLIT: (usize, u8) = (16, 1);
const F_IGNORE_RESOURCE_CALENDAR: (usize, u8) = (17, 5);
// Link record layout.
const L_PREDECESSOR: usize = 4;
const L_SUCCESSOR: usize = 8;
const L_TYPE: usize = 12;
const L_LAG: usize = 14;
const L_LAG_FORMAT: usize = 18;
// Assignment record fields the reader does not need. Work is in thousandths of a minute.
const A_REGULAR_WORK: usize = 36;
const A_REMAINING_WORK: usize = 44;
const A_RESUME: usize = 60;
const A_STOP: usize = 104;
/// Work contours: how the remaining work, and the done work, spread over time. Project rebuilds
/// the task from them.
const AV_CONTOUR: u16 = 0x31;
const AV_DONE_CONTOUR: u16 = 0x32;
const ASSIGNMENT_VAR_FLAGS: u16 = 0x0F40;
/// Entry flags of a task's variable data.
const TASK_VAR_FLAGS: u16 = 0x0B40;
/// The baseline's duration format, as Project stores it with a baseline.
const TV_BASELINE_FORMAT: u16 = 0xB3;
const BASELINE_FORMAT: [u8; 2] = [0x35, 0];
/// The item bits Project sets on a task, and on an assignment, that have a baseline.
const T_BASELINE_BITS: [(usize, u8); 8] = [(20, 1), (20, 2), (21, 0), (21, 1), (21, 2), (21, 3), (21, 4), (27, 4)];
const A_BASELINE_BITS: [(usize, u8); 5] = [(13, 7), (14, 0), (14, 6), (14, 7), (32, 6)];
// An assignment's baseline: work, cost, another amount, start, finish.
const AV_BASELINE_WORK: u16 = 0x10;
const AV_BASELINE_COST: u16 = 0x20;
const AV_BASELINE_OTHER: u16 = 0x10B;
const AV_BASELINE_START: u16 = 0x92;
const AV_BASELINE_FINISH: u16 = 0x93;
/// The done-work contour: count, segment length and the offset of a closing copy in the header,
/// one segment from 8, the copy (work, rate, span) at 36.
const DONE_CONTOUR_LEN: usize = 56;
const DONE_SEGMENT_LEN: u16 = 0x18;
const DONE_TAIL: usize = 36;
const A_ACTUAL_WORK: usize = 28;
/// An assignment with work done carries both flags in its FixedMeta item.
const A_WORK_DONE_FLAGS: [(usize, u8); 2] = [(10, 3), (14, 2)];
const C_FIRST: usize = 8;
// A uniform contour of one segment (72 bytes): rate (units × 160000) at 8, work (thousandths of a
// minute) at 16, span (working minutes × 80) at 24, segment count at 32.
const C_SEGMENTS: usize = 32;
const UNIFORM_CONTOUR_LEN: usize = 72;
/// With work done, Project adds a second closing copy and 8 zero bytes.
const PROGRESS_CONTOUR_LEN: usize = 108;
/// The closing copy (work, rate, span) of a remaining-work contour.
const C_TAIL: usize = 44;
/// The resource of the assignment Project gives a task without resources.
pub(super) const PLACEHOLDER_RESOURCE: u32 = -65535i32 as u32;
// MSPDI duration formats: 7 = days, 8 = elapsed days, 5 = hours.
const FORMAT_ELAPSED_DAYS: u8 = 8;
const FORMAT_DAYS: u8 = 7;
/// Elapsed minutes, hours, days, weeks, months and percent.
const ELAPSED_FORMATS: [u16; 6] = [4, 6, 8, 10, 12, 20];

/// `original` with the project of `xml` (Project XML, as Etapix writes it) saved into it.
pub fn update_mpp(original: &[u8], xml: &str) -> Result<Vec<u8>> {
    let mut file = cfb::Compound::parse(original).map_err(|e| MppError(e.0))?;
    let before = read_project(&mpp_to_mspdi(original)?).map_err(|e| MppError(e.0))?;
    let after = read_project(xml).map_err(|e| MppError(e.0))?;
    same_structure(&before, &after)?;
    let dates = Dates::parse(xml)?;

    let mut tasks = Store::load(&file, "TBkndTask");
    for (old, new) in before.tasks.iter().zip(&after.tasks) {
        let at = tasks.find(T_UID, new.uid as u32).ok_or_else(|| MppError(format!("tarefa {} não encontrada no .mpp", new.uid)))?;
        let progress = dates.assignments.iter().find(|a| a.task == new.uid as u32).map(|a| (a.stop, a.resume));
        write_task(&mut tasks, at, old, new, dates.tasks.get(&new.uid), progress.unwrap_or_default());
    }
    tasks.save(&mut file)?;

    let mut links = Store::load(&file, "TBkndCons");
    for t in &after.tasks {
        for l in &t.preds {
            let pred = after.tasks[l.pred].uid as u32;
            let at = links.find_link(pred, t.uid as u32).ok_or_else(|| MppError("vínculo não encontrado no .mpp".into()))?;
            let r = links.record(at);
            put_u16(r, L_TYPE, link_code(l.kind) as u16);
            put_u32(r, L_LAG, (l.lag * 10) as u32);
            if ELAPSED_FORMATS.contains(&u16_at(r, L_LAG_FORMAT)) != l.lag_elapsed {
                put_u16(r, L_LAG_FORMAT, if l.lag_elapsed { FORMAT_ELAPSED_DAYS } else { FORMAT_DAYS } as u16);
            }
        }
    }
    links.save(&mut file)?;

    let mut assignments = Store::load(&file, "TBkndAssn");
    let pairs: HashMap<i64, (&Task, &Task)> = before.tasks.iter().zip(&after.tasks).map(|(o, n)| (n.uid, (o, n))).collect();
    let keeps = |uid: i64| pairs.get(&uid).is_some_and(|&(o, n)| keeps_file_dates(o, n));
    for a in &dates.assignments {
        // Etapix's placeholder assignments (no resource) of tracked tasks go to the file's own, below.
        if (a.resource as i32) < 0 || keeps(a.task as i64) {
            continue;
        }
        let at = assignments.find_assignment(a.task, a.resource).ok_or_else(|| MppError("atribuição não encontrada no .mpp".into()))?;
        assignments.write_assignment(at, a)?;
    }
    // Project gives every task without resources a placeholder assignment that carries its work
    // and dates: it rebuilds the task's duration from there.
    for (old, t) in before.tasks.iter().zip(&after.tasks).filter(|(_, t)| !t.summary) {
        let Some(&(start, finish)) = dates.tasks.get(&t.uid).filter(|_| !keeps_file_dates(old, t)) else { continue };
        if let Some(at) = assignments.find_assignment(t.uid as u32, PLACEHOLDER_RESOURCE) {
            let tracked = dates.assignments.iter().find(|a| a.task == t.uid as u32 && a.resource == PLACEHOLDER_RESOURCE);
            let untracked = AssignmentDates::unstarted(t.uid as u32, PLACEHOLDER_RESOURCE, 1.0, t.duration as f64, start, finish);
            assignments.write_assignment(at, tracked.unwrap_or(&untracked))?;
        }
    }
    // A baseline saved on a task is saved on each of its assignments too: Project reads it there.
    for (_, t) in before.tasks.iter().zip(&after.tasks).filter(|(o, t)| o.baseline != t.baseline) {
        let Some(b) = &t.baseline else { continue };
        let rows: Vec<usize> = (0..assignments.rows.len()).filter(|&k| assignments.is_live_assignment_of(k, t.uid as u32)).collect();
        for row in rows {
            write_assignment_baseline(&mut assignments, row, b);
        }
    }
    assignments.save(&mut file)?;
    sync_counts(&mut file)?;
    Ok(file.to_bytes())
}

/// The project's `Props` keeps, per table, its number of records and the size of its variable
/// data; Project fails to open a file where they lag behind the tables.
fn sync_counts(file: &mut cfb::Compound) -> Result<()> {
    const TABLES: [(u32, &str); 5] = [(1, "TBkndTask"), (2, "TBkndRsc"), (3, "TBkndCal"), (4, "TBkndAssn"), (5, "TBkndCons")];
    let path = "/   114/Props";
    let Some(mut props) = file.get(path).map(<[u8]>::to_vec) else { return Ok(()) };
    for (n, table) in TABLES {
        let get = |s: &str| file.get(&format!("/   114/{table}/{s}")).map(<[u8]>::len).unwrap_or(0);
        let rows = file.get(&format!("/   114/{table}/FixedMeta")).map_or(0, |m| u32_at(m, 8));
        let var = get("Var2Data") as u32;
        set_prop(&mut props, ROW_COUNT | n, |v| put_u32(v, 0, rows));
        set_prop(&mut props, VAR_SIZE | n, |v| put_u32(v, 0, var));
    }
    file.set(path, props).map_err(|e| MppError(e.0))
}

/// Changes, in place, every value of entry `key` in a `Props` stream (a key can appear more
/// than once; readers take the last).
pub(super) fn set_prop(props: &mut [u8], key: u32, change: impl Fn(&mut [u8])) {
    let mut o = 16;
    while o + 12 <= props.len() {
        let len = u32_at(props, o) as usize;
        if len > props.len() - o - 12 {
            return;
        }
        if u32_at(props, o + 4) == key && len > 0 {
            change(&mut props[o + 12..o + 12 + len]);
        }
        o += 12 + len;
    }
}



/// Says what Etapix cannot write into the file yet, when the edits include it.
fn same_structure(before: &Project, after: &Project) -> Result<()> {
    let unsupported = |what: &str| err(format!("{what}: essa alteração ainda não pode ser gravada direto no .mpp"));
    let shape = |p: &Project| p.tasks.iter().map(|t| (t.uid, t.id, t.parent, t.summary)).collect::<Vec<_>>();
    if shape(before) != shape(after) {
        return unsupported("tarefas incluídas, excluídas ou movidas na estrutura");
    }
    let links = |p: &Project| p.tasks.iter().map(|t| t.preds.iter().map(|l| p.tasks[l.pred].uid).collect::<Vec<_>>()).collect::<Vec<_>>();
    if links(before) != links(after) {
        return unsupported("vínculos incluídos ou excluídos");
    }
    let assignments = |p: &Project| {
        let mut v: Vec<_> = p.assignments.iter().map(|a| (p.tasks[a.task].uid, p.resources[a.resource].uid)).collect();
        v.sort();
        v
    };
    if assignments(before) != assignments(after) {
        return unsupported("recursos atribuídos ou removidos de tarefas");
    }
    let resources = |p: &Project| {
        p.resources
            .iter()
            .map(|r| format!("{} {} {:?} {:?} {} {:?}", r.uid, r.name, r.kind, r.rates, r.max_units, r.calendar.map(|c| working_time(&p.calendars[c]))))
            .collect::<Vec<_>>()
    };
    if resources(before) != resources(after) {
        return unsupported("recursos alterados");
    }
    if working_time(&before.calendars[before.calendar]) != working_time(&after.calendars[after.calendar]) {
        return unsupported("calendários alterados");
    }
    let task_calendar = |p: &Project, t: &Task| t.calendar.map(|c| working_time(&p.calendars[c]));
    if before.tasks.iter().zip(&after.tasks).any(|(a, b)| task_calendar(before, a) != task_calendar(after, b)) {
        return unsupported("calendário de tarefa alterado");
    }
    let options = |p: &Project| (p.start, p.critical_slack_limit, p.multiple_critical_paths, p.honor_constraints, p.status_date);
    if options(before) != options(after) {
        return unsupported("opções do projeto alteradas");
    }
    Ok(())
}

/// A calendar's working time (names change between saves: Project names a resource's calendar
/// after it).
pub(super) fn working_time(c: &Calendar) -> String {
    format!("{:?} {:?}", c.weekdays, c.exceptions)
}

/// Writes task `new` over the record that holds `old`, field by field where they differ, and the
/// dates Etapix scheduled for it.
fn write_task(tasks: &mut Store, at: usize, old: &Task, new: &Task, dates: Option<&(NaiveDateTime, NaiveDateTime)>, (stop, resume): (Option<NaiveDateTime>, Option<NaiveDateTime>)) {
    if old.name != new.name {
        tasks.set_text(new.uid as u32, TV_NAME, &new.name);
    }
    if old.baseline != new.baseline {
        if let Some(b) = &new.baseline {
            write_baseline(tasks, new.uid as u32, b);
            for bit in T_BASELINE_BITS {
                set_flag(tasks.item(at), bit, true);
            }
        }
    }
    let r = tasks.record(at);
    if (old.duration, old.elapsed) != (new.duration, new.elapsed) {
        put_u32(r, T_DURATION, (new.duration * 10) as u32);
        if old.elapsed != new.elapsed {
            r[T_DURATION_FORMAT] = if new.elapsed { FORMAT_ELAPSED_DAYS } else { FORMAT_DAYS };
        }
    }
    if old.task_type != new.task_type {
        put_u16(r, T_TYPE, task_type_code(new));
    }
    if old.priority != new.priority {
        put_u16(r, T_PRIORITY, new.priority as u16);
    }
    if old.constraint != new.constraint {
        let (kind, date) = constraint(&new.constraint);
        put_u16(r, T_CONSTRAINT, kind as u16);
        put_date(r, T_CONSTRAINT_DATE, date);
    }
    if old.deadline != new.deadline {
        put_date(r, T_DEADLINE, new.deadline);
    }
    if old.fixed_cost != new.fixed_cost {
        put_f64(r, T_FIXED_COST, new.fixed_cost * 100.0);
    }
    if old.fixed_cost_accrual != new.fixed_cost_accrual {
        r[T_FIXED_COST_ACCRUAL] = new.fixed_cost_accrual;
    }
    if old.leveling_delay != new.leveling_delay {
        put_u32(r, T_LEVELING_DELAY, (new.leveling_delay * 10) as u32);
    }
    let (p, q) = (&old.progress, &new.progress);
    if p.percent_complete != q.percent_complete {
        put_u16(r, T_PERCENT, q.percent_complete as u16);
    }
    if (old.duration, p.percent_complete) != (new.duration, q.percent_complete) {
        put_u32(r, T_ACTUAL_DURATION, (new.duration * q.percent_complete.min(100) as i64 / 100 * 10) as u32);
        let remaining = new.duration * (100 - q.percent_complete.min(100) as i64) / 100;
        put_u32(r, T_REMAINING_DURATION, (remaining * 10) as u32);
    }
    if p.actual_start != q.actual_start {
        put_date(r, T_ACTUAL_START, q.actual_start);
    }
    if p.actual_finish != q.actual_finish {
        put_date(r, T_ACTUAL_FINISH, q.actual_finish);
    }
    if p != q {
        write_progress(r, q, stop, resume);
    }
    if let Some(&(start, finish)) = dates.filter(|_| !keeps_file_dates(old, new)) {
        put_date(r, T_START, Some(start));
        put_date(r, T_FINISH, Some(finish));
        // A manual task's own dates live in the second block; NA there makes it automatic.
        let r2 = tasks.second(at);
        if r2.len() >= T2_MANUAL_FINISH + 4 {
            let manual = new.manual.is_some();
            put_date(r2, T2_MANUAL_START, manual.then_some(start));
            put_date(r2, T2_MANUAL_FINISH, manual.then_some(finish));
        }
    }
    let manual = new.manual.is_some();
    set_flag(tasks.item2(at), F2_MANUAL, manual);
    let item = tasks.item(at);
    let q = &new.progress;
    set_flag(item, F_SOME_WORK_DONE, q.percent_complete > 0);
    set_flag(item, F_NOT_IN_PROGRESS, q.actual_start.is_none() || q.actual_finish.is_some());
    set_flag(item, F_MILESTONE, new.duration == 0 && !new.summary);
    set_flag(item, F_EFFORT_DRIVEN, new.effort_driven);
    set_flag(item, F_LEVELING_CAN_SPLIT, new.leveling_can_split);
    set_flag(item, F_IGNORE_RESOURCE_CALENDAR, new.ignore_resource_calendar);
}

/// Where a task's done work stops and its remaining work resumes, as Project keeps them: at the
/// actual start when nothing is done yet, at the actual finish when all is.
fn write_progress(r: &mut [u8], q: &crate::model::Progress, stop: Option<NaiveDateTime>, resume: Option<NaiveDateTime>) {
    let (stop, resume, started) = match (q.actual_start, q.actual_finish) {
        (None, _) => (None, None, None),
        (Some(_), Some(finish)) => (Some(finish), Some(finish), None),
        (Some(start), None) if q.percent_complete == 0 => (Some(start), Some(start), Some(start)),
        (Some(start), None) => (stop.or(Some(start)), resume.or(stop).or(Some(start)), Some(start)),
    };
    put_date(r, T_STOP, stop);
    put_date(r, T_RESUME, resume);
    put_date(r, T_STARTED_AT, started);
}

/// A task's baseline 0, in its variable data: start, finish, duration (tenths of a minute), work
/// (thousandths of a minute) and cost (cents).
fn write_baseline(tasks: &mut Store, uid: u32, b: &crate::model::Baseline) {
    let date = |d: NaiveDateTime| {
        let mut v = vec![0u8; 4];
        put_date(&mut v, 0, Some(d));
        v
    };
    tasks.set_or_add_var(uid, TV_BASELINE_START, TASK_VAR_FLAGS, date(b.start));
    tasks.set_or_add_var(uid, TV_BASELINE_FINISH, TASK_VAR_FLAGS, date(b.finish));
    tasks.set_or_add_var(uid, TV_BASELINE_DURATION, TASK_VAR_FLAGS, ((b.duration * 10) as u32).to_le_bytes().to_vec());
    tasks.set_or_add_var(uid, TV_BASELINE_WORK, TASK_VAR_FLAGS, (b.work * 1000.0).to_le_bytes().to_vec());
    tasks.set_or_add_var(uid, TV_BASELINE_COST, TASK_VAR_FLAGS, (b.cost * 100.0).to_le_bytes().to_vec());
    tasks.set_or_add_var(uid, TV_BASELINE_FORMAT, TASK_VAR_FLAGS, BASELINE_FORMAT.to_vec());
}

/// An assignment's baseline 0: its dates, with work and cost left at 0, as Project saves them for
/// a task's assignments.
fn write_assignment_baseline(assignments: &mut Store, row: usize, b: &crate::model::Baseline) {
    let uid = u32_at(assignments.record(row), A_UID);
    let date = |d: NaiveDateTime| {
        let mut v = vec![0u8; 4];
        put_date(&mut v, 0, Some(d));
        v
    };
    for ty in [AV_BASELINE_WORK, AV_BASELINE_COST, AV_BASELINE_OTHER] {
        assignments.set_or_add_var(uid, ty, ASSIGNMENT_VAR_FLAGS, 0f64.to_le_bytes().to_vec());
    }
    assignments.set_or_add_var(uid, AV_BASELINE_START, ASSIGNMENT_VAR_FLAGS, date(b.start));
    assignments.set_or_add_var(uid, AV_BASELINE_FINISH, ASSIGNMENT_VAR_FLAGS, date(b.finish));
    let item = assignments.item(row);
    for bit in A_BASELINE_BITS {
        set_flag(item, bit, true);
    }
}

/// Tasks whose dates, and assignments, stay as the file has them: a manual task's dates are what
/// the user typed (unless its mode, manual start or duration changed), and a started task's come
/// from its progress, which Etapix does not write yet.
fn keeps_file_dates(old: &Task, new: &Task) -> bool {
    let manual = new.manual.is_some() && old.manual == new.manual;
    let started = new.progress.actual_start.is_some() && old.progress == new.progress;
    (manual || started) && old.duration == new.duration
}

fn task_type_code(t: &Task) -> u16 {
    match t.task_type {
        crate::model::TaskType::FixedUnits => 0,
        crate::model::TaskType::FixedDuration => 1,
        crate::model::TaskType::FixedWork => 2,
    }
}

/// What only the XML has: the dates Etapix scheduled, by task UID, and each assignment's.
struct Dates {
    tasks: HashMap<i64, (NaiveDateTime, NaiveDateTime)>,
    assignments: Vec<AssignmentDates>,
}

struct AssignmentDates {
    task: u32,
    resource: u32,
    units: f64,
    /// Minutes.
    work: f64,
    start: NaiveDateTime,
    finish: NaiveDateTime,
    /// Work done (minutes), and where the done work stops and the remaining work resumes.
    actual: f64,
    stop: Option<NaiveDateTime>,
    resume: Option<NaiveDateTime>,
}

impl AssignmentDates {
    /// An assignment that has not started: all its work remains, from its start.
    fn unstarted(task: u32, resource: u32, units: f64, work: f64, start: NaiveDateTime, finish: NaiveDateTime) -> Self {
        AssignmentDates { task, resource, units, work, start, finish, actual: 0.0, stop: None, resume: None }
    }
}

impl Dates {
    fn parse(xml: &str) -> Result<Self> {
        let doc = Document::parse(xml).map_err(|e| MppError(format!("XML inválido: {e}")))?;
        let child = |n: roxmltree::Node, tag: &str| n.children().find(|c| c.has_tag_name(tag)).and_then(|c| c.text()).unwrap_or_default().to_string();
        let date = |s: String| parse_datetime(&s).map_err(|e| MppError(e.0));
        let mut out = Dates { tasks: HashMap::new(), assignments: Vec::new() };
        for n in doc.descendants() {
            if n.has_tag_name("Task") {
                let uid = child(n, "UID").parse().unwrap_or(-1);
                if let (Ok(s), Ok(f)) = (date(child(n, "Start")), date(child(n, "Finish"))) {
                    out.tasks.insert(uid, (s, f));
                }
            } else if n.has_tag_name("Assignment") {
                // Timephased work: type 2 is done, type 1 remaining.
                let periods = |ty: &'static str, at: &'static str| n.children().filter(move |c| c.has_tag_name("TimephasedData") && child(*c, "Type") == ty).filter_map(move |c| parse_datetime(&child(c, at)).ok());
                let stop = periods("2", "Finish").max();
                let resume = periods("1", "Start").min();
                let actual = child(n, "ActualWork");
                out.assignments.push(AssignmentDates {
                    task: child(n, "TaskUID").parse::<i64>().unwrap_or(-1) as u32,
                    resource: child(n, "ResourceUID").parse::<i64>().unwrap_or(-1) as u32,
                    units: child(n, "Units").parse().unwrap_or(1.0),
                    work: parse_duration(&child(n, "Work")).map_err(|e| MppError(e.0))? as f64,
                    start: date(child(n, "Start"))?,
                    finish: date(child(n, "Finish"))?,
                    actual: if actual.is_empty() { 0.0 } else { parse_duration(&actual).map_err(|e| MppError(e.0))? as f64 },
                    stop,
                    resume,
                });
            }
        }
        Ok(out)
    }
}

/// One entity storage's streams, editable in place.
struct Store {
    name: String,
    meta: Vec<u8>,
    data: Vec<u8>,
    meta2: Vec<u8>,
    data2: Vec<u8>,
    var_meta: Vec<u8>,
    var_data: Vec<u8>,
    /// (item in `meta`, record in `data`, second block in `data2`), in file order.
    rows: Vec<(Range<usize>, Range<usize>, Range<usize>)>,
    /// Each row's item in `meta2`.
    items2: Vec<Range<usize>>,
}

impl Store {
    fn load(file: &cfb::Compound, name: &str) -> Self {
        let get = |s: &str| file.get(&format!("/   114/{name}/{s}")).map(<[u8]>::to_vec).unwrap_or_default();
        let (meta, data, meta2, data2) = (get("FixedMeta"), get("FixedData"), get("Fixed2Meta"), get("Fixed2Data"));
        let second = spans(&meta2, &data2);
        let rows: Vec<_> = spans(&meta, &data).into_iter().enumerate().map(|(k, (item, rec))| (item, rec, second.get(k).map_or(0..0, |s| s.1.clone()))).collect();
        let items2 = (0..rows.len()).map(|k| second.get(k).map_or(0..0, |s| s.0.clone())).collect();
        Store { name: name.into(), meta, data, meta2, data2, var_meta: get("VarMeta"), var_data: get("Var2Data"), rows, items2 }
    }

    fn save(mut self, file: &mut cfb::Compound) -> Result<()> {
        self.count_vars();
        self.sort_var_index();
        let streams = [("FixedMeta", self.meta), ("FixedData", self.data), ("Fixed2Meta", self.meta2), ("Fixed2Data", self.data2), ("VarMeta", self.var_meta), ("Var2Data", self.var_data)];
        for (s, bytes) in streams {
            let path = format!("/   114/{}/{s}", self.name);
            if file.get(&path).is_some() {
                file.set(&path, bytes).map_err(|e| MppError(e.0))?;
            }
        }
        Ok(())
    }

    /// Project looks variable values up by (UID, field type) in a sorted index: values added at
    /// its end would not be found.
    fn sort_var_index(&mut self) {
        if self.var_meta.len() < 24 {
            return;
        }
        let mut entries: Vec<[u8; 12]> = self.var_meta[24..].as_chunks::<12>().0.to_vec();
        entries.sort_by_key(|e| (u32_at(e, 0), u16_at(e, 8)));
        self.var_meta.truncate(24);
        self.var_meta.extend(entries.into_iter().flatten());
    }

    /// Each live record's item holds, at 2, how many variable values the record has.
    fn count_vars(&mut self) {
        let Some(uid_at) = uid_offset(&self.name) else { return };
        let mut counts: HashMap<u32, u16> = HashMap::new();
        for k in 0..u32_at(&self.var_meta, 8) as usize {
            *counts.entry(u32_at(&self.var_meta, 24 + 12 * k)).or_default() += 1;
        }
        for (item, rec, _) in &self.rows {
            let record = &self.data[rec.clone()];
            if record.len() < uid_at + 4 || record.len() == 16 || is_deleted(&self.meta[item.clone()]) {
                continue;
            }
            let n = counts.get(&u32_at(record, uid_at)).copied().unwrap_or(0);
            put_u16(&mut self.meta[item.clone()], ITEM_VAR_COUNT, n);
        }
    }

    /// The first live record that `matches` (index entries and deleted records are skipped).
    fn find_live(&self, matches: impl Fn(&[u8]) -> bool) -> Option<usize> {
        self.rows.iter().position(|(item, r, _)| r.len() > 16 && !is_deleted(&self.meta[item.clone()]) && matches(&self.data[r.clone()]))
    }

    fn is_live_assignment_of(&self, row: usize, task: u32) -> bool {
        let (item, r, _) = &self.rows[row];
        let r = &self.data[r.clone()];
        r.len() > A_FINISH && !is_deleted(&self.meta[item.clone()]) && u32_at(r, A_TASK) == task
    }

    fn find(&self, offset: usize, value: u32) -> Option<usize> {
        self.find_live(|r| u32_at(r, offset) == value)
    }

    fn find_link(&self, pred: u32, succ: u32) -> Option<usize> {
        self.find_live(|r| r.len() >= 20 && u32_at(r, L_PREDECESSOR) == pred && u32_at(r, L_SUCCESSOR) == succ)
    }

    fn find_assignment(&self, task: u32, resource: u32) -> Option<usize> {
        self.find_live(|r| r.len() > A_FINISH && u32_at(r, A_TASK) == task && u32_at(r, A_RESOURCE) == resource)
    }

    /// Work and dates of an assignment: all its work is regular; the done part stops at `stop`
    /// and the rest resumes at `resume` (both at the start before any work is done). Its
    /// contours spread the remaining and the done work evenly.
    fn write_assignment(&mut self, row: usize, a: &AssignmentDates) -> Result<()> {
        let r = self.record(row);
        let uid = u32_at(r, A_UID);
        let remaining = a.work - a.actual;
        let changed = f64_at(r, A_WORK) != a.work * 1000.0 || f64_at(r, A_UNITS) != a.units * 10000.0 || f64_at(r, A_ACTUAL_WORK) != a.actual * 1000.0;
        put_f64(r, A_UNITS, a.units * 10000.0);
        put_f64(r, A_WORK, a.work * 1000.0);
        put_f64(r, A_REGULAR_WORK, a.work * 1000.0);
        put_f64(r, A_ACTUAL_WORK, a.actual * 1000.0);
        put_f64(r, A_REMAINING_WORK, remaining * 1000.0);
        put_date(r, A_START, Some(a.start));
        put_date(r, A_FINISH, Some(a.finish));
        let stop = a.stop.unwrap_or(a.start);
        put_date(r, A_STOP, Some(stop));
        put_date(r, A_RESUME, Some(a.resume.unwrap_or(stop)));
        let item = self.item(row);
        for flag in A_WORK_DONE_FLAGS {
            set_flag(item, flag, a.actual > 0.0);
        }
        if !changed {
            return Ok(());
        }
        let Some(contour) = self.var(uid, AV_CONTOUR) else { return Ok(()) };
        if !matches!(contour.len(), UNIFORM_CONTOUR_LEN | PROGRESS_CONTOUR_LEN) || u32_at(contour, C_SEGMENTS) != 1 {
            return err("trabalho de atribuição com distribuição irregular (nivelada ou editada no Project): essa alteração ainda não pode ser gravada direto no .mpp");
        }
        let mut contour = contour[..UNIFORM_CONTOUR_LEN].to_vec();
        let rate = a.units * 160_000.0;
        uniform_segment(&mut contour, C_FIRST, rate, remaining, a.units);
        let tail = closing_copy(rate, remaining, a.units);
        contour[C_TAIL..].copy_from_slice(&tail);
        if a.actual > 0.0 {
            contour.extend_from_slice(&tail);
            contour.extend_from_slice(&[0; 8]);
        }
        self.set_var(uid, AV_CONTOUR, contour);
        if a.actual > 0.0 {
            let mut done = vec![0u8; DONE_CONTOUR_LEN];
            put_u16(&mut done, 0, 1);
            put_u16(&mut done, 2, DONE_SEGMENT_LEN);
            put_u32(&mut done, 4, DONE_TAIL as u32);
            uniform_segment(&mut done, C_FIRST, a.units * 10_000.0, a.actual, a.units);
            put_f64(&mut done, DONE_TAIL, a.actual * 1000.0);
            put_f64(&mut done, DONE_TAIL + 8, a.units * 10_000.0);
            put_u32(&mut done, DONE_TAIL + 16, span_of(a.actual, a.units));
            self.set_or_add_var(uid, AV_DONE_CONTOUR, ASSIGNMENT_VAR_FLAGS, done);
        }
        Ok(())
    }

    fn record(&mut self, row: usize) -> &mut [u8] {
        &mut self.data[self.rows[row].1.clone()]
    }

    fn second(&mut self, row: usize) -> &mut [u8] {
        &mut self.data2[self.rows[row].2.clone()]
    }

    fn item(&mut self, row: usize) -> &mut [u8] {
        &mut self.meta[self.rows[row].0.clone()]
    }

    fn item2(&mut self, row: usize) -> &mut [u8] {
        &mut self.meta2[self.items2[row].clone()]
    }

    /// Where `VarMeta` indexes the value of field `ty` of record `uid`.
    fn var_entry(&self, uid: u32, ty: u16) -> Option<usize> {
        let count = u32_at(&self.var_meta, 8) as usize;
        (0..count).map(|k| 24 + 12 * k).find(|&e| e + 12 <= self.var_meta.len() && u32_at(&self.var_meta, e) == uid && u16_at(&self.var_meta, e + 8) == ty)
    }

    fn var(&self, uid: u32, ty: u16) -> Option<&[u8]> {
        let at = u32_at(&self.var_meta, self.var_entry(uid, ty)? + 4) as usize;
        self.var_data.get(at + 4..at + 4 + u32_at(&self.var_data, at) as usize)
    }

    /// Sets a variable value, adding its index entry when the record has none yet.
    fn set_or_add_var(&mut self, uid: u32, ty: u16, flags: u16, value: Vec<u8>) {
        if self.var_entry(uid, ty).is_none() {
            let count = u32_at(&self.var_meta, 8);
            self.var_meta.extend_from_slice(&uid.to_le_bytes());
            self.var_meta.extend_from_slice(&0u32.to_le_bytes());
            self.var_meta.extend_from_slice(&ty.to_le_bytes());
            self.var_meta.extend_from_slice(&flags.to_le_bytes());
            put_u32(&mut self.var_meta, 8, count + 1);
        }
        self.set_var(uid, ty, value);
    }

    /// Replaces a text value (UTF-16, zero-terminated).
    fn set_text(&mut self, uid: u32, ty: u16, text: &str) {
        self.set_var(uid, ty, text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect());
    }

    /// Replaces a variable value: the new value goes at the end of `Var2Data` and the index
    /// points there; the old bytes stay as unused space, the way Project leaves edited values.
    fn set_var(&mut self, uid: u32, ty: u16, mut value: Vec<u8>) {
        let Some(e) = self.var_entry(uid, ty) else { return };
        let at = self.var_data.len();
        self.var_data.extend_from_slice(&(value.len() as u32).to_le_bytes());
        self.var_data.append(&mut value);
        put_u32(&mut self.var_meta, e + 4, at as u32);
        let total = self.var_data.len() as u32;
        put_u32(&mut self.var_meta, 20, total);
    }
}

pub(super) fn put_u16(b: &mut [u8], o: usize, v: u16) {
    if let Some(s) = b.get_mut(o..o + 2) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

pub(super) fn put_u32(b: &mut [u8], o: usize, v: u32) {
    if let Some(s) = b.get_mut(o..o + 4) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

pub(super) fn put_f64(b: &mut [u8], o: usize, v: f64) {
    if let Some(s) = b.get_mut(o..o + 8) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

/// One contour segment at `at`: the rate, the work (thousandths of a minute) and its span in
/// working minutes × 80.
fn uniform_segment(b: &mut [u8], at: usize, rate: f64, work: f64, units: f64) {
    put_f64(b, at, rate);
    put_f64(b, at + 8, work * 1000.0);
    put_u32(b, at + 16, span_of(work, units));
}

/// A contour's closing copy: work, rate, then the span 24 bytes in.
fn closing_copy(rate: f64, work: f64, units: f64) -> [u8; 28] {
    let mut b = [0u8; 28];
    put_f64(&mut b, 0, work * 1000.0);
    put_f64(&mut b, 8, rate);
    put_u32(&mut b, 24, span_of(work, units));
    b
}

fn span_of(work: f64, units: f64) -> u32 {
    let span = if units > 0.0 { work / units } else { 0.0 };
    (span * 80.0).round() as u32
}

/// The inverse of [`date_at`]: tenths of a minute, then days since 1983-12-31; NA for `None`.
pub(super) fn put_date(b: &mut [u8], o: usize, d: Option<NaiveDateTime>) {
    let Some(s) = b.get_mut(o..o + 4) else { return };
    match d {
        None => s.copy_from_slice(&NA),
        Some(d) => {
            let days = (d.date() - epoch()).num_days() as u16;
            let tenths = (d.time().num_seconds_from_midnight() / 6) as u16;
            s[..2].copy_from_slice(&tenths.to_le_bytes());
            s[2..].copy_from_slice(&days.to_le_bytes());
        }
    }
}

pub(super) fn set_flag(item: &mut [u8], (byte, bit): (usize, u8), on: bool) {
    if let Some(b) = item.get_mut(byte) {
        *b = if on { *b | 1 << bit } else { *b & !(1 << bit) };
    }
}
