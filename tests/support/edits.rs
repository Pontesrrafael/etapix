//! The edits the `.mpp` writer is checked with.

use pjsched::model::{Constraint, Project};

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
