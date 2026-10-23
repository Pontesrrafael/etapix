//! Native reader for Microsoft Project `.mpp` files (the format of Project 2010 and later, whose
//! data lives in the `   114` storage), without Project installed.
//!
//! The layout was worked out black-box: files saved by Project with known content (the golden
//! corpus and controlled variations in `tests/mpp-corpus`, built by `tools/mpp_corpus.ps1` and
//! `tools/mpp_variations.ps1`) were compared with the Project XML that the same Project session
//! saved. Nothing of Project's code was used.
//!
//! The reader produces Project XML (MSPDI) with the elements [`crate::mspdi::read_project`] uses,
//! so `.mpp` files go through the same, tested path as `.xml` ones.
//!
//! Layout, per entity storage (`TBkndTask`, `TBkndRsc`, `TBkndAssn`, `TBkndCons`, `TBkndCal`):
//! - `FixedMeta` lists the records of `FixedData` (16-byte header, then one item per record:
//!   offset at +4, flags in the following bytes); `Fixed2Meta`/`Fixed2Data` hold a second block
//!   per record, in the same order.
//! - `VarMeta` (24-byte header, then 12-byte entries: unique id, offset, field type) indexes
//!   `Var2Data`, where each value is a 32-bit length followed by the bytes.
//! - Dates are two 16-bit words: time in tenths of a minute, then days since 1983-12-31.
//!   Durations and lags are in tenths of a minute; work in thousandths; units in ten-thousandths.
//! - The project storage's `Props` is a list of (length, key, flags, value) entries.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::ops::Range;

use chrono::{Duration, NaiveDate, NaiveDateTime};

use crate::cfb;

mod update;
pub use update::update_mpp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MppError(pub String);

impl std::fmt::Display for MppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

type Result<T> = std::result::Result<T, MppError>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(MppError(msg.into()))
}

fn u8_at(b: &[u8], o: usize) -> u8 {
    b.get(o).copied().unwrap_or(0)
}
fn u16_at(b: &[u8], o: usize) -> u16 {
    b.get(o..o + 2).map_or(0, |s| u16::from_le_bytes([s[0], s[1]]))
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn i32_at(b: &[u8], o: usize) -> i32 {
    u32_at(b, o) as i32
}
fn f64_at(b: &[u8], o: usize) -> f64 {
    b.get(o..o + 8).map_or(0.0, |s| f64::from_le_bytes(s.try_into().unwrap()))
}

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1983, 12, 31).unwrap()
}

/// A stored date (tenths of a minute, days since 1983-12-31); `None` for "NA".
fn date_at(b: &[u8], o: usize) -> Option<NaiveDateTime> {
    let (tenths, days) = (u16_at(b, o), u16_at(b, o + 2));
    if days == 0 || days == 0xFFFF || b.len() < o + 4 {
        return None;
    }
    Some(epoch().and_hms_opt(0, 0, 0).unwrap() + Duration::days(days as i64) + Duration::seconds(tenths as i64 * 6))
}

fn day_at(b: &[u8], o: usize) -> NaiveDate {
    epoch() + Duration::days(u16_at(b, o) as i64)
}

