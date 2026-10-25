//! PDF export written from scratch (PDF 1.4, no external crate): vector lines, rectangles,
//! polygons and Type1 Helvetica text in WinAnsiEncoding (covering Portuguese accents).
//!
//! Exports:
//! - [`gantt_pdf`]: task table on the left + Gantt chart on the right, paginated on A4 landscape.
//! - [`report_pdf`] / [`all_reports_pdf`]: the reports of the `Relatório` tab.

#![allow(dead_code, private_interfaces)]

use std::fmt::Write as _;
use std::path::Path;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::doc::Doc;
use crate::{fmt, Report};

/// A4 landscape dimensions in PDF points (1 pt = 1/72 inch).
const PAGE_W: f32 = 842.0;
const PAGE_H: f32 = 595.0;
const MARGIN: f32 = 28.0;

type Rgb = (u8, u8, u8);

const TEAL: Rgb = (0x0B, 0x6B, 0x66);
const NAVY: Rgb = (0x1E, 0x2F, 0x3E);
const EMERALD: Rgb = (0x14, 0xC3, 0x8E);
const MANUAL_FILL: Rgb = (0xA6, 0xF0, 0xDF);
const MANUAL_EDGE: Rgb = (0x2B, 0xAF, 0x9A);
const CRITICAL_FILL: Rgb = (0xE0, 0x5A, 0x4F);
const CRITICAL_EDGE: Rgb = (0xB0, 0x3A, 0x30);
const HEADER_BG: Rgb = (0xEE, 0xF3, 0xF6);
const STRIPE_BG: Rgb = (0xF8, 0xFB, 0xFC);
const NONWORKING_BG: Rgb = (0xF1, 0xF5, 0xF8);
const GRID_LINE: Rgb = (0xDC, 0xE5, 0xEA);
const BORDER_LINE: Rgb = (0xA8, 0xB6, 0xC0);
const LINK_COLOR: Rgb = (0x3D, 0x5A, 0x6C);
const MUTED: Rgb = (0x64, 0x6E, 0x78);
const WHITE: Rgb = (0xFF, 0xFF, 0xFF);

/// One page's PDF content stream, using top-down coordinates (`(0, 0)` at top-left).
#[derive(Default)]
struct Canvas {
    stream: String,
}

impl Canvas {
    fn y(top_down_y: f32) -> f32 {
        PAGE_H - top_down_y
    }

    fn set_fill(&mut self, (r, g, b): Rgb) {
        let _ = writeln!(self.stream, "{:.3} {:.3} {:.3} rg", r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    }

    fn set_stroke(&mut self, (r, g, b): Rgb, width: f32) {
        let _ = writeln!(
            self.stream,
            "{:.2} w {:.3} {:.3} {:.3} RG",
            width,
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0
        );
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, fill: Rgb) {
        self.set_fill(fill);
        let py = Self::y(y + h);
        let _ = writeln!(self.stream, "{x:.2} {py:.2} {w:.2} {h:.2} re f");
    }

    fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, stroke: Rgb, width: f32) {
        self.set_stroke(stroke, width);
        let py = Self::y(y + h);
        let _ = writeln!(self.stream, "{x:.2} {py:.2} {w:.2} {h:.2} re S");
    }

    fn fill_stroke_rect(&mut self, (x, y, w, h): (f32, f32, f32, f32), fill: Rgb, stroke: Rgb, width: f32) {
        self.set_fill(fill);
        self.set_stroke(stroke, width);
        let py = Self::y(y + h);
        let _ = writeln!(self.stream, "{x:.2} {py:.2} {w:.2} {h:.2} re B");
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, stroke: Rgb, width: f32) {
        self.set_stroke(stroke, width);
        let (py1, py2) = (Self::y(y1), Self::y(y2));
        let _ = writeln!(self.stream, "{x1:.2} {py1:.2} m {x2:.2} {py2:.2} l S");
    }

