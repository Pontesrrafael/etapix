//! Reader for Microsoft Project XML (MSPDI), the documented interchange format.
//!
//! Units: durations are ISO-8601 (`PT40H0M0S`), link lags and slack are tenths of working
//! minutes, `LevelingDelay` is tenths of *elapsed* minutes (`LevelingDelayFormat` 8 = "ed").

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use roxmltree::{Document, Node};

use crate::calendar::{Calendar, Calendars, Exception, Shift};
use crate::model::{Assignment, Constraint, Link, LinkType, Piece, Progress, Project, Rate, Resource, ResourceKind, Task, TaskType, Wm};

pub const NS: &str = "http://schemas.microsoft.com/project";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MspdiError(pub String);

impl std::fmt::Display for MspdiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MSPDI: {}", self.0)
    }
}

impl std::error::Error for MspdiError {}

type Result<T> = std::result::Result<T, MspdiError>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(MspdiError(msg.into()))
}

/// Scheduled values Project wrote for one task, used as the expected side of golden tests.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskResult {
    pub uid: i64,
    pub name: String,
    pub start: NaiveDateTime,
    pub finish: NaiveDateTime,
    pub early_start: NaiveDateTime,
    pub late_start: NaiveDateTime,
    /// Working minutes.
    pub total_slack: Wm,
    pub free_slack: Wm,
    pub critical: bool,
    /// Elapsed minutes.
    pub leveling_delay: i64,
    /// Working minutes.
    pub duration: Wm,
    /// Minutes of work.
    pub work: Wm,
    /// Currency units.
    pub cost: f64,
}

/// One assignment as Project wrote it.
#[derive(Debug, Clone, PartialEq)]
pub struct AssignmentResult {
    pub task_uid: i64,
    pub resource: String,
    pub units: f64,
    /// Minutes of work.
    pub work: Wm,
    pub start: NaiveDateTime,
    pub finish: NaiveDateTime,
    /// Currency units.
    pub cost: f64,
}

pub fn read_assignment_results(xml: &str) -> Result<Vec<AssignmentResult>> {
    let doc = Document::parse(xml).map_err(|e| MspdiError(e.to_string()))?;
    let root = doc.root_element();
    let names: HashMap<i64, String> = child(root, "Resources")
        .into_iter()
        .flat_map(|r| children(r, "Resource"))
        .map(|r| Ok((int(r, "UID")?, text(r, "Name").unwrap_or_default().to_string())))
        .collect::<Result<_>>()?;
    child(root, "Assignments")
        .into_iter()
        .flat_map(|a| children(a, "Assignment"))
        .filter(|a| text(*a, "ResourceUID") != Some("-65535"))
        .map(|a| {
            Ok(AssignmentResult {
                task_uid: int(a, "TaskUID")?,
                resource: names.get(&int(a, "ResourceUID")?).cloned().unwrap_or_default(),
                units: float(a, "Units")?,
                work: parse_duration(req(a, "Work")?)?,
                start: parse_datetime(req(a, "Start")?)?,
                finish: parse_datetime(req(a, "Finish")?)?,
                cost: text(a, "Cost").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100.0,
            })
        })
        .collect()
}

fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|c| c.has_tag_name((NS, name)))
}

fn children<'a>(node: Node<'a, 'a>, name: &'a str) -> impl Iterator<Item = Node<'a, 'a>> {
    node.children().filter(move |c| c.has_tag_name((NS, name)))
}

fn text<'a>(node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
    child(node, name).and_then(|c| c.text())
}

fn req<'a>(node: Node<'a, 'a>, name: &str) -> Result<&'a str> {
    text(node, name).ok_or_else(|| MspdiError(format!("missing <{name}>")))
}

fn int(node: Node, name: &str) -> Result<i64> {
    let s = req(node, name)?;
    s.trim().parse().map_err(|_| MspdiError(format!("<{name}> is not an integer: {s}")))
}

fn float(node: Node, name: &str) -> Result<f64> {
    let s = req(node, name)?;
    s.trim().parse().map_err(|_| MspdiError(format!("<{name}> is not a number: {s}")))
}

pub fn parse_datetime(s: &str) -> Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").or_else(|_| err(format!("bad datetime {s}")))
}