fn iso(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Tenths of a minute as an MSPDI duration.
fn duration(tenths: i64) -> String {
    let (sign, t) = if tenths < 0 { ("-", -tenths) } else { ("", tenths) };
    let minutes = t / 10;
    let seconds = (t % 10) * 6;
    format!("{sign}PT{}H{}M{}S", minutes / 60, minutes % 60, seconds)
}

fn minutes_duration(minutes: f64) -> String {
    duration((minutes * 10.0).round() as i64)
}

fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// One entity storage: its fixed records (with their FixedMeta item), second blocks and
/// variable data.
struct Table {
    /// (meta item, record, second block), in file order.
    rows: Vec<(Vec<u8>, Vec<u8>, Vec<u8>)>,
    var: HashMap<(u32, u16), Vec<u8>>,
}

/// Where each record lives: its item in `meta` and its bytes in `data`.
fn spans(meta: &[u8], data: &[u8]) -> Vec<(Range<usize>, Range<usize>)> {
    let n = u32_at(meta, 8) as usize;
    if n == 0 || meta.len() < 16 {
        return Vec::new();
    }
    let total = (u32_at(meta, 12) as usize).min(data.len());
    // Items are usually (length - 16) / count bytes long, but some files pad the list: the right
    // stride is the one whose offsets grow inside the data. The first record need not start at 0:
    // Project leaves the space of deleted records in place.
    let valid = |stride: usize| {
        let offsets: Vec<usize> = (0..n).map(|k| u32_at(meta, 16 + stride * k + 4) as usize).collect();
        16 + stride * n <= meta.len() && offsets.windows(2).all(|w| w[0] < w[1]) && offsets.last().is_some_and(|&o| o < total.max(1))
    };
    let guess = (meta.len() - 16) / n;
    let Some(stride) = std::iter::once(guess).chain(6..guess).find(|&s| valid(s)) else {
        return Vec::new();
    };
    let offsets: Vec<usize> = (0..n).map(|k| u32_at(meta, 16 + stride * k + 4) as usize).collect();
    (0..n)
        .map(|k| {
            let start = offsets[k].min(total);
            let end = offsets.get(k + 1).copied().unwrap_or(total).clamp(start, total);
            (16 + stride * k..16 + stride * (k + 1), start..end)
        })
        .collect()
}

fn records(meta: &[u8], data: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    spans(meta, data).into_iter().map(|(item, rec)| (meta[item].to_vec(), data[rec].to_vec())).collect()
}

fn table(streams: &BTreeMap<String, Vec<u8>>, name: &str) -> Table {
    let get = |s: &str| streams.get(&format!("/   114/{name}/{s}")).map(Vec::as_slice).unwrap_or_default();
    let first = records(get("FixedMeta"), get("FixedData"));
    let second = records(get("Fixed2Meta"), get("Fixed2Data"));
    let rows = first.into_iter().enumerate().map(|(k, (item, rec))| (item, rec, second.get(k).map(|s| s.1.clone()).unwrap_or_default())).collect();
    let (vmeta, vdata) = (get("VarMeta"), get("Var2Data"));
    let mut var = HashMap::new();
    let count = u32_at(vmeta, 8) as usize;
    for k in 0..count {
        let e = 24 + 12 * k;
        if e + 12 > vmeta.len() {
            break;
        }
        let (uid, off, ty) = (u32_at(vmeta, e), u32_at(vmeta, e + 4) as usize, u32_at(vmeta, e + 8) as u16);
        let len = u32_at(vdata, off) as usize;
        if let Some(v) = vdata.get(off + 4..off + 4 + len) {
            var.insert((uid, ty), v.to_vec());
        }
    }
    Table { rows, var }
}

impl Table {
    /// Records with a unique id (the real rows: index entries are 16 bytes long).
    fn entities(&self) -> impl Iterator<Item = &(Vec<u8>, Vec<u8>, Vec<u8>)> {
        self.rows.iter().filter(|(_, r, _)| r.len() > 16)
    }
    fn text(&self, uid: u32, ty: u16) -> Option<String> {
        self.var.get(&(uid, ty)).map(|v| utf16(v))
    }
}

fn props(p: &[u8]) -> HashMap<u32, Vec<u8>> {
    let mut out = HashMap::new();
    let mut o = 16;
    while o + 12 <= p.len() {
        let (len, key) = (u32_at(p, o) as usize, u32_at(p, o + 4));
        if len > p.len() - o - 12 {
            break;
        }
        out.insert(key, p[o + 12..o + 12 + len].to_vec());
        o += 12 + len;
    }
    out
}

const PROJECT_START: u32 = 0x0240_0002;
const PROJECT_CALENDAR_NAME: u32 = 0x0240_000E;
const CRITICAL_SLACK_LIMIT: u32 = 0x0240_0014;
const DEFAULT_START_TIME: u32 = 0x0240_001C;
const DEFAULT_FINISH_TIME: u32 = 0x0240_0021;
const MULTIPLE_CRITICAL_PATHS: u32 = 0x0240_0039;
const HONOR_CONSTRAINTS: u32 = 0x0240_003A;
const STATUS_DATE: u32 = 0x0240_0045;

/// Shifts of a weekday or exception block: type, count, total, start times (u16 at +8) and
/// lengths (u32 at +20), in tenths of a minute. `None` when the block says "default".
fn shifts(block: &[u8]) -> Option<Vec<(u32, u32)>> {
    if u16_at(block, 0) == 1 {
        return None;
    }
    let n = (u16_at(block, 2) as usize).min(5);
    Some(
        (0..n)
            .map(|i| {
                let from = u16_at(block, 8 + 2 * i) as u32 / 10;
                (from, from + u32_at(block, 20 + 4 * i) / 10)
            })
            .collect(),
    )
}

fn hhmm(minutes: u32) -> String {
    let m = minutes % (24 * 60);
    format!("{:02}:{:02}:00", m / 60, m % 60)
}

fn working_times(out: &mut String, shifts: &[(u32, u32)]) {
    if shifts.is_empty() {
        return;
    }
    out.push_str("<WorkingTimes>");
    for &(a, b) in shifts {
        let _ = write!(out, "<WorkingTime><FromTime>{}</FromTime><ToTime>{}</ToTime></WorkingTime>", hhmm(a), hhmm(b));
    }
    out.push_str("</WorkingTimes>");
}

/// The parts of an `.mpp` the reader needs.
struct Mpp {
    props: HashMap<u32, Vec<u8>>,
    calendars: Table,
    tasks: Table,
    resources: Table,
    assignments: Table,
    links: Table,
}

/// A calendar record: its base calendar and owner resource (`None` for a base calendar), or
/// unknown base for the 16-byte records Project writes for some resource calendars.
struct CalendarRow {
    uid: u32,
    base: Option<i32>,
    resource: Option<i32>,
}

/// Reads an `.mpp` file and returns the same project as Project XML (MSPDI).
pub fn mpp_to_mspdi(data: &[u8]) -> Result<String> {
    let streams = cfb::read_streams(data).map_err(|e| MppError(e.0))?;
    let Some(props_stream) = streams.get("/   114/Props") else {
        return err("este .mpp é de uma versão do Project anterior à 2010, que o Etapix ainda não lê sem o Project");
    };
    let mpp = Mpp {
        props: props(props_stream),
        calendars: table(&streams, "TBkndCal"),
        tasks: table(&streams, "TBkndTask"),
        resources: table(&streams, "TBkndRsc"),
        assignments: table(&streams, "TBkndAssn"),
        links: table(&streams, "TBkndCons"),
    };
    let calendars = calendar_rows(&mpp);
    let project_calendar = project_calendar(&mpp, &calendars)?;

    let mut x = String::with_capacity(64 * 1024);
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Project xmlns=\"http://schemas.microsoft.com/project\">");
    write_project_options(&mut x, &mpp, project_calendar)?;
    write_calendars(&mut x, &mpp, &calendars, project_calendar);
    write_tasks(&mut x, &mpp);
    write_resources(&mut x, &mpp, &calendars);
    write_assignments(&mut x, &mpp);
    x.push_str("</Project>\n");
    Ok(x)
}

