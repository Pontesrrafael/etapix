//! The Gantt chart as Project draws it: a two-tier timescale (weeks over day letters, or months
//! over weeks when zoomed out), nonworking days shaded, the current date line, blue task bars,
//! black summary brackets, milestone diamonds, resource names next to the bars and
//! finish-to-start arrows.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime};
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

use crate::doc::Doc;
use crate::fmt;

pub const BAR: Color32 = Color32::from_rgb(0x14, 0xC3, 0x8E);
pub const BAR_EDGE: Color32 = Color32::from_rgb(0x0B, 0x6B, 0x66);
const MANUAL: Color32 = Color32::from_rgb(0xA6, 0xF0, 0xDF);
const MANUAL_EDGE: Color32 = Color32::from_rgb(0x2B, 0xAF, 0x9A);
const PROGRESS: Color32 = Color32::from_rgb(0x0B, 0x6B, 0x66);
const NONWORKING: Color32 = Color32::from_rgb(0xEE, 0xF3, 0xF6);
const LINE: Color32 = Color32::from_rgb(0xDC, 0xE5, 0xEA);
const TEXT: Color32 = Color32::from_rgb(0x1E, 0x2F, 0x3E);
const TODAY: Color32 = Color32::from_rgb(0x14, 0xC3, 0x8E);

/// Horizontal mapping: the chart starts on the Sunday before the project start.
pub struct Scale {
    pub first: NaiveDate,
    pub day: f32,
    pub left: f32,
}

impl Scale {
    pub fn new(doc: &Doc, day: f32, left: f32) -> Scale {
        let start = doc.project.start.date();
        let first = start - Duration::days(start.weekday().num_days_from_sunday() as i64 + 7);
        Scale { first, day, left }
    }

    pub fn x(&self, at: NaiveDateTime) -> f32 {
        let minutes = (at - self.first.and_hms_opt(0, 0, 0).unwrap()).num_minutes() as f32;
        self.left + minutes / 1440.0 * self.day
    }

    pub fn x_date(&self, d: NaiveDate) -> f32 {
        self.left + (d - self.first).num_days() as f32 * self.day
    }

    /// The date under screen position `x`.
    pub fn date_at(&self, x: f32) -> NaiveDate {
        self.first + Duration::days(((x - self.left) / self.day).floor() as i64)
    }

    /// Total width in pixels covering the project plus some room after it.
    pub fn width(&self, doc: &Doc) -> f32 {
        let last = doc.schedule.iter().map(|s| doc.dt(s.finish).date()).max().unwrap_or(doc.project.start.date());
        self.x_date(last + Duration::days(60)) - self.left
    }
}

fn months_label(d: NaiveDate) -> String {
    format!("{}/{}", fmt::MONTHS[d.month0() as usize], d.format("%y"))
}

/// The two header rows (`rect` is 40 px high).
pub fn header(p: &Painter, rect: Rect, s: &Scale) {
    p.rect_filled(rect, 0.0, Color32::WHITE);
    let mid = rect.top() + rect.height() / 2.0;
    let font = FontId::proportional(11.0);
    let (d0, d1) = (s.date_at(rect.left()), s.date_at(rect.right()) + Duration::days(1));
    let stroke = Stroke::new(1.0_f32, LINE);
    if s.day >= 10.0 {
        // Weeks (from Sunday) over days.
        let mut w = d0 - Duration::days(d0.weekday().num_days_from_sunday() as i64);
        while w <= d1 {
            let x = s.x_date(w);
            p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], stroke);
            p.text(Pos2::new(x + 4.0, rect.top() + 10.0), Align2::LEFT_CENTER, fmt::week_label(w), font.clone(), TEXT);
            w += Duration::days(7);
        }
        let mut d = d0;
        while d <= d1 {
            let x = s.x_date(d);
            p.line_segment([Pos2::new(x, mid), Pos2::new(x, rect.bottom())], stroke);
            let letter = fmt::DAY_LETTERS[d.weekday().num_days_from_sunday() as usize];
            p.text(Pos2::new(x + s.day / 2.0, mid + 10.0), Align2::CENTER_CENTER, letter, font.clone(), TEXT);
            d += Duration::days(1);
        }
    } else {
        // Months over weeks.
        let mut m = NaiveDate::from_ymd_opt(d0.year(), d0.month(), 1).unwrap();
        while m <= d1 {
            let x = s.x_date(m);
            p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, mid)], stroke);
            p.text(Pos2::new(x + 4.0, rect.top() + 10.0), Align2::LEFT_CENTER, months_label(m), font.clone(), TEXT);
            m = if m.month() == 12 { NaiveDate::from_ymd_opt(m.year() + 1, 1, 1) } else { NaiveDate::from_ymd_opt(m.year(), m.month() + 1, 1) }.unwrap();
        }
        let mut w = d0 - Duration::days(d0.weekday().num_days_from_sunday() as i64);
        while w <= d1 {
            let x = s.x_date(w);
            p.line_segment([Pos2::new(x, mid), Pos2::new(x, rect.bottom())], stroke);
            if s.day * 7.0 >= 22.0 {
                p.text(Pos2::new(x + 3.0, mid + 10.0), Align2::LEFT_CENTER, w.format("%d").to_string(), font.clone(), TEXT);
            }
            w += Duration::days(7);
        }
    }
    p.line_segment([Pos2::new(rect.left(), mid), Pos2::new(rect.right(), mid)], stroke);
    p.line_segment([Pos2::new(rect.left(), rect.bottom()), Pos2::new(rect.right(), rect.bottom())], Stroke::new(1.0_f32, Color32::from_gray(170)));
}

