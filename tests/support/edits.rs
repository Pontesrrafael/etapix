//! The edits the `.mpp` writer is checked with.

use pjsched::model::{Constraint, Link, LinkType, Project, Task};

#[allow(dead_code)] // not every test binary uses both edit sets
/// A user's edits, spread over the tasks so each kind lands on several files.
pub fn edit(p: &mut Project) {
    let start = p.start;
    for k in 0..p.tasks.len() {
        if p.tasks[k].summary {
            continue;
        }
        match k % 5 {
            0 => p.tasks[k].name.push_str(" (revisada)"),
            1 if p.tasks[k].duration > 0 && p.tasks[k].manual.is_none() => {
                let longer = p.tasks[k].duration + 480;
                pjsched::edit::set_duration(p, k, longer);
            }
            2 => p.tasks[k].priority = 700,
            3 if p.tasks[k].constraint == Constraint::AsSoonAsPossible => {
                p.tasks[k].constraint = Constraint::StartNoEarlierThan(start + chrono::Duration::days(3));
            }
            4 => p.tasks[k].deadline = Some(start + chrono::Duration::days(60)),
            _ => {}
        }
    }
}

/// Changes to the structure: the last task removed (with its links and assignments), a new task
/// linked after the first (with the first resource, when there is one), and a new link.
#[allow(dead_code)] // not every test binary uses both edit sets
pub fn restructure(p: &mut Project) {
    if let Some(last) = p.tasks.len().checked_sub(1).filter(|&l| l > 0 && !p.tasks[l].summary) {
        p.tasks.remove(last);
        for t in &mut p.tasks {
            t.preds.retain(|l| l.pred != last);
        }
        p.assignments.retain(|a| a.task != last);
    }
    let uid = p.tasks.iter().map(|t| t.uid).max().unwrap_or(0) + 1;
    let mut task = Task::new(uid, p.tasks.len() as i64 + 1, "Nova tarefa", 960);
    if !p.tasks.is_empty() && !p.tasks[0].summary {
        task.preds.push(Link { pred: 0, kind: LinkType::FinishToStart, lag: 0, lag_elapsed: false });
    }
    p.tasks.push(task);
    let new = p.tasks.len() - 1;
    if !p.resources.is_empty() {
        let _ = pjsched::edit::add_assignment(p, new, 0, 1.0);
    }
    // A new link between existing tasks: the second waits for the first, when it does not already.
    if p.tasks.len() > 2 && !p.tasks[0].summary && !p.tasks[1].summary && p.tasks[1].preds.is_empty() && p.tasks[1].parent.is_none() {
        p.tasks[1].preds.push(Link { pred: 0, kind: LinkType::FinishToStart, lag: 0, lag_elapsed: false });
    }
}