    fn dashed_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, stroke: Rgb, width: f32) {
        self.set_stroke(stroke, width);
        let (py1, py2) = (Self::y(y1), Self::y(y2));
        let _ = writeln!(self.stream, "[2 2] 0 d {x1:.2} {py1:.2} m {x2:.2} {py2:.2} l S [] 0 d");
    }

    fn polygon(&mut self, pts: &[(f32, f32)], fill: Rgb) {
        if pts.is_empty() {
            return;
        }
        self.set_fill(fill);
        let (x0, y0) = pts[0];
        let _ = write!(self.stream, "{:.2} {:.2} m ", x0, Self::y(y0));
        for &(x, y) in &pts[1..] {
            let _ = write!(self.stream, "{:.2} {:.2} l ", x, Self::y(y));
        }
        self.stream.push_str("h f\n");
    }

    fn text(&mut self, x: f32, baseline_y: f32, text: &str, size: f32, bold: bool, color: Rgb) {
        if text.is_empty() {
            return;
        }
        self.set_fill(color);
        let font = if bold { "F2" } else { "F1" };
        let py = Self::y(baseline_y);
        let escaped = pdf_escape(text);
        let _ = writeln!(self.stream, "BT /{font} {size:.2} Tf {x:.2} {py:.2} Td ({escaped}) Tj ET");
    }

    fn text_clipped(&mut self, (x, baseline_y): (f32, f32), clip: (f32, f32, f32, f32), text: &str, size: f32, bold: bool, color: Rgb) {
        if text.is_empty() {
            return;
        }
        let (cx, cy, cw, ch) = clip;
        let max_chars = ((cw / (size * 0.50)).floor() as usize).max(1);
        let fitted = truncate_chars(text, max_chars);
        let pcy = Self::y(cy + ch);
        let _ = writeln!(self.stream, "q {cx:.2} {pcy:.2} {cw:.2} {ch:.2} re W n");
        self.text(x, baseline_y, &fitted, size, bold, color);
        self.stream.push_str("Q\n");
    }

    fn text_right(&mut self, right_x: f32, baseline_y: f32, text: &str, size: f32, bold: bool, color: Rgb) {
        let w = estimate_width(text, size, bold);
        self.text((right_x - w).max(0.0), baseline_y, text, size, bold, color);
    }

    fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let py = Self::y(y + h);
        let _ = writeln!(self.stream, "q {x:.2} {py:.2} {w:.2} {h:.2} re W n");
    }

    fn pop_clip(&mut self) {
        self.stream.push_str("Q\n");
    }
}

/// Encodes UTF-8 text into a PDF string literal under `WinAnsiEncoding`.
fn pdf_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' | '\n' | '\t' => out.push(' '),
            ' '..='~' => out.push(c),
            '\u{00A0}'..='\u{00FF}' => {
                let _ = write!(out, "\\{:03o}", c as u8);
            }
            '•' => out.push_str("\\225"),
            '–' => out.push_str("\\226"),
            '—' => out.push_str("\\227"),
            '€' => out.push_str("\\200"),
            '…' => out.push_str("..."),
            _ => out.push('?'),
        }
    }
    out
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    if max_chars <= 2 {
        return text.chars().take(max_chars).collect();
    }
    let mut s: String = text.chars().take(max_chars - 1).collect();
    s.push('…');
    s
}

fn estimate_width(text: &str, size: f32, bold: bool) -> f32 {
    let factor = if bold { 0.55 } else { 0.51 };
    text.chars().count() as f32 * size * factor
}

/// Serializes `pages` into a complete PDF 1.4 byte buffer.
fn build_pdf(pages: &[Canvas]) -> Vec<u8> {
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets: Vec<usize> = Vec::new();
    let begin_obj = |out: &mut String, offsets: &mut Vec<usize>| -> usize {
        offsets.push(out.len());
        let id = offsets.len();
        let _ = writeln!(out, "{id} 0 obj");
        id
    };

    // 1: Catalog
    begin_obj(&mut out, &mut offsets);
    out.push_str("<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // 2: Pages
    let kids: String = (0..pages.len()).map(|i| format!("{} 0 R", 6 + i * 2)).collect::<Vec<_>>().join(" ");
    begin_obj(&mut out, &mut offsets);
    let _ = writeln!(out, "<< /Type /Pages /Kids [{kids}] /Count {} >>\nendobj", pages.len());

    // 3: Font F1 (Helvetica)
    begin_obj(&mut out, &mut offsets);
    out.push_str("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n");

    // 4: Font F2 (Helvetica-Bold)
    begin_obj(&mut out, &mut offsets);
    out.push_str("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>\nendobj\n");

    for page in pages {
        let content_id = begin_obj(&mut out, &mut offsets);
        let len = page.stream.len();
        let _ = write!(out, "<< /Length {len} >>\nstream\n{}endstream\nendobj\n", page.stream);

        begin_obj(&mut out, &mut offsets);
        let _ = writeln!(
            out,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W:.0} {PAGE_H:.0}] \
             /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content_id} 0 R >>\nendobj"
        );
    }

    let xref_pos = out.len();
    let total_objs = offsets.len() + 1;
    let _ = writeln!(out, "xref\n0 {total_objs}\n0000000000 65535 f ");
    for off in offsets {
        let _ = writeln!(out, "{off:010} 00000 n ");
    }
    let _ = writeln!(out, "trailer\n<< /Size {total_objs} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF");
    out.into_bytes()
}

