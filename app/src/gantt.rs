//! Gantt chart drawn row by row next to the task table: one bar per task on a day grid
//! (critical tasks red, summaries as brackets, milestones as diamonds, done part darker) and
//! the baseline as a thin grey bar under it.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::doc::Doc;

/// The days the chart shows.
pub struct Span {
    pub first: NaiveDate,
    pub days: i64,
}

impl Span {
    pub fn of(doc: &Doc) -> Span {
        let mut lo = doc.project.start.date();
        let mut hi = lo + Duration::days(14);
        for s in &doc.schedule {
            lo = lo.min(doc.dt(s.start).date());
            hi = hi.max(doc.dt(s.finish).date());
        }
        let first = lo - Duration::days(lo.weekday().num_days_from_monday() as i64);
        Span { first, days: (hi - first).num_days() + 8 }
    }

    fn x(&self, at: NaiveDateTime, left: f32, day: f32) -> f32 {
        let minutes = (at - self.first.and_hms_opt(0, 0, 0).unwrap()).num_minutes() as f32;
        left + minutes / 1440.0 * day
    }
}

pub fn header(ui: &mut egui::Ui, span: &Span, day: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(span.days as f32 * day, 24.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let text = ui.visuals().text_color();
    for d in 0..span.days {
        let date = span.first + Duration::days(d);
        let x = rect.left() + d as f32 * day;
        if date.weekday().num_days_from_monday() == 0 || day >= 30.0 {
            let label = date.format("%d/%m").to_string();
            painter.text(Pos2::new(x + 2.0, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(11.0), text);
            painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(1.0_f32, Color32::from_gray(120)));
        }
    }
}

pub fn row(ui: &mut egui::Ui, doc: &Doc, span: &Span, i: usize, day: f32, height: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(span.days as f32 * day, height), Sense::hover());
    let painter = ui.painter_at(rect);
    // Weekends shaded.
    for d in 0..span.days {
        let date = span.first + Duration::days(d);
        if date.weekday().num_days_from_monday() >= 5 {
            let x = rect.left() + d as f32 * day;
            painter.rect_filled(Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(day, height)), 0.0, Color32::from_black_alpha(18));
        }
    }
    let t = &doc.project.tasks[i];
    let s = &doc.schedule[i];
    let (x0, x1) = (span.x(doc.dt(s.start), rect.left(), day), span.x(doc.dt(s.finish), rect.left(), day));
    let mid = rect.center().y;
    let color = if s.critical { Color32::from_rgb(214, 69, 65) } else { Color32::from_rgb(66, 133, 214) };

    if let Some(b) = &t.baseline {
        let (b0, b1) = (span.x(b.start, rect.left(), day), span.x(b.finish, rect.left(), day));
        painter.rect_filled(Rect::from_min_max(Pos2::new(b0, mid + 6.0), Pos2::new(b1.max(b0 + 2.0), mid + 9.0)), 1.0, Color32::from_gray(150));
    }
    if t.summary {
        painter.rect_filled(Rect::from_min_max(Pos2::new(x0, mid - 4.0), Pos2::new(x1, mid + 1.0)), 0.0, Color32::from_gray(60));
        for x in [x0, x1] {
            painter.add(egui::Shape::convex_polygon(
                vec![Pos2::new(x - 4.0, mid + 1.0), Pos2::new(x + 4.0, mid + 1.0), Pos2::new(x, mid + 6.0)],
                Color32::from_gray(60),
                Stroke::NONE,
            ));
        }
    } else if t.duration == 0 {
        painter.add(egui::Shape::convex_polygon(
            vec![Pos2::new(x0, mid - 7.0), Pos2::new(x0 + 7.0, mid), Pos2::new(x0, mid + 7.0), Pos2::new(x0 - 7.0, mid)],
            Color32::from_gray(40),
            Stroke::NONE,
        ));
    } else {
        for &(a, b) in &s.segments {
            let (a, b) = (span.x(doc.dt(a), rect.left(), day), span.x(doc.dt(b), rect.left(), day));
            painter.rect_filled(Rect::from_min_max(Pos2::new(a, mid - 6.0), Pos2::new(b.max(a + 2.0), mid + 5.0)), 2.0, color);
        }
        let pct = t.progress.percent_complete as f32 / 100.0;
        if pct > 0.0 {
            let done = x0 + (x1 - x0) * pct;
            painter.rect_filled(Rect::from_min_max(Pos2::new(x0, mid - 2.0), Pos2::new(done, mid + 1.0)), 0.0, Color32::from_gray(30));
        }
    }
    // Links into this task: a short elbow from each predecessor's finish.
    for link in &t.preds {
        let p = &doc.schedule[link.pred];
        let px = span.x(doc.dt(p.finish), rect.left(), day);
        let stroke = Stroke::new(1.0_f32, Color32::from_gray(110));
        painter.line_segment([Pos2::new(px, rect.top()), Pos2::new(px, mid)], stroke);
        painter.line_segment([Pos2::new(px, mid), Pos2::new(x0, mid)], stroke);
    }
    if response.hovered() {
        let tip = format!(
            "{}\n{} → {}\nfolga total: {:.2} d{}",
            t.name,
            doc.dt(s.start).format("%d/%m/%Y %H:%M"),
            doc.dt(s.finish).format("%d/%m/%Y %H:%M"),
            s.total_slack as f64 / crate::doc::DAY as f64,
            if s.critical { " (crítica)" } else { "" }
        );
        response.on_hover_text(tip);
    }
}