fn write_project_options(x: &mut String, mpp: &Mpp, calendar: u32) -> Result<()> {
    let p = &mpp.props;
    let start = p.get(&PROJECT_START).and_then(|v| date_at(v, 0)).ok_or_else(|| MppError("projeto sem data de início".into()))?;
    let _ = write!(x, "<StartDate>{}</StartDate><CalendarUID>{calendar}</CalendarUID>", iso(start));
    if let Some(v) = p.get(&CRITICAL_SLACK_LIMIT) {
        let _ = write!(x, "<CriticalSlackLimit>{}</CriticalSlackLimit>", i32_at(v, 0));
    }
    if let Some(v) = p.get(&MULTIPLE_CRITICAL_PATHS) {
        let _ = write!(x, "<MultipleCriticalPaths>{}</MultipleCriticalPaths>", (u8_at(v, 0) != 0) as u8);
    }
    if let Some(v) = p.get(&HONOR_CONSTRAINTS) {
        let _ = write!(x, "<HonorConstraints>{}</HonorConstraints>", (u8_at(v, 0) != 0) as u8);
    }
    if let Some(d) = p.get(&STATUS_DATE).and_then(|v| date_at(v, 0)) {
        let _ = write!(x, "<StatusDate>{}</StatusDate>", iso(d));
    }
    Ok(())
}