fn project_title(doc: &Doc) -> String {
    doc.path
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Projeto".into())
}

fn draw_page_banner(c: &mut Canvas, doc: &Doc, subtitle: &str, page_num: usize, total_pages: usize) {
    let x = MARGIN;
    let y = MARGIN;
    let w = PAGE_W - 2.0 * MARGIN;
    let h = 24.0;
    c.fill_rect(x, y, w, h, TEAL);
    let title = format!("Etapix — {} ({subtitle})", project_title(doc));
    c.text(x + 8.0, y + 16.0, &title, 11.0, true, WHITE);

    let finish = doc
        .schedule
        .iter()
        .map(|s| doc.dt(s.finish))
        .max()
        .unwrap_or(doc.project.start);
    let meta = format!(
        "Início: {}   Término: {}   Página {page_num} de {total_pages}",
        fmt::date(doc.project.start),
        fmt::date(finish)
    );
    c.text_right(x + w - 8.0, y + 16.0, &meta, 9.0, false, WHITE);
}

// ---------------------------------------------------------------------------------------------
// Gantt PDF export

const TABLE_COLS: [(&str, f32); 6] = [
    ("Id", 24.0),
    ("Nome da Tarefa", 142.0),
    ("Duração", 46.0),
    ("Início", 58.0),
    ("Término", 58.0),
    ("Pred.", 48.0),
];

struct PdfScale {
    first: NaiveDate,
    day_w: f32,
    left: f32,
}

impl PdfScale {
    fn new(doc: &Doc, left: f32, width: f32) -> Self {
        let min_dt = doc.schedule.iter().map(|s| doc.dt(s.start)).min().unwrap_or(doc.project.start);
        let max_dt = doc.schedule.iter().map(|s| doc.dt(s.finish)).max().unwrap_or(doc.project.start);
        let start_date = min_dt.date().min(doc.project.start.date());
        let first = start_date - Duration::days(start_date.weekday().num_days_from_sunday() as i64 + 1);
        let last = max_dt.date().max(start_date) + Duration::days(5);
        let span_days = (last - first).num_days().max(7) as f32;
        let day_w = (width / span_days).clamp(0.4, 28.0);
        PdfScale { first, day_w, left }
    }

    fn x(&self, dt: NaiveDateTime) -> f32 {
        let mins = (dt - self.first.and_hms_opt(0, 0, 0).unwrap()).num_minutes() as f32;
        self.left + (mins / 1440.0) * self.day_w
    }

    fn x_date(&self, d: NaiveDate) -> f32 {
        self.left + (d - self.first).num_days() as f32 * self.day_w
    }

    fn date_at(&self, x: f32) -> NaiveDate {
        self.first + Duration::days(((x - self.left) / self.day_w).floor() as i64)
    }
}

