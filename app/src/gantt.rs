//! The Gantt chart as Project draws it: a two-tier timescale (weeks over day letters, or months
//! over weeks when zoomed out), nonworking days shaded, the current date line, blue task bars,
//! black summary brackets, milestone diamonds, resource names next to the bars and
//! finish-to-start arrows.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime};
use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use crate::doc::{Doc, GanttEdit};
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture { Move, Resize, Link, Progress }

impl Gesture {
    fn cursor(self) -> egui::CursorIcon {
        match self {
            Self::Move => egui::CursorIcon::Grab,
            Self::Resize | Self::Progress => egui::CursorIcon::ResizeHorizontal,
            Self::Link => egui::CursorIcon::Crosshair,
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Move => "Arraste na horizontal para mover o início (Não iniciar antes de); até outra barra para vincular. Esc cancela.",
            Self::Resize => "Arraste a ponta direita para alterar a duração em dias úteis.",
            Self::Link => "Arraste o início até outra barra para criar um vínculo Término-Início. Ctrl + arrastar ajusta o andamento.",
            Self::Progress => "Arraste para ajustar o percentual concluído. Esc cancela.",
        }
    }
}

struct Drag {
    task: usize,
    version: u64,
    kind: Gesture,
    origin: Pos2,
    start: NaiveDateTime,
    finish: NaiveDateTime,
    moved: bool,
}

/// Only pointer state: the project stays untouched until release, so Esc needs no undo.
#[derive(Default)]
pub struct Interaction { drag: Option<Drag> }

fn bar_hit(doc: &Doc, scale: &Scale, task: usize, mid: f32, pos: Pos2) -> bool {
    if (pos.y - mid).abs() > 7.0 { return false; }
    let sch = &doc.schedule[task];
    if doc.project.tasks[task].duration == 0 {
        return (pos.x - scale.x(doc.dt(sch.start))).abs() <= 7.0;
    }
    sch.segments.iter().any(|&(a, b)| pos.x >= scale.x(doc.dt(a)) - 4.0 && pos.x <= scale.x(doc.dt(b)) + 4.0)
}

fn zone(doc: &Doc, scale: &Scale, task: usize, mid: f32, pos: Pos2, ctrl: bool) -> Option<Gesture> {
    if doc.project.tasks[task].summary || !bar_hit(doc, scale, task, mid, pos) { return None; }
    let t = &doc.project.tasks[task];
    let sch = &doc.schedule[task];
    let (x0, x1) = (scale.x(doc.dt(sch.start)), scale.x(doc.dt(sch.finish)));
    if ctrl { return Some(Gesture::Progress); }
    if t.duration == 0 { return Some(if pos.x < x0 - 3.0 { Gesture::Link } else { Gesture::Move }); }
    let progress = x0 + (x1 - x0) * t.progress.percent_complete as f32 / 100.0;
    if t.progress.percent_complete > 0 && (pos.x - progress).abs() <= 3.0 && (pos.y - mid).abs() <= 2.5 {
        return Some(Gesture::Progress);
    }
    let edge = ((x1 - x0) / 3.0).clamp(0.5, 5.0);
    Some(if pos.x <= x0 + edge { Gesture::Link } else if pos.x >= x1 - edge { Gesture::Resize } else { Gesture::Move })
}

