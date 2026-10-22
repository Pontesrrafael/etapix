//! Small vector icons for the ribbon, drawn with the painter (no image assets).

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

pub const BLUE: Color32 = Color32::from_rgb(0x0B, 0x6B, 0x66);
pub const LIGHT_BLUE: Color32 = Color32::from_rgb(0x4E, 0xE7, 0xC7);
pub const GREEN: Color32 = Color32::from_rgb(0x14, 0xC3, 0x8E);
pub const DARK: Color32 = Color32::from_rgb(0x1E, 0x2F, 0x3E);
pub const RED: Color32 = Color32::from_rgb(196, 62, 54);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Gantt,
    Task,
    Summary,
    Milestone,
    Link,
    Unlink,
    Indent,
    Outdent,
    Pin,
    Auto,
    Info,
    Level,
    Clear,
    Baseline,
    ZoomIn,
    ZoomOut,
    Whole,
    Calendar,
    Sheet,
    Scroll,
    Assign,
    Percent(u8),
    Paste,
    Cut,
    Copy,
    Brush,
    Bold,
    Italic,
    Underline,
    OnTrack,
    Respect,
    Inactive,
    Inspect,
    Move,
    Mode,
    Deliverable,
    Notes,
    Details,
    TimelineAdd,
    Find,
    Eraser,
    Fill,
    Team,
    Pool,
    AddResource,
    NextOver,
    Options,
    Dashboard,
    Cost,
    InProgress,
    Intro,
    Custom,
    Recent,
    Compare,
    Export,
    Subproject,
    AddIn,
    Fields,
    Links,
    Wbs,
    WorkTime,
    Calc,
    MoveProject,
    StatusDate,
    Update,
    Spelling,
    Usage,
    Network,
    Month,
    Sort,
    Outline,
    Tables,
    Highlight,
    Filter,
    Group,
    NewWindow,
    Switch,
    Arrange,
    Hide,
    Macro,
    Text,
    Grid,
    Layout,
    Column,
    Format,
    Drawing,
    Help,
    Save,
    Undo,
    Redo,
}

fn line(p: &Painter, a: Pos2, b: Pos2, w: f32, c: Color32) {
    p.line_segment([a, b], Stroke::new(w, c));
}

fn bar(p: &Painter, r: Rect, c: Color32) {
    p.rect_filled(r, 1.0, c);
}

