use chrono::NaiveDateTime;

use crate::calendar::Calendar;

/// Minutes of working time on some calendar (or of elapsed time for elapsed durations).
pub type Wm = i64;

/// Task priority at which a task is never moved by leveling.
pub const PRIORITY_DO_NOT_LEVEL: u32 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    FinishToFinish,
    FinishToStart,
    StartToFinish,
    StartToStart,
}

#[derive(Debug, Clone)]
pub struct Link {
    /// Index of the predecessor in [`Project::tasks`].
    pub pred: usize,
    pub kind: LinkType,
    /// Lag in minutes (negative = lead): working minutes on the project calendar, or elapsed
    /// minutes when `lag_elapsed`.
    pub lag: Wm,
    pub lag_elapsed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    AsSoonAsPossible,
    AsLateAsPossible,
    MustStartOn(NaiveDateTime),
    MustFinishOn(NaiveDateTime),
    StartNoEarlierThan(NaiveDateTime),
    StartNoLaterThan(NaiveDateTime),
    FinishNoEarlierThan(NaiveDateTime),
    FinishNoLaterThan(NaiveDateTime),
}

/// Which of duration, units and work an edit keeps fixed (see [`crate::edit`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    FixedUnits,
    FixedDuration,
    FixedWork,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub uid: i64,
    pub id: i64,
    pub name: String,
    /// Duration in working minutes of the task's calendar, or elapsed minutes when `elapsed`.
    pub duration: Wm,
    pub elapsed: bool,
    pub task_type: TaskType,
    /// Adding or removing resources keeps total work (always true for fixed work).
    pub effort_driven: bool,
    pub priority: u32,
    pub constraint: Constraint,
    /// Limits the late finish without moving the task.
    pub deadline: Option<NaiveDateTime>,
    pub leveling_can_split: bool,
    pub preds: Vec<Link>,
    /// Summary tasks are not scheduled; they roll up their subtasks (see [`crate::cpm`]).
    pub summary: bool,
    pub parent: Option<usize>,
    /// Index into [`Project::calendars`].
    pub calendar: Option<usize>,
    pub ignore_resource_calendar: bool,
    pub progress: Progress,
    /// Manually scheduled: the task sits at `manual_start` whatever its links say.
    pub manual: Option<NaiveDateTime>,
    pub fixed_cost: f64,
    /// Baseline 0, when saved.
    pub baseline: Option<Baseline>,
    /// When fixed cost accrues (MSPDI code: 1 start, 2 prorated, 3 end); kept for round trips.
    pub fixed_cost_accrual: u8,
    /// Leveling delay saved in the file (elapsed minutes): reopening keeps the leveled dates.
    pub leveling_delay: i64,
}

/// A task's saved plan (baseline 0): dates, duration (working minutes), work (minutes), cost.
#[derive(Debug, Clone, PartialEq)]
pub struct Baseline {
    pub start: NaiveDateTime,
    pub finish: NaiveDateTime,
    pub duration: Wm,
    pub work: f64,
    pub cost: f64,
}

/// Tracking data. An actual start pins the start (beating links and constraints); an actual
/// finish pins the finish; a resume later than where the done work ends splits the task.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub percent_complete: u32,
    pub actual_start: Option<NaiveDateTime>,
    pub actual_finish: Option<NaiveDateTime>,
    pub resume: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Work,
    Material,
    Cost,
}

/// Rates in force from `from` on (cost rate table A), in currency per hour (per unit for
/// materials).
#[derive(Debug, Clone, PartialEq)]
pub struct Rate {
    pub from: NaiveDateTime,
    pub standard: f64,
    pub overtime: f64,
    pub per_use: f64,
}

#[derive(Debug, Clone)]
pub struct Resource {
    pub uid: i64,
    pub name: String,
    pub kind: ResourceKind,
    /// Ordered by `from`; the first applies before its own date too.
    pub rates: Vec<Rate>,
    /// 1.0 = 100%.
    pub max_units: f64,
    /// Index into [`Project::calendars`]; `None` = the project calendar.
    pub calendar: Option<usize>,
}

impl Resource {
    pub fn rate_at(&self, at: NaiveDateTime) -> Rate {
        let zero = Rate { from: at, standard: 0.0, overtime: 0.0, per_use: 0.0 };
        self.rates.iter().rev().find(|r| r.from <= at).or(self.rates.first()).cloned().unwrap_or(zero)
    }
}

/// A stretch of an assignment's work at a constant rate (units of the resource).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub len: Wm,
    pub rate: f64,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub task: usize,
    pub resource: usize,
    /// Nominal units (1.0 = 100%). The actual rate lives in `profile` and can differ: a
    /// fixed-work task stretched to a longer duration keeps its units but works slower.
    pub units: f64,
    /// Working minutes (on the task's calendar) between the task start and the assignment start.
    pub offset: Wm,
    /// Part of the work done as overtime (minutes), charged at the overtime rate.
    pub overtime_work: Wm,
    pub profile: Vec<Piece>,
}

impl Assignment {
    /// An assignment working `len` minutes at `rate` from the task start.
    pub fn uniform(task: usize, resource: usize, units: f64, len: Wm, rate: f64) -> Self {
        Assignment { task, resource, units, offset: 0, overtime_work: 0, profile: vec![Piece { len, rate }] }
    }

    /// Work in minutes.
    pub fn work(&self) -> f64 {
        self.profile.iter().map(|p| p.len as f64 * p.rate).sum()
    }

    /// Working minutes from the assignment start to its finish.
    pub fn span(&self) -> Wm {
        self.profile.iter().map(|p| p.len).sum()
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub start: NaiveDateTime,
    /// Tasks with total slack at or below this (working minutes) are critical.
    pub critical_slack_limit: Wm,
    /// Every task without successors closes its own critical path (its early finish is its late
    /// finish) instead of all of them ending at the project finish.
    pub multiple_critical_paths: bool,
    /// "Tasks will always honor their constraint dates" (on by default): when off, links win
    /// over must/no-later-than constraints in the forward pass.
    pub honor_constraints: bool,
    /// Date progress was reported to (rescheduling moves unfinished work past it).
    pub status_date: Option<NaiveDateTime>,
    pub calendars: Vec<Calendar>,
    /// Index of the project calendar in `calendars`.
    pub calendar: usize,
    pub tasks: Vec<Task>,
    pub resources: Vec<Resource>,
    pub assignments: Vec<Assignment>,
}