/// The chart's input and preview. `rows` is (row height, vertical scroll offset).
/// `rows` is the row height, the scroll offset and the tasks on display, top to bottom.
pub fn interact(ui: &mut egui::Ui, body: Rect, scale: &Scale, doc: &Doc, state: &mut Interaction, rows: (f32, f32, &[usize]), resp: &egui::Response) -> Option<Result<GanttEdit, String>> {
    let (height, scroll, shown) = rows;
    let mid = |i: usize| {
        let row = shown.iter().position(|&k| k == i).unwrap_or(usize::MAX / 2);
        body.top() + row as f32 * height - scroll + height / 2.0
    };
    let row_at = |pos: Pos2| {
        if !body.contains(pos) { return None; }
        let row = ((pos.y - body.top() + scroll) / height).floor() as usize;
        shown.get(row).copied()
    };
    let (pos, pressed, released, down, ctrl, escape) = ui.input(|i| (
        i.pointer.interact_pos(), i.pointer.primary_pressed(), i.pointer.primary_released(),
        i.pointer.primary_down(), i.modifiers.ctrl, i.key_pressed(egui::Key::Escape),
    ));
    if escape || state.drag.as_ref().is_some_and(|d| d.version != doc.version()) {
        state.drag = None;
        return None;
    }
    if let Some(pos) = pos {
        if let Some(task) = row_at(pos) {
            if let Some(kind) = zone(doc, scale, task, mid(task), pos, ctrl) {
                if resp.hovered() && state.drag.is_none() {
                    ui.ctx().set_cursor_icon(kind.cursor());
                    resp.clone().on_hover_text_at_pointer(format!("{}\n\n{}", tooltip(doc, task), kind.hint()));
                    if matches!(kind, Gesture::Progress | Gesture::Link) {
                        let sch = &doc.schedule[task];
                        let x0 = scale.x(doc.dt(sch.start));
                        let x1 = scale.x(doc.dt(sch.finish));
                        let x = if kind == Gesture::Progress { x0 + (x1 - x0) * doc.project.tasks[task].progress.percent_complete as f32 / 100.0 } else { x0 };
                        let handle = Rect::from_center_size(Pos2::new(x, mid(task)), Vec2::splat(5.0));
                        ui.painter_at(body).rect_filled(handle, 0.0, Color32::WHITE);
                        ui.painter_at(body).rect_stroke(handle, 0.0, Stroke::new(1.0_f32, BAR_EDGE));
                    }
                }
                if pressed && resp.is_pointer_button_down_on() {
                    state.drag = Some(Drag { task, version: doc.version(), kind, origin: pos,
                        start: doc.dt(doc.schedule[task].start), finish: doc.dt(doc.schedule[task].finish), moved: false });
                }
            } else if resp.hovered() && state.drag.is_none() && bar_hit(doc, scale, task, mid(task), pos) {
                resp.clone().on_hover_text_at_pointer(tooltip(doc, task));
            }
        }
    }
    let drag = state.drag.as_mut()?;
    let Some(pos) = pos else { state.drag = None; return None; };
    drag.moved |= pos.distance(drag.origin) >= 4.0;
    if drag.kind == Gesture::Move && (pos.y - drag.origin.y).abs() > height / 2.0 {
        drag.kind = Gesture::Link;
    }
    ui.ctx().set_cursor_icon(if drag.kind == Gesture::Move { egui::CursorIcon::Grabbing } else { drag.kind.cursor() });
    let delta = ((pos.x - drag.origin.x) / scale.day).round() as i64;
    let i = drag.task;
    let mut result = None;
    let mut ghost = None;
    let label = match drag.kind {
        Gesture::Move => match doc.gantt_start(i, drag.start + Duration::days(delta)) {
            Ok(start) => {
                if delta != 0 { result = Some(Ok(GanttEdit::Start(i, start))); }
                let dx = scale.x(start) - scale.x(drag.start);
                ghost = Some((scale.x(drag.start) + dx, scale.x(drag.finish) + dx));
                format!("Não iniciar antes de {}", fmt::date(start))
            }
            Err(e) => { result = Some(Err(e.clone())); e }
        },
        Gesture::Resize => match doc.gantt_duration(i, drag.finish + Duration::days(delta)) {
            Ok(minutes) => {
                let finish = drag.finish + Duration::days(delta);
                result = Some(Ok(GanttEdit::Duration(i, minutes)));
                ghost = Some((scale.x(drag.start), scale.x(finish)));
                format!("Duração: {} · Esc cancela", fmt::days(minutes as f64 / crate::doc::DAY as f64))
            }
            Err(e) => { result = Some(Err(e.clone())); e }
        },
        Gesture::Progress => {
            let x0 = scale.x(drag.start);
            let width = scale.x(drag.finish) - x0;
            let percent = if width > 0.0 { (((pos.x - x0) / width).clamp(0.0, 1.0) * 100.0).round() as u32 }
                else if pos.x >= x0 { 100 } else { 0 };
            result = Some(Ok(GanttEdit::Percent(i, percent)));
            ghost = Some((x0, x0 + width * percent as f32 / 100.0));
            format!("Concluído: {percent}%")
        }
        Gesture::Link => {
            let target = row_at(pos).filter(|&to| to != i && !doc.project.tasks[to].summary && bar_hit(doc, scale, to, mid(to), pos));
            let start = Pos2::new(scale.x(drag.finish), mid(i));
            let p = ui.painter_at(body);
            let color = if target.is_some() { BAR_EDGE } else { Color32::from_rgb(180, 65, 50) };
            p.arrow(start, pos - start, Stroke::new(1.5_f32, color));
            if let Some(to) = target {
                result = Some(Ok(GanttEdit::Link(i, to)));
                format!("Término-Início: {} → {}", doc.project.tasks[i].name, doc.project.tasks[to].name)
            } else { "Solte sobre outra barra para vincular. Esc cancela.".into() }
        }
    };
    if drag.moved {
        let p = ui.painter_at(body);
        if let Some((a, b)) = ghost {
            let r = Rect::from_min_max(Pos2::new(a, mid(i) - 7.0), Pos2::new(b.max(a + 2.0), mid(i) + 7.0));
            p.rect_filled(r, 0.0, BAR.gamma_multiply(0.3));
            p.rect_stroke(r, 0.0, Stroke::new(1.5_f32, BAR_EDGE));
        }
        egui::Area::new(ui.id().with("gantt-drag-tip")).order(egui::Order::Tooltip).interactable(false)
            .fixed_pos(pos + Vec2::new(14.0, 18.0)).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| { ui.label(label); });
            });
    }
    if released {
        let moved = drag.moved;
        state.drag = None;
        return if moved { result } else { None };
    }
    if !down { state.drag = None; }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, RawInput};

    struct User {
        ctx: egui::Context,
        doc: Doc,
        state: Interaction,
        time: f64,
        error: Option<String>,
    }

    impl User {
        fn new() -> Self {
            let mut doc = Doc::blank();
            doc.set_project_start(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()).unwrap();
            doc.add_task("A", 3.0).unwrap();
            doc.add_task("B", 2.0).unwrap();
            Self { ctx: egui::Context::default(), doc, state: Interaction::default(), time: 0.0, error: None }
        }

        fn frame(&mut self, events: Vec<Event>, modifiers: Modifiers) {
            self.time += 0.05;
            let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 400.0))), events,
                modifiers, time: Some(self.time), ..Default::default() };
            let _ = self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let body = Rect::from_min_max(Pos2::new(0.0, 40.0), Pos2::new(1000.0, 400.0));
                    let scale = Scale::new(&self.doc, 40.0, 0.0);
                    let resp = ui.interact(body, ui.id().with("chart"), egui::Sense::click_and_drag());
                    let shown: Vec<usize> = (0..self.doc.project.tasks.len()).collect();
                    if let Some(edit) = interact(ui, body, &scale, &self.doc, &mut self.state, (24.0, 0.0, &shown), &resp) {
                        let v = self.doc.begin();
                        self.error = edit.and_then(|edit| self.doc.edit_gantt(edit)).err();
                        self.doc.end(v);
                    }
                });
            });
        }

        fn point(&self, task: usize, fraction: f32) -> Pos2 {
            let s = Scale::new(&self.doc, 40.0, 0.0);
            let sch = &self.doc.schedule[task];
            let x = s.x(self.doc.dt(sch.start)) * (1.0 - fraction) + s.x(self.doc.dt(sch.finish)) * fraction;
            Pos2::new(x, 52.0 + task as f32 * 24.0)
        }

        fn press(&mut self, pos: Pos2, modifiers: Modifiers) {
            self.frame(vec![Event::PointerMoved(pos)], modifiers);
            self.frame(vec![Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers }], modifiers);
        }

        fn release(&mut self, pos: Pos2, modifiers: Modifiers) {
            self.frame(vec![Event::PointerMoved(pos)], modifiers);
            self.frame(vec![Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers }], modifiers);
        }
    }

    #[test]
    fn pointer_move_previews_then_commits_once_and_escape_cancels() {
        let mut u = User::new();
        let start = u.doc.schedule[0].start;
        let from = u.point(0, 0.5);
        u.press(from, Modifiers::NONE);
        u.frame(vec![Event::PointerMoved(from + Vec2::new(40.0, 0.0))], Modifiers::NONE);
        assert_eq!(u.doc.schedule[0].start, start);
        assert!(!u.doc.can_undo());
        u.release(from + Vec2::new(80.0, 0.0), Modifiers::NONE);
        assert_eq!(u.doc.dt(u.doc.schedule[0].start), u.doc.dt(start) + Duration::days(2));
        assert!(u.error.is_none());
        u.doc.undo().unwrap();
        assert_eq!(u.doc.schedule[0].start, start);
        assert!(!u.doc.can_undo());
        u.press(from, Modifiers::NONE);
        u.frame(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
        u.release(from + Vec2::new(120.0, 0.0), Modifiers::NONE);
        assert_eq!(u.doc.schedule[0].start, start);
        assert!(!u.doc.can_undo());
    }

    #[test]
    fn pointer_resize_link_and_ctrl_progress_use_separate_zones() {
        let mut u = User::new();
        let end = u.point(0, 1.0);
        u.press(end + Vec2::new(0.0, -4.0), Modifiers::NONE);
        u.release(end + Vec2::new(80.0, -4.0), Modifiers::NONE);
        assert_eq!(u.doc.days(0), 5.0);
        u.doc.undo().unwrap();
        assert!(!u.doc.can_undo());
        let from = u.point(0, 0.0);
        let to = u.point(1, 0.5);
        u.press(from, Modifiers::NONE);
        u.release(to, Modifiers::NONE);
        assert_eq!(u.doc.project.tasks[1].preds[0].pred, 0);
        u.doc.undo().unwrap();
        assert!(u.doc.project.tasks[1].preds.is_empty());
        let middle = u.point(0, 0.5);
        u.press(middle, Modifiers::NONE);
        u.release(to, Modifiers::NONE);
        assert_eq!(u.doc.project.tasks[1].preds[0].pred, 0);
        u.doc.undo().unwrap();
        let half = u.point(0, 0.5);
        u.press(from, Modifiers::CTRL);
        u.release(half, Modifiers::CTRL);
        assert_eq!(u.doc.project.tasks[0].progress.percent_complete, 50);
        u.doc.undo().unwrap();
        assert_eq!(u.doc.project.tasks[0].progress.percent_complete, 0);
        assert!(!u.doc.can_undo());
        u.doc.set_percent(0, 25).unwrap();
        let edge = u.point(0, 0.25);
        let three_quarters = u.point(0, 0.75);
        u.press(edge, Modifiers::NONE);
        u.release(three_quarters, Modifiers::NONE);
        assert_eq!(u.doc.project.tasks[0].progress.percent_complete, 75);
        u.doc.undo().unwrap();
        assert_eq!(u.doc.project.tasks[0].progress.percent_complete, 25);
        assert!(!u.doc.can_undo());
    }

    #[test]
    fn dropping_a_link_outside_a_bar_or_changing_the_document_cancels() {
        let mut u = User::new();
        let from = u.point(0, 0.0);
        u.press(from, Modifiers::NONE);
        u.release(Pos2::new(900.0, 300.0), Modifiers::NONE);
        assert!(u.doc.project.tasks[1].preds.is_empty());
        assert!(!u.doc.can_undo());
        let from = u.point(0, 0.5);
        u.press(from, Modifiers::NONE);
        u.doc.rename(0, "Alterada durante o arrasto");
        u.release(from + Vec2::new(80.0, 0.0), Modifiers::NONE);
        assert_eq!(u.doc.project.tasks[0].constraint, pjsched::model::Constraint::AsSoonAsPossible);
        assert!(!u.doc.can_undo());
    }
}
