//! The open document: a project, its schedule and the edits the UI can make. Kept free of UI
//! code so it can be tested directly.

use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, Local, NaiveDateTime};
use pjsched::calendar::Calendars;
use pjsched::cost::{costs, Costs};
use pjsched::cpm::{self, TaskSchedule};
use pjsched::edit;
use pjsched::leveling::{self, LevelingOptions};
use pjsched::model::{Constraint, Link, LinkType, Project, Resource, ResourceKind, Task, Wm};
use pjsched::mspdi::read_project;
use pjsched::mspdi_write::write_project;
use pjsched::tracking::{save_baseline, tracking, Tracking};

/// Working minutes in a day of the standard calendar (durations are entered in days).
pub const DAY: Wm = 480;

/// An edit committed when a Gantt gesture ends. Previews never change the document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GanttEdit {
    Start(usize, NaiveDateTime),
    Duration(usize, Wm),
    Percent(usize, u32),
    Link(usize, usize),
}

pub struct Doc {
    pub project: Project,
    pub path: Option<PathBuf>,
    pub cals: Calendars,
    pub schedule: Vec<TaskSchedule>,
    pub costs: Costs,
    pub tracking: Vec<Tracking>,
    pub dirty: bool,
    /// Bumped by every change, so a caller can tell whether an action changed anything.
    version: u64,
    /// The project before the action in progress (see `begin`/`end`).
    pending: Option<Project>,
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// The largest task UID this document has used: a deleted task's UID is never given to a new
    /// one, as in Project (a `.mpp` keeps the deleted task's record under it).
    last_uid: i64,
}

/// How many actions Desfazer can go back.
const UNDO_LIMIT: usize = 200;

impl Doc {
    /// An empty project starting next Monday 08:00.
    pub fn blank() -> Self {
        let today = Local::now().date_naive();
        let monday = today + Duration::days((7 - today.weekday().num_days_from_monday() as i64) % 7);
        let start = monday.and_hms_opt(8, 0, 0).unwrap();
        Doc::from_project(Project::new(start), None).expect("an empty project always schedules")
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let xml = if crate::mpp::is_mpp(path) {
            crate::mpp::read_mpp(path)?
        } else {
            std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?
        };
        let project = read_project(&xml).map_err(|e| e.to_string())?;
        Doc::from_project(project, Some(path.to_path_buf()))
    }

    fn from_project(project: Project, path: Option<PathBuf>) -> Result<Self, String> {
        let (cals, schedule, costs, tracking) = compute(&project)?;
        let last_uid = project.tasks.iter().map(|t| t.uid).max().unwrap_or(0);
        Ok(Doc { project, path, cals, schedule, costs, tracking, dirty: false, version: 0, pending: None, undo: Vec::new(), redo: Vec::new(), last_uid })
    }

    /// Writes `.mpp` (natively into the original file, or through the installed Project) or Project
    /// XML, by the file's extension.
    /// Returns a notice for the user when the save had to rebuild the `.mpp`.
    pub fn save(&mut self, path: &Path) -> Result<Option<String>, String> {
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let xml = write_project(&self.project, &self.cals, &self.schedule, &name);
        let notice = if crate::mpp::is_mpp(path) {
            crate::mpp::write_mpp(path, &xml, self.path.as_deref())?
        } else {
            std::fs::write(path, xml).map_err(|e| format!("{}: {e}", path.display()))?;
            None
        };
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(notice)
    }

    /// Applies `change` and reschedules; a change that cannot be scheduled (a dependency cycle,
    /// an unsupported combination) is rolled back and reported.
    pub fn apply(&mut self, change: impl FnOnce(&mut Project) -> Result<(), String>) -> Result<(), String> {
        let before = self.project.clone();
        let outcome = change(&mut self.project).and_then(|()| compute(&self.project));
        match outcome {
            Ok((cals, schedule, costs, tracking)) => {
                (self.cals, self.schedule, self.costs, self.tracking) = (cals, schedule, costs, tracking);
                self.dirty = true;
                self.version += 1;
                Ok(())
            }
            Err(e) => {
                self.project = before;
                Err(e)
            }
        }
    }

    #[cfg(test)]
    pub fn add_task(&mut self, name: &str, days: f64) -> Result<usize, String> {
        let uid = self.next_task_uid();
        let id = self.project.tasks.len() as i64 + 1;
        let duration = (days * DAY as f64).round() as Wm;
        self.apply(|p| {
            p.tasks.push(Task::new(uid, id, name, duration));
            Ok(())
        })?;
        Ok(self.project.tasks.len() - 1)
    }