/// The calendars of the project. Resource 0 is Project's placeholder: its calendar is left out.
fn calendar_rows(mpp: &Mpp) -> Vec<CalendarRow> {
    let mut rows: Vec<CalendarRow> = mpp
        .calendars
        .rows
        .iter()
        .filter_map(|(_, r, _)| match r.len() {
            // (base uid, resource uid, calendar uid); -1 = none.
            12 => {
                let (base, resource) = (i32_at(r, 0), i32_at(r, 4));
                Some(CalendarRow { uid: u32_at(r, 8), base: Some(base), resource: (resource >= 0).then_some(resource) })
            }
            // A resource calendar recorded by uid only (its base is found by name, below).
            16 if u32_at(r, 0) < 0xFFFF_0000 => Some(CalendarRow { uid: u32_at(r, 0), base: None, resource: Some(-1) }),
            _ => None,
        })
        .filter(|c| c.resource != Some(0))
        .collect();
    rows.sort_by_key(|c| c.uid);
    rows
}

fn calendar_name(mpp: &Mpp, uid: u32) -> Option<String> {
    mpp.calendars.text(uid, 1)
}

fn project_calendar(mpp: &Mpp, calendars: &[CalendarRow]) -> Result<u32> {
    let wanted = mpp.props.get(&PROJECT_CALENDAR_NAME).map(|v| utf16(v)).unwrap_or_default();
    let bases = || calendars.iter().filter(|c| c.resource.is_none());
    bases()
        .find(|c| calendar_name(mpp, c.uid).as_deref() == Some(wanted.as_str()))
        .or_else(|| bases().next())
        .map(|c| c.uid)
        .ok_or_else(|| MppError("projeto sem calendário base".into()))
}

/// For a resource calendar without a recorded base: the base calendar named after the
/// resource that uses it (Project names a resource's own base that way), else the project's.
fn implied_base(mpp: &Mpp, calendars: &[CalendarRow], uid: u32, project_calendar: u32) -> i32 {
    let owner = mpp.resources.entities().find(|(_, r, _)| i32_at(r, 154) == uid as i32).and_then(|(_, r, _)| mpp.resources.text(u32_at(r, 0), 1));
    owner
        .and_then(|name| calendars.iter().find(|c| c.resource.is_none() && calendar_name(mpp, c.uid).as_deref() == Some(name.as_str())))
        .map_or(project_calendar as i32, |c| c.uid as i32)
}