/// Background of the chart body: nonworking days and the current date.
pub fn background(p: &Painter, rect: Rect, s: &Scale, doc: &Doc) {
    p.rect_filled(rect, 0.0, Color32::WHITE);
    let (d0, d1) = (s.date_at(rect.left()), s.date_at(rect.right()));
    if s.day >= 3.0 {
        let mut d = d0;
        while d <= d1 {
            if !doc.is_working_day(d) {
                let x = s.x_date(d);
                p.rect_filled(Rect::from_min_max(Pos2::new(x, rect.top()), Pos2::new(x + s.day, rect.bottom())), 0.0, NONWORKING);
            }
            d += Duration::days(1);
        }
    }
    let now = s.x(Local::now().naive_local());
    dashed(p, Pos2::new(now, rect.top()), rect.bottom(), TODAY);
}

fn dashed(p: &Painter, top: Pos2, bottom: f32, color: Color32) {
    let mut y = top.y;
    while y < bottom {
        p.line_segment([Pos2::new(top.x, y), Pos2::new(top.x, (y + 4.0).min(bottom))], Stroke::new(1.5_f32, color));
        y += 7.0;
    }
}

/// What the Gantt shows besides the bars (Formato > Estilos de Barra).
#[derive(Clone, Copy, Default)]
pub struct Options {
    /// Critical tasks in red.
    pub critical: bool,
    /// Baseline as a grey bar under each task.
    pub baseline: bool,
    /// Total slack as a thin line after each task.
    pub slack: bool,
}

const CRITICAL: Color32 = Color32::from_rgb(0xE0, 0x5A, 0x4F);
const CRITICAL_EDGE: Color32 = Color32::from_rgb(0xB0, 0x3A, 0x30);

/// Bar for task `i` in the row whose vertical centre is `mid`.
pub fn bar(p: &Painter, s: &Scale, doc: &Doc, i: usize, mid: f32, opts: Options) {
    let t = &doc.project.tasks[i];
    let sch = &doc.schedule[i];
    let (x0, x1) = (s.x(doc.dt(sch.start)), s.x(doc.dt(sch.finish)));
    let font = FontId::proportional(11.0);
    if opts.baseline {
        if let Some(b) = &t.baseline {
            let (b0, b1) = (s.x(b.start), s.x(b.finish));
            p.rect_filled(Rect::from_min_max(Pos2::new(b0, mid + 6.0), Pos2::new(b1.max(b0 + 2.0), mid + 9.0)), 0.0, Color32::from_gray(160));
        }
    }
    if opts.slack && !t.summary && sch.total_slack > 0 {
        // Slack in working days, drawn at calendar-day scale (as Project's slack bar).
        let days = sch.total_slack as f32 / crate::doc::DAY as f32;
        let end = x1 + days * s.day * 7.0 / 5.0;
        p.line_segment([Pos2::new(x1, mid + 4.0), Pos2::new(end, mid + 4.0)], Stroke::new(2.0_f32, Color32::from_rgb(0x2B, 0xAF, 0x9A)));
    }
    if t.summary {
        p.rect_filled(Rect::from_min_max(Pos2::new(x0, mid - 4.0), Pos2::new(x1, mid + 1.0)), 0.0, TEXT);
        for x in [x0, x1] {
            p.add(Shape::convex_polygon(vec![Pos2::new(x - 4.0, mid - 4.0), Pos2::new(x + 4.0, mid - 4.0), Pos2::new(x, mid + 6.0)], TEXT, Stroke::NONE));
        }
        return;
    }
    if t.duration == 0 {
        p.add(Shape::convex_polygon(
            vec![Pos2::new(x0, mid - 6.0), Pos2::new(x0 + 6.0, mid), Pos2::new(x0, mid + 6.0), Pos2::new(x0 - 6.0, mid)],
            TEXT,
            Stroke::NONE,
        ));
        p.text(Pos2::new(x0 + 10.0, mid), Align2::LEFT_CENTER, doc.dt(sch.start).format("%d/%m").to_string(), font, TEXT);
        return;
    }
    let (fill, edge) = if opts.critical && sch.critical {
        (CRITICAL, CRITICAL_EDGE)
    } else if t.manual.is_some() {
        (MANUAL, MANUAL_EDGE)
    } else {
        (BAR, BAR_EDGE)
    };
    for &(a, b) in &sch.segments {
        let (a, b) = (s.x(doc.dt(a)), s.x(doc.dt(b)));
        let r = Rect::from_min_max(Pos2::new(a, mid - 5.0), Pos2::new(b.max(a + 2.0), mid + 5.0));
        p.rect_filled(r, 0.0, fill);
        p.rect_stroke(r, 0.0, Stroke::new(1.0_f32, edge));
    }
    if sch.segments.len() > 1 {
        // Splits: a dotted line through the gaps.
        for w in sch.segments.windows(2) {
            let (a, b) = (s.x(doc.dt(w[0].1)), s.x(doc.dt(w[1].0)));
            let mut x = a;
            while x < b {
                p.line_segment([Pos2::new(x, mid), Pos2::new((x + 2.0).min(b), mid)], Stroke::new(1.0_f32, edge));
                x += 4.0;
            }
        }
    }
    let pct = t.progress.percent_complete as f32 / 100.0;
    if pct > 0.0 {
        let done = x0 + (x1 - x0) * pct;
        p.rect_filled(Rect::from_min_max(Pos2::new(x0, mid - 2.0), Pos2::new(done, mid + 2.0)), 0.0, PROGRESS);
    }
    let names = doc.resources_text(i);
    if !names.is_empty() {
        p.text(Pos2::new(x1 + 6.0, mid), Align2::LEFT_CENTER, names, font, TEXT);
    }
}