    /// Removes a task and every link, assignment and parent reference to it.
    /// A UID no task of this document has had.
    fn next_task_uid(&mut self) -> i64 {
        self.last_uid = self.last_uid.max(self.project.tasks.iter().map(|t| t.uid).max().unwrap_or(0)) + 1;
        self.last_uid
    }

    pub fn delete_task(&mut self, i: usize) -> Result<(), String> {
        self.apply(|p| {
            let remap = |k: usize| if k > i { k - 1 } else { k };
            p.tasks.remove(i);
            p.assignments.retain(|a| a.task != i);
            p.assignments.iter_mut().for_each(|a| a.task = remap(a.task));
            for t in &mut p.tasks {
                t.preds.retain(|l| l.pred != i);
                t.preds.iter_mut().for_each(|l| l.pred = remap(l.pred));
                t.parent = t.parent.filter(|&x| x != i).map(remap);
            }
            renumber(p);
            refresh_summaries(p);
            Ok(())
        })
    }

    /// Inserts a task at row `at` (an empty row past the end appends), shifting every reference.
    pub fn insert_task(&mut self, at: usize, name: &str, days: f64) -> Result<usize, String> {
        let at = at.min(self.project.tasks.len());
        let uid = self.next_task_uid();
        let duration = (days * DAY as f64).round() as Wm;
        self.apply(|p| {
            let shift = |k: usize| if k >= at { k + 1 } else { k };
            for t in &mut p.tasks {
                t.preds.iter_mut().for_each(|l| l.pred = shift(l.pred));
                t.parent = t.parent.map(shift);
            }
            p.assignments.iter_mut().for_each(|a| a.task = shift(a.task));
            let mut task = Task::new(uid, 0, name, duration);
            // A new row right under a summary (or one of its subtasks) joins it, as in Project.
            task.parent = match at.checked_sub(1).map(|k| &p.tasks[k]) {
                Some(prev) if prev.summary => Some(at - 1),
                Some(prev) => prev.parent,
                None => None,
            };
            p.tasks.insert(at, task);
            renumber(p);
            Ok(())
        })?;
        Ok(at)
    }

    pub fn add_milestone(&mut self, at: usize) -> Result<usize, String> {
        self.insert_task(at, "Novo marco", 0.0)
    }

    /// Outline level of task `i` (1 = top).
    pub fn outline_level(&self, i: usize) -> usize {
        let (mut l, mut up) = (1, self.project.tasks[i].parent);
        while let Some(p) = up {
            l += 1;
            up = self.project.tasks[p].parent;
        }
        l
    }

    /// Indents a task under the nearest task above at its level, which becomes a summary
    /// (Project's "Recuar Tarefa").
    pub fn indent(&mut self, i: usize) -> Result<(), String> {
        let level = self.outline_level(i);
        let parent = (0..i).rev().find(|&k| self.outline_level(k) == level).ok_or("não há tarefa acima no mesmo nível")?;
        self.apply(|p| {
            p.tasks[i].parent = Some(parent);
            refresh_summaries(p);
            Ok(())
        })
    }

    /// Moves a task one level up; the tasks below it at its old level become its subtasks
    /// (Project's "Avançar Tarefa").
    pub fn outdent(&mut self, i: usize) -> Result<(), String> {
        let Some(parent) = self.project.tasks[i].parent else {
            return Err("a tarefa já está no primeiro nível".into());
        };
        self.apply(|p| {
            let grand = p.tasks[parent].parent;
            for k in i + 1..p.tasks.len() {
                if p.tasks[k].parent == Some(parent) {
                    p.tasks[k].parent = Some(i);
                }
            }
            p.tasks[i].parent = grand;
            refresh_summaries(p);
            Ok(())
        })
    }

    /// Links the given tasks in order, finish-to-start (Project's "Vincular as Tarefas").
    pub fn link(&mut self, rows: &[usize]) -> Result<(), String> {
        if rows.len() < 2 {
            return Err("selecione duas ou mais tarefas para vincular".into());
        }
        self.apply(|p| {
            for w in rows.windows(2) {
                if !p.tasks[w[1]].preds.iter().any(|l| l.pred == w[0]) {
                    p.tasks[w[1]].preds.push(Link { pred: w[0], kind: LinkType::FinishToStart, lag: 0, lag_elapsed: false });
                }
            }
            Ok(())
        })
    }

    /// Removes the links into the given tasks (Project's "Desvincular Tarefas").
    pub fn unlink(&mut self, rows: &[usize]) -> Result<(), String> {
        self.apply(|p| {
            rows.iter().for_each(|&r| p.tasks[r].preds.clear());
            Ok(())
        })
    }