fn write_calendars(x: &mut String, mpp: &Mpp, calendars: &[CalendarRow], project_calendar: u32) {
    let p = &mpp.props;
    let day_start = p.get(&DEFAULT_START_TIME).map_or(5400, |v| u16_at(v, 0)) as u32 / 10;
    let day_finish = p.get(&DEFAULT_FINISH_TIME).map_or(10800, |v| u16_at(v, 0)) as u32 / 10;
    // Project's default week, from the project's default start and finish times.
    let default_day = |weekday: usize| -> Vec<(u32, u32)> {
        if weekday == 0 || weekday == 6 {
            vec![]
        } else {
            vec![(day_start, 12 * 60), (13 * 60, day_finish)]
        }
    };
    x.push_str("<Calendars>");
    for c in calendars {
        let is_base = c.resource.is_none();
        let base = match c.base {
            _ if is_base => -1,
            Some(b) if b > 0 => b,
            Some(_) => project_calendar as i32,
            None => implied_base(mpp, calendars, c.uid, project_calendar),
        };
        // A resource calendar is named after its resource.
        let name = calendar_name(mpp, c.uid).or_else(|| c.resource.filter(|&r| r > 0).and_then(|r| mpp.resources.text(r as u32, 1))).unwrap_or_default();
        let _ = write!(
            x,
            "<Calendar><UID>{}</UID><Name>{}</Name><IsBaseCalendar>{}</IsBaseCalendar><BaseCalendarUID>{base}</BaseCalendarUID><WeekDays>",
            c.uid,
            esc(&name),
            is_base as u8,
        );
        let blob = mpp.calendars.var.get(&(c.uid, 8)).map(Vec::as_slice).unwrap_or_default();
        for d in 0..7 {
            let day = blob.get(d * 60..d * 60 + 60).and_then(shifts).or_else(|| is_base.then(|| default_day(d)));
            // A derived calendar's default day comes from its base.
            if let Some(s) = day {
                let _ = write!(x, "<WeekDay><DayType>{}</DayType><DayWorking>{}</DayWorking>", d + 1, !s.is_empty() as u8);
                working_times(x, &s);
                x.push_str("</WeekDay>");
            }
        }
        x.push_str("</WeekDays><Exceptions>");
        write_exceptions(x, blob);
        x.push_str("</Exceptions></Calendar>");
    }
    x.push_str("</Calendars>");
}

/// Exceptions: a count at 420, then 92-byte entries: first and last day, occurrences, the
/// working times (a weekday block at +12), the recurrence type at +72 (0 = a plain dated range,
/// as 1) and its data at +76: weekly days and period; monthly day or position and period;
/// yearly month and day or position.
fn write_exceptions(x: &mut String, blob: &[u8]) {
    let n = if blob.len() >= 424 { u32_at(blob, 420) as usize } else { 0 };
    for e in (0..n).map_while(|k| blob.get(424 + 92 * k..424 + 92 * (k + 1))) {
        let s = shifts(&e[12..72]).unwrap_or_default();
        let ty = match u32_at(e, 72) {
            0 => 1,
            t => t,
        };
        let _ = write!(
            x,
            "<Exception><TimePeriod><FromDate>{}T00:00:00</FromDate><ToDate>{}T23:59:00</ToDate></TimePeriod><Occurrences>{}</Occurrences><Type>{ty}</Type><DayWorking>{}</DayWorking>",
            day_at(e, 0).format("%Y-%m-%d"),
            day_at(e, 2).format("%Y-%m-%d"),
            u32_at(e, 4),
            !s.is_empty() as u8
        );
        let period = u16_at(e, 78).max(1);
        let _ = match ty {
            1 => write!(x, "<Period>{period}</Period>"),
            6 => write!(x, "<Period>{period}</Period><DaysOfWeek>{}</DaysOfWeek>", u16_at(e, 76)),
            4 => write!(x, "<Period>{period}</Period><MonthDay>{}</MonthDay>", u16_at(e, 76)),
            5 => write!(x, "<Period>{period}</Period><MonthPosition>{}</MonthPosition><MonthItem>{}</MonthItem>", u8_at(e, 76), u8_at(e, 77)),
            2 => write!(x, "<Month>{}</Month><MonthDay>{}</MonthDay>", u8_at(e, 76), u8_at(e, 77)),
            3 => write!(x, "<Month>{}</Month><MonthPosition>{}</MonthPosition><MonthItem>{}</MonthItem>", u8_at(e, 76), u8_at(e, 77), u8_at(e, 78)),
            _ => Ok(()),
        };
        working_times(x, &s);
        x.push_str("</Exception>");
    }
}

