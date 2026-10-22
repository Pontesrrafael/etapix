//! Help: the Central de Ajuda (catalog), the Como usar panel, the guided tour and the
//! first-visit suggestion (docs/GUIA_TUTORIAIS_ETAPIX.md).
//!
//! The tour finds controls through `App::target`: every control a tour can show registers its
//! rectangle each frame under a stable id ("tarefa.inserir"); the tour reads the previous frame's
//! rectangles. The tour only navigates: it never changes the project.

pub mod content;

use std::collections::{BTreeSet, HashMap};

use eframe::egui::{self, Align2, Color32, FontId, Id, Order, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use crate::{fonts, icons, App, Tab};
use content::{Missing, Tutorial, CATALOG, START};

const EMERALD: Color32 = Color32::from_rgb(0x14, 0xC3, 0x8E);
const NAVY: Color32 = Color32::from_rgb(0x1E, 0x2F, 0x3E);
const TEAL: Color32 = Color32::from_rgb(0x0B, 0x6B, 0x66);
const MINT_BG: Color32 = Color32::from_rgb(0xDD, 0xF6, 0xEE);
const MUTED: Color32 = Color32::from_rgb(100, 110, 120);

pub struct Tour {
    pub tutorial: &'static str,
    pub step: usize,
    /// Steps found missing (with `Missing::Skip`), left out of the count.
    skipped: BTreeSet<usize>,
    /// Frames spent waiting for the step's tab to be drawn and its control to appear.
    waited: u32,
    forward: bool,
}

#[derive(Default)]
pub struct Help {
    /// The Como usar panel: tutorial and the procedure to open.
    pub panel: Option<&'static str>,
    open_procedure: Option<usize>,
    pub catalog: bool,
    pub tour: Option<Tour>,
    /// Control rectangles registered this frame, and those of the previous frame.
    targets: HashMap<&'static str, Rect>,
    last: HashMap<&'static str, Rect>,
    /// Suggestions already answered with "Ver passo a passo" in this session.
    shown: BTreeSet<&'static str>,
}

/// A tutorial's saved state: `concluido:<v>`, `dispensado:<v>`, `em_andamento:<etapa>`.
fn state(id: &str) -> Option<String> {
    fonts::setting(&format!("tour.{id}"))
}

fn set_state(id: &str, value: String) {
    fonts::save_settings(&[(&format!("tour.{id}"), value)]);
}

fn suggestions_on() -> bool {
    fonts::setting("ajuda_sugerir").as_deref() != Some("0")
}

/// Whether the first-visit suggestion should appear for this tutorial: never answered, or
/// answered for an older version.
fn worth_suggesting(t: &Tutorial) -> bool {
    match state(t.id) {
        None => true,
        Some(s) => {
            let version = s.split(':').nth(1).and_then(|v| v.parse::<u32>().ok());
            (s.starts_with("concluido") || s.starts_with("dispensado")) && version.is_some_and(|v| v < t.version)
        }
    }
}

impl App {
    /// Registers a control the tour can point at (only the part on screen counts).
    pub(crate) fn target(&mut self, ui: &egui::Ui, id: &'static str, rect: Rect) {
        let visible = rect.intersect(ui.clip_rect());
        if visible.width() > 4.0 && visible.height() > 4.0 && visible.width() >= rect.width() * 0.6 {
            self.help.targets.insert(id, visible);
        }
    }

    pub(crate) fn help_frame_start(&mut self) {
        self.help.last = std::mem::take(&mut self.help.targets);
    }

    pub(crate) fn open_help(&mut self, id: &'static str, procedure: usize) {
        self.help.panel = Some(id);
        self.help.open_procedure = Some(procedure);
    }

    pub(crate) fn start_tour(&mut self, id: &'static str) {
        let Some(t) = content::tutorial(id) else { return };
        if t.tour.is_empty() {
            // Nothing to show on screen: the step-by-step text is the help for this one.
            self.open_help(id, 0);
            return;
        }
        self.editing = None;
        self.help.catalog = false;
        self.help.tour = Some(Tour { tutorial: id, step: 0, skipped: BTreeSet::new(), waited: 0, forward: true });
        set_state(id, "em_andamento:0".into());
    }

    fn end_tour(&mut self, finished: bool) {
        if let Some(tour) = self.help.tour.take() {
            let t = content::tutorial(tour.tutorial).unwrap();
            set_state(t.id, format!("{}:{}", if finished { "concluido" } else { "dispensado" }, t.version));
        }
    }

    /// Moves to the next (or previous) step that is not known to be missing.
    fn tour_move(&mut self, forward: bool) {
        let Some(tour) = &mut self.help.tour else { return };
        let total = content::tutorial(tour.tutorial).unwrap().tour.len();
        let mut s = tour.step as i64;
        loop {
            s += if forward { 1 } else { -1 };
            if s < 0 {
                return;
            }
            if s as usize >= total {
                self.end_tour(true);
                return;
            }
            if !tour.skipped.contains(&(s as usize)) {
                break;
            }
        }
        tour.step = s as usize;
        tour.waited = 0;
        tour.forward = forward;
        set_state(tour.tutorial, format!("em_andamento:{}", tour.step));
    }

    pub(crate) fn tour_next(&mut self) {
        self.tour_move(true);
    }

    pub(crate) fn tour_prev(&mut self) {
        self.tour_move(false);
    }

    pub(crate) fn tour_active(&self) -> bool {
        self.help.tour.is_some()
    }

    // ----------------------------------------------------------------------------------------
    // First-visit suggestion

    /// The tutorial to suggest now, if any: the grid's first, then each tab's own.
    fn suggestion(&self) -> Option<&'static Tutorial> {
        if !suggestions_on() || self.help.tour.is_some() || self.tab == Tab::File {
            return None;
        }
        let grade = content::tutorial("grade").unwrap();
        let candidates = [grade, content::for_tab(self.tab)];
        candidates.into_iter().find(|t| !t.tour.is_empty() && !self.help.shown.contains(t.id) && worth_suggesting(t))
    }

    /// A strip under the ribbon: "Primeira vez aqui? Fazer o tour · Ver passo a passo · Dispensar".
    pub(crate) fn help_banner(&mut self, ctx: &egui::Context) {
        let Some(t) = self.suggestion() else { return };
        let mut choice = None;
        egui::TopBottomPanel::top("help-banner")
            .exact_height(28.0)
            .frame(egui::Frame::none().fill(MINT_BG).inner_margin(egui::Margin::symmetric(10.0, 4.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Primeira vez aqui? Conheça: {}.", t.title)).color(NAVY).size(12.0));
                    if ui.link("Fazer o tour").clicked() {
                        choice = Some(0);
                    }
                    ui.label("·");
                    if ui.link("Ver passo a passo").clicked() {
                        choice = Some(1);
                    }
                    ui.label("·");
                    if ui.link("Dispensar").clicked() {
                        choice = Some(2);
                    }
                });
            });
        match choice {
            Some(0) => self.start_tour(t.id),
            Some(1) => {
                self.help.shown.insert(t.id);
                self.open_help(t.id, 0);
            }
            Some(_) => set_state(t.id, format!("dispensado:{}", t.version)),
            None => {}
        }
    }

    // ----------------------------------------------------------------------------------------
    // Como usar panel

    /// The panel at the right (a floating window when the window is narrow). Call before the
    /// central panel.
    pub(crate) fn help_panel(&mut self, ctx: &egui::Context) {
        let Some(id) = self.help.panel else { return };
        let Some(t) = content::tutorial(id) else {
            self.help.panel = None;
            return;
        };
        let narrow = ctx.screen_rect().width() < 900.0;
        let mut close = false;
        let mut body = |ui: &mut egui::Ui, app: &mut App| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                icons::draw(ui.painter(), r, t.icon);
                ui.label(RichText::new(format!("Como usar: {}", t.title)).strong().size(15.0).color(NAVY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Fechar").on_hover_text("Fechar (Esc)").clicked() {
                        close = true;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                ui.label(t.summary);
                if !t.tour.is_empty() {
                    let done = state(t.id).is_some_and(|s| s.starts_with("concluido"));
                    if ui.button(if done { "↻  Refazer o tour" } else { "▶  Fazer o tour" }).clicked() {
                        app.start_tour(t.id);
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("O que você pode fazer aqui").strong().color(TEAL));
                for c in t.capabilities {
                    ui.label(format!("•  {c}"));
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Passo a passo").strong().color(TEAL));
                let open = app.help.open_procedure.take();
                for (k, p) in t.procedures.iter().enumerate() {
                    let mut header = egui::CollapsingHeader::new(RichText::new(p.title).strong()).id_salt((t.id, k)).default_open(k == 0);
                    if let Some(o) = open {
                        header = header.open(Some(o == k));
                    }
                    header.show(ui, |ui| {
                        for (n, s) in p.steps.iter().enumerate() {
                            ui.label(format!("{}. {s}", n + 1));
                        }
                        ui.label(RichText::new(format!("Como conferir: {}", p.check)).color(TEAL));
                        ui.label(RichText::new(format!("Se não funcionar: {}", p.fails)).color(MUTED));
                    });
                }
                if !t.tips.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Dicas").strong().color(TEAL));
                    for tip in t.tips {
                        ui.label(format!("•  {tip}"));
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Limitações").strong().color(TEAL));
                ui.label(RichText::new(t.limits).color(MUTED));
                ui.add_space(10.0);
                if ui.link("Central de Ajuda").clicked() {
                    app.help.catalog = true;
                }
                let mut on = suggestions_on();
                if ui.checkbox(&mut on, "Sugerir ajuda na primeira vez que abro uma aba").changed() {
                    fonts::save_settings(&[("ajuda_sugerir", if on { "1" } else { "0" }.into())]);
                }
            });
        };
        if narrow {
            let mut open = true;
            egui::Window::new("Como usar").id(Id::new("help-window")).open(&mut open).default_width(340.0).show(ctx, |ui| body(ui, self));
            close |= !open;
        } else {
            let max = ctx.screen_rect().width() * 0.4;
            egui::SidePanel::right("help-panel")
                .default_width(360.0)
                .width_range(260.0..=max)
                .frame(egui::Frame::none().fill(Color32::WHITE).inner_margin(12.0).stroke(Stroke::new(1.0_f32, Color32::from_gray(210))))
                .show(ctx, |ui| body(ui, self));
        }
        if close {
            self.help.panel = None;
        }
    }

    // ----------------------------------------------------------------------------------------
    // Central de Ajuda

    pub(crate) fn help_catalog(&mut self, ctx: &egui::Context) {
        if !self.help.catalog {
            return;
        }
        let mut open = true;
        let mut go: Option<(&'static str, usize, bool)> = None;
        egui::Window::new("Central de Ajuda").open(&mut open).collapsible(false).default_size([780.0, 560.0]).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                ui.label(RichText::new("Por onde começar").strong().size(15.0).color(NAVY));
                for (label, id, proc_) in START {
                    if ui.link(*label).clicked() {
                        go = Some((id, *proc_, false));
                    }
                }
                for (group, cards) in CATALOG {
                    ui.add_space(10.0);
                    ui.label(RichText::new(*group).strong().size(14.0).color(TEAL));
                    ui.horizontal_wrapped(|ui| {
                        for c in cards.iter().filter(|c| cfg!(windows) || !c.windows_only) {
                            let t = content::tutorial(c.tutorial).unwrap();
                            let benefit = if c.tutorial == "arquivo" && !cfg!(windows) {
                                "No Linux, .mpp não abre: use Project XML (.xml), que o Project lê e grava."
                            } else {
                                c.benefit
                            };
                            egui::Frame::none().stroke(Stroke::new(1.0_f32, Color32::from_gray(215))).rounding(6.0).inner_margin(10.0).show(ui, |ui| ui.vertical(|ui| {
                                ui.set_width(210.0);
                                ui.set_min_height(96.0);
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                                    icons::draw(ui.painter(), r, c.icon);
                                    ui.label(RichText::new(c.title).strong().color(NAVY));
                                });
                                ui.add(egui::Label::new(RichText::new(benefit).size(12.0).color(MUTED)).wrap());
                                ui.horizontal(|ui| {
                                    if t.tab.is_some() && ui.button("Ir para").clicked() {
                                        go = Some((c.tutorial, c.procedure, true));
                                    }
                                    if ui.button("Como usar").clicked() {
                                        go = Some((c.tutorial, c.procedure, false));
                                    }
                                });
                            }));
                        }
                    });
                }
            });
        });
        self.help.catalog = open;
        if let Some((id, procedure, navigate)) = go {
            if navigate {
                if let Some(tab) = content::tutorial(id).and_then(|t| t.tab) {
                    self.tab = tab;
                }
            }
            self.open_help(id, procedure);
            self.help.catalog = false;
        }
    }

    // ----------------------------------------------------------------------------------------
    // Guided tour

    /// The veil, the highlight and the balloon. Call last in the frame.
    pub(crate) fn help_tour(&mut self, ctx: &egui::Context) {
        if self.help.tour.is_none() {
            return;
        }
        // Escape ends the tour at any moment, even while a step is still being prepared.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.end_tour(false);
            return;
        }
        let Some(tour) = &mut self.help.tour else { return };
        let t = content::tutorial(tour.tutorial).unwrap();
        let Some(step) = t.tour.get(tour.step) else {
            self.end_tour(true);
            return;
        };
        // Prepare: open the step's tab and give it a frame or two to draw its controls.
        if let Some(tab) = step.tab {
            if self.tab != tab {
                self.tab = tab;
                tour.waited = 0;
                ctx.request_repaint();
                return;
            }
        }
        let target = step.target.and_then(|id| self.help.last.get(id).copied());
        if step.target.is_some() && target.is_none() {
            if tour.waited < 2 {
                tour.waited += 1;
                ctx.request_repaint();
                return;
            }
            if step.missing == Missing::Skip {
                tour.skipped.insert(tour.step);
                let forward = tour.forward;
                // Skipping backwards past the first step goes forward instead.
                if !forward && (0..tour.step).all(|s| tour.skipped.contains(&s)) {
                    self.tour_move(true);
                } else {
                    self.tour_move(forward);
                }
                ctx.request_repaint();
                return;
            }
        }
        let total = t.tour.len() - tour.skipped.len();
        let number = (0..=tour.step).filter(|s| !tour.skipped.contains(s)).count();
        let first = (0..tour.step).all(|s| tour.skipped.contains(&s));
        let last = number >= total;
        let screen = ctx.screen_rect();

        // Veil with a hole over the control; it takes every click outside the balloon.
        egui::Area::new(Id::new("tour-veil")).order(Order::Foreground).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            let veil = NAVY.gamma_multiply(0.45);
            let p = ui.painter();
            match target {
                Some(r) => {
                    let hole = r.expand(4.0);
                    for part in [
                        Rect::from_min_max(screen.min, Pos2::new(screen.right(), hole.top())),
                        Rect::from_min_max(Pos2::new(screen.left(), hole.bottom()), screen.max),
                        Rect::from_min_max(Pos2::new(screen.left(), hole.top()), Pos2::new(hole.left(), hole.bottom())),
                        Rect::from_min_max(Pos2::new(hole.right(), hole.top()), Pos2::new(screen.right(), hole.bottom())),
                    ] {
                        p.rect_filled(part, 0.0, veil);
                    }
                    p.rect_stroke(hole, 4.0, Stroke::new(2.0_f32, EMERALD));
                }
                None => {
                    p.rect_filled(screen, 0.0, veil);
                }
            }
            ui.allocate_rect(screen, Sense::click_and_drag());
        });

        // The balloon: under the control, above it when there is no room, or centred.
        let width = 320.0_f32.min(screen.width() - 32.0);
        let height_guess = 150.0;
        let pos = match target {
            Some(r) if r.bottom() + 12.0 + height_guess < screen.bottom() => Pos2::new(r.center().x - width / 2.0, r.bottom() + 12.0),
            Some(r) => Pos2::new(r.center().x - width / 2.0, (r.top() - 12.0 - height_guess).max(screen.top() + 8.0)),
            None => Pos2::new(screen.center().x - width / 2.0, screen.center().y - height_guess / 2.0),
        };
        let pos = Pos2::new(pos.x.clamp(screen.left() + 16.0, screen.right() - width - 16.0), pos.y);
        let mut action = None;
        egui::Area::new(Id::new("tour-balloon")).order(Order::Tooltip).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(Color32::WHITE).stroke(Stroke::new(1.5_f32, TEAL)).inner_margin(12.0).show(ui, |ui| {
                ui.set_width(width - 24.0);
                ui.label(RichText::new(format!("Passo {number} de {total}")).size(11.0).color(MUTED));
                ui.label(RichText::new(step.title).strong().size(15.0).color(NAVY));
                ui.label(step.text);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!first, egui::Button::new("Anterior")).clicked() {
                        action = Some(-1);
                    }
                    if ui.button(if last { "Concluir" } else { "Próximo" }).clicked() {
                        action = Some(1);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.link("Pular tour").clicked() {
                            action = Some(0);
                        }
                    });
                });
            });
        });
        // The arrows move between steps (Escape is handled above).
        let (right, left) = ctx.input(|i| (i.key_pressed(egui::Key::ArrowRight), i.key_pressed(egui::Key::ArrowLeft)));
        let action = action.or(if right {
            Some(1)
        } else if left && !first {
            Some(-1)
        } else {
            None
        });
        match action {
            Some(1) => self.tour_next(),
            Some(-1) => self.tour_prev(),
            Some(_) => self.end_tour(false),
            None => {}
        }
    }
}

/// Paints the small "?" that opens the grid's help, in the grid's top-left corner.
pub(crate) fn corner_button(ui: &mut egui::Ui, rect: Rect) -> egui::Response {
    let resp = ui.interact(rect, Id::new("grid-help"), Sense::click()).on_hover_text("Como usar a grade");
    let c = rect.center();
    let p = ui.painter();
    p.circle_stroke(c, 7.0, Stroke::new(1.2_f32, if resp.hovered() { EMERALD } else { TEAL }));
    p.text(c, Align2::CENTER_CENTER, "?", FontId::proportional(10.0), TEAL);
    resp
}

/// Every tutorial id, for tests.
#[cfg(test)]
pub fn ids() -> Vec<&'static str> {
    content::TUTORIALS.iter().map(|t| t.id).collect()
}