    /// A start date typed in the grid: a manual task moves there; an automatic one gets a
    /// start-no-earlier-than constraint, as Project does.
    pub fn set_start(&mut self, i: usize, date: chrono::NaiveDate) -> Result<(), String> {
        let at = date.and_hms_opt(0, 0, 0).unwrap();
        self.apply(|p| {
            let t = &mut p.tasks[i];
            if t.manual.is_some() {
                t.manual = Some(at);
            } else {
                t.constraint = Constraint::StartNoEarlierThan(at);
            }
            Ok(())
        })
    }

    fn gantt_task(&self, i: usize) -> Result<&Task, String> {
        let t = self.project.tasks.get(i).ok_or("a tarefa não existe mais")?;
        if t.summary {
            return Err("edite as subtarefas da tarefa de resumo".into());
        }
        Ok(t)
    }

    /// Snap a dragged start to this task's working calendar, including resource holidays.
    pub fn gantt_start(&self, i: usize, at: NaiveDateTime) -> Result<NaiveDateTime, String> {
        let t = self.gantt_task(i)?;
        if t.progress.actual_start.is_some() {
            return Err("a tarefa já tem início real; ajuste o andamento antes de movê-la".into());
        }
        Ok(self.dt(self.cals.task(i).snap_start(self.cals.clock.at(at))))
    }

    /// Resize in whole working days, subtracting gaps in a split task.
    pub fn gantt_duration(&self, i: usize, finish: NaiveDateTime) -> Result<Wm, String> {
        let t = self.gantt_task(i)?;
        if t.progress.actual_finish.is_some() {
            return Err("a tarefa já tem término real; ajuste o andamento antes de redimensioná-la".into());
        }
        let tl = self.cals.task(i);
        let sch = &self.schedule[i];
        let finish = self.cals.clock.at(finish);
        let gaps: Wm = sch.segments.windows(2).map(|w| (tl.w(finish.min(w[1].0)) - tl.w(w[0].1)).max(0)).sum();
        let minutes = (tl.w(finish) - tl.w(sch.start) - gaps).max(0);
        Ok(((minutes as f64 / DAY as f64).round() as Wm).min(10_000) * DAY)
    }

    /// Called inside one begin/end pair. Invalid links roll back through apply.
    pub fn edit_gantt(&mut self, change: GanttEdit) -> Result<(), String> {
        match change {
            GanttEdit::Start(i, at) => {
                let at = self.gantt_start(i, at)?;
                let t = &self.project.tasks[i];
                if t.constraint == Constraint::StartNoEarlierThan(at) && t.manual.is_none() && t.leveling_delay == 0 {
                    return Ok(());
                }
                self.apply(|p| {
                    let t = &mut p.tasks[i];
                    t.manual = None;
                    t.constraint = Constraint::StartNoEarlierThan(at);
                    t.leveling_delay = 0;
                    Ok(())
                })
            }
            GanttEdit::Duration(i, minutes) => {
                self.gantt_task(i)?;
                if self.project.tasks[i].progress.actual_finish.is_some() {
                    return Err("a tarefa já tem término real".into());
                }
                if !(0..=10_000 * DAY).contains(&minutes) || minutes % DAY != 0 {
                    return Err("use uma duração inteira entre 0 e 10000 dias úteis".into());
                }
                if minutes == 0 && self.project.tasks[i].task_type == pjsched::model::TaskType::FixedWork
                    && self.project.assignments.iter().any(|a| a.task == i) {
                    return Err("uma tarefa de trabalho fixo com atribuições precisa de ao menos um dia útil".into());
                }
                if self.project.tasks[i].duration == minutes { return Ok(()); }
                self.set_duration_days(i, minutes as f64 / DAY as f64)
            }
            GanttEdit::Percent(i, percent) => {
                let t = self.gantt_task(i)?;
                if t.progress.percent_complete == percent.min(100) { return Ok(()); }
                self.set_percent(i, percent)
            }
            GanttEdit::Link(from, to) => {
                self.gantt_task(from)?;
                self.gantt_task(to)?;
                if from == to { return Err("uma tarefa não pode depender de si mesma".into()); }
                if self.project.tasks[to].preds.iter().any(|l| l.pred == from) {
                    return Err("já existe um vínculo entre essas tarefas".into());
                }
                self.link(&[from, to])
            }
        }
    }