// Task record layout (FixedData) and flags (FixedMeta item).
const T_UID: usize = 4;
const T_ID: usize = 0;
const T_FIXED_COST: usize = 48;
const T_CONSTRAINT: usize = 64;
const T_CONSTRAINT_DATE: usize = 66;
const T_LEVELING_DELAY: usize = 70;
const T_PRIORITY: usize = 78;
const T_DURATION: usize = 84;
const T_PERCENT: usize = 92;
const T_START: usize = 96;
const T_FINISH: usize = 100;
const T_ACTUAL_START: usize = 120;
const T_ACTUAL_FINISH: usize = 124;
const T_RESUME: usize = 132;
const T_TYPE: usize = 140;
const T_DURATION_FORMAT: usize = 164;
const T_FIXED_COST_ACCRUAL: usize = 166;
const T_OUTLINE_LEVEL: usize = 172;
const T_CALENDAR: usize = 178;
const T_DEADLINE: usize = 182;
/// Manual start and finish, in the second block: a manual task's dates, which its scheduled
/// dates (`T_START`, `T_FINISH`) do not follow.
const T2_MANUAL_START: usize = 50;
const T2_MANUAL_FINISH: usize = 54;
// Variable data of a task.
const TV_NAME: u16 = 0x0E;
const TV_BASELINE_START: u16 = 0x2B;
const TV_BASELINE_FINISH: u16 = 0x2C;
const TV_BASELINE_DURATION: u16 = 0x1B;
const TV_BASELINE_WORK: u16 = 0x01;
const TV_BASELINE_COST: u16 = 0x06;

/// Links grouped by successor: (predecessor, type, lag in tenths, lag format).
fn links(mpp: &Mpp) -> HashMap<u32, Vec<(u32, u16, i32, u16)>> {
    let mut by_successor: HashMap<u32, Vec<_>> = HashMap::new();
    for (_, r, _) in mpp.links.rows.iter().filter(|(_, r, _)| r.len() >= 20) {
        by_successor.entry(u32_at(r, 8)).or_default().push((u32_at(r, 4), u16_at(r, 12), i32_at(r, 14), u16_at(r, 18)));
    }
    by_successor
}