/// Draws `icon` inside `r` (square, about 32 px for large buttons, 16 px for small ones).
pub fn draw(p: &Painter, r: Rect, icon: Icon) {
    let s = r.width();
    let at = |x: f32, y: f32| Pos2::new(r.left() + x * s, r.top() + y * s);
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
    let w = (s / 16.0).max(1.0);
    match icon {
        Icon::Gantt => {
            p.rect_stroke(rect(0.05, 0.05, 0.95, 0.95), 1.0, Stroke::new(w, DARK));
            bar(p, rect(0.15, 0.2, 0.55, 0.35), BLUE);
            bar(p, rect(0.4, 0.45, 0.8, 0.6), BLUE);
            bar(p, rect(0.25, 0.7, 0.6, 0.85), LIGHT_BLUE);
        }
        Icon::Task => {
            bar(p, rect(0.1, 0.35, 0.9, 0.65), BLUE);
            line(p, at(0.75, 0.1), at(0.75, 0.3), w * 1.5, GREEN);
            line(p, at(0.65, 0.2), at(0.85, 0.2), w * 1.5, GREEN);
        }
        Icon::Summary => {
            bar(p, rect(0.1, 0.3, 0.9, 0.45), DARK);
            for x in [0.1, 0.9] {
                p.add(Shape::convex_polygon(vec![at(x - 0.08, 0.45), at(x + 0.08, 0.45), at(x, 0.6)], DARK, Stroke::NONE));
            }
            bar(p, rect(0.25, 0.7, 0.75, 0.85), BLUE);
        }
        Icon::Milestone => {
            p.add(Shape::convex_polygon(vec![at(0.5, 0.15), at(0.85, 0.5), at(0.5, 0.85), at(0.15, 0.5)], DARK, Stroke::NONE));
        }
        Icon::Link | Icon::Unlink => {
            p.rect_stroke(rect(0.1, 0.3, 0.5, 0.55), 4.0, Stroke::new(w * 1.5, DARK));
            p.rect_stroke(rect(0.5, 0.45, 0.9, 0.7), 4.0, Stroke::new(w * 1.5, BLUE));
            if icon == Icon::Unlink {
                line(p, at(0.15, 0.85), at(0.85, 0.15), w * 1.5, RED);
            }
        }
        Icon::Indent | Icon::Outdent => {
            for y in [0.2, 0.5, 0.8] {
                line(p, at(0.45, y), at(0.95, y), w, DARK);
            }
            let (a, b) = if icon == Icon::Indent { (0.05, 0.35) } else { (0.35, 0.05) };
            line(p, at(a, 0.5), at(b, 0.5), w * 1.5, GREEN);
            p.add(Shape::convex_polygon(vec![at(b, 0.38), at(b, 0.62), at(b + (b - a) * 0.4, 0.5)], GREEN, Stroke::NONE));
        }
        Icon::Pin => {
            p.circle_filled(at(0.5, 0.35), s * 0.18, RED);
            line(p, at(0.5, 0.5), at(0.5, 0.9), w * 1.5, DARK);
        }
        Icon::Auto => {
            bar(p, rect(0.1, 0.4, 0.75, 0.6), BLUE);
            p.add(Shape::convex_polygon(vec![at(0.7, 0.25), at(0.95, 0.5), at(0.7, 0.75)], GREEN, Stroke::NONE));
        }
        Icon::Info => {
            p.circle_stroke(at(0.5, 0.5), s * 0.4, Stroke::new(w * 1.5, BLUE));
            p.circle_filled(at(0.5, 0.3), s * 0.06, BLUE);
            line(p, at(0.5, 0.42), at(0.5, 0.75), w * 2.0, BLUE);
        }
        Icon::Level => {
            bar(p, rect(0.1, 0.55, 0.3, 0.9), BLUE);
            bar(p, rect(0.4, 0.55, 0.6, 0.9), BLUE);
            bar(p, rect(0.7, 0.55, 0.9, 0.9), BLUE);
            line(p, at(0.05, 0.45), at(0.95, 0.45), w * 1.5, RED);
        }
        Icon::Clear => {
            bar(p, rect(0.15, 0.45, 0.85, 0.65), LIGHT_BLUE);
            line(p, at(0.2, 0.2), at(0.8, 0.85), w * 1.5, RED);
            line(p, at(0.8, 0.2), at(0.2, 0.85), w * 1.5, RED);
        }
        Icon::Baseline => {
            bar(p, rect(0.1, 0.3, 0.85, 0.5), BLUE);
            bar(p, rect(0.1, 0.6, 0.85, 0.7), Color32::GRAY);
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            p.circle_stroke(at(0.42, 0.42), s * 0.28, Stroke::new(w * 1.5, DARK));
            line(p, at(0.62, 0.62), at(0.9, 0.9), w * 2.0, DARK);
            line(p, at(0.3, 0.42), at(0.54, 0.42), w * 1.5, DARK);
            if icon == Icon::ZoomIn {
                line(p, at(0.42, 0.3), at(0.42, 0.54), w * 1.5, DARK);
            }
        }
        Icon::Whole => {
            p.rect_stroke(rect(0.1, 0.2, 0.9, 0.8), 1.0, Stroke::new(w, DARK));
            bar(p, rect(0.15, 0.45, 0.85, 0.55), BLUE);
        }
        Icon::Calendar => {
            p.rect_stroke(rect(0.1, 0.2, 0.9, 0.9), 1.0, Stroke::new(w, DARK));
            bar(p, rect(0.1, 0.2, 0.9, 0.35), RED);
            for (x, y) in [(0.25, 0.5), (0.5, 0.5), (0.75, 0.5), (0.25, 0.72), (0.5, 0.72)] {
                bar(p, Rect::from_center_size(at(x, y), Vec2::splat(s * 0.1)), DARK);
            }
        }
        Icon::Sheet => {
            p.rect_stroke(rect(0.1, 0.15, 0.9, 0.85), 0.0, Stroke::new(w, DARK));
            for y in [0.38, 0.62] {
                line(p, at(0.1, y), at(0.9, y), w, DARK);
            }
            line(p, at(0.4, 0.15), at(0.4, 0.85), w, DARK);
        }
        Icon::Scroll => {
            bar(p, rect(0.45, 0.4, 0.95, 0.6), BLUE);
            line(p, at(0.05, 0.5), at(0.4, 0.5), w * 1.5, GREEN);
            p.add(Shape::convex_polygon(vec![at(0.3, 0.35), at(0.45, 0.5), at(0.3, 0.65)], GREEN, Stroke::NONE));
        }
        Icon::Assign => {
            p.circle_filled(at(0.4, 0.3), s * 0.15, DARK);
            p.add(Shape::convex_polygon(vec![at(0.15, 0.85), at(0.4, 0.5), at(0.65, 0.85)], DARK, Stroke::NONE));
            line(p, at(0.75, 0.35), at(0.75, 0.65), w * 1.5, GREEN);
            line(p, at(0.6, 0.5), at(0.9, 0.5), w * 1.5, GREEN);
        }
        Icon::Percent(pct) => {
            p.rect_stroke(rect(0.05, 0.3, 0.95, 0.7), 0.0, Stroke::new(w, DARK));
            if pct > 0 {
                bar(p, rect(0.05, 0.3, 0.05 + 0.9 * pct as f32 / 100.0, 0.7), BLUE);
            }
        }
        Icon::Save => {
            p.rect_stroke(rect(0.12, 0.12, 0.88, 0.88), 1.5, Stroke::new(w * 1.3, Color32::WHITE));
            bar(p, rect(0.3, 0.12, 0.7, 0.38), Color32::WHITE);
            bar(p, rect(0.3, 0.58, 0.7, 0.88), Color32::WHITE);
        }
        Icon::Undo | Icon::Redo => {
            let (a, b) = if icon == Icon::Undo { (0.2, 0.8) } else { (0.8, 0.2) };
            line(p, at(a, 0.45), at(b, 0.45), w * 1.6, Color32::WHITE);
            line(p, at(b, 0.45), at(b, 0.8), w * 1.6, Color32::WHITE);
            let tip = if icon == Icon::Undo { 0.38 } else { 0.62 };
            p.add(Shape::convex_polygon(vec![at(a, 0.45), at(tip, 0.25), at(tip, 0.65)], Color32::WHITE, Stroke::NONE));
        }
        Icon::Paste => {
            p.rect_stroke(rect(0.2, 0.18, 0.8, 0.92), 2.0, Stroke::new(w * 1.2, DARK));
            bar(p, rect(0.35, 0.1, 0.65, 0.25), DARK);
            bar(p, rect(0.42, 0.45, 0.92, 0.95), Color32::WHITE);
            p.rect_stroke(rect(0.42, 0.45, 0.92, 0.95), 1.0, Stroke::new(w, BLUE));
        }
        Icon::Cut => {
            p.circle_stroke(at(0.3, 0.75), s * 0.14, Stroke::new(w * 1.4, DARK));
            p.circle_stroke(at(0.7, 0.75), s * 0.14, Stroke::new(w * 1.4, DARK));
            line(p, at(0.38, 0.65), at(0.7, 0.1), w * 1.4, DARK);
            line(p, at(0.62, 0.65), at(0.3, 0.1), w * 1.4, DARK);
        }
        Icon::Copy => {
            p.rect_stroke(rect(0.12, 0.12, 0.62, 0.7), 1.0, Stroke::new(w * 1.2, DARK));
            bar(p, rect(0.38, 0.32, 0.88, 0.9), Color32::WHITE);
            p.rect_stroke(rect(0.38, 0.32, 0.88, 0.9), 1.0, Stroke::new(w * 1.2, BLUE));
        }
        Icon::Brush => {
            bar(p, rect(0.15, 0.12, 0.85, 0.42), GREEN);
            line(p, at(0.5, 0.42), at(0.5, 0.92), w * 3.0, DARK);
        }
        Icon::Bold | Icon::Italic | Icon::Underline => {
            let label = match icon { Icon::Bold => "N", Icon::Italic => "I", _ => "S" };
            let font = eframe::egui::FontId::new(s * 0.8, if icon == Icon::Bold { eframe::egui::FontFamily::Name("semibold".into()) } else { eframe::egui::FontFamily::Proportional });
            p.text(at(0.5, 0.48), eframe::egui::Align2::CENTER_CENTER, label, font, DARK);
            if icon == Icon::Underline {
                line(p, at(0.25, 0.9), at(0.75, 0.9), w * 1.4, DARK);
            }
        }
        Icon::OnTrack => {
            bar(p, rect(0.1, 0.55, 0.9, 0.8), LIGHT_BLUE);
            line(p, at(0.2, 0.3), at(0.4, 0.5), w * 2.0, GREEN);
            line(p, at(0.4, 0.5), at(0.85, 0.1), w * 2.0, GREEN);
        }
        Icon::Respect => {
            bar(p, rect(0.05, 0.2, 0.45, 0.4), BLUE);
            bar(p, rect(0.5, 0.62, 0.95, 0.82), BLUE);
            line(p, at(0.45, 0.3), at(0.55, 0.3), w, DARK);
            line(p, at(0.55, 0.3), at(0.55, 0.6), w, DARK);
        }
        Icon::Inactive => {
            p.rect_stroke(rect(0.1, 0.35, 0.9, 0.65), 0.0, Stroke::new(w, Color32::GRAY));
            line(p, at(0.15, 0.85), at(0.85, 0.15), w * 1.5, RED);
        }
        Icon::Inspect | Icon::Find => {
            p.circle_stroke(at(0.42, 0.42), s * 0.28, Stroke::new(w * 1.5, if icon == Icon::Inspect { BLUE } else { DARK }));
            line(p, at(0.62, 0.62), at(0.9, 0.9), w * 2.2, DARK);
        }
        Icon::Move => {
            bar(p, rect(0.1, 0.4, 0.6, 0.6), BLUE);
            line(p, at(0.6, 0.5), at(0.92, 0.5), w * 1.5, GREEN);
            p.add(Shape::convex_polygon(vec![at(0.8, 0.35), at(0.95, 0.5), at(0.8, 0.65)], GREEN, Stroke::NONE));
        }
        Icon::Mode => {
            draw(p, Rect::from_min_size(at(0.0, 0.0), Vec2::splat(s * 0.55)), Icon::Pin);
            draw(p, Rect::from_min_size(at(0.45, 0.45), Vec2::splat(s * 0.55)), Icon::Auto);
        }
        Icon::Deliverable => {
            p.add(Shape::convex_polygon(vec![at(0.5, 0.1), at(0.9, 0.3), at(0.9, 0.75), at(0.5, 0.95), at(0.1, 0.75), at(0.1, 0.3)], LIGHT_BLUE, Stroke::new(w, BLUE)));
        }
        Icon::Notes => {
            bar(p, rect(0.15, 0.1, 0.85, 0.9), Color32::from_rgb(0xFF, 0xF3, 0xB0));
            for y in [0.3, 0.5, 0.7] {
                line(p, at(0.25, y), at(0.75, y), w, DARK);
            }
        }
        Icon::Details => {
            p.rect_stroke(rect(0.1, 0.1, 0.9, 0.9), 0.0, Stroke::new(w, DARK));
            line(p, at(0.1, 0.5), at(0.9, 0.5), w * 1.5, BLUE);
        }
        Icon::TimelineAdd => {
            bar(p, rect(0.05, 0.45, 0.95, 0.62), LIGHT_BLUE);
            bar(p, rect(0.25, 0.25, 0.6, 0.42), BLUE);
            line(p, at(0.8, 0.05), at(0.8, 0.35), w * 1.5, GREEN);
            line(p, at(0.65, 0.2), at(0.95, 0.2), w * 1.5, GREEN);
        }
        Icon::Eraser => {
            p.add(Shape::convex_polygon(vec![at(0.1, 0.65), at(0.5, 0.25), at(0.85, 0.6), at(0.45, 0.95)], Color32::from_rgb(0xF2, 0x9C, 0xB3), Stroke::new(w, DARK)));
        }
        Icon::Fill => {
            bar(p, rect(0.2, 0.1, 0.8, 0.3), LIGHT_BLUE);
            bar(p, rect(0.2, 0.4, 0.8, 0.6), LIGHT_BLUE);
            line(p, at(0.5, 0.65), at(0.5, 0.95), w * 1.5, GREEN);
        }
        Icon::Team | Icon::Pool | Icon::AddResource => {
            for (x, c) in [(0.32, DARK), (0.62, BLUE)] {
                p.circle_filled(at(x, 0.3), s * 0.13, c);
                p.add(Shape::convex_polygon(vec![at(x - 0.2, 0.85), at(x, 0.48), at(x + 0.2, 0.85)], c, Stroke::NONE));
            }
            if icon == Icon::AddResource {
                line(p, at(0.85, 0.05), at(0.85, 0.35), w * 1.5, GREEN);
                line(p, at(0.7, 0.2), at(1.0, 0.2), w * 1.5, GREEN);
            }
            if icon == Icon::Pool {
                p.circle_stroke(at(0.5, 0.55), s * 0.45, Stroke::new(w, GREEN));
            }
        }
        Icon::NextOver => {
            p.circle_filled(at(0.35, 0.3), s * 0.13, DARK);
            p.add(Shape::convex_polygon(vec![at(0.15, 0.85), at(0.35, 0.48), at(0.55, 0.85)], DARK, Stroke::NONE));
            p.add(Shape::convex_polygon(vec![at(0.65, 0.3), at(0.95, 0.5), at(0.65, 0.7)], RED, Stroke::NONE));
        }
        Icon::Options | Icon::Custom => {
            p.circle_stroke(at(0.5, 0.5), s * 0.3, Stroke::new(w * 2.0, DARK));
            p.circle_filled(at(0.5, 0.5), s * 0.1, DARK);
        }
        Icon::Dashboard => {
            p.rect_stroke(rect(0.08, 0.12, 0.92, 0.88), 1.0, Stroke::new(w, DARK));
            bar(p, rect(0.18, 0.55, 0.3, 0.8), GREEN);
            bar(p, rect(0.38, 0.4, 0.5, 0.8), BLUE);
            p.circle_stroke(at(0.72, 0.5), s * 0.14, Stroke::new(w * 2.0, LIGHT_BLUE));
        }
        Icon::Cost => {
            p.circle_filled(at(0.5, 0.5), s * 0.4, Color32::from_rgb(0xF2, 0xC1, 0x4E));
            p.text(at(0.5, 0.5), eframe::egui::Align2::CENTER_CENTER, "$", eframe::egui::FontId::proportional(s * 0.55), DARK);
        }
        Icon::InProgress => {
            bar(p, rect(0.1, 0.25, 0.9, 0.45), LIGHT_BLUE);
            bar(p, rect(0.1, 0.25, 0.55, 0.45), BLUE);
            bar(p, rect(0.1, 0.6, 0.9, 0.8), LIGHT_BLUE);
            bar(p, rect(0.1, 0.6, 0.3, 0.8), BLUE);
        }
        Icon::Intro | Icon::Help => {
            p.circle_stroke(at(0.5, 0.5), s * 0.4, Stroke::new(w * 1.5, BLUE));
            p.text(at(0.5, 0.52), eframe::egui::Align2::CENTER_CENTER, "?", eframe::egui::FontId::proportional(s * 0.55), BLUE);
        }
        Icon::Recent => {
            p.circle_stroke(at(0.5, 0.5), s * 0.38, Stroke::new(w * 1.5, DARK));
            line(p, at(0.5, 0.5), at(0.5, 0.25), w * 1.5, DARK);
            line(p, at(0.5, 0.5), at(0.7, 0.6), w * 1.5, DARK);
        }
        Icon::Compare => {
            p.rect_stroke(rect(0.05, 0.15, 0.45, 0.85), 0.0, Stroke::new(w, DARK));
            p.rect_stroke(rect(0.55, 0.15, 0.95, 0.85), 0.0, Stroke::new(w, BLUE));
            line(p, at(0.45, 0.5), at(0.55, 0.5), w * 1.5, GREEN);
        }
        Icon::Export => {
            p.rect_stroke(rect(0.1, 0.25, 0.65, 0.9), 0.0, Stroke::new(w, DARK));
            line(p, at(0.4, 0.55), at(0.92, 0.15), w * 1.6, GREEN);
            p.add(Shape::convex_polygon(vec![at(0.7, 0.1), at(0.95, 0.1), at(0.95, 0.35)], GREEN, Stroke::NONE));
        }
        Icon::Subproject => {
            draw(p, Rect::from_min_size(at(0.0, 0.0), Vec2::splat(s * 0.65)), Icon::Gantt);
            bar(p, rect(0.5, 0.6, 0.95, 0.95), BLUE);
        }
        Icon::AddIn => {
            bar(p, rect(0.15, 0.15, 0.45, 0.45), BLUE);
            bar(p, rect(0.55, 0.15, 0.85, 0.45), GREEN);
            bar(p, rect(0.15, 0.55, 0.45, 0.85), LIGHT_BLUE);
            line(p, at(0.7, 0.55), at(0.7, 0.85), w * 1.5, DARK);
            line(p, at(0.55, 0.7), at(0.85, 0.7), w * 1.5, DARK);
        }
        Icon::Fields | Icon::Column | Icon::Tables => {
            p.rect_stroke(rect(0.1, 0.15, 0.9, 0.85), 0.0, Stroke::new(w, DARK));
            for x in [0.37, 0.63] {
                line(p, at(x, 0.15), at(x, 0.85), w, DARK);
            }
            line(p, at(0.1, 0.35), at(0.9, 0.35), w, DARK);
            if icon == Icon::Column {
                bar(p, rect(0.37, 0.15, 0.63, 0.85), LIGHT_BLUE);
            }
        }
        Icon::Links => {
            draw(p, r, Icon::Link);
        }
        Icon::Wbs | Icon::Outline => {
            for (x, y) in [(0.1, 0.2), (0.3, 0.5), (0.3, 0.8)] {
                bar(p, rect(x, y - 0.08, x + 0.12, y + 0.08), BLUE);
                line(p, at(x + 0.2, y), at(0.9, y), w * 1.2, DARK);
            }
        }
        Icon::WorkTime => {
            draw(p, r, Icon::Calendar);
        }
        Icon::Calc => {
            p.rect_stroke(rect(0.2, 0.08, 0.8, 0.92), 2.0, Stroke::new(w * 1.2, DARK));
            bar(p, rect(0.3, 0.16, 0.7, 0.34), LIGHT_BLUE);
            for (x, y) in [(0.36, 0.5), (0.5, 0.5), (0.64, 0.5), (0.36, 0.66), (0.5, 0.66), (0.64, 0.66), (0.36, 0.82), (0.5, 0.82), (0.64, 0.82)] {
                bar(p, Rect::from_center_size(at(x, y), Vec2::splat(s * 0.08)), DARK);
            }
        }
        Icon::MoveProject => {
            draw(p, Rect::from_min_size(at(0.0, 0.1), Vec2::splat(s * 0.7)), Icon::Gantt);
            line(p, at(0.55, 0.85), at(0.95, 0.85), w * 1.5, GREEN);
            p.add(Shape::convex_polygon(vec![at(0.85, 0.75), at(1.0, 0.85), at(0.85, 0.95)], GREEN, Stroke::NONE));
        }
        Icon::StatusDate => {
            draw(p, r, Icon::Calendar);
            line(p, at(0.75, 0.1), at(0.75, 0.95), w * 1.5, GREEN);
        }
        Icon::Update => {
            p.circle_stroke(at(0.5, 0.5), s * 0.32, Stroke::new(w * 2.0, GREEN));
            p.add(Shape::convex_polygon(vec![at(0.72, 0.1), at(0.92, 0.25), at(0.68, 0.35)], GREEN, Stroke::NONE));
        }
        Icon::Spelling => {
            p.text(at(0.4, 0.38), eframe::egui::Align2::CENTER_CENTER, "ABC", eframe::egui::FontId::proportional(s * 0.32), DARK);
            line(p, at(0.45, 0.75), at(0.6, 0.9), w * 2.0, GREEN);
            line(p, at(0.6, 0.9), at(0.9, 0.55), w * 2.0, GREEN);
        }
        Icon::Usage => {
            draw(p, Rect::from_min_size(at(0.0, 0.0), Vec2::splat(s * 0.6)), Icon::Sheet);
            bar(p, rect(0.5, 0.55, 0.65, 0.95), BLUE);
            bar(p, rect(0.72, 0.4, 0.87, 0.95), GREEN);
        }
        Icon::Network => {
            for (x, y) in [(0.15, 0.2), (0.15, 0.7), (0.62, 0.45)] {
                p.rect_stroke(rect(x, y, x + 0.25, y + 0.2), 1.0, Stroke::new(w * 1.2, BLUE));
            }
            line(p, at(0.4, 0.3), at(0.62, 0.55), w, DARK);
            line(p, at(0.4, 0.8), at(0.62, 0.55), w, DARK);
        }
        Icon::Month => {
            draw(p, r, Icon::Calendar);
        }
        Icon::Sort => {
            line(p, at(0.3, 0.1), at(0.3, 0.9), w * 1.5, DARK);
            p.add(Shape::convex_polygon(vec![at(0.15, 0.72), at(0.45, 0.72), at(0.3, 0.92)], DARK, Stroke::NONE));
            p.text(at(0.68, 0.3), eframe::egui::Align2::CENTER_CENTER, "A", eframe::egui::FontId::proportional(s * 0.38), BLUE);
            p.text(at(0.68, 0.72), eframe::egui::Align2::CENTER_CENTER, "Z", eframe::egui::FontId::proportional(s * 0.38), BLUE);
        }
        Icon::Highlight => {
            bar(p, rect(0.05, 0.6, 0.95, 0.85), Color32::from_rgb(0xFF, 0xE8, 0x4A));
            line(p, at(0.3, 0.55), at(0.75, 0.1), w * 3.0, DARK);
        }
        Icon::Filter => {
            p.add(Shape::convex_polygon(vec![at(0.1, 0.15), at(0.9, 0.15), at(0.58, 0.55), at(0.42, 0.55)], BLUE, Stroke::NONE));
            bar(p, rect(0.42, 0.55, 0.58, 0.9), BLUE);
        }
        Icon::Group => {
            for y in [0.15, 0.6] {
                bar(p, rect(0.1, y, 0.4, y + 0.25), BLUE);
                line(p, at(0.5, y + 0.12), at(0.9, y + 0.12), w * 1.4, DARK);
            }
        }
        Icon::NewWindow | Icon::Switch | Icon::Arrange | Icon::Hide => {
            p.rect_stroke(rect(0.1, 0.15, 0.7, 0.7), 0.0, Stroke::new(w, DARK));
            bar(p, rect(0.1, 0.15, 0.7, 0.25), BLUE);
            if icon != Icon::Hide {
                p.rect_stroke(rect(0.3, 0.35, 0.9, 0.9), 0.0, Stroke::new(w, DARK));
                bar(p, rect(0.3, 0.35, 0.9, 0.45), if icon == Icon::NewWindow { GREEN } else { LIGHT_BLUE });
            }
        }
        Icon::Macro => {
            p.rect_stroke(rect(0.1, 0.15, 0.9, 0.85), 1.0, Stroke::new(w, DARK));
            p.add(Shape::convex_polygon(vec![at(0.4, 0.32), at(0.68, 0.5), at(0.4, 0.68)], GREEN, Stroke::NONE));
        }
        Icon::Text => {
            p.text(at(0.5, 0.5), eframe::egui::Align2::CENTER_CENTER, "A", eframe::egui::FontId::proportional(s * 0.8), DARK);
        }
        Icon::Grid => {
            for k in 0..4 {
                let v = 0.1 + k as f32 * 0.266;
                line(p, at(v, 0.1), at(v, 0.9), w, DARK);
                line(p, at(0.1, v), at(0.9, v), w, DARK);
            }
        }
        Icon::Layout => {
            bar(p, rect(0.1, 0.2, 0.5, 0.35), BLUE);
            bar(p, rect(0.45, 0.6, 0.9, 0.75), BLUE);
            line(p, at(0.5, 0.28), at(0.5, 0.6), w, DARK);
        }
        Icon::Format => {
            draw(p, r, Icon::Gantt);
            bar(p, rect(0.6, 0.6, 0.95, 0.95), Color32::from_rgb(0xF2, 0x9C, 0x4E));
        }
        Icon::Drawing => {
            p.circle_stroke(at(0.35, 0.4), s * 0.22, Stroke::new(w * 1.4, BLUE));
            p.rect_stroke(rect(0.45, 0.5, 0.9, 0.9), 0.0, Stroke::new(w * 1.4, GREEN));
        }
    }
}