/// Finish-to-start arrows (the other link types are drawn the same way from their anchors).
/// `mid_of` gives a task's row centre, or `None` for a task not on display.
pub fn links(p: &Painter, s: &Scale, doc: &Doc, mid_of: impl Fn(usize) -> Option<f32>, row: f32) {
    let stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x3D, 0x5A, 0x6C));
    for (i, t) in doc.project.tasks.iter().enumerate() {
        let xs = s.x(doc.dt(doc.schedule[i].start));
        let Some(ys) = mid_of(i) else { continue };
        for link in &t.preds {
            let pf = s.x(doc.dt(doc.schedule[link.pred].finish));
            let Some(yp) = mid_of(link.pred) else { continue };
            let down = ys > yp;
            let edge = if down { ys - 6.0 } else { ys + 6.0 };
            if xs >= pf + 4.0 {
                // Right from the predecessor, then down (or up) into the successor's start.
                let x = xs + 3.0;
                p.line_segment([Pos2::new(pf, yp), Pos2::new(x, yp)], stroke);
                p.line_segment([Pos2::new(x, yp), Pos2::new(x, edge)], stroke);
                let dir = if down { 1.0 } else { -1.0 };
                p.add(Shape::convex_polygon(
                    vec![Pos2::new(x - 3.0, edge - 4.0 * dir), Pos2::new(x + 3.0, edge - 4.0 * dir), Pos2::new(x, edge)],
                    stroke.color,
                    Stroke::NONE,
                ));
            } else {
                // Back-routed: out to the right, along the row gap, into the successor from the left.
                let out = pf + 6.0;
                let gap = if down { yp + row / 2.0 } else { yp - row / 2.0 };
                let back = xs - 8.0;
                p.line_segment([Pos2::new(pf, yp), Pos2::new(out, yp)], stroke);
                p.line_segment([Pos2::new(out, yp), Pos2::new(out, gap)], stroke);
                p.line_segment([Pos2::new(out, gap), Pos2::new(back, gap)], stroke);
                p.line_segment([Pos2::new(back, gap), Pos2::new(back, ys)], stroke);
                p.line_segment([Pos2::new(back, ys), Pos2::new(xs, ys)], stroke);
                p.add(Shape::convex_polygon(
                    vec![Pos2::new(xs - 4.0, ys - 3.0), Pos2::new(xs - 4.0, ys + 3.0), Pos2::new(xs, ys)],
                    stroke.color,
                    Stroke::NONE,
                ));
            }
        }
    }
}

/// Tooltip text for task `i`.
pub fn tooltip(doc: &Doc, i: usize) -> String {
    let t = &doc.project.tasks[i];
    let s = &doc.schedule[i];
    format!(
        "{}\nInício: {}\nTérmino: {}\nDuração: {}\nFolga total: {}{}",
        t.name,
        fmt::date(doc.dt(s.start)),
        fmt::date(doc.dt(s.finish)),
        fmt::days(doc.shown_days(i)),
        fmt::days(s.total_slack as f64 / crate::doc::DAY as f64),
        if s.critical { "\nTarefa crítica" } else { "" }
    )
}