fn write_tasks(x: &mut String, mpp: &Mpp) {
    let links = links(mpp);
    let mut rows: Vec<_> = mpp.tasks.entities().collect();
    rows.sort_by_key(|(_, r, _)| u32_at(r, T_ID));
    x.push_str("<Tasks>");
    for (item, r, r2) in rows {
        let uid = u32_at(r, T_UID);
        let flag = |o: usize, b: u8| (u8_at(item, o) >> b) & 1;
        let tag = |name: &str, d: Option<_>| d.map(|d| format!("<{name}>{}</{name}>", iso(d))).unwrap_or_default();
        let date = |name: &str, o: usize| tag(name, date_at(r, o));
        let manual_start = date_at(r2, T2_MANUAL_START);
        let manual_finish = manual_start.and(date_at(r2, T2_MANUAL_FINISH));
        let calendar = i32_at(r, T_CALENDAR);
        let _ = write!(
            x,
            "<Task><UID>{uid}</UID><ID>{}</ID><Name>{}</Name><Type>{}</Type><OutlineLevel>{}</OutlineLevel><Priority>{}</Priority>\
             <Summary>{}</Summary><Milestone>{}</Milestone><EffortDriven>{}</EffortDriven><LevelingCanSplit>{}</LevelingCanSplit>\
             <IgnoreResourceCalendar>{}</IgnoreResourceCalendar>{}{}<Duration>{}</Duration><DurationFormat>{}</DurationFormat>\
             <ConstraintType>{}</ConstraintType>{}{}<CalendarUID>{}</CalendarUID>",
            u32_at(r, T_ID),
            esc(&mpp.tasks.text(uid, TV_NAME).unwrap_or_default()),
            u16_at(r, T_TYPE).min(2),
            u16_at(r, T_OUTLINE_LEVEL),
            u16_at(r, T_PRIORITY),
            flag(12, 3),
            flag(10, 1),
            flag(13, 3),
            flag(16, 1),
            flag(17, 5),
            tag("Start", manual_start.or(date_at(r, T_START))),
            tag("Finish", manual_finish.or(date_at(r, T_FINISH))),
            duration(i32_at(r, T_DURATION) as i64),
            u8_at(r, T_DURATION_FORMAT),
            u16_at(r, T_CONSTRAINT),
            date("ConstraintDate", T_CONSTRAINT_DATE),
            date("Deadline", T_DEADLINE),
            if calendar > 0 { calendar } else { -1 },
        );
        // Manually scheduled tasks keep their own start in the second block.
        match manual_start {
            Some(ms) => {
                let _ = write!(x, "<Manual>1</Manual><ManualStart>{}</ManualStart>", iso(ms));
            }
            None => x.push_str("<Manual>0</Manual>"),
        }
        let _ = write!(
            x,
            "<FixedCost>{}</FixedCost><FixedCostAccrual>{}</FixedCostAccrual><LevelingDelay>{}</LevelingDelay><PercentComplete>{}</PercentComplete>{}{}{}",
            f64_at(r, T_FIXED_COST),
            u8_at(r, T_FIXED_COST_ACCRUAL),
            i32_at(r, T_LEVELING_DELAY),
            u16_at(r, T_PERCENT),
            date("ActualStart", T_ACTUAL_START),
            date("ActualFinish", T_ACTUAL_FINISH),
            date("Resume", T_RESUME),
        );
        for (pred, ty, lag, fmt) in links.get(&uid).into_iter().flatten() {
            let _ = write!(
                x,
                "<PredecessorLink><PredecessorUID>{pred}</PredecessorUID><Type>{ty}</Type><LinkLag>{lag}</LinkLag><LagFormat>{fmt}</LagFormat></PredecessorLink>"
            );
        }
        write_baseline(x, &mpp.tasks, uid);
        x.push_str("</Task>");
    }
    x.push_str("</Tasks>");
}

fn write_baseline(x: &mut String, tasks: &Table, uid: u32) {
    let var = |ty: u16| tasks.var.get(&(uid, ty));
    let (Some(start), Some(finish)) = (var(TV_BASELINE_START).and_then(|v| date_at(v, 0)), var(TV_BASELINE_FINISH).and_then(|v| date_at(v, 0))) else {
        return;
    };
    let _ = write!(x, "<Baseline><Number>0</Number><Start>{}</Start><Finish>{}</Finish>", iso(start), iso(finish));
    if let Some(d) = var(TV_BASELINE_DURATION) {
        let _ = write!(x, "<Duration>{}</Duration>", duration(i32_at(d, 0) as i64));
    }
    if let Some(w) = var(TV_BASELINE_WORK) {
        let _ = write!(x, "<Work>{}</Work>", minutes_duration(f64_at(w, 0) / 1000.0));
    }
    if let Some(c) = var(TV_BASELINE_COST) {
        let _ = write!(x, "<Cost>{}</Cost>", f64_at(c, 0));
    }
    x.push_str("</Baseline>");
}

// Resource record layout.
const R_UID: usize = 0;
const R_ID: usize = 4;
const R_MAX_UNITS: usize = 8;
const R_STANDARD_RATE: usize = 16;
const R_OVERTIME_RATE: usize = 24;
const R_COST_PER_USE: usize = 72;
const R_CALENDAR: usize = 154;
const RV_NAME: u16 = 0x01;
/// Cost rate table A, when its rates change over time.
const RV_RATE_TABLE: u16 = 0x3D;

