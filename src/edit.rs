//! Edits that make Project recalculate duration, units or work, per task type.
//!
//! Rules observed in golden batch 3 (`tests/golden/b3/B3-*`), with D = duration,
//! u = nominal units and w = work of an assignment:
//!
//! | Edit | Fixed units | Fixed duration | Fixed work |
//! |---|---|---|---|
//! | add resource, not effort-driven | new w = D·u, D kept | new w = D·u, D kept | (always effort-driven) |
//! | add/remove resource, effort-driven | total work kept, D = W / Σu, w ∝ u | total work kept, D kept, w ∝ u; the new assignment's units become w/D | as fixed units |
//! | set duration | w = D·u | w = D·u | w kept, rate = w/D |
//! | set units | w kept, span = w/u | span kept, w = D·u | w kept, span = w/u |
//! | set work | extra work appended at u (or trimmed from the end) | rate scaled, span kept | as fixed units |
//!
//! Task work changes are spread over the assignments in proportion to their current work.
//! Fixed-units and fixed-work tasks then last as long as their longest assignment.

use crate::model::{Assignment, Piece, Project, TaskType, Wm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    NoSuchAssignment { task: usize, resource: usize },
    AlreadyAssigned { task: usize, resource: usize },
}

fn assignment_indices(p: &Project, task: usize) -> Vec<usize> {
    (0..p.assignments.len()).filter(|&i| p.assignments[i].task == task).collect()
}

fn find(p: &Project, task: usize, resource: usize) -> Result<usize, EditError> {
    p.assignments
        .iter()
        .position(|a| a.task == task && a.resource == resource)
        .ok_or(EditError::NoSuchAssignment { task, resource })
}

fn is_effort_driven(p: &Project, task: usize) -> bool {
    let t = &p.tasks[task];
    t.effort_driven || t.task_type == TaskType::FixedWork
}

/// Fixed-units and fixed-work tasks last as long as their longest assignment.
fn fit_duration(p: &mut Project, task: usize) {
    if p.tasks[task].task_type == TaskType::FixedDuration {
        return;
    }
    if let Some(span) = assignment_indices(p, task).iter().map(|&i| p.assignments[i].offset + p.assignments[i].span()).max() {
        p.tasks[task].duration = span;
    }
}

/// Redistributes `total` work over the task's assignments in proportion to units, as
/// effort-driven scheduling does after a resource is added or removed.
fn redistribute_by_units(p: &mut Project, task: usize, total: f64) {
    let ids = assignment_indices(p, task);
    let sum_units: f64 = ids.iter().map(|&i| p.assignments[i].units).sum();
    if ids.is_empty() || sum_units <= 0.0 {
        return;
    }
    if p.tasks[task].task_type == TaskType::FixedDuration {
        let d = p.tasks[task].duration;
        for &i in &ids {
            let share = total * p.assignments[i].units / sum_units;
            p.assignments[i].profile = vec![Piece { len: d, rate: share / d as f64 }];
        }
    } else {
        let d = (total / sum_units).round() as Wm;
        for &i in &ids {
            let u = p.assignments[i].units;
            p.assignments[i].profile = vec![Piece { len: d, rate: u }];
        }
        p.tasks[task].duration = d;
    }
}

pub fn add_assignment(p: &mut Project, task: usize, resource: usize, units: f64) -> Result<(), EditError> {
    if find(p, task, resource).is_ok() {
        return Err(EditError::AlreadyAssigned { task, resource });
    }
    let existing = assignment_indices(p, task);
    let total: f64 = existing.iter().map(|&i| p.assignments[i].work()).sum();
    let d = p.tasks[task].duration;
    p.assignments.push(Assignment::uniform(task, resource, units, d, units));
    if is_effort_driven(p, task) && !existing.is_empty() {
        redistribute_by_units(p, task, total);
        if p.tasks[task].task_type == TaskType::FixedDuration {
            // Only the new assignment's nominal units follow its share; existing ones keep theirs.
            let new = p.assignments.len() - 1;
            p.assignments[new].units = p.assignments[new].profile[0].rate;
        }
    }
    Ok(())
}

pub fn remove_assignment(p: &mut Project, task: usize, resource: usize) -> Result<(), EditError> {
    let total: f64 = assignment_indices(p, task).iter().map(|&i| p.assignments[i].work()).sum();
    p.assignments.remove(find(p, task, resource)?);
    if is_effort_driven(p, task) {
        redistribute_by_units(p, task, total);
    }
    Ok(())
}

pub fn set_duration(p: &mut Project, task: usize, duration: Wm) {
    p.tasks[task].duration = duration;
    let fixed_work = p.tasks[task].task_type == TaskType::FixedWork;
    for i in assignment_indices(p, task) {
        let a = &mut p.assignments[i];
        let rate = if fixed_work { a.work() / duration as f64 } else { a.units };
        a.profile = vec![Piece { len: duration, rate }];
    }
}

pub fn set_units(p: &mut Project, task: usize, resource: usize, units: f64) -> Result<(), EditError> {
    let i = find(p, task, resource)?;
    let a = &mut p.assignments[i];
    a.profile = if p.tasks[task].task_type == TaskType::FixedDuration {
        vec![Piece { len: a.span(), rate: units }]
    } else {
        vec![Piece { len: (a.work() / units).round() as Wm, rate: units }]
    };
    a.units = units;
    fit_duration(p, task);
    Ok(())
}

pub fn set_assignment_work(p: &mut Project, task: usize, resource: usize, work: f64) -> Result<(), EditError> {
    let i = find(p, task, resource)?;
    adjust_work(&mut p.assignments[i], work, p.tasks[task].task_type);
    fit_duration(p, task);
    Ok(())
}

pub fn set_task_work(p: &mut Project, task: usize, work: f64) {
    let ids = assignment_indices(p, task);
    let current: f64 = ids.iter().map(|&i| p.assignments[i].work()).sum();
    let task_type = p.tasks[task].task_type;
    for &i in &ids {
        let share = if current > 0.0 { p.assignments[i].work() / current } else { 1.0 / ids.len() as f64 };
        adjust_work(&mut p.assignments[i], work * share, task_type);
    }
    fit_duration(p, task);
}

fn adjust_work(a: &mut Assignment, work: f64, task_type: TaskType) {
    let current = a.work();
    if task_type == TaskType::FixedDuration {
        let scale = if current > 0.0 { work / current } else { 0.0 };
        a.profile.iter_mut().for_each(|piece| piece.rate *= scale);
    } else if work > current {
        let len = ((work - current) / a.units).round() as Wm;
        a.profile.push(Piece { len, rate: a.units });
    } else {
        // Trim from the end until the remaining pieces hold `work`.
        let mut left = work;
        let mut kept = Vec::new();
        for piece in &a.profile {
            if left <= 0.0 {
                break;
            }
            let len = piece.len.min((left / piece.rate).round() as Wm);
            kept.push(Piece { len, rate: piece.rate });
            left -= len as f64 * piece.rate;
        }
        a.profile = kept;
    }
}
