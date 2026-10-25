//! Which tasks the grid and the chart show, and in what order: Project's filters, sorting and
//! outline (collapsed summaries, "Mostrar Nível N"). Only the display changes; task ids and the
//! file keep their order.

use std::collections::HashSet;

use crate::doc::Doc;

/// Exibir > Filtro.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Critical,
    Incomplete,
    Complete,
    Milestones,
    Summaries,
    /// Tasks that should have finished by today and have not.
    Late,
}

impl Filter {
    pub const ALL: [Filter; 7] = [Filter::All, Filter::Critical, Filter::Incomplete, Filter::Complete, Filter::Milestones, Filter::Summaries, Filter::Late];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "[Sem Filtro]",
            Filter::Critical => "Tarefas Críticas",
            Filter::Incomplete => "Tarefas Não Concluídas",
            Filter::Complete => "Tarefas Concluídas",
            Filter::Milestones => "Marcos",
            Filter::Summaries => "Tarefas de Resumo",
            Filter::Late => "Tarefas Atrasadas",
        }
    }

    fn keeps(self, doc: &Doc, i: usize, today: chrono::NaiveDate) -> bool {
        let (t, s) = (&doc.project.tasks[i], &doc.schedule[i]);
        let pct = t.progress.percent_complete;
        match self {
            Filter::All => true,
            Filter::Critical => s.critical,
            Filter::Incomplete => pct < 100,
            Filter::Complete => pct >= 100,
            Filter::Milestones => !t.summary && t.duration == 0,
            Filter::Summaries => t.summary,
            Filter::Late => pct < 100 && doc.dt(s.finish).date() < today,
        }
    }
}

/// Exibir > Classificar.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Id,
    Start,
    Finish,
    Duration,
    Name,
}

impl Sort {
    pub const ALL: [Sort; 5] = [Sort::Id, Sort::Start, Sort::Finish, Sort::Duration, Sort::Name];

    pub fn label(self) -> &'static str {
        match self {
            Sort::Id => "por Identificação",
            Sort::Start => "por Data de Início",
            Sort::Finish => "por Data de Término",
            Sort::Duration => "por Duração",
            Sort::Name => "por Nome",
        }
    }
}

/// How the rows are picked and ordered.
#[derive(Clone, Default)]
pub struct Display {
    pub filter: Filter,
    pub sort: Sort,
    /// Summaries (by UID) whose subtasks are hidden.
    pub collapsed: HashSet<i64>,
}

impl Display {
    /// The tasks shown, top to bottom (task indices). Sorting keeps the outline, as Project's
    /// "Manter estrutura de tópicos": subtasks are ordered among their siblings, under their
    /// summary. A filter keeps the summaries of the tasks it shows.
    pub fn rows(&self, doc: &Doc, today: chrono::NaiveDate) -> Vec<usize> {
        let tasks = &doc.project.tasks;
        let n = tasks.len();
        let mut keep = vec![self.filter == Filter::All; n];
        if self.filter != Filter::All {
            for i in 0..n {
                if self.filter.keeps(doc, i, today) {
                    let mut cur = Some(i);
                    while let Some(k) = cur {
                        keep[k] = true;
                        cur = tasks[k].parent;
                    }
                }
            }
        }
        let mut out = Vec::new();
        self.walk(doc, None, &keep, &mut out);
        out
    }

    fn walk(&self, doc: &Doc, parent: Option<usize>, keep: &[bool], out: &mut Vec<usize>) {
        let tasks = &doc.project.tasks;
        let mut children: Vec<usize> = (0..tasks.len()).filter(|&i| tasks[i].parent == parent).collect();
        let key = |i: &usize| {
            let s = &doc.schedule[*i];
            match self.sort {
                Sort::Id => (0, String::new(), *i),
                Sort::Start => (s.start, String::new(), *i),
                Sort::Finish => (s.finish, String::new(), *i),
                Sort::Duration => (tasks[*i].duration, String::new(), *i),
                Sort::Name => (0, tasks[*i].name.to_lowercase(), *i),
            }
        };
        children.sort_by_key(key);
        for i in children {
            if !keep[i] {
                continue;
            }
            out.push(i);
            if !self.collapsed.contains(&tasks[i].uid) {
                self.walk(doc, Some(i), keep, out);
            }
        }
    }

    /// "Mostrar Nível N" (Exibir > Estrutura de Tópicos): summaries at level N and below are
    /// collapsed; `None` shows every subtask.
    pub fn show_level(&mut self, doc: &Doc, level: Option<usize>) {
        self.collapsed.clear();
        if let Some(level) = level {
            for (i, t) in doc.project.tasks.iter().enumerate() {
                if t.summary && doc.outline_level(i) >= level {
                    self.collapsed.insert(t.uid);
                }
            }
        }
    }

    pub fn toggle(&mut self, uid: i64) {
        if !self.collapsed.remove(&uid) {
            self.collapsed.insert(uid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resumo > (B, A), then C; B is longer and starts later than A.
    fn sample() -> Doc {
        let mut d = Doc::blank();
        d.add_task("Resumo", 1.0).unwrap();
        d.add_task("B", 3.0).unwrap();
        d.add_task("A", 1.0).unwrap();
        d.add_task("C", 0.0).unwrap();
        d.indent(1).unwrap();
        d.indent(2).unwrap();
        d.set_predecessors(1, "3").unwrap();
        d
    }

    fn names(d: &Doc, rows: &[usize]) -> Vec<String> {
        rows.iter().map(|&i| d.project.tasks[i].name.clone()).collect()
    }

    #[test]
    fn sorting_keeps_the_outline() {
        let d = sample();
        let today = d.project.start.date();
        let mut v = Display::default();
        assert_eq!(names(&d, &v.rows(&d, today)), ["Resumo", "B", "A", "C"]);
        v.sort = Sort::Name;
        assert_eq!(names(&d, &v.rows(&d, today)), ["C", "Resumo", "A", "B"]);
        // B waits for A: by start, A comes first under the summary.
        v.sort = Sort::Start;
        let by_start = names(&d, &v.rows(&d, today));
        let at = |n: &str| by_start.iter().position(|x| x == n).unwrap();
        assert!(at("Resumo") < at("A") && at("A") < at("B"));
    }

    #[test]
    fn filters_keep_summaries_and_collapse_hides_subtasks() {
        let d = sample();
        let today = d.project.start.date();
        let mut v = Display { filter: Filter::Milestones, ..Default::default() };
        assert_eq!(names(&d, &v.rows(&d, today)), ["C"]);
        v.filter = Filter::All;
        v.show_level(&d, Some(1));
        assert_eq!(names(&d, &v.rows(&d, today)), ["Resumo", "C"]);
        v.toggle(d.project.tasks[0].uid);
        assert_eq!(names(&d, &v.rows(&d, today)).len(), 4);
    }
}
