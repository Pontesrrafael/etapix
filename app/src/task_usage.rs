//! Read-only task/assignment work by day. Materials and costs are not work hours.

use chrono::{Duration, NaiveDate};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use pjsched::model::ResourceKind;

use crate::doc::Doc;

struct Work {
    task: Vec<Vec<f64>>,
    assignment: Vec<Vec<f64>>,
    total_task: Vec<f64>,
}

/// Integrate profile rates on the assignment calendar, carrying the profile across splits.
/// The offset counts task working minutes, while each piece counts assignment minutes.
fn daily_work(doc: &Doc, first: NaiveDate, days: usize) -> Work {
    let project = &doc.project;
    let mut out = Work {
        task: vec![vec![0.0; days]; project.tasks.len()],
        assignment: vec![vec![0.0; days]; project.assignments.len()],
        total_task: vec![0.0; project.tasks.len()],
    };
    for (k, a) in project.assignments.iter().enumerate() {
        if project.resources[a.resource].kind != ResourceKind::Work { continue; }
        out.total_task[a.task] += a.work() / 60.0;
        let tl = doc.cals.assignment(k);
        let task_tl = doc.cals.task(a.task);
        let mut offset = a.offset;
        let (mut piece, mut used) = (0, 0);
        for &(start, finish) in &doc.schedule[a.task].segments {
            let span = task_tl.w(finish) - task_tl.w(start);
            if offset >= span { offset -= span; continue; }
            let begin = task_tl.start_at(task_tl.w(start) + offset);
            offset = 0;
            let (mut at, end) = (tl.w(begin), tl.w(finish));
            while at < end && piece < a.profile.len() {
                let p = a.profile[piece];
                let len = (p.len - used).min(end - at);
                for day in 0..days {
                    let date = first + Duration::days(day as i64);
                    let from = doc.cals.clock.at(date.and_hms_opt(0, 0, 0).unwrap());
                    let to = from + 1440;
                    let minutes = ((at + len).min(tl.w(to)) - at.max(tl.w(from))).max(0);
                    out.assignment[k][day] += minutes as f64 * p.rate / 60.0;
                }
                at += len;
                used += len;
                if used >= p.len { piece += 1; used = 0; }
            }
        }
        for day in 0..days { out.task[a.task][day] += out.assignment[k][day]; }
    }
    // A leaf contributes once to each ancestor, irrespective of the outline's depth.
    let direct = out.task.clone();
    let totals = out.total_task.clone();
    for (i, task) in project.tasks.iter().enumerate().filter(|(_, t)| !t.summary) {
        let mut parent = task.parent;
        while let Some(up) = parent {
            out.total_task[up] += totals[i];
            for (sum, value) in out.task[up].iter_mut().zip(&direct[i]) { *sum += value; }
            parent = project.tasks[up].parent;
        }
    }
    out
}