/// Exports the task table and Gantt chart of `doc` to `path`, paginated on A4 landscape.
pub fn gantt_pdf(doc: &Doc, path: &Path) -> Result<(), String> {
    let bytes = gantt_pdf_bytes(doc);
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn gantt_pdf_bytes(doc: &Doc) -> Vec<u8> {
    let top = MARGIN + 28.0;
    let head_h = 26.0;
    let row_h = 18.0;
    let body_top = top + head_h;
    let body_h = PAGE_H - MARGIN - body_top;
    let rows_per_page = ((body_h / row_h).floor() as usize).max(1);
    let n = doc.project.tasks.len();
    let total_pages = n.div_ceil(rows_per_page).max(1);

    let table_w: f32 = TABLE_COLS.iter().map(|c| c.1).sum();
    let chart_left = MARGIN + table_w;
    let chart_w = PAGE_W - MARGIN - chart_left;
    let scale = PdfScale::new(doc, chart_left, chart_w);

    let mut pages = Vec::with_capacity(total_pages);
    for page_idx in 0..total_pages {
        let mut c = Canvas::default();
        draw_page_banner(&mut c, doc, "Gráfico de Gantt", page_idx + 1, total_pages);

        let start_row = page_idx * rows_per_page;
        let end_row = (start_row + rows_per_page).min(n);
        let page_rows = end_row.saturating_sub(start_row);
        let used_h = (page_rows.max(1) as f32) * row_h;

        // Table header
        c.fill_rect(MARGIN, top, table_w, head_h, HEADER_BG);
        let mut cx = MARGIN;
        for &(title, w) in &TABLE_COLS {
            c.line(cx + w, top, cx + w, top + head_h + used_h, GRID_LINE, 0.5);
            c.text_clipped((cx + 3.0, top + 16.0), (cx + 1.0, top, w - 2.0, head_h), title, 8.0, true, NAVY);
            cx += w;
        }

        // Gantt header and background
        draw_gantt_header(&mut c, &scale, chart_left, top, chart_w, head_h);
        draw_gantt_background(&mut c, doc, &scale, chart_left, body_top, chart_w, used_h);

        // Task rows
        for (local_r, i) in (start_row..end_row).enumerate() {
            let ry = body_top + local_r as f32 * row_h;
            if local_r % 2 == 1 {
                c.fill_rect(MARGIN, ry, table_w, row_h, STRIPE_BG);
            }
            c.line(MARGIN, ry + row_h, MARGIN + table_w + chart_w, ry + row_h, GRID_LINE, 0.5);

            let t = &doc.project.tasks[i];
            let s = &doc.schedule[i];
            let indent = (doc.outline_level(i).saturating_sub(1) as f32) * 7.0;
            let cells = [
                (t.id.to_string(), 0.0, false),
                (t.name.clone(), indent, t.summary),
                (fmt::days(doc.shown_days(i)), 0.0, false),
                (fmt::date(doc.dt(s.start)), 0.0, false),
                (fmt::date(doc.dt(s.finish)), 0.0, false),
                (doc.predecessors_text(i), 0.0, false),
            ];
            let mut x = MARGIN;
            for ((text, pad, bold), &(_, w)) in cells.iter().zip(&TABLE_COLS) {
                let tx = (x + 3.0 + pad).min(x + w - 4.0);
                c.text_clipped((tx, ry + 12.0), (x + 1.0, ry, w - 2.0, row_h), text, 7.5, *bold, NAVY);
                x += w;
            }
        }

        // Gantt bars and links clipped to chart body
        c.push_clip(chart_left, body_top, chart_w, used_h);
        draw_gantt_links(&mut c, doc, &scale, start_row, end_row, body_top, row_h);
        for (local_r, i) in (start_row..end_row).enumerate() {
            let mid = body_top + local_r as f32 * row_h + row_h / 2.0;
            draw_gantt_bar(&mut c, doc, &scale, i, mid);
        }
        c.pop_clip();

        // Outer frame and pane divider
        c.line(MARGIN, body_top, MARGIN + table_w + chart_w, body_top, BORDER_LINE, 0.8);
        c.line(chart_left, top, chart_left, body_top + used_h, BORDER_LINE, 0.8);
        c.stroke_rect(MARGIN, top, table_w + chart_w, head_h + used_h, BORDER_LINE, 0.8);

        pages.push(c);
    }

    build_pdf(&pages)
}

fn draw_gantt_header(c: &mut Canvas, s: &PdfScale, left: f32, top: f32, width: f32, height: f32) {
    c.fill_rect(left, top, width, height, HEADER_BG);
    let mid = top + height / 2.0;
    c.line(left, mid, left + width, mid, GRID_LINE, 0.6);
    c.push_clip(left, top, width, height);

    let (d0, d1) = (s.date_at(left), s.date_at(left + width) + Duration::days(1));
    if s.day_w >= 10.0 {
        let mut w = d0 - Duration::days(d0.weekday().num_days_from_sunday() as i64);
        while w <= d1 {
            let x = s.x_date(w);
            if x >= left {
                c.line(x, top, x, top + height, GRID_LINE, 0.5);
            }
            c.text(x.max(left) + 3.0, top + 9.5, &fmt::week_label(w), 7.0, false, NAVY);
            w += Duration::days(7);
        }
        let mut d = d0;
        while d <= d1 {
            let x = s.x_date(d);
            if x >= left {
                c.line(x, mid, x, top + height, GRID_LINE, 0.5);
            }
            let letter = fmt::DAY_LETTERS[d.weekday().num_days_from_sunday() as usize];
            c.text(x + s.day_w * 0.35, mid + 9.5, letter, 6.5, false, NAVY);
            d += Duration::days(1);
        }
    } else {
        let mut m = NaiveDate::from_ymd_opt(d0.year(), d0.month(), 1).unwrap();
        while m <= d1 {
            let x = s.x_date(m);
            if x >= left {
                c.line(x, top, x, mid, GRID_LINE, 0.5);
            }
            let label = format!("{}/{}", fmt::MONTHS[m.month0() as usize], m.format("%y"));
            c.text(x.max(left) + 3.0, top + 9.5, &label, 7.0, false, NAVY);
            m = if m.month() == 12 {
                NaiveDate::from_ymd_opt(m.year() + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(m.year(), m.month() + 1, 1).unwrap()
            };
        }
        let mut w = d0 - Duration::days(d0.weekday().num_days_from_sunday() as i64);
        while w <= d1 {
            let x = s.x_date(w);
            if x >= left {
                c.line(x, mid, x, top + height, GRID_LINE, 0.5);
            }
            if s.day_w * 7.0 >= 16.0 {
                c.text(x + 2.0, mid + 9.5, &w.format("%d").to_string(), 6.5, false, MUTED);
            }
            w += Duration::days(7);
        }
    }
    c.pop_clip();
}

fn draw_gantt_background(c: &mut Canvas, doc: &Doc, s: &PdfScale, left: f32, top: f32, width: f32, height: f32) {
    c.push_clip(left, top, width, height);
    if s.day_w >= 3.0 {
        let (d0, d1) = (s.date_at(left), s.date_at(left + width) + Duration::days(1));
        let mut d = d0;
        while d <= d1 {
            if !doc.is_working_day(d) {
                let x = s.x_date(d);
                c.fill_rect(x, top, s.day_w, height, NONWORKING_BG);
            }
            d += Duration::days(1);
        }
    }
    c.pop_clip();
}

fn draw_gantt_bar(c: &mut Canvas, doc: &Doc, s: &PdfScale, i: usize, mid: f32) {
    let t = &doc.project.tasks[i];
    let sch = &doc.schedule[i];
    let (x0, x1) = (s.x(doc.dt(sch.start)), s.x(doc.dt(sch.finish)));

    if let Some(b) = &t.baseline {
        let (b0, b1) = (s.x(b.start), s.x(b.finish));
        c.fill_rect(b0, mid + 4.0, (b1 - b0).max(2.0), 2.0, (160, 165, 170));
    }

    if t.summary {
        let w = (x1 - x0).max(2.0);
        c.fill_rect(x0, mid - 3.5, w, 3.5, NAVY);
        for x in [x0, x0 + w] {
            c.polygon(&[(x - 3.0, mid - 3.5), (x + 3.0, mid - 3.5), (x, mid + 3.5)], NAVY);
        }
        return;
    }

    if t.duration == 0 {
        c.polygon(&[(x0, mid - 4.5), (x0 + 4.5, mid), (x0, mid + 4.5), (x0 - 4.5, mid)], NAVY);
        c.text(x0 + 7.0, mid + 2.5, &doc.dt(sch.start).format("%d/%m").to_string(), 6.5, false, NAVY);
        return;
    }

    let (fill, edge) = if sch.critical {
        (CRITICAL_FILL, CRITICAL_EDGE)
    } else if t.manual.is_some() {
        (MANUAL_FILL, MANUAL_EDGE)
    } else {
        (EMERALD, TEAL)
    };

    for &(a, b) in &sch.segments {
        let (sx0, sx1) = (s.x(doc.dt(a)), s.x(doc.dt(b)));
        c.fill_stroke_rect((sx0, mid - 4.0, (sx1 - sx0).max(2.0), 8.0), fill, edge, 0.7);
    }
    if sch.segments.len() > 1 {
        for w in sch.segments.windows(2) {
            let (a, b) = (s.x(doc.dt(w[0].1)), s.x(doc.dt(w[1].0)));
            c.dashed_line(a, mid, b, mid, edge, 0.7);
        }
    }

    let pct = t.progress.percent_complete as f32 / 100.0;
    if pct > 0.0 {
        let done_w = ((x1 - x0).max(2.0)) * pct.clamp(0.0, 1.0);
        c.fill_rect(x0, mid - 1.5, done_w, 3.0, TEAL);
    }

    let names = doc.resources_text(i);
    if !names.is_empty() {
        c.text(x1.max(x0 + 2.0) + 4.0, mid + 2.5, &names, 6.5, false, NAVY);
    }
}

fn draw_gantt_links(c: &mut Canvas, doc: &Doc, s: &PdfScale, start_row: usize, end_row: usize, body_top: f32, row_h: f32) {
    let mid_of = |r: usize| body_top + (r - start_row) as f32 * row_h + row_h / 2.0;
    for i in start_row..end_row {
        let xs = s.x(doc.dt(doc.schedule[i].start));
        let ys = mid_of(i);
        for link in &doc.project.tasks[i].preds {
            if !(start_row..end_row).contains(&link.pred) {
                continue;
            }
            let pf = s.x(doc.dt(doc.schedule[link.pred].finish));
            let yp = mid_of(link.pred);
            let down = ys > yp;
            let edge = if down { ys - 4.5 } else { ys + 4.5 };
            if xs >= pf + 3.0 {
                let x = xs + 2.0;
                c.line(pf, yp, x, yp, LINK_COLOR, 0.7);
                c.line(x, yp, x, edge, LINK_COLOR, 0.7);
                let dir = if down { 1.0 } else { -1.0 };
                c.polygon(&[(x - 2.2, edge - 3.0 * dir), (x + 2.2, edge - 3.0 * dir), (x, edge)], LINK_COLOR);
            } else {
                let out = pf + 4.0;
                let gap = if down { yp + row_h / 2.0 } else { yp - row_h / 2.0 };
                let back = xs - 5.0;
                c.line(pf, yp, out, yp, LINK_COLOR, 0.7);
                c.line(out, yp, out, gap, LINK_COLOR, 0.7);
                c.line(out, gap, back, gap, LINK_COLOR, 0.7);
                c.line(back, gap, back, ys, LINK_COLOR, 0.7);
                c.line(back, ys, xs, ys, LINK_COLOR, 0.7);
                c.polygon(&[(xs - 3.0, ys - 2.2), (xs - 3.0, ys + 2.2), (xs, ys)], LINK_COLOR);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Report PDF export

fn money(v: f64) -> String {
    let s = format!("{v:.2}").replace('.', ",");
    format!("R$ {s}")
}

struct ReportSection {
    title: String,
    summary_line: Option<String>,
    headers: Vec<(&'static str, f32)>,
    rows: Vec<Vec<String>>,
    empty_note: &'static str,
}

fn dashboard_section(doc: &Doc) -> ReportSection {
    let n = doc.project.tasks.len();
    let finish = doc.schedule.iter().map(|s| s.finish).max();
    let top_cost: f64 = (0..n).filter(|&i| doc.project.tasks[i].parent.is_none()).map(|i| doc.costs.task[i]).sum();
    let done = doc.project.tasks.iter().filter(|t| t.progress.percent_complete >= 100).count();
    let late = (0..n).filter(|&i| doc.schedule[i].total_slack < 0).count();
    let rows = vec![
        vec!["Início".into(), fmt::date(doc.project.start)],
        vec!["Término".into(), finish.map(|f| fmt::date(doc.dt(f))).unwrap_or_else(|| "-".into())],
        vec!["Tarefas".into(), n.to_string()],
        vec!["Concluídas".into(), done.to_string()],
        vec!["Com folga negativa".into(), late.to_string()],
        vec!["Recursos".into(), doc.project.resources.len().to_string()],
        vec!["Custo total".into(), money(top_cost)],
    ];
    ReportSection {
        title: "Visão Geral do Projeto".into(),
        summary_line: None,
        headers: vec![("Indicador", 240.0), ("Valor", 240.0)],
        rows,
        empty_note: "",
    }
}

fn resources_section(doc: &Doc) -> ReportSection {
    let rows = doc
        .project
        .resources
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let tasks = doc.project.assignments.iter().filter(|a| a.resource == k).count();
            vec![
                r.name.clone(),
                format!("{:.0}%", r.max_units * 100.0),
                tasks.to_string(),
                money(doc.costs.resource[k]),
            ]
        })
        .collect();
    ReportSection {
        title: "Relatório: Recursos".into(),
        summary_line: None,
        headers: vec![("Recurso", 280.0), ("Unid. máximas", 140.0), ("Tarefas", 140.0), ("Custo", 226.0)],
        rows,
        empty_note: "O projeto ainda não tem recursos cadastrados.",
    }
}

fn costs_section(doc: &Doc) -> ReportSection {
    let rows: Vec<Vec<String>> = (0..doc.project.tasks.len())
        .filter(|&i| doc.costs.task[i] != 0.0)
        .map(|i| vec![doc.project.tasks[i].id.to_string(), doc.project.tasks[i].name.clone(), money(doc.costs.task[i])])
        .collect();
    let total: f64 = (0..doc.project.tasks.len())
        .filter(|&i| doc.project.tasks[i].parent.is_none())
        .map(|i| doc.costs.task[i])
        .sum();
    ReportSection {
        title: "Relatório: Custos".into(),
        summary_line: Some(format!("Custo total do projeto: {}", money(total))),
        headers: vec![("Id", 60.0), ("Tarefa", 500.0), ("Custo", 226.0)],
        rows,
        empty_note: "Nenhuma tarefa tem custo (atribua recursos com taxas ou custos fixos).",
    }
}

fn in_progress_section(doc: &Doc) -> ReportSection {
    let today = chrono::Local::now().naive_local();
    let rows: Vec<Vec<String>> = (0..doc.project.tasks.len())
        .filter(|&i| {
            let t = &doc.project.tasks[i];
            let late = t.progress.percent_complete < 100 && doc.dt(doc.schedule[i].finish) < today;
            !t.summary && ((1..100).contains(&t.progress.percent_complete) || late)
        })
        .map(|i| {
            let t = &doc.project.tasks[i];
            let late = doc.dt(doc.schedule[i].finish) < today;
            vec![
                t.id.to_string(),
                t.name.clone(),
                format!("{}%", t.progress.percent_complete),
                fmt::date(doc.dt(doc.schedule[i].finish)),
                if late { "Atrasada".into() } else { "Em andamento".into() },
            ]
        })
        .collect();
    ReportSection {
        title: "Relatório: Em Andamento".into(),
        summary_line: None,
        headers: vec![
            ("Id", 50.0),
            ("Tarefa", 356.0),
            ("% concluída", 110.0),
            ("Término", 130.0),
            ("Situação", 140.0),
        ],
        rows,
        empty_note: "Nenhuma tarefa em andamento ou atrasada.",
    }
}

/// Exports the given `report` from the `Relatório` tab to `path` as an A4 landscape PDF.
pub fn report_pdf(doc: &Doc, report: Report, path: &Path) -> Result<(), String> {
    let sections = match report {
        Report::Dashboard => vec![
            dashboard_section(doc),
            resources_section(doc),
            costs_section(doc),
            in_progress_section(doc),
        ],
        Report::Resources => vec![resources_section(doc)],
        Report::Costs => vec![costs_section(doc)],
        Report::InProgress => vec![in_progress_section(doc)],
    };
    let subtitle = match report {
        Report::Dashboard => "Relatório: Painéis",
        Report::Resources => "Relatório: Recursos",
        Report::Costs => "Relatório: Custos",
        Report::InProgress => "Relatório: Em Andamento",
    };
    let bytes = sections_pdf_bytes(doc, subtitle, &sections);
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Exports all reports of the `Relatório` tab (`Painéis`, `Recursos`, `Custos`, `Em Andamento`)
/// into a single PDF file.
pub fn all_reports_pdf(doc: &Doc, path: &Path) -> Result<(), String> {
    report_pdf(doc, Report::Dashboard, path)
}

fn sections_pdf_bytes(doc: &Doc, subtitle: &str, sections: &[ReportSection]) -> Vec<u8> {
    let row_h = 18.0;
    let head_h = 20.0;
    let max_y = PAGE_H - MARGIN;

    let mut pages: Vec<Canvas> = Vec::new();
    let mut c = Canvas::default();
    let mut y = MARGIN + 36.0;

    let draw_table_header = |c: &mut Canvas, y: f32, headers: &[(&str, f32)]| {
        let total_w: f32 = headers.iter().map(|h| h.1).sum();
        c.fill_stroke_rect((MARGIN, y, total_w, head_h), HEADER_BG, BORDER_LINE, 0.6);
        let mut x = MARGIN;
        for &(label, w) in headers {
            c.line(x + w, y, x + w, y + head_h, GRID_LINE, 0.5);
            c.text_clipped((x + 6.0, y + 13.5), (x + 1.0, y, w - 2.0, head_h), label, 8.5, true, NAVY);
            x += w;
        }
    };

    for section in sections {
        let needed = 42.0 + head_h + row_h;
        if y + needed > max_y && y > MARGIN + 40.0 {
            pages.push(c);
            c = Canvas::default();
            y = MARGIN + 36.0;
        }

        c.text(MARGIN, y + 12.0, &section.title, 12.0, true, TEAL);
        y += 18.0;

        if let Some(summary) = &section.summary_line {
            c.text(MARGIN, y + 10.0, summary, 9.5, true, NAVY);
            y += 16.0;
        }

        if section.rows.is_empty() {
            c.text(MARGIN, y + 11.0, section.empty_note, 9.0, false, MUTED);
            y += 24.0;
            continue;
        }

        draw_table_header(&mut c, y, &section.headers);
        y += head_h;

        let total_w: f32 = section.headers.iter().map(|h| h.1).sum();
        for (r_idx, row) in section.rows.iter().enumerate() {
            if y + row_h > max_y {
                pages.push(c);
                c = Canvas::default();
                y = MARGIN + 36.0;
                c.text(MARGIN, y + 12.0, &format!("{} (continuação)", section.title), 11.0, true, TEAL);
                y += 18.0;
                draw_table_header(&mut c, y, &section.headers);
                y += head_h;
            }
            if r_idx % 2 == 1 {
                c.fill_rect(MARGIN, y, total_w, row_h, STRIPE_BG);
            }
            c.stroke_rect(MARGIN, y, total_w, row_h, GRID_LINE, 0.5);
            let mut x = MARGIN;
            for (cell, &(_, w)) in row.iter().zip(&section.headers) {
                c.line(x + w, y, x + w, y + row_h, GRID_LINE, 0.5);
                c.text_clipped((x + 6.0, y + 12.5), (x + 1.0, y, w - 2.0, row_h), cell, 8.5, false, NAVY);
                x += w;
            }
            y += row_h;
        }
        y += 20.0;
    }

    pages.push(c);
    let total_pages = pages.len();
    for (idx, page) in pages.iter_mut().enumerate() {
        draw_page_banner(page, doc, subtitle, idx + 1, total_pages);
    }
    build_pdf(&pages)
}

impl crate::App {
    /// Opens a Save dialog and exports the task table + Gantt chart to PDF.
    pub(crate) fn export_gantt_pdf(&mut self) {
        if self.headless {
            return;
        }
        let stem = self.file_name().trim_end_matches(".mpp").trim_end_matches(".xml").to_string();
        let picked = rfd::FileDialog::new()
            .add_filter("Documento PDF (*.pdf)", &["pdf"])
            .set_file_name(format!("{stem}-gantt.pdf"))
            .save_file();
        if let Some(path) = picked {
            let r = gantt_pdf(&self.doc, &path);
            self.report(r);
        }
    }

    /// Opens a Save dialog and exports `report` from the `Relatório` tab to PDF.
    pub(crate) fn export_report_pdf(&mut self, report: Report) {
        if self.headless {
            return;
        }
        let stem = self.file_name().trim_end_matches(".mpp").trim_end_matches(".xml").to_string();
        let suffix = match report {
            Report::Dashboard => "paineis",
            Report::Resources => "recursos",
            Report::Costs => "custos",
            Report::InProgress => "em-andamento",
        };
        let picked = rfd::FileDialog::new()
            .add_filter("Documento PDF (*.pdf)", &["pdf"])
            .set_file_name(format!("{stem}-{suffix}.pdf"))
            .save_file();
        if let Some(path) = picked {
            let r = report_pdf(&self.doc, report, &path);
            self.report(r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gantt_pdf_paginates_and_encodes_portuguese_text() {
        let mut d = Doc::blank();
        d.add_task("Fase 1 — Fundação", 1.0).unwrap();
        let a = d.add_task("Escavação", 3.0).unwrap();
        let b = d.add_task("Concretagem", 2.0).unwrap();
        let m = d.add_milestone(3).unwrap();
        d.indent(a).unwrap();
        d.indent(b).unwrap();
        d.link(&[a, b, m]).unwrap();
        d.set_resources(a, "Ana, Bruno").unwrap();
        d.set_percent(a, 50).unwrap();
        d.save_baseline().unwrap();
        for k in 0..30 {
            d.add_task(&format!("Tarefa extra {k}"), (k % 5 + 1) as f64).unwrap();
        }

        let dir = std::env::temp_dir().join("etapix-pdf-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gantt-teste.pdf");
        gantt_pdf(&d, &path).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with("%PDF-1.4\n"));
        assert!(raw.ends_with("%%EOF\n"));
        assert!(raw.contains("/MediaBox [0 0 842 595]"));
        assert!(raw.contains("/Count 2"));
        // "Fundação" in WinAnsi octal: ç = \347, ã = \343.
        assert!(raw.contains("Funda\\347\\343o"));
        assert!(raw.contains("Ana, Bruno"));
    }

    #[test]
    fn report_pdf_exports_every_report_tab_view() {
        let mut d = Doc::blank();
        let a = d.add_task("Fundação", 4.0).unwrap();
        let b = d.add_task("Estrutura", 2.0).unwrap();
        d.set_resources(a, "Equipe Civil").unwrap();
        d.set_percent(a, 40).unwrap();
        d.apply(|p| {
            p.tasks[b].fixed_cost = 1250.50;
            Ok(())
        })
        .unwrap();

        let dir = std::env::temp_dir().join("etapix-pdf-test");
        std::fs::create_dir_all(&dir).unwrap();
        for (report, file, expected) in [
            (Report::Dashboard, "relatorio-paineis.pdf", "Vis\\343o Geral do Projeto"),
            (Report::Resources, "relatorio-recursos.pdf", "Equipe Civil"),
            (Report::Costs, "relatorio-custos.pdf", "R$ 1250,50"),
            (Report::InProgress, "relatorio-andamento.pdf", "40%"),
        ] {
            let path = dir.join(file);
            report_pdf(&d, report, &path).unwrap();
            let raw = std::fs::read_to_string(&path).unwrap();
            assert!(raw.starts_with("%PDF-1.4\n"));
            assert!(raw.contains(expected), "missing {expected} in {file}");
        }

        let all = dir.join("relatorios-todos.pdf");
        all_reports_pdf(&d, &all).unwrap();
        assert!(all.exists());
    }
}
