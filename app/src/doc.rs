//! The open document: a project, its schedule and the edits the UI can make. Kept free of UI
//! code so it can be tested directly.

use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, Local, NaiveDateTime};
use pjsched::calendar::Calendars;
use pjsched::cost::{costs, Costs};
use pjsched::cpm::{self, TaskSchedule};
use pjsched::edit;
use pjsched::leveling::{self, LevelingOptions};
use pjsched::model::{Link, LinkType, Project, Task, Wm};
use pjsched::mspdi::read_project;
use pjsched::mspdi_write::write_project;
use pjsched::tracking::{save_baseline, tracking, Tracking};

/// Working minutes in a day of the standard calendar (durations are entered in days).
pub const DAY: Wm = 480;

pub struct Doc {
    pub project: Project,
    pub path: Option<PathBuf>,
    pub cals: Calendars,
    pub schedule: Vec<TaskSchedule>,
    pub costs: Costs,
    pub tracking: Vec<Tracking>,
    pub dirty: bool,
}

impl Doc {
    /// An empty project starting next Monday 08:00.
    pub fn blank() -> Self {
        let today = Local::now().date_naive();
        let monday = today + Duration::days((7 - today.weekday().num_days_from_monday() as i64) % 7);
        let start = monday.and_hms_opt(8, 0, 0).unwrap();
        Doc::from_project(Project::new(start), None).expect("an empty project always schedules")
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let xml = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let project = read_project(&xml).map_err(|e| e.to_string())?;
        Doc::from_project(project, Some(path.to_path_buf()))
    }

    fn from_project(project: Project, path: Option<PathBuf>) -> Result<Self, String> {
        let (cals, schedule, costs, tracking) = compute(&project)?;
        Ok(Doc { project, path, cals, schedule, costs, tracking, dirty: false })
    }

    /// Writes Microsoft Project XML (MSPDI), which Project opens as is.
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let xml = write_project(&self.project, &self.cals, &self.schedule, &name);
        std::fs::write(path, xml).map_err(|e| format!("{}: {e}", path.display()))?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
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
                Ok(())
            }
            Err(e) => {
                self.project = before;
                Err(e)
            }
        }
    }

    pub fn add_task(&mut self, name: &str, days: f64) -> Result<usize, String> {
        let uid = self.project.tasks.iter().map(|t| t.uid).max().unwrap_or(0) + 1;
        let id = self.project.tasks.len() as i64 + 1;
        let duration = (days * DAY as f64).round() as Wm;
        self.apply(|p| {
            p.tasks.push(Task::new(uid, id, name, duration));
            Ok(())
        })?;
        Ok(self.project.tasks.len() - 1)
    }

    /// Removes a task and every link, assignment and parent reference to it.
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
            p.tasks.iter_mut().enumerate().for_each(|(k, t)| t.id = k as i64 + 1);
            Ok(())
        })
    }

    pub fn rename(&mut self, i: usize, name: &str) {
        self.project.tasks[i].name = name.to_string();
        self.dirty = true;
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

    /// Replaces the task's predecessors with finish-to-start links to the given task IDs
    /// (comma-separated, as in Project's Predecessors column).
    pub fn set_predecessors(&mut self, i: usize, ids: &str) -> Result<(), String> {
        let mut links = Vec::new();
        for part in ids.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
            let id: i64 = part.parse().map_err(|_| format!("predecessora inválida: {part}"))?;
            let pred = self
                .project
                .tasks
                .iter()
                .position(|t| t.id == id)
                .ok_or_else(|| format!("não existe tarefa {id}"))?;
            if pred == i {
                return Err("uma tarefa não pode depender de si mesma".into());
            }
            links.push(Link { pred, kind: LinkType::FinishToStart, lag: 0, lag_elapsed: false });
        }
        self.apply(|p| {
            p.tasks[i].preds = links;
            Ok(())
        })
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

    pub fn predecessors_text(&self, i: usize) -> String {
        self.project.tasks[i].preds.iter().map(|l| self.project.tasks[l.pred].id.to_string()).collect::<Vec<_>>().join(";")
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

type Computed = (Calendars, Vec<TaskSchedule>, Costs, Vec<Tracking>);

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

        let dir = std::env::temp_dir().join("pjapp-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cadeia.xml");
        d.save(&path).unwrap();
        let back = Doc::open(&path).unwrap();
        assert_eq!(back.project.tasks.len(), 2);
        assert_eq!(back.schedule[1].finish, d.schedule[1].finish);
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