    /// Assigns resources by name (comma-separated) at 100%, creating unknown ones; names left
    /// out are unassigned. The task type decides what happens to duration and work.
    pub fn set_resources(&mut self, i: usize, names: &str) -> Result<(), String> {
        let wanted: Vec<String> = names.split([',', ';']).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect();
        self.apply(|p| {
            let mut ids = Vec::new();
            for name in &wanted {
                let r = match p.resources.iter().position(|r| r.name.eq_ignore_ascii_case(name)) {
                    Some(r) => r,
                    None => {
                        let uid = p.resources.iter().map(|r| r.uid).max().unwrap_or(0) + 1;
                        p.resources.push(Resource {
                            uid,
                            name: name.clone(),
                            kind: ResourceKind::Work,
                            rates: Vec::new(),
                            max_units: 1.0,
                            calendar: None,
                        });
                        p.resources.len() - 1
                    }
                };
                ids.push(r);
            }
            let current: Vec<usize> = p.assignments.iter().filter(|a| a.task == i).map(|a| a.resource).collect();
            for &r in current.iter().filter(|r| !ids.contains(r)) {
                edit::remove_assignment(p, i, r).map_err(|e| format!("{e:?}"))?;
            }
            for &r in ids.iter().filter(|r| !current.contains(r)) {
                edit::add_assignment(p, i, r, 1.0).map_err(|e| format!("{e:?}"))?;
            }
            Ok(())
        })
    }

    /// Adds a work resource at 100% (Recurso > Adicionar Recursos).
    pub fn add_resource(&mut self, name: &str) -> Result<(), String> {
        if self.project.resources.iter().any(|r| r.name.eq_ignore_ascii_case(name)) {
            return Err(format!("já existe o recurso {name}"));
        }
        let uid = self.project.resources.iter().map(|r| r.uid).max().unwrap_or(0) + 1;
        let name = name.to_string();
        self.apply(|p| {
            p.resources.push(Resource { uid, name, kind: ResourceKind::Work, rates: Vec::new(), max_units: 1.0, calendar: None });
            Ok(())
        })
    }

    /// Manually scheduled tasks stay where they are put; automatic ones follow their links.
    pub fn set_manual(&mut self, i: usize, manual: bool) -> Result<(), String> {
        let start = self.dt(self.schedule[i].start);
        self.apply(|p| {
            p.tasks[i].manual = manual.then_some(start);
            Ok(())
        })
    }

    pub fn set_project_start(&mut self, date: chrono::NaiveDate) -> Result<(), String> {
        self.apply(|p| {
            p.start = date.and_hms_opt(8, 0, 0).unwrap();
            Ok(())
        })
    }

    pub fn rename(&mut self, i: usize, name: &str) {
        if self.project.tasks[i].name != name {
            self.project.tasks[i].name = name.to_string();
            self.dirty = true;
            self.version += 1;
        }
    }

    /// Starts a user action: everything until `end` undoes as one step, as in Project.
    pub fn begin(&mut self) -> u64 {
        if self.pending.is_none() {
            self.pending = Some(self.project.clone());
        }
        self.version
    }

    /// Ends the action started at `version`; it joins the undo history if it changed anything.
    pub fn end(&mut self, version: u64) {
        if let Some(before) = self.pending.take() {
            if self.version != version {
                self.undo.push(before);
                if self.undo.len() > UNDO_LIMIT {
                    self.undo.remove(0);
                }
                self.redo.clear();
            }
        }
    }

    /// Changes so far (see `begin`); also invalidates a Gantt preview after another edit.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Desfazer: back to the project before the last action.
    pub fn undo(&mut self) -> Result<(), String> {
        let Some(previous) = self.undo.pop() else { return Ok(()) };
        let current = std::mem::replace(&mut self.project, previous);
        self.redo.push(current);
        self.recompute()
    }

    /// Refazer: the action Desfazer took back.
    pub fn redo(&mut self) -> Result<(), String> {
        let Some(next) = self.redo.pop() else { return Ok(()) };
        let current = std::mem::replace(&mut self.project, next);
        self.undo.push(current);
        self.recompute()
    }

    fn recompute(&mut self) -> Result<(), String> {
        let (cals, schedule, costs, tracking) = compute(&self.project)?;
        (self.cals, self.schedule, self.costs, self.tracking) = (cals, schedule, costs, tracking);
        self.dirty = true;
        self.version += 1;
        Ok(())
    }

    /// Sets the duration in working days; the task type decides what else changes.
    pub fn set_duration_days(&mut self, i: usize, days: f64) -> Result<(), String> {
        if !(0.0..=10_000.0).contains(&days) {
            return Err(format!("duração inválida: {days}"));
        }
        self.apply(|p| {
            edit::set_duration(p, i, (days * DAY as f64).round() as Wm);
            Ok(())
        })
    }

    /// Sets the finish, as typing in Project's Término column does: the duration becomes the
    /// working time from the task's start to the end of that day, on the task's calendar.
    pub fn set_finish(&mut self, i: usize, date: chrono::NaiveDate) -> Result<(), String> {
        if date < self.dt(self.schedule[i].start).date() {
            return Err("o término não pode ser antes do início da tarefa".into());
        }
        let (clock, tl) = (&self.cals.clock, self.cals.task(i));
        let end = tl.w(clock.at(date.and_hms_opt(0, 0, 0).unwrap() + Duration::days(1)));
        let worked = (end - tl.w(self.schedule[i].start)).max(0);
        self.set_duration_days(i, worked as f64 / DAY as f64)
    }