/// Two weeks per page, with a fixed header and independently scrollable rows/columns.
pub fn show(ui: &mut egui::Ui, doc: &Doc, first: &mut Option<NaiveDate>) {
    const DAYS: usize = 14;
    let start = doc.schedule.iter().map(|s| doc.dt(s.start).date()).min().unwrap_or(doc.project.start.date());
    let date = first.get_or_insert(start);
    ui.horizontal_wrapped(|ui| {
        ui.strong("Uso da Tarefa");
        if ui.button("◀ Anterior").clicked() { *date -= Duration::days(DAYS as i64); }
        if ui.button("Início do projeto").clicked() { *date = start; }
        if ui.button("Próximo ▶").clicked() { *date += Duration::days(DAYS as i64); }
        ui.label(format!("{} a {}", date.format("%d/%m/%Y"), (*date + Duration::days(DAYS as i64 - 1)).format("%d/%m/%Y")));
    });
    ui.label("Trabalho planejado em horas · somente leitura · materiais e custos: —");
    let work = daily_work(doc, *date, DAYS);
    let mut rows = Vec::new();
    for task in 0..doc.project.tasks.len() {
        rows.push((task, None));
        rows.extend(doc.project.assignments.iter().enumerate().filter(|(_, a)| a.task == task).map(|(k, _)| (task, Some(k))));
    }
    egui::ScrollArea::horizontal().id_salt("usage-horizontal").show(ui, |ui| {
        TableBuilder::new(ui).striped(true).resizable(true)
            .column(Column::initial(260.0).at_least(160.0))
            .column(Column::initial(90.0))
            .columns(Column::initial(76.0), DAYS)
            .header(36.0, |mut header| {
                header.col(|ui| { ui.strong("Tarefa / Recurso"); });
                header.col(|ui| { ui.strong("Total (h)"); });
                for day in 0..DAYS {
                    let day = *date + Duration::days(day as i64);
                    header.col(|ui| { ui.strong(day.format("%d/%m/%Y").to_string()); });
                }
            })
            .body(|body| body.rows(24.0, rows.len(), |mut row| {
                let (task, assignment) = rows[row.index()];
                let (name, values, total, is_work) = match assignment {
                    Some(k) => {
                        let a = &doc.project.assignments[k];
                        let r = &doc.project.resources[a.resource];
                        (format!("    {}", r.name), &work.assignment[k], a.work() / 60.0, r.kind == ResourceKind::Work)
                    }
                    None => (format!("{}{}", "  ".repeat(doc.outline_level(task) - 1), doc.project.tasks[task].name), &work.task[task], work.total_task[task], true),
                };
                row.col(|ui| { if assignment.is_none() { ui.strong(name); } else { ui.label(name); } });
                row.col(|ui| { ui.label(if is_work { format!("{total:.1}").replace('.', ",") } else { "—".into() }); });
                for value in values {
                    row.col(|ui| { ui.label(if is_work && *value > 0.0001 { format!("{value:.1}").replace('.', ",") } else { "—".into() }); });
                }
            }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use pjsched::calendar::Exception;
    use pjsched::model::Piece;

    fn monday() -> NaiveDate { NaiveDate::from_ymd_opt(2026, 11, 2).unwrap() }

    #[test]
    fn daily_work_uses_profiles_offsets_holidays_and_summary_rollups() {
        let mut d = Doc::blank();
        d.set_project_start(monday()).unwrap();
        d.add_task("Fase", 1.0).unwrap();
        d.add_task("Execução", 3.0).unwrap();
        d.indent(1).unwrap();
        d.set_resources(1, "Ana, Material").unwrap();
        d.apply(|p| {
            p.calendars[0].exceptions.push(Exception { from: monday() + Duration::days(1), to: monday() + Duration::days(1), shifts: vec![] });
            p.resources[1].kind = ResourceKind::Material;
            p.assignments[0].offset = 480;
            p.assignments[0].profile = vec![Piece { len: 480, rate: 0.5 }, Piece { len: 480, rate: 1.0 }];
            Ok(())
        }).unwrap();
        let work = daily_work(&d, monday(), 14);
        assert_eq!(&work.assignment[0][..5], &[0.0, 0.0, 4.0, 8.0, 0.0]);
        assert_eq!(work.task[0], work.task[1]);
        assert_eq!(work.task[1], work.assignment[0]);
        assert_eq!(work.total_task, vec![12.0, 12.0]);
        assert_eq!(work.assignment[1], vec![0.0; 14]);
    }

    #[test]
    fn daily_work_carries_the_profile_across_a_resume_gap_and_weekend() {
        let mut d = Doc::blank();
        d.set_project_start(monday()).unwrap();
        let task = d.add_task("Interrompida", 2.0).unwrap();
        d.set_resources(task, "Ana").unwrap();
        d.set_percent(task, 50).unwrap();
        d.apply(|p| {
            p.tasks[task].progress.resume = Some((monday() + Duration::days(7)).and_hms_opt(9, 0, 0).unwrap());
            Ok(())
        }).unwrap();
        let work = daily_work(&d, monday(), 14);
        assert_eq!(work.assignment[0][0], 8.0);
        assert_eq!(work.assignment[0][7], 8.0);
        assert_eq!(work.assignment[0].iter().sum::<f64>(), 16.0);
        assert!(work.assignment[0][1..7].iter().all(|&h| h == 0.0));
    }

    #[test]
    fn usage_view_renders_and_ignores_grid_edit_shortcuts() {
        let ctx = egui::Context::default();
        crate::setup_style(&ctx, None);
        let mut app = crate::App::new(None);
        app.headless = true;
        app.suggest = false;
        app.doc.add_task("Execução", 3.0).unwrap();
        app.doc.set_resources(0, "Ana").unwrap();
        app.view = crate::View::Usage;
        app.select_only(0);
        let version = app.doc.version();
        for frame in 0..3 {
            let events = if frame == 1 { vec![egui::Event::Text("alteração oculta".into()), egui::Event::Key {
                key: egui::Key::Delete, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE,
            }] } else { vec![] };
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1366.0, 800.0))),
                events, time: Some(frame as f64 * 0.1), ..Default::default() };
            let _ = ctx.run(input, |ctx| app.frame_ui(ctx));
        }
        assert_eq!(app.doc.version(), version);
        assert_eq!(app.doc.project.tasks[0].name, "Execução");
        assert!(app.editing.is_none());
    }
}