fn write_resources(x: &mut String, mpp: &Mpp, calendars: &[CalendarRow]) {
    x.push_str("<Resources>");
    for (item, r, _) in mpp.resources.entities() {
        let uid = u32_at(r, R_UID);
        // Work resources carry flag 0x10 in their index item; material ones do not.
        let kind = if u8_at(item, 12) & 0x10 != 0 { 1 } else { 0 };
        let calendar = i32_at(r, R_CALENDAR);
        let known = calendars.iter().any(|c| c.uid as i32 == calendar);
        let _ = write!(
            x,
            "<Resource><UID>{uid}</UID><ID>{}</ID><Name>{}</Name><Type>{kind}</Type><MaxUnits>{}</MaxUnits><StandardRate>{}</StandardRate>\
             <StandardRateFormat>2</StandardRateFormat><OvertimeRate>{}</OvertimeRate><OvertimeRateFormat>2</OvertimeRateFormat>\
             <CostPerUse>{}</CostPerUse><CalendarUID>{}</CalendarUID>",
            u32_at(r, R_ID),
            esc(&mpp.resources.text(uid, RV_NAME).unwrap_or_default()),
            f64_at(r, R_MAX_UNITS) / 10000.0,
            f64_at(r, R_STANDARD_RATE),
            f64_at(r, R_OVERTIME_RATE),
            f64_at(r, R_COST_PER_USE),
            if calendar > 0 && known { calendar } else { -1 },
        );
        if let Some(table) = mpp.resources.var.get(&(uid, RV_RATE_TABLE)) {
            write_rates(x, table);
        }
        x.push_str("</Resource>");
    }
    x.push_str("</Resources>");
}

/// A rate table: a count, then 44-byte periods from offset 16: standard rate and format,
/// overtime rate and format, cost per use (cents), and the period's end (tenths of a minute
/// since 1983-12-31). Each period starts where the previous one ends.
fn write_rates(x: &mut String, table: &[u8]) {
    let n = u16_at(table, 0) as usize;
    let mut from = NaiveDate::from_ymd_opt(1984, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
    x.push_str("<Rates>");
    for e in (0..n).map_while(|k| table.get(16 + 44 * k..16 + 44 * (k + 1))) {
        let to = epoch().and_hms_opt(0, 0, 0).unwrap() + Duration::seconds(u32_at(e, 40) as i64 * 6);
        let _ = write!(
            x,
            "<Rate><RatesFrom>{}</RatesFrom><RatesTo>{}</RatesTo><RateTable>0</RateTable><StandardRate>{}</StandardRate>\
             <StandardRateFormat>{}</StandardRateFormat><OvertimeRate>{}</OvertimeRate><OvertimeRateFormat>{}</OvertimeRateFormat>\
             <CostPerUse>{}</CostPerUse></Rate>",
            iso(from),
            iso(to),
            f64_at(e, 0),
            u16_at(e, 8),
            f64_at(e, 16),
            u16_at(e, 24),
            f64_at(e, 32)
        );
        from = to;
    }
    x.push_str("</Rates>");
}

// Assignment record layout.
const A_UID: usize = 0;
const A_TASK: usize = 4;
const A_RESOURCE: usize = 8;
const A_UNITS: usize = 12;
const A_WORK: usize = 20;
const A_START: usize = 52;
const A_FINISH: usize = 56;

fn write_assignments(x: &mut String, mpp: &Mpp) {
    x.push_str("<Assignments>");
    for (_, r, _) in mpp.assignments.entities() {
        let (Some(start), Some(finish)) = (date_at(r, A_START), date_at(r, A_FINISH)) else { continue };
        let _ = write!(
            x,
            "<Assignment><UID>{}</UID><TaskUID>{}</TaskUID><ResourceUID>{}</ResourceUID><Units>{}</Units><Work>{}</Work>\
             <Start>{}</Start><Finish>{}</Finish></Assignment>",
            u32_at(r, A_UID),
            u32_at(r, A_TASK),
            i32_at(r, A_RESOURCE),
            f64_at(r, A_UNITS) / 10000.0,
            minutes_duration(f64_at(r, A_WORK) / 1000.0),
            iso(start),
            iso(finish)
        );
    }
    x.push_str("</Assignments>");
}
