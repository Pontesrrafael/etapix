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

/// Bytes Project writes for an empty ("NA") date.
const NA: [u8; 4] = [0xFF; 4];
/// Remaining duration (tenths of a minute): Project takes the duration as done + remaining.
const T_REMAINING_DURATION: usize = 88;
// Flags in a task's FixedMeta item: (byte, bit).
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
/// Work contour: how the work spreads over time. Project rebuilds the task from it.
const AV_CONTOUR: u16 = 0x31;
// A uniform contour of one segment (72 bytes): rate (units × 160000) at 8, work (thousandths of a
// minute) at 16, span (working minutes × 80) at 24, segment count at 32.
const C_RATE: usize = 8;
const C_WORK: usize = 16;
const C_SPAN: usize = 24;
const C_SEGMENTS: usize = 32;
const UNIFORM_CONTOUR_LEN: usize = 72;
/// The resource of the assignment Project gives a task without resources.
const PLACEHOLDER_RESOURCE: u32 = -65535i32 as u32;
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
        write_task(&mut tasks, at, old, new, dates.tasks.get(&new.uid));
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
        // Etapix adds a placeholder assignment (no resource) to tracked tasks; the file has its own.
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
            let work = t.duration as f64;
            assignments.write_assignment(at, &AssignmentDates { task: t.uid as u32, resource: PLACEHOLDER_RESOURCE, units: 1.0, work, start, finish })?;
        }
    }
    assignments.save(&mut file)?;
    Ok(file.to_bytes())
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
    let tracked = |a: &Task, b: &Task| a.progress != b.progress || (b.progress.actual_start.is_some() && a.duration != b.duration);
    if before.tasks.iter().zip(&after.tasks).any(|(a, b)| tracked(a, b)) {
        return unsupported("andamento de tarefas, ou duração de tarefa iniciada,");
    }
    if before.tasks.iter().zip(&after.tasks).any(|(a, b)| a.baseline != b.baseline) {
        return unsupported("linha de base alterada");
    }
    Ok(())
}

/// A calendar's working time (names change between saves: Project names a resource's calendar
/// after it).
fn working_time(c: &Calendar) -> String {
    format!("{:?} {:?}", c.weekdays, c.exceptions)
}

