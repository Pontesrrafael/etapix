//! Small vector icons for the ribbon, drawn with the painter (no image assets).

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

pub const BLUE: Color32 = Color32::from_rgb(68, 114, 196);
pub const LIGHT_BLUE: Color32 = Color32::from_rgb(142, 180, 227);
pub const GREEN: Color32 = Color32::from_rgb(33, 115, 70);
pub const DARK: Color32 = Color32::from_rgb(64, 64, 64);
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
    Report,
    Sheet,
    Scroll,
    Assign,
    Percent(u8),
    Delete,
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
        Icon::Report => {
            bar(p, rect(0.15, 0.55, 0.3, 0.9), BLUE);
            bar(p, rect(0.4, 0.35, 0.55, 0.9), GREEN);
            bar(p, rect(0.65, 0.15, 0.8, 0.9), LIGHT_BLUE);
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
        Icon::Delete => {
            line(p, at(0.2, 0.2), at(0.8, 0.8), w * 2.0, RED);
            line(p, at(0.8, 0.2), at(0.2, 0.8), w * 2.0, RED);
        }
    }
}