/// `PT40H0M0S` -> 2400 minutes.
pub fn parse_duration(s: &str) -> Result<Wm> {
    let (neg, body) = s.strip_prefix('-').map_or((false, s), |b| (true, b));
    let body = body.strip_prefix("PT").ok_or_else(|| MspdiError(format!("bad duration {s}")))?;
    let mut minutes = 0f64;
    let mut num = String::new();
    for c in body.chars() {
        match c {
            'H' | 'M' | 'S' => {
                let v: f64 = num.parse().map_err(|_| MspdiError(format!("bad duration {s}")))?;
                minutes += match c { 'H' => v * 60.0, 'M' => v, _ => v / 60.0 };
                num.clear();
            }
            _ => num.push(c),
        }
    }
    let m = minutes.round() as Wm;
    Ok(if neg { -m } else { m })
}

fn minute_of_day(s: &str) -> Result<u32> {
    let t = chrono::NaiveTime::parse_from_str(s, "%H:%M:%S").or_else(|_| err(format!("bad time {s}")))?;
    use chrono::Timelike;
    // 24:00:00 is not a NaiveTime; MSPDI writes midnight end-of-day as 00:00:00.
    Ok(match t.hour() * 60 + t.minute() { 0 => 24 * 60, m => m })
}

fn working_times(node: Node, working: bool) -> Result<Vec<Shift>> {
    if !working {
        return Ok(vec![]);
    }
    child(node, "WorkingTimes")
        .into_iter()
        .flat_map(|w| children(w, "WorkingTime"))
        .map(|w| Ok((minute_of_day(req(w, "FromTime")?)?, minute_of_day(req(w, "ToTime")?)?)))
        .collect()
}

/// Reads every calendar, resolving derived calendars (resource calendars) onto their base:
/// listed weekdays replace the base's, exceptions add to (and win over) the base's.
/// Returns the calendars and a map from calendar UID to index.
fn read_calendars(root: Node) -> Result<(Vec<Calendar>, HashMap<i64, usize>)> {
    let nodes: Vec<Node> = child(root, "Calendars").into_iter().flat_map(|c| children(c, "Calendar")).collect();
    let by_uid: HashMap<i64, Node> = nodes.iter().map(|n| Ok((int(*n, "UID")?, *n))).collect::<Result<_>>()?;
    let mut calendars = Vec::new();
    let mut index = HashMap::new();
    fn resolve<'a>(
        uid: i64,
        by_uid: &HashMap<i64, Node<'a, 'a>>,
        calendars: &mut Vec<Calendar>,
        index: &mut HashMap<i64, usize>,
        depth: usize,
    ) -> Result<usize> {
        if let Some(&i) = index.get(&uid) {
            return Ok(i);
        }
        let node = *by_uid.get(&uid).ok_or_else(|| MspdiError(format!("calendar {uid} not found")))?;
        let base = int(node, "BaseCalendarUID").unwrap_or(-1);
        let mut cal = if text(node, "IsBaseCalendar") != Some("1") && base >= 0 && depth < 8 {
            let b = resolve(base, by_uid, calendars, index, depth + 1)?;
            calendars[b].clone()
        } else {
            Calendar { name: String::new(), weekdays: Default::default(), exceptions: vec![] }
        };
        cal.name = text(node, "Name").unwrap_or_default().to_string();
        for day in child(node, "WeekDays").into_iter().flat_map(|w| children(w, "WeekDay")) {
            // DayType 0 is a dated exception; the same data appears under <Exceptions>.
            let day_type = int(day, "DayType")?;
            if (1..=7).contains(&day_type) {
                cal.weekdays[(day_type - 1) as usize] = working_times(day, req(day, "DayWorking")? == "1")?;
            }
        }
        for exc in child(node, "Exceptions").into_iter().flat_map(|e| children(e, "Exception")) {
            let period = child(exc, "TimePeriod").ok_or_else(|| MspdiError("exception without period".into()))?;
            let from: NaiveDate = parse_datetime(req(period, "FromDate")?)?.date();
            let to: NaiveDate = parse_datetime(req(period, "ToDate")?)?.date();
            let shifts = working_times(exc, req(exc, "DayWorking")? == "1")?;
            let every = text(exc, "Period").and_then(|p| p.parse::<i64>().ok()).unwrap_or(1).max(1);
            match text(exc, "Type").unwrap_or("1") {
                // Daily: one block, or every `Period` days.
                "1" if every == 1 => cal.exceptions.push(Exception { from, to, shifts }),
                "1" => {
                    let mut d = from;
                    while d <= to {
                        cal.exceptions.push(Exception { from: d, to: d, shifts: shifts.clone() });
                        d += chrono::Duration::days(every);
                    }
                }
                // Weekly: the DaysOfWeek bits (Sunday = 1 ... Saturday = 64) every `Period` weeks.
                "6" => {
                    let bits = int(exc, "DaysOfWeek")?;
                    let week0 = from - chrono::Duration::days(from.weekday().num_days_from_sunday() as i64);
                    let mut d = from;
                    while d <= to {
                        let week = (d - week0).num_days() / 7;
                        if week % every == 0 && bits & (1 << d.weekday().num_days_from_sunday()) != 0 {
                            cal.exceptions.push(Exception { from: d, to: d, shifts: shifts.clone() });
                        }
                        d += chrono::Duration::days(1);
                    }
                }
                // Monthly (4 by day, 5 by position) every `Period` months; yearly (2 by day,
                // 3 by position) in `Month` (0-based).
                t @ ("2" | "3" | "4" | "5") => {
                    let yearly = t == "2" || t == "3";
                    let month = if yearly { Some(int(exc, "Month")? as u32 + 1) } else { None };
                    let by_day = t == "2" || t == "4";
                    let (mut y, mut m) = (from.year(), from.month());
                    let mut k = 0;
                    while NaiveDate::from_ymd_opt(y, m, 1).unwrap() <= to {
                        let wanted = match month {
                            Some(mm) => m == mm,
                            None => k % every == 0,
                        };
                        if wanted {
                            let day = if by_day {
                                NaiveDate::from_ymd_opt(y, m, int(exc, "MonthDay")? as u32)
                            } else {
                                nth_in_month(y, m, int(exc, "MonthPosition")?, int(exc, "MonthItem")?)?
                            };
                            if let Some(d) = day.filter(|d| from <= *d && *d <= to) {
                                cal.exceptions.push(Exception { from: d, to: d, shifts: shifts.clone() });
                            }
                        }
                        k += 1;
                        (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                    }
                }
                t => return err(format!("calendar exception type {t} is not supported")),
            }
        }
        calendars.push(cal);
        index.insert(uid, calendars.len() - 1);
        Ok(calendars.len() - 1)
    }
    for &uid in by_uid.keys().collect::<std::collections::BTreeSet<_>>() {
        resolve(uid, &by_uid, &mut calendars, &mut index, 0)?;
    }
    Ok((calendars, index))
}