/// Writes task `new` over the record that holds `old`, field by field where they differ, and the
/// dates Etapix scheduled for it.
fn write_task(tasks: &mut Store, at: usize, old: &Task, new: &Task, dates: Option<&(NaiveDateTime, NaiveDateTime)>) {
    if old.name != new.name {
        tasks.set_text(new.uid as u32, TV_NAME, &new.name);
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
        let remaining = new.duration * (100 - q.percent_complete.min(100) as i64) / 100;
        put_u32(r, T_REMAINING_DURATION, (remaining * 10) as u32);
    }
    if p.actual_start != q.actual_start {
        put_date(r, T_ACTUAL_START, q.actual_start);
    }
    if p.actual_finish != q.actual_finish {
        put_date(r, T_ACTUAL_FINISH, q.actual_finish);
    }
    if p.resume != q.resume {
        put_date(r, T_RESUME, q.resume);
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
    let item = tasks.item(at);
    set_flag(item, F_MILESTONE, new.duration == 0 && !new.summary);
    set_flag(item, F_EFFORT_DRIVEN, new.effort_driven);
    set_flag(item, F_LEVELING_CAN_SPLIT, new.leveling_can_split);
    set_flag(item, F_IGNORE_RESOURCE_CALENDAR, new.ignore_resource_calendar);
}

/// Tasks whose dates, and assignments, stay as the file has them: a manual task's dates are what
/// the user typed (unless its mode, manual start or duration changed), and a started task's come
/// from its progress, which Etapix does not write yet.
fn keeps_file_dates(old: &Task, new: &Task) -> bool {
    let manual = new.manual.is_some() && old.manual == new.manual;
    let started = new.progress.actual_start.is_some();
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
                out.assignments.push(AssignmentDates {
                    task: child(n, "TaskUID").parse::<i64>().unwrap_or(-1) as u32,
                    resource: child(n, "ResourceUID").parse::<i64>().unwrap_or(-1) as u32,
                    units: child(n, "Units").parse().unwrap_or(1.0),
                    work: parse_duration(&child(n, "Work")).map_err(|e| MppError(e.0))? as f64,
                    start: date(child(n, "Start"))?,
                    finish: date(child(n, "Finish"))?,
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
}

impl Store {
    fn load(file: &cfb::Compound, name: &str) -> Self {
        let get = |s: &str| file.get(&format!("/   114/{name}/{s}")).map(<[u8]>::to_vec).unwrap_or_default();
        let (meta, data, meta2, data2) = (get("FixedMeta"), get("FixedData"), get("Fixed2Meta"), get("Fixed2Data"));
        let second = spans(&meta2, &data2);
        let rows = spans(&meta, &data).into_iter().enumerate().map(|(k, (item, rec))| (item, rec, second.get(k).map_or(0..0, |s| s.1.clone()))).collect();
        Store { name: name.into(), meta, data, meta2, data2, var_meta: get("VarMeta"), var_data: get("Var2Data"), rows }
    }

    fn save(self, file: &mut cfb::Compound) -> Result<()> {
        let streams = [("FixedMeta", self.meta), ("FixedData", self.data), ("Fixed2Meta", self.meta2), ("Fixed2Data", self.data2), ("VarMeta", self.var_meta), ("Var2Data", self.var_data)];
        for (s, bytes) in streams {
            let path = format!("/   114/{}/{s}", self.name);
            if file.get(&path).is_some() {
                file.set(&path, bytes).map_err(|e| MppError(e.0))?;
            }
        }
        Ok(())
    }

    /// The row whose record holds `value` at `offset` (index entries are 16 bytes and skipped).
    fn find(&self, offset: usize, value: u32) -> Option<usize> {
        self.rows.iter().position(|(_, r, _)| r.len() > 16 && u32_at(&self.data[r.clone()], offset) == value)
    }

    fn find_link(&self, pred: u32, succ: u32) -> Option<usize> {
        self.rows.iter().position(|(_, r, _)| {
            let r = &self.data[r.clone()];
            r.len() >= 20 && u32_at(r, L_PREDECESSOR) == pred && u32_at(r, L_SUCCESSOR) == succ
        })
    }

    fn find_assignment(&self, task: u32, resource: u32) -> Option<usize> {
        self.rows.iter().position(|(_, r, _)| {
            let r = &self.data[r.clone()];
            r.len() > A_FINISH && u32_at(r, A_TASK) == task && u32_at(r, A_RESOURCE) == resource
        })
    }

    /// Work and dates of an assignment that has not started: all its work is regular and
    /// remaining, the stop and resume dates sit at its start, and its contour spreads the work
    /// evenly.
    fn write_assignment(&mut self, row: usize, a: &AssignmentDates) -> Result<()> {
        let r = self.record(row);
        let uid = u32_at(r, A_UID);
        let changed = f64_at(r, A_WORK) != a.work * 1000.0 || f64_at(r, A_UNITS) != a.units * 10000.0;
        put_f64(r, A_UNITS, a.units * 10000.0);
        for o in [A_WORK, A_REGULAR_WORK, A_REMAINING_WORK] {
            put_f64(r, o, a.work * 1000.0);
        }
        put_date(r, A_START, Some(a.start));
        put_date(r, A_FINISH, Some(a.finish));
        put_date(r, A_RESUME, Some(a.start));
        put_date(r, A_STOP, Some(a.start));
        let Some(mut contour) = self.var(uid, AV_CONTOUR).map(<[u8]>::to_vec) else { return Ok(()) };
        if !changed {
            return Ok(());
        }
        if contour.len() != UNIFORM_CONTOUR_LEN || u32_at(&contour, C_SEGMENTS) != 1 {
            return err("trabalho de atribuição com distribuição irregular (nivelada ou editada no Project): essa alteração ainda não pode ser gravada direto no .mpp");
        }
        let span = if a.units > 0.0 { a.work / a.units } else { 0.0 };
        put_f64(&mut contour, C_RATE, a.units * 160_000.0);
        put_f64(&mut contour, C_WORK, a.work * 1000.0);
        put_u32(&mut contour, C_SPAN, (span * 80.0).round() as u32);
        self.set_var(uid, AV_CONTOUR, contour);
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

    /// Where `VarMeta` indexes the value of field `ty` of record `uid`.
    fn var_entry(&self, uid: u32, ty: u16) -> Option<usize> {
        let count = u32_at(&self.var_meta, 8) as usize;
        (0..count).map(|k| 24 + 12 * k).find(|&e| e + 12 <= self.var_meta.len() && u32_at(&self.var_meta, e) == uid && u16_at(&self.var_meta, e + 8) == ty)
    }

    fn var(&self, uid: u32, ty: u16) -> Option<&[u8]> {
        let at = u32_at(&self.var_meta, self.var_entry(uid, ty)? + 4) as usize;
        self.var_data.get(at + 4..at + 4 + u32_at(&self.var_data, at) as usize)
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

fn put_u16(b: &mut [u8], o: usize, v: u16) {
    if let Some(s) = b.get_mut(o..o + 2) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

fn put_u32(b: &mut [u8], o: usize, v: u32) {
    if let Some(s) = b.get_mut(o..o + 4) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

fn put_f64(b: &mut [u8], o: usize, v: f64) {
    if let Some(s) = b.get_mut(o..o + 8) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

/// The inverse of [`date_at`]: tenths of a minute, then days since 1983-12-31; NA for `None`.
fn put_date(b: &mut [u8], o: usize, d: Option<NaiveDateTime>) {
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

fn set_flag(item: &mut [u8], (byte, bit): (usize, u8), on: bool) {
    if let Some(b) = item.get_mut(byte) {
        *b = if on { *b | 1 << bit } else { *b & !(1 << bit) };
    }
}