    /// Replaces the task's predecessors, written as in Project's Predecessoras column: ids
    /// separated by ";" (or ","), each with an optional link type (TI, II, TT, IT, or the English
    /// FS, SS, FF, SF) and lag in working days ("2", "3II", "4TI+2 dias", "5TT-1d").
    pub fn set_predecessors(&mut self, i: usize, text: &str) -> Result<(), String> {
        let mut links = Vec::new();
        for part in text.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
            let link = self.parse_link(part)?;
            if link.pred == i {
                return Err("uma tarefa não pode depender de si mesma".into());
            }
            links.push(link);
        }
        self.apply(|p| {
            p.tasks[i].preds = links;
            Ok(())
        })
    }

    fn parse_link(&self, part: &str) -> Result<Link, String> {
        let bad = || format!("predecessora inválida: {part} (use, por exemplo, 3, 3II ou 3TI+2 dias)");
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        let id: i64 = digits.parse().map_err(|_| bad())?;
        let rest = part[digits.len()..].trim().to_uppercase();
        let (kind, rest) = match rest.get(..2) {
            Some("TI" | "FS") => (LinkType::FinishToStart, &rest[2..]),
            Some("II" | "SS") => (LinkType::StartToStart, &rest[2..]),
            Some("TT" | "FF") => (LinkType::FinishToFinish, &rest[2..]),
            Some("IT" | "SF") => (LinkType::StartToFinish, &rest[2..]),
            _ => (LinkType::FinishToStart, rest.as_str()),
        };
        let rest = rest.trim();
        let lag = if rest.is_empty() {
            0
        } else {
            let (sign, num) = match rest.chars().next() {
                Some('+') => (1.0, &rest[1..]),
                Some('-') => (-1.0, &rest[1..]),
                _ => return Err(bad()),
            };
            let days = crate::fmt::parse_days(num).ok_or_else(bad)?;
            (sign * days * DAY as f64).round() as Wm
        };
        let pred = self.project.tasks.iter().position(|t| t.id == id).ok_or_else(|| format!("não existe tarefa {id}"))?;
        Ok(Link { pred, kind, lag, lag_elapsed: false })
    }

    /// Percent complete; starting a task records its actual start, finishing it its actual finish.
    pub fn set_percent(&mut self, i: usize, pct: u32) -> Result<(), String> {
        let (start, finish) = (self.cals.clock.dt(self.schedule[i].start), self.cals.clock.dt(self.schedule[i].finish));
        self.apply(|p| {
            let pr = &mut p.tasks[i].progress;
            pr.percent_complete = pct.min(100);
            pr.actual_start = (pct > 0).then_some(pr.actual_start.unwrap_or(start));
            pr.actual_finish = (pct >= 100).then_some(pr.actual_finish.unwrap_or(finish));
            Ok(())
        })
    }

    /// Levels resources (Project's defaults) and keeps the result as saved leveling delays.
    pub fn level(&mut self) -> Result<(), String> {
        self.clear_leveling()?;
        let leveled =
            leveling::level(&self.project, &self.cals, LevelingOptions::default()).map_err(|e| format!("{e:?}"))?;
        self.apply(|p| {
            for (t, s) in p.tasks.iter_mut().zip(&leveled) {
                t.leveling_delay = s.leveling_delay;
            }
            Ok(())
        })
    }

    pub fn clear_leveling(&mut self) -> Result<(), String> {
        self.apply(|p| {
            p.tasks.iter_mut().for_each(|t| t.leveling_delay = 0);
            Ok(())
        })
    }

    pub fn save_baseline(&mut self) -> Result<(), String> {
        let (cals, schedule, costs) = (self.cals.clone(), self.schedule.clone(), self.costs.clone());
        self.apply(|p| {
            save_baseline(p, &cals, &schedule, &costs);
            Ok(())
        })
    }

    pub fn dt(&self, at: pjsched::calendar::At) -> NaiveDateTime {
        self.cals.clock.dt(at)
    }

    /// Duration of task `i` in working days.
    pub fn days(&self, i: usize) -> f64 {
        self.project.tasks[i].duration as f64 / DAY as f64
    }

    /// Duration as the grid shows it: a summary spans its subtasks' working time.
    pub fn shown_days(&self, i: usize) -> f64 {
        if !self.project.tasks[i].summary {
            return self.days(i);
        }
        let (s, tl) = (&self.schedule[i], self.cals.project());
        (tl.w(s.finish) - tl.w(s.start)) as f64 / DAY as f64
    }

    /// Whether the project calendar has working time on `date` (weekends and holidays do not).
    pub fn is_working_day(&self, date: chrono::NaiveDate) -> bool {
        let (clock, tl) = (&self.cals.clock, self.cals.project());
        let day = date.and_hms_opt(0, 0, 0).unwrap();
        tl.w(clock.at(day + Duration::days(1))) > tl.w(clock.at(day))
    }

    /// Predecessors as Project writes them in pt-BR: "2;3II;4TI+2 dias".
    pub fn predecessors_text(&self, i: usize) -> String {
        self.project.tasks[i]
            .preds
            .iter()
            .map(|l| {
                let id = self.project.tasks[l.pred].id;
                let kind = match l.kind {
                    LinkType::FinishToStart => "TI",
                    LinkType::StartToStart => "II",
                    LinkType::FinishToFinish => "TT",
                    LinkType::StartToFinish => "IT",
                };
                if l.lag == 0 {
                    if kind == "TI" { id.to_string() } else { format!("{id}{kind}") }
                } else {
                    let days = l.lag as f64 / DAY as f64;
                    let sign = if days < 0.0 { "-" } else { "+" };
                    format!("{id}{kind}{sign}{}", crate::fmt::days(days.abs()))
                }
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    pub fn resources_text(&self, i: usize) -> String {
        self.project
            .assignments
            .iter()
            .filter(|a| a.task == i)
            .map(|a| self.project.resources[a.resource].name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// IDs follow row order.
fn renumber(p: &mut Project) {
    p.tasks.iter_mut().enumerate().for_each(|(k, t)| t.id = k as i64 + 1);
}

/// A task is a summary exactly when some task has it as parent.
fn refresh_summaries(p: &mut Project) {
    let parents: Vec<usize> = p.tasks.iter().filter_map(|t| t.parent).collect();
    for (k, t) in p.tasks.iter_mut().enumerate() {
        t.summary = parents.contains(&k);
    }
}

type Computed =(Calendars, Vec<TaskSchedule>, Costs, Vec<Tracking>);

fn compute(project: &Project) -> Result<Computed, String> {
    let cals = Calendars::new(project).map_err(|e| e.0)?;
    let schedule = cpm::schedule(project, &cals, None).map_err(|e| match e {
        cpm::ScheduleError::DependencyCycle => "os vínculos formam um ciclo".to_string(),
        cpm::ScheduleError::Unsupported(m) => format!("não suportado: {m}"),
    })?;
    let c = costs(project, &cals, &schedule);
    let t = tracking(project, &cals, &schedule, &c);
    Ok((cals, schedule, c, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gantt_doc() -> Doc {
        let start = chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap().and_hms_opt(9, 0, 0).unwrap();
        let mut p = Project::new(start);
        p.tasks.push(Task::new(1, 1, "A", 2 * DAY));
        p.tasks.push(Task::new(2, 2, "B", DAY));
        Doc::from_project(p, None).unwrap()
    }

    #[test]
    fn gantt_move_snaps_to_resource_calendar_sets_snet_and_undoes_once() {
        let mut d = gantt_doc();
        d.set_resources(0, "Ana").unwrap();
        let saturday = d.project.start + Duration::days(5);
        d.apply(|p| {
            let mut c = p.calendars[0].clone();
            c.exceptions.push(pjsched::calendar::Exception { from: saturday.date() + Duration::days(2), to: saturday.date() + Duration::days(2), shifts: vec![] });
            p.calendars.push(c);
            p.resources[0].calendar = Some(1);
            p.tasks[0].manual = Some(p.start);
            Ok(())
        }).unwrap();
        let old = d.schedule[0].start;
        let snapped = d.gantt_start(0, saturday).unwrap();
        assert_eq!(snapped, saturday + Duration::days(3));
        let v = d.begin();
        d.edit_gantt(GanttEdit::Start(0, saturday)).unwrap();
        d.end(v);
        assert_eq!(d.project.tasks[0].constraint, Constraint::StartNoEarlierThan(snapped));
        assert_eq!(d.dt(d.schedule[0].start), snapped);
        assert!(d.project.tasks[0].manual.is_none());
        d.undo().unwrap();
        assert_eq!(d.schedule[0].start, old);
        assert!(d.project.tasks[0].manual.is_some());
        assert!(!d.can_undo());
        d.redo().unwrap();
        assert_eq!(d.dt(d.schedule[0].start), snapped);
    }

    #[test]
    fn gantt_resize_counts_working_days_and_preserves_fixed_work() {
        let mut d = gantt_doc();
        d.set_resources(0, "Ana").unwrap();
        d.apply(|p| { p.tasks[0].task_type = pjsched::model::TaskType::FixedWork; Ok(()) }).unwrap();
        let finish = (d.project.start + Duration::days(7)).date().and_hms_opt(18, 0, 0).unwrap();
        assert_eq!(d.gantt_duration(0, finish).unwrap(), 6 * DAY);
        let work = d.project.assignments[0].work();
        let v = d.begin();
        d.edit_gantt(GanttEdit::Duration(0, 6 * DAY)).unwrap();
        d.end(v);
        assert_eq!(d.days(0), 6.0);
        assert_eq!(d.project.assignments[0].work(), work);
        d.undo().unwrap();
        assert_eq!(d.days(0), 2.0);
        assert!(!d.can_undo());
        assert!(d.edit_gantt(GanttEdit::Duration(0, -DAY)).is_err());
        assert!(d.edit_gantt(GanttEdit::Duration(0, DAY + 1)).is_err());
        assert!(d.edit_gantt(GanttEdit::Duration(0, 0)).is_err());
        assert!(d.project.assignments[0].work().is_finite());
    }

    #[test]
    fn gantt_links_reject_self_duplicate_summary_and_cycles_without_an_undo_step() {
        let mut d = gantt_doc();
        let v = d.begin();
        d.edit_gantt(GanttEdit::Link(0, 1)).unwrap();
        d.end(v);
        assert_eq!(d.project.tasks[1].preds[0].kind, LinkType::FinishToStart);
        let before = d.version();
        for edit in [GanttEdit::Link(0, 0), GanttEdit::Link(0, 1), GanttEdit::Link(1, 0), GanttEdit::Link(0, 99)] {
            let v = d.begin();
            assert!(d.edit_gantt(edit).is_err());
            d.end(v);
            assert_eq!(d.version(), before);
        }
        d.undo().unwrap();
        assert!(d.project.tasks[1].preds.is_empty());
        assert!(d.project.tasks[0].preds.is_empty());
        assert!(!d.can_undo());
        d.indent(1).unwrap();
        assert!(d.edit_gantt(GanttEdit::Link(0, 1)).is_err());
        assert!(d.gantt_start(0, d.project.start).is_err());
    }

    #[test]
    fn gantt_progress_tracks_actuals_and_noops_do_not_add_history() {
        let mut d = gantt_doc();
        let v = d.begin();
        d.edit_gantt(GanttEdit::Percent(0, 100)).unwrap();
        d.end(v);
        assert!(d.project.tasks[0].progress.actual_finish.is_some());
        assert!(d.gantt_start(0, d.project.start).is_err());
        assert!(d.gantt_duration(0, d.project.start).is_err());
        let v = d.begin();
        d.edit_gantt(GanttEdit::Percent(0, 100)).unwrap();
        d.end(v);
        d.undo().unwrap();
        assert_eq!(d.project.tasks[0].progress, pjsched::model::Progress::default());
        assert!(!d.can_undo());
        d.edit_gantt(GanttEdit::Percent(0, 50)).unwrap();
        d.edit_gantt(GanttEdit::Percent(0, 0)).unwrap();
        assert!(d.project.tasks[0].progress.actual_start.is_none());
    }

    #[test]
    fn build_a_chain_level_and_round_trip_through_mspdi() {
        let mut d = Doc::blank();
        let a = d.add_task("Fundação", 3.0).unwrap();
        let b = d.add_task("Estrutura", 2.0).unwrap();
        d.set_predecessors(b, "1").unwrap();
        assert_eq!(d.schedule[b].start, d.cals.task(b).snap_start(d.schedule[a].finish));
        assert!(d.schedule[a].critical && d.schedule[b].critical);

        // A cycle is refused and the project is left as it was.
        assert!(d.set_predecessors(a, "2").is_err());
        assert!(d.project.tasks[a].preds.is_empty());

        let dir = std::env::temp_dir().join("etapix-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cadeia.xml");
        d.save(&path).unwrap();
        let back = Doc::open(&path).unwrap();
        assert_eq!(back.project.tasks.len(), 2);
        assert_eq!(back.schedule[1].finish, d.schedule[1].finish);
    }

    #[test]
    fn outline_insert_and_resources() {
        let mut d = Doc::blank();
        d.add_task("Fase 1", 1.0).unwrap();
        d.add_task("A", 2.0).unwrap();
        d.add_task("B", 3.0).unwrap();
        d.indent(1).unwrap();
        d.indent(2).unwrap();
        assert!(d.project.tasks[0].summary);
        assert_eq!(d.schedule[0].finish, d.schedule[1].finish.max(d.schedule[2].finish));
        d.link(&[1, 2]).unwrap();
        assert_eq!(d.predecessors_text(2), "2");
        // Inserting inside the summary keeps links pointing at the same tasks.
        d.insert_task(2, "A2", 1.0).unwrap();
        assert_eq!(d.project.tasks[2].parent, Some(0));
        assert_eq!(d.predecessors_text(3), "2");
        d.set_resources(1, "Ana, Bruno").unwrap();
        assert_eq!(d.resources_text(1), "Ana, Bruno");
        d.outdent(2).unwrap();
        assert_eq!(d.project.tasks[2].parent, None);
        assert_eq!(d.project.tasks[3].parent, Some(2));
    }

    #[test]
    fn undo_and_redo_whole_actions() {
        let mut d = Doc::blank();
        let v = d.begin();
        d.add_task("A", 1.0).unwrap();
        d.add_task("B", 2.0).unwrap();
        d.end(v);
        let v = d.begin();
        d.set_duration_days(1, 5.0).unwrap();
        d.end(v);
        // An action that changes nothing leaves no step behind.
        let v = d.begin();
        d.rename(0, "A");
        d.end(v);
        d.undo().unwrap();
        assert_eq!(d.days(1), 2.0);
        d.undo().unwrap();
        assert!(d.project.tasks.is_empty() && !d.can_undo());
        d.redo().unwrap();
        d.redo().unwrap();
        assert_eq!(d.project.tasks.len(), 2);
        assert_eq!(d.days(1), 5.0);
        assert!(!d.can_redo());
    }

    #[test]
    fn predecessors_with_types_and_lags_as_in_project() {
        let mut d = Doc::blank();
        d.add_task("A", 2.0).unwrap();
        d.add_task("B", 2.0).unwrap();
        let c = d.add_task("C", 1.0).unwrap();
        d.set_predecessors(c, "1II; 2TI+2 dias").unwrap();
        assert_eq!(d.predecessors_text(c), "1II;2TI+2 dias");
        assert_eq!(d.project.tasks[c].preds[1].lag, 2 * DAY);
        d.set_predecessors(c, "2tt-1d").unwrap();
        assert_eq!(d.predecessors_text(c), "2TT-1 dia");
        assert!(d.set_predecessors(c, "2XY").is_err());
        assert!(d.set_predecessors(c, "Ana").is_err());
        // What the column shows can be typed back unchanged.
        let shown = d.predecessors_text(c);
        d.set_predecessors(c, &shown).unwrap();
        assert_eq!(d.predecessors_text(c), shown);
    }

    #[test]
    fn finish_sets_the_duration_in_working_days() {
        let mut d = Doc::blank();
        let i = d.add_task("A", 1.0).unwrap();
        let start = d.dt(d.schedule[i].start).date();
        // Three working days later, whatever the weekday: count them on the calendar.
        let mut end = start;
        let mut working = 1;
        while working < 3 {
            end += Duration::days(1);
            if d.is_working_day(end) {
                working += 1;
            }
        }
        d.set_finish(i, end).unwrap();
        assert_eq!(d.days(i), 3.0);
        assert_eq!(d.dt(d.schedule[i].finish).date(), end);
        assert!(d.set_finish(i, start - Duration::days(1)).is_err());
    }

    #[test]
    fn delete_keeps_links_consistent() {
        let mut d = Doc::blank();
        d.add_task("A", 1.0).unwrap();
        d.add_task("B", 1.0).unwrap();
        let c = d.add_task("C", 1.0).unwrap();
        d.set_predecessors(c, "1;2").unwrap();
        d.delete_task(0).unwrap();
        assert_eq!(d.project.tasks[1].name, "C");
        assert_eq!(d.predecessors_text(1), "1");
    }
}

#[cfg(test)]
mod mpp_tests {
    use super::*;

    /// Needs Microsoft Project installed: `cargo test -p etapix -- --ignored mpp`.
    #[test]
    #[ignore]
    fn mpp_round_trip_through_installed_project() {
        let dir = std::env::temp_dir().join("etapix-test");
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = Doc::blank();
        d.add_task("Fundação", 3.0).unwrap();
        let b = d.add_task("Estrutura", 2.0).unwrap();
        d.set_predecessors(b, "1").unwrap();
        let path = dir.join("teste-etapix.mpp");
        let _ = std::fs::remove_file(&path);
        d.save(&path).unwrap();
        assert!(std::fs::read(&path).unwrap().starts_with(&[0xD0, 0xCF, 0x11, 0xE0]), "not an OLE compound file");
        let back = Doc::open(&path).unwrap();
        assert_eq!(back.project.tasks.len(), 2);
        assert_eq!(back.project.tasks[1].name, "Estrutura");
        assert_eq!(back.schedule[1].finish, d.schedule[1].finish);
    }
}