/// The `position`-th (0-3, or 4 = last) day of kind `item` (3 = Sunday ... 9 = Saturday,
/// 0 = any day) in a month.
fn nth_in_month(y: i32, m: u32, position: i64, item: i64) -> Result<Option<NaiveDate>> {
    let days: Vec<NaiveDate> = (1..=31)
        .filter_map(|d| NaiveDate::from_ymd_opt(y, m, d))
        .filter(|d| match item {
            0 => true,
            3..=9 => d.weekday().num_days_from_sunday() as i64 == item - 3,
            _ => false,
        })
        .collect();
    if !(0..=9).contains(&item) || matches!(item, 1 | 2) {
        return err(format!("calendar exception day kind {item} is not supported yet"));
    }
    Ok(match position {
        4 => days.last().copied(),
        p => days.get(p as usize).copied(),
    })
}

/// A rate value in currency per hour (per unit for materials, format 8).
fn per_hour(value: f64, format: Option<&str>) -> f64 {
    match format {
        Some("1") => value * 60.0,
        Some("3") => value / 8.0,
        Some("4") => value / 40.0,
        Some("5") => value / 160.0,
        _ => value,
    }
}

/// Cost rate table A: its dated `<Rate>` periods, or the resource's own rates when there are none.
/// Rates are plain currency per period; cost per use is in cents.
fn read_rates(node: Node) -> Result<Vec<Rate>> {
    let num = |n: Node, tag: &str| text(n, tag).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let rate = |n: Node, from: NaiveDateTime| Rate {
        from,
        standard: per_hour(num(n, "StandardRate"), text(n, "StandardRateFormat")),
        overtime: per_hour(num(n, "OvertimeRate"), text(n, "OvertimeRateFormat")),
        per_use: num(n, "CostPerUse") / 100.0,
    };
    let mut rates = Vec::new();
    for r in child(node, "Rates").into_iter().flat_map(|r| children(r, "Rate")) {
        if text(r, "RateTable").unwrap_or("0") == "0" {
            rates.push(rate(r, parse_datetime(req(r, "RatesFrom")?)?));
        }
    }
    if rates.is_empty() {
        let epoch = NaiveDate::from_ymd_opt(1984, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
        rates.push(rate(node, epoch));
    } else if let Some(first) = rates.first_mut() {
        // The first period's per-use cost is written in cents on the resource itself.
        first.per_use = first.per_use.max(num(node, "CostPerUse") / 100.0);
    }
    rates.sort_by_key(|r| r.from);
    Ok(rates)
}

/// MSPDI duration and lag formats counted in elapsed time (em, eh, ed, ew, emo and their
/// estimated variants).
fn is_elapsed_format(format: Option<&str>) -> bool {
    matches!(format.and_then(|f| f.parse::<i64>().ok()), Some(4 | 6 | 8 | 10 | 12 | 36 | 38 | 40 | 42 | 44))
}

fn link_type(code: i64) -> Result<LinkType> {
    Ok(match code {
        0 => LinkType::FinishToFinish,
        1 => LinkType::FinishToStart,
        2 => LinkType::StartToFinish,
        3 => LinkType::StartToStart,
        _ => return err(format!("unknown link type {code}")),
    })
}

fn is_real_task(node: Node) -> bool {
    text(node, "OutlineLevel") != Some("0") && text(node, "IsNull") != Some("1")
}

pub fn read_project(xml: &str) -> Result<Project> {
    let doc = Document::parse(xml).map_err(|e| MspdiError(e.to_string()))?;
    let root = doc.root_element();
    let start = parse_datetime(req(root, "StartDate")?)?;
    let (calendars, cal_index) = read_calendars(root)?;
    let calendar_of = |uid: i64| -> Result<Option<usize>> {
        if uid < 0 {
            return Ok(None);
        }
        cal_index.get(&uid).copied().map(Some).ok_or_else(|| MspdiError(format!("calendar {uid} not found")))
    };
    let calendar = calendar_of(int(root, "CalendarUID")?)?.ok_or_else(|| MspdiError("no project calendar".into()))?;

    let task_nodes: Vec<Node> = child(root, "Tasks")
        .into_iter()
        .flat_map(|t| children(t, "Task"))
        .filter(|t| is_real_task(*t))
        .collect();
    let index: HashMap<i64, usize> =
        task_nodes.iter().enumerate().map(|(i, t)| Ok((int(*t, "UID")?, i))).collect::<Result<_>>()?;

    let mut tasks = Vec::with_capacity(task_nodes.len());
    // Open ancestors by outline level, to find each task's parent.
    let mut open: Vec<(i64, usize)> = Vec::new();
    for (i, node) in task_nodes.iter().enumerate() {
        let summary = text(*node, "Summary") == Some("1");
        if summary && !matches!(int(*node, "ConstraintType")?, 0 | 4) {
            return err("constraints other than start-no-earlier-than on summary tasks are not supported yet");
        }
        let level = int(*node, "OutlineLevel")?;
        while open.last().is_some_and(|&(l, _)| l >= level) {
            open.pop();
        }
        let parent = open.last().map(|&(_, p)| p);
        open.push((level, i));
        let date = || parse_datetime(req(*node, "ConstraintDate")?);
        let constraint = match int(*node, "ConstraintType")? {
            0 => Constraint::AsSoonAsPossible,
            1 => Constraint::AsLateAsPossible,
            2 => Constraint::MustStartOn(date()?),
            3 => Constraint::MustFinishOn(date()?),
            4 => Constraint::StartNoEarlierThan(date()?),
            5 => Constraint::StartNoLaterThan(date()?),
            6 => Constraint::FinishNoEarlierThan(date()?),
            7 => Constraint::FinishNoLaterThan(date()?),
            c => return err(format!("unknown constraint type {c}")),
        };
        let deadline = text(*node, "Deadline").map(parse_datetime).transpose()?;
        let preds = children(*node, "PredecessorLink")
            .map(|l| {
                let uid = int(l, "PredecessorUID")?;
                Ok(Link {
                    pred: *index.get(&uid).ok_or_else(|| MspdiError(format!("unknown predecessor {uid}")))?,
                    kind: link_type(int(l, "Type")?)?,
                    lag: int(l, "LinkLag")? / 10,
                    lag_elapsed: is_elapsed_format(text(l, "LagFormat")),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let task_type = match int(*node, "Type")? {
            0 => TaskType::FixedUnits,
            1 => TaskType::FixedDuration,
            2 => TaskType::FixedWork,
            t => return err(format!("unknown task type {t}")),
        };
        tasks.push(Task {
            uid: int(*node, "UID")?,
            id: int(*node, "ID")?,
            name: text(*node, "Name").unwrap_or_default().to_string(),
            duration: parse_duration(req(*node, "Duration")?)?,
            elapsed: is_elapsed_format(text(*node, "DurationFormat")),
            task_type,
            effort_driven: text(*node, "EffortDriven") == Some("1"),
            priority: int(*node, "Priority")? as u32,
            constraint,
            deadline,
            leveling_can_split: text(*node, "LevelingCanSplit") != Some("0"),
            preds,
            summary,
            parent,
            calendar: calendar_of(text(*node, "CalendarUID").map_or(Ok(-1), |_| int(*node, "CalendarUID"))?)?,
            ignore_resource_calendar: text(*node, "IgnoreResourceCalendar") == Some("1"),
            manual: if text(*node, "Manual") == Some("1") {
                Some(parse_datetime(text(*node, "ManualStart").or(text(*node, "Start")).unwrap_or_default())?)
            } else {
                None
            },
            // Money is stored in cents.
            fixed_cost: text(*node, "FixedCost").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100.0,
            fixed_cost_accrual: text(*node, "FixedCostAccrual").and_then(|v| v.parse().ok()).unwrap_or(3),
            leveling_delay: text(*node, "LevelingDelay").map_or(Ok(0), |_| int(*node, "LevelingDelay"))? / 10,
            progress: Progress {
                percent_complete: text(*node, "PercentComplete").map_or(Ok(0), |_| int(*node, "PercentComplete"))? as u32,
                actual_start: text(*node, "ActualStart").map(parse_datetime).transpose()?,
                actual_finish: text(*node, "ActualFinish").map(parse_datetime).transpose()?,
                resume: text(*node, "Resume").map(parse_datetime).transpose()?,
            },
        });
    }

    let mut resources = Vec::new();
    let mut res_index = HashMap::new();
    for node in child(root, "Resources").into_iter().flat_map(|r| children(r, "Resource")) {
        let uid = int(node, "UID")?;
        if uid == 0 || text(node, "IsNull") == Some("1") {
            continue; // UID 0 is Project's placeholder resource
        }
        res_index.insert(uid, resources.len());
        let kind = match text(node, "Type") {
            Some("0") => ResourceKind::Material,
            Some("2") => ResourceKind::Cost,
            _ => ResourceKind::Work,
        };
        resources.push(Resource {
            uid,
            name: text(node, "Name").unwrap_or_default().to_string(),
            kind,
            rates: read_rates(node)?,
            max_units: if kind == ResourceKind::Work { float(node, "MaxUnits")? } else { 1.0 },
            calendar: calendar_of(text(node, "CalendarUID").map_or(Ok(-1), |_| int(node, "CalendarUID"))?)?,
        });
    }

    // Assignment timing is measured on the task's calendar, which depends on the assignments
    // themselves; read them first, then measure.
    let mut assignments = Vec::new();
    let mut timing = Vec::new();
    for node in child(root, "Assignments").into_iter().flat_map(|a| children(a, "Assignment")) {
        let (task_uid, res_uid) = (int(node, "TaskUID")?, int(node, "ResourceUID")?);
        let (Some(&task), Some(&resource)) = (index.get(&task_uid), res_index.get(&res_uid)) else {
            continue; // unassigned placeholder rows
        };
        let mut a = Assignment::uniform(task, resource, float(node, "Units")?, 0, 0.0);
        a.overtime_work = text(node, "OvertimeWork").map(parse_duration).transpose()?.unwrap_or(0);
        assignments.push(a);
        timing.push((
            parse_datetime(req(task_nodes[task], "Start")?)?,
            parse_datetime(req(node, "Start")?)?,
            parse_datetime(req(node, "Finish")?)?,
            parse_duration(req(node, "Work")?)? as f64,
            // Actual (2) and remaining (1) work per stretch, when present.
            children(node, "TimephasedData")
                .filter(|t| matches!(text(*t, "Type"), Some("1" | "2")))
                .map(|t| {
                    Ok((
                        parse_datetime(req(t, "Start")?)?,
                        parse_datetime(req(t, "Finish")?)?,
                        parse_duration(req(t, "Value")?)?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?,
        ));
    }
    let mut project = Project {
        start,
        critical_slack_limit: text(root, "CriticalSlackLimit").map_or(Ok(0), |_| int(root, "CriticalSlackLimit"))?,
        multiple_critical_paths: text(root, "MultipleCriticalPaths") == Some("1"),
        // MSPDI writes 0 for "honor constraints" on (see mspdi_write).
        honor_constraints: text(root, "HonorConstraints") != Some("1"),
        status_date: text(root, "StatusDate").filter(|d| !d.starts_with("NA")).map(parse_datetime).transpose()?,
        calendars,
        calendar,
        tasks,
        resources,
        assignments,
    };
    let cals = Calendars::new(&project).map_err(|e| MspdiError(e.0))?;
    for (k, (a, (task_start, a_start, a_finish, work, mut chunks))) in project.assignments.iter_mut().zip(timing).enumerate() {
        // Lengths count on the assignment's own calendar, the offset on the task's.
        let (tl, atl, clock) = (cals.task(a.task), cals.assignment(k), &cals.clock);
        let len = atl.w(clock.at(a_finish)) - atl.w(clock.at(a_start));
        a.offset = tl.w(clock.at(a_start)) - tl.w(clock.at(task_start));
        // The timephased stretches give the real rates (a stretched fixed-work assignment that
        // later got more work runs slow, then at full units); without them, one flat rate.
        chunks.sort_by_key(|c| c.0);
        let mut profile: Vec<Piece> = Vec::new();
        for (s, f, value) in chunks {
            let l = atl.w(clock.at(f)) - atl.w(clock.at(s));
            if l <= 0 {
                continue;
            }
            let rate = value as f64 / l as f64;
            match profile.last_mut() {
                Some(last) if (last.rate - rate).abs() < 1e-6 => last.len += l,
                _ => profile.push(Piece { len: l, rate }),
            }
        }
        let covered: Wm = profile.iter().map(|p| p.len).sum();
        a.profile = if !profile.is_empty() && covered == len {
            // Timephased stretches hold regular work only; scale them to the total work
            // (overtime included) so the profile's work matches the assignment's.
            let timephased: f64 = profile.iter().map(|p| p.len as f64 * p.rate).sum();
            if timephased > 0.0 && (timephased - work).abs() > 0.5 {
                profile.iter_mut().for_each(|p| p.rate *= work / timephased);
            }
            profile
        } else {
            vec![Piece { len, rate: if len > 0 { work / len as f64 } else { 0.0 } }]
        };
    }
    Ok(project)
}

/// Reads the scheduled values Project computed, keyed by task UID order.
pub fn read_results(xml: &str) -> Result<Vec<TaskResult>> {
    let doc = Document::parse(xml).map_err(|e| MspdiError(e.to_string()))?;
    let root = doc.root_element();
    child(root, "Tasks")
        .into_iter()
        .flat_map(|t| children(t, "Task"))
        .filter(|t| is_real_task(*t))
        .map(|t| {
            Ok(TaskResult {
                uid: int(t, "UID")?,
                name: text(t, "Name").unwrap_or_default().to_string(),
                start: parse_datetime(req(t, "Start")?)?,
                finish: parse_datetime(req(t, "Finish")?)?,
                early_start: parse_datetime(req(t, "EarlyStart")?)?,
                late_start: parse_datetime(req(t, "LateStart")?)?,
                total_slack: int(t, "TotalSlack")? / 10,
                free_slack: int(t, "FreeSlack")? / 10,
                critical: req(t, "Critical")? == "1",
                leveling_delay: int(t, "LevelingDelay")? / 10,
                duration: parse_duration(req(t, "Duration")?)?,
                work: parse_duration(req(t, "Work")?)?,
                cost: text(t, "Cost").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100.0,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timephased_profile_keeps_slow_then_full_rate() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/b3/B3-FW-4-work-80h.xml");
        let p = read_project(&std::fs::read_to_string(path).unwrap()).unwrap();
        let r2 = p.assignments.iter().find(|a| p.resources[a.resource].name == "R2").unwrap();
        let rates: Vec<(Wm, String)> = r2.profile.iter().map(|x| (x.len, format!("{:.3}", x.rate))).collect();
        assert_eq!(rates, vec![(48 * 60, "0.333".into()), (24 * 60, "1.000".into())]);
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("PT40H0M0S"), Ok(2400));
        assert_eq!(parse_duration("PT0H30M0S"), Ok(30));
        assert_eq!(parse_duration("-PT8H0M0S"), Ok(-480));
    }
}
