//! pjapp: a desktop project planner for internal use, laid out like Microsoft Project (ribbon,
//! timeline, task grid next to the Gantt chart, status bar). It opens and saves `.mpp` (through
//! the installed Project) and Project XML.

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod doc;
mod fmt;
mod gantt;
mod icons;
mod mpp;

use std::collections::BTreeSet;

use doc::Doc;
use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, Vec2};
use icons::Icon;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1366.0, 800.0]).with_title("pjapp"),
        ..Default::default()
    };
    let path = std::env::args().nth(1);
    eframe::run_native(
        "pjapp",
        options,
        Box::new(move |cc| {
            setup_style(&cc.egui_ctx);
            Ok(Box::new(App::new(path)))
        }),
    )
}

/// Light theme and Segoe UI (Windows' UI font, as Project uses) when it is installed.
fn setup_style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::light());
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert("segoe".into(), egui::FontData::from_owned(bytes));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "segoe".into());
        ctx.set_fonts(fonts);
    }
}

const GREEN: Color32 = Color32::from_rgb(33, 115, 70);
const RIBBON_BG: Color32 = Color32::from_rgb(243, 243, 243);
const GRID_LINE: Color32 = Color32::from_rgb(212, 212, 212);
const HEADER_BG: Color32 = Color32::from_rgb(246, 246, 246);
const SELECTED: Color32 = Color32::from_rgb(214, 228, 243);
const TEXT: Color32 = Color32::from_rgb(38, 38, 38);
const ROW: f32 = 20.0;
const HEAD: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Col {
    Num,
    Ind,
    Mode,
    Name,
    Dur,
    Start,
    Finish,
    Pred,
    Res,
    Add,
}

const COLUMNS: [(Col, &str, f32); 10] = [
    (Col::Num, "", 34.0),
    (Col::Ind, "", 24.0),
    (Col::Mode, "Modo da\nTarefa", 60.0),
    (Col::Name, "Nome da Tarefa", 240.0),
    (Col::Dur, "Duração", 72.0),
    (Col::Start, "Início", 96.0),
    (Col::Finish, "Término", 96.0),
    (Col::Pred, "Predecessoras", 96.0),
    (Col::Res, "Nomes dos\nRecursos", 120.0),
    (Col::Add, "Adicionar Nova Coluna", 150.0),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    File,
    Task,
    Resource,
    Report,
    Project,
    View,
}

const TABS: [(Tab, &str); 6] = [
    (Tab::File, "Arquivo"),
    (Tab::Task, "Tarefa"),
    (Tab::Resource, "Recurso"),
    (Tab::Report, "Relatório"),
    (Tab::Project, "Projeto"),
    (Tab::View, "Exibir"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Gantt,
    Sheet,
}

/// What a ribbon button, shortcut or menu asks for; run after the frame's UI is laid out.
#[derive(Clone, Copy)]
enum Act {
    New,
    Open,
    Save,
    SaveAs,
    Percent(u32),
    Indent,
    Outdent,
    Link,
    Unlink,
    Manual(bool),
    InsertTask,
    InsertSummary,
    InsertMilestone,
    Delete,
    ScrollTo,
    Info,
    Level,
    ClearLevel,
    Baseline,
    ProjectInfo,
    Overview,
    ZoomIn,
    ZoomOut,
    Whole,
    Timeline,
    View(View),
}

struct Editing {
    row: usize,
    col: Col,
    text: String,
    fresh: bool,
}

struct App {
    doc: Doc,
    message: Option<String>,
    tab: Tab,
    view: View,
    selected: BTreeSet<usize>,
    cursor: (usize, Col),
    editing: Option<Editing>,
    scroll_y: f32,
    grid_x: f32,
    gantt_x: f32,
    split: f32,
    day: f32,
    timeline: bool,
    info: Option<usize>,
    project_info: Option<String>,
    overview: bool,
    scroll_to_start: bool,
    /// The chart's last laid-out width, used by "Projeto Inteiro".
    chart_w: f32,
}

impl App {
    fn new(path: Option<String>) -> Self {
        let (doc, message) = match path.map(|p| Doc::open(std::path::Path::new(&p))) {
            Some(Ok(d)) => (d, None),
            Some(Err(e)) => (Doc::blank(), Some(e)),
            None => (Doc::blank(), None),
        };
        App {
            doc,
            message,
            tab: Tab::Task,
            view: View::Gantt,
            selected: BTreeSet::new(),
            cursor: (0, Col::Name),
            editing: None,
            scroll_y: 0.0,
            grid_x: 0.0,
            gantt_x: 0.0,
            split: 640.0,
            day: 22.0,
            timeline: true,
            info: None,
            project_info: None,
            overview: false,
            scroll_to_start: true,
            chart_w: 700.0,
        }
    }

    fn report(&mut self, r: Result<(), String>) {
        if let Err(e) = r {
            self.message = Some(e);
        }
    }

    fn file_name(&self) -> String {
        let name = self.doc.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string());
        name.unwrap_or_else(|| "Projeto1".into())
    }

    fn title(&self) -> String {
        format!("{}{} - pjapp", self.file_name(), if self.doc.dirty { " *" } else { "" })
    }

    fn set_doc(&mut self, doc: Doc) {
        self.doc = doc;
        self.selected.clear();
        self.cursor = (0, Col::Name);
        self.editing = None;
        self.scroll_y = 0.0;
        self.scroll_to_start = true;
    }

    fn open(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Arquivos do Project", &["mpp", "xml"])
            .add_filter("Projeto (*.mpp)", &["mpp"])
            .add_filter("Formato XML (*.xml)", &["xml"])
            .pick_file();
        if let Some(path) = picked {
            match Doc::open(&path) {
                Ok(d) => self.set_doc(d),
                Err(e) => self.message = Some(e),
            }
        }
    }

    fn save(&mut self, ask: bool) {
        let path = match (&self.doc.path, ask) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new()
                .add_filter("Projeto (*.mpp)", &["mpp"])
                .add_filter("Formato XML (*.xml)", &["xml"])
                .set_file_name(format!("{}.mpp", self.file_name().trim_end_matches(".mpp").trim_end_matches(".xml")))
                .save_file(),
        };
        if let Some(p) = path {
            let r = self.doc.save(&p);
            self.report(r);
        }
    }

    fn n(&self) -> usize {
        self.doc.project.tasks.len()
    }

    /// Selected task rows, or the cursor's row when nothing else is selected.
    fn targets(&self) -> Vec<usize> {
        let n = self.n();
        let mut rows: Vec<usize> = self.selected.iter().copied().filter(|&r| r < n).collect();
        if rows.is_empty() && self.cursor.0 < n {
            rows.push(self.cursor.0);
        }
        rows
    }

    fn select_only(&mut self, row: usize) {
        self.selected.clear();
        self.selected.insert(row);
    }

    fn run(&mut self, act: Act) {
        let rows = self.targets();
        let r = match act {
            Act::New => {
                self.set_doc(Doc::blank());
                self.tab = Tab::Task;
                Ok(())
            }
            Act::Open => {
                self.open();
                self.tab = Tab::Task;
                Ok(())
            }
            Act::Save => {
                self.save(false);
                self.tab = Tab::Task;
                Ok(())
            }
            Act::SaveAs => {
                self.save(true);
                self.tab = Tab::Task;
                Ok(())
            }
            Act::Percent(p) => rows.iter().try_for_each(|&i| if self.doc.project.tasks[i].summary { Ok(()) } else { self.doc.set_percent(i, p) }),
            Act::Indent => rows.iter().try_for_each(|&i| self.doc.indent(i)),
            Act::Outdent => rows.iter().rev().try_for_each(|&i| self.doc.outdent(i)),
            Act::Link => self.doc.link(&rows),
            Act::Unlink => self.doc.unlink(&rows),
            Act::Manual(m) => rows.iter().try_for_each(|&i| self.doc.set_manual(i, m)),
            Act::InsertTask => self.doc.insert_task(self.cursor.0, "<Nova Tarefa>", 1.0).map(|i| self.select_only(i)),
            Act::InsertMilestone => self.doc.add_milestone(self.cursor.0).map(|i| self.select_only(i)),
            Act::InsertSummary => self.insert_summary(rows),
            Act::Delete => {
                let r = rows.iter().rev().try_for_each(|&i| self.doc.delete_task(i));
                self.selected.clear();
                r
            }
            Act::ScrollTo => {
                if let Some(&i) = rows.first() {
                    let x = gantt::Scale::new(&self.doc, self.day, 0.0).x(self.doc.dt(self.doc.schedule[i].start));
                    self.gantt_x = (x - 2.0 * self.day).max(0.0);
                }
                Ok(())
            }
            Act::Info => {
                self.info = rows.first().copied();
                Ok(())
            }
            Act::Level => self.doc.level(),
            Act::ClearLevel => self.doc.clear_leveling(),
            Act::Baseline => self.doc.save_baseline(),
            Act::ProjectInfo => {
                self.project_info = Some(self.doc.project.start.format("%d/%m/%Y").to_string());
                Ok(())
            }
            Act::Overview => {
                self.overview = true;
                Ok(())
            }
            Act::ZoomIn => {
                self.day = (self.day * 1.5).min(120.0);
                Ok(())
            }
            Act::ZoomOut => {
                self.day = (self.day / 1.5).max(1.5);
                Ok(())
            }
            Act::Whole => {
                self.whole_project();
                Ok(())
            }
            Act::Timeline => {
                self.timeline = !self.timeline;
                Ok(())
            }
            Act::View(v) => {
                self.view = v;
                Ok(())
            }
        };
        self.report(r);
    }

    /// Project's "Inserir Resumo": a summary above the selection with the selection under it, or
    /// a new summary with one subtask.
    fn insert_summary(&mut self, rows: Vec<usize>) -> Result<(), String> {
        let at = rows.first().copied().unwrap_or(self.cursor.0.min(self.n()));
        let summary = self.doc.insert_task(at, "<Nova Tarefa Resumo>", 1.0)?;
        if rows.is_empty() {
            let sub = self.doc.insert_task(summary + 1, "<Nova Tarefa>", 1.0)?;
            self.doc.indent(sub)?;
        } else {
            for r in rows {
                self.doc.indent(r + 1)?;
            }
        }
        self.select_only(summary);
        Ok(())
    }

    /// Zoom so the whole project fits in the chart.
    fn whole_project(&mut self) {
        let doc = &self.doc;
        let (Some(lo), Some(hi)) = (doc.schedule.iter().map(|s| s.start).min(), doc.schedule.iter().map(|s| s.finish).max()) else {
            return;
        };
        let days = ((doc.dt(hi) - doc.dt(lo)).num_minutes() as f32 / 1440.0).max(1.0) + 2.0;
        self.day = (self.chart_w / days).clamp(1.5, 120.0);
        let x = gantt::Scale::new(doc, self.day, 0.0).x(doc.dt(lo));
        self.gantt_x = (x - self.day).max(0.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Ribbon

fn big_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> bool {
    let font = FontId::proportional(12.0);
    let galley = ui.painter().layout(label.to_string(), font, TEXT, 80.0);
    let w = galley.size().x.max(36.0) + 12.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 72.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(rect, 0.0, Color32::from_rgb(218, 218, 218));
    }
    icons::draw(p, Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 21.0), Vec2::splat(30.0)), icon);
    let color = if enabled { TEXT } else { Color32::from_gray(160) };
    p.galley_with_override_text_color(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.top() + 40.0), galley, color);
    enabled && resp.clicked()
}

fn small_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> bool {
    let font = FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, TEXT);
    let w = galley.size().x + if label.is_empty() { 24.0 } else { 30.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(rect, 0.0, Color32::from_rgb(218, 218, 218));
    }
    icons::draw(p, Rect::from_min_size(Pos2::new(rect.left() + 4.0, rect.top() + 3.0), Vec2::splat(16.0)), icon);
    let color = if enabled { TEXT } else { Color32::from_gray(160) };
    p.galley_with_override_text_color(Pos2::new(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0), galley, color);
    enabled && resp.clicked()
}

/// A ribbon group: its buttons on top, its name underneath, a separator after it.
fn group(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui)) {
    let font = FontId::proportional(11.0);
    let title = ui.painter().layout_no_wrap(name.to_string(), font, Color32::from_gray(100));
    let buttons = ui.horizontal_top(|ui| {
        ui.set_height(72.0);
        ui.set_min_width(title.size().x + 8.0);
        ui.spacing_mut().item_spacing.x = 2.0;
        add(ui);
    });
    let r = buttons.response.rect;
    ui.painter().galley(Pos2::new(r.center().x - title.size().x / 2.0, r.top() + 76.0), title, Color32::from_gray(100));
    let (rect, _) = ui.allocate_exact_size(Vec2::new(9.0, 88.0), Sense::hover());
    ui.painter().line_segment([Pos2::new(rect.center().x, rect.top() + 4.0), Pos2::new(rect.center().x, rect.bottom() - 4.0)], Stroke::new(1.0_f32, Color32::from_gray(205)));
}

fn column(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        add(ui);
    });
}

impl App {
    fn tabs(&mut self, ui: &mut egui::Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, GREEN);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            for (tab, name) in TABS {
                let font = FontId::proportional(13.0);
                let galley = ui.painter().layout_no_wrap(name.to_string(), font, Color32::WHITE);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 22.0, 26.0), Sense::click());
                let active = self.tab == tab && tab != Tab::File;
                let p = ui.painter();
                if active {
                    p.rect_filled(rect, 0.0, RIBBON_BG);
                } else if resp.hovered() {
                    p.rect_filled(rect, 0.0, Color32::from_rgb(25, 90, 55));
                }
                let color = if active { GREEN } else { Color32::WHITE };
                p.galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, color);
                if resp.clicked() {
                    self.tab = tab;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(self.title()).color(Color32::WHITE).size(12.0));
            });
        });
    }

    fn ribbon(&mut self, ui: &mut egui::Ui) -> Vec<Act> {
        let mut acts = Vec::new();
        let has = !self.targets().is_empty();
        let many = self.targets().len() >= 2;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            match self.tab {
                Tab::File => {}
                Tab::Task => {
                    group(ui, "Exibir", |ui| {
                        if big_button(ui, Icon::Gantt, "Gráfico de\nGantt", true) {
                            acts.push(Act::View(View::Gantt));
                        }
                    });
                    group(ui, "Cronograma", |ui| {
                        column(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                for p in [0u8, 25, 50, 75, 100] {
                                    if small_button(ui, Icon::Percent(p), "", has) {
                                        acts.push(Act::Percent(p as u32));
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                if small_button(ui, Icon::Outdent, "", has) {
                                    acts.push(Act::Outdent);
                                }
                                if small_button(ui, Icon::Indent, "", has) {
                                    acts.push(Act::Indent);
                                }
                                if small_button(ui, Icon::Link, "", many) {
                                    acts.push(Act::Link);
                                }
                                if small_button(ui, Icon::Unlink, "", has) {
                                    acts.push(Act::Unlink);
                                }
                            });
                        });
                    });
                    group(ui, "Tarefas", |ui| {
                        if big_button(ui, Icon::Pin, "Agendar\nManualmente", has) {
                            acts.push(Act::Manual(true));
                        }
                        if big_button(ui, Icon::Auto, "Agendamento\nAutomático", has) {
                            acts.push(Act::Manual(false));
                        }
                    });
                    group(ui, "Inserir", |ui| {
                        if big_button(ui, Icon::Task, "Tarefa", true) {
                            acts.push(Act::InsertTask);
                        }
                        column(ui, |ui| {
                            if small_button(ui, Icon::Summary, "Resumo", true) {
                                acts.push(Act::InsertSummary);
                            }
                            if small_button(ui, Icon::Milestone, "Marco", true) {
                                acts.push(Act::InsertMilestone);
                            }
                            if small_button(ui, Icon::Delete, "Excluir", has) {
                                acts.push(Act::Delete);
                            }
                        });
                    });
                    group(ui, "Propriedades", |ui| {
                        if big_button(ui, Icon::Info, "Informações", has) {
                            acts.push(Act::Info);
                        }
                    });
                    group(ui, "Editar", |ui| {
                        if big_button(ui, Icon::Scroll, "Rolar até\na Tarefa", has) {
                            acts.push(Act::ScrollTo);
                        }
                    });
                }
                Tab::Resource => {
                    group(ui, "Atribuições", |ui| {
                        if big_button(ui, Icon::Assign, "Atribuir\nRecursos", has) {
                            self.editing = None;
                            self.cursor.1 = Col::Res;
                        }
                    });
                    group(ui, "Nível", |ui| {
                        if big_button(ui, Icon::Level, "Nivelar\nTudo", true) {
                            acts.push(Act::Level);
                        }
                        if big_button(ui, Icon::Clear, "Limpar\nNivelamento", true) {
                            acts.push(Act::ClearLevel);
                        }
                    });
                }
                Tab::Report => {
                    group(ui, "Exibir Relatórios", |ui| {
                        if big_button(ui, Icon::Report, "Visão Geral\ndo Projeto", true) {
                            acts.push(Act::Overview);
                        }
                    });
                }
                Tab::Project => {
                    group(ui, "Propriedades", |ui| {
                        if big_button(ui, Icon::Info, "Informações\ndo Projeto", true) {
                            acts.push(Act::ProjectInfo);
                        }
                    });
                    group(ui, "Cronograma", |ui| {
                        if big_button(ui, Icon::Baseline, "Definir Linha\nde Base", true) {
                            acts.push(Act::Baseline);
                        }
                    });
                }
                Tab::View => {
                    group(ui, "Modos de Exibição de Tarefa", |ui| {
                        if big_button(ui, Icon::Gantt, "Gráfico de\nGantt", true) {
                            acts.push(Act::View(View::Gantt));
                        }
                        if big_button(ui, Icon::Sheet, "Planilha\nde Tarefas", true) {
                            acts.push(Act::View(View::Sheet));
                        }
                    });
                    group(ui, "Zoom", |ui| {
                        column(ui, |ui| {
                            if small_button(ui, Icon::ZoomIn, "Ampliar", true) {
                                acts.push(Act::ZoomIn);
                            }
                            if small_button(ui, Icon::ZoomOut, "Reduzir", true) {
                                acts.push(Act::ZoomOut);
                            }
                        });
                        if big_button(ui, Icon::Whole, "Projeto\nInteiro", true) {
                            acts.push(Act::Whole);
                        }
                    });
                    group(ui, "Modo Divisão", |ui| {
                        let mut on = self.timeline;
                        if ui.checkbox(&mut on, "Linha do Tempo").changed() {
                            acts.push(Act::Timeline);
                        }
                    });
                }
            }
        });
        acts
    }

    /// Backstage (the "Arquivo" tab): a green column of commands over the whole window.
    fn backstage(&mut self, ctx: &egui::Context) -> Vec<Act> {
        let mut acts = Vec::new();
        egui::SidePanel::left("backstage").exact_width(170.0).resizable(false).frame(egui::Frame::none().fill(GREEN)).show(ctx, |ui| {
            ui.add_space(12.0);
            let back = ui.add(egui::Label::new(egui::RichText::new("  ←").size(22.0).color(Color32::WHITE)).sense(Sense::click()));
            if back.clicked() {
                self.tab = Tab::Task;
            }
            ui.add_space(16.0);
            for (act, name) in [(Act::New, "Novo"), (Act::Open, "Abrir"), (Act::Save, "Salvar"), (Act::SaveAs, "Salvar como")] {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(170.0, 36.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(25, 90, 55));
                }
                ui.painter().text(Pos2::new(rect.left() + 24.0, rect.center().y), Align2::LEFT_CENTER, name, FontId::proportional(15.0), Color32::WHITE);
                if resp.clicked() {
                    acts.push(act);
                }
            }
        });
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::WHITE).inner_margin(32.0)).show(ctx, |ui| {
            ui.label(egui::RichText::new("Novo").size(30.0).color(TEXT));
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(160.0, 190.0), Sense::click());
                let p = ui.painter();
                p.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, if resp.hovered() { GREEN } else { GRID_LINE }));
                p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(16.0, 16.0), Vec2::new(128.0, 128.0)), 0.0, Color32::from_gray(250));
                icons::draw(p, Rect::from_center_size(rect.min + Vec2::new(80.0, 80.0), Vec2::splat(60.0)), Icon::Gantt);
                p.text(Pos2::new(rect.center().x, rect.bottom() - 26.0), Align2::CENTER_CENTER, "Projeto em Branco", FontId::proportional(13.0), TEXT);
                if resp.clicked() {
                    acts.push(Act::New);
                }
                ui.add_space(16.0);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(160.0, 190.0), Sense::click());
                let p = ui.painter();
                p.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, if resp.hovered() { GREEN } else { GRID_LINE }));
                icons::draw(p, Rect::from_center_size(rect.min + Vec2::new(80.0, 80.0), Vec2::splat(60.0)), Icon::Sheet);
                p.text(Pos2::new(rect.center().x, rect.bottom() - 26.0), Align2::CENTER_CENTER, "Abrir (.mpp, .xml)", FontId::proportional(13.0), TEXT);
                if resp.clicked() {
                    acts.push(Act::Open);
                }
            });
            ui.add_space(24.0);
            ui.label(egui::RichText::new("Os arquivos .mpp são lidos e gravados pelo Microsoft Project instalado nesta máquina.").color(Color32::from_gray(110)));
        });
        acts
    }
}

// ---------------------------------------------------------------------------------------------
// Timeline strip

impl App {
    fn timeline_strip(&self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, Color32::WHITE);
        let doc = &self.doc;
        let start = doc.dt(doc.schedule.iter().map(|s| s.start).min().unwrap_or(doc.cals.project_start));
        let finish = doc.dt(doc.schedule.iter().map(|s| s.finish).max().unwrap_or(doc.cals.project_start));
        let start = start.min(doc.project.start);
        let band = Rect::from_min_max(Pos2::new(rect.left() + 110.0, rect.top() + 22.0), Pos2::new(rect.right() - 110.0, rect.bottom() - 10.0));
        p.rect_filled(band, 0.0, Color32::from_rgb(242, 242, 242));
        p.rect_stroke(band, 0.0, Stroke::new(1.0_f32, Color32::from_gray(200)));
        let small = FontId::proportional(11.0);
        let label = |x: f32, align: Align2, title: &str, date: String| {
            p.text(Pos2::new(x, band.top() + 8.0), align, title, small.clone(), Color32::from_gray(90));
            p.text(Pos2::new(x, band.top() + 24.0), align, date, FontId::proportional(12.0), TEXT);
        };
        label(rect.left() + 100.0, Align2::RIGHT_CENTER, "Início", fmt::date(start));
        label(rect.right() - 100.0, Align2::LEFT_CENTER, "Término", fmt::date(finish));
        // Month ticks over the project's span.
        let total = (finish - start).num_minutes().max(1) as f32;
        let x_of = |dt: chrono::NaiveDateTime| band.left() + (dt - start).num_minutes() as f32 / total * band.width();
        let mut m = chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&start), chrono::Datelike::month(&start), 1).unwrap();
        let step_week = (finish - start).num_days() < 70;
        let mut d = if step_week { start.date() } else { m };
        loop {
            let next = if step_week {
                d + chrono::Duration::days(7)
            } else {
                m = m.checked_add_months(chrono::Months::new(1)).unwrap();
                m
            };
            let x = x_of(d.and_hms_opt(0, 0, 0).unwrap());
            if x > band.right() {
                break;
            }
            if x >= band.left() {
                p.line_segment([Pos2::new(x, rect.top() + 6.0), Pos2::new(x, band.top())], Stroke::new(1.0_f32, Color32::from_gray(190)));
                let text = if step_week { d.format("%d/%m").to_string() } else { format!("{}/{}", fmt::MONTHS[chrono::Datelike::month0(&d) as usize], d.format("%y")) };
                p.text(Pos2::new(x + 3.0, rect.top() + 12.0), Align2::LEFT_CENTER, text, small.clone(), Color32::from_gray(90));
            }
            d = next;
        }
        let today = x_of(chrono::Local::now().naive_local());
        if (band.left()..=band.right()).contains(&today) {
            p.line_segment([Pos2::new(today, band.top()), Pos2::new(today, band.bottom())], Stroke::new(1.5_f32, GREEN));
            p.text(Pos2::new(today + 3.0, band.bottom() - 7.0), Align2::LEFT_CENTER, "Hoje", small.clone(), GREEN);
        }
        p.text(band.center() + Vec2::new(0.0, 4.0), Align2::CENTER_CENTER, "Adicione tarefas com datas à linha do tempo", FontId::proportional(12.0), Color32::from_gray(140));
        p.line_segment([Pos2::new(rect.left(), rect.bottom() - 1.0), Pos2::new(rect.right(), rect.bottom() - 1.0)], Stroke::new(1.0_f32, Color32::from_gray(190)));
    }
}

// ---------------------------------------------------------------------------------------------
// Grid and chart

impl App {
    fn cell_text(&self, row: usize, col: Col) -> String {
        if row >= self.n() {
            return String::new();
        }
        let t = &self.doc.project.tasks[row];
        let s = &self.doc.schedule[row];
        match col {
            Col::Num => t.id.to_string(),
            Col::Name => t.name.clone(),
            Col::Dur => fmt::days(self.doc.shown_days(row)),
            Col::Start => fmt::date(self.doc.dt(s.start)),
            Col::Finish => fmt::date(self.doc.dt(s.finish)),
            Col::Pred => self.doc.predecessors_text(row),
            Col::Res => self.doc.resources_text(row),
            Col::Ind | Col::Mode | Col::Add => String::new(),
        }
    }

    fn editable(&self, row: usize, col: Col) -> bool {
        let summary = row < self.n() && self.doc.project.tasks[row].summary;
        match col {
            Col::Name => true,
            Col::Dur | Col::Start | Col::Pred | Col::Res => !summary,
            _ => false,
        }
    }

    fn begin_edit(&mut self, row: usize, col: Col, text: Option<String>) {
        if !self.editable(row, col) {
            return;
        }
        let text = text.unwrap_or_else(|| {
            let t = self.cell_text(row, col);
            if col == Col::Start { t.rsplit(' ').next().unwrap_or("").to_string() } else { t }
        });
        self.editing = Some(Editing { row, col, text, fresh: true });
    }

    /// Applies an edit; returns the row it landed on (a new task typed into an empty row goes
    /// right after the last task).
    fn commit(&mut self, e: Editing) -> usize {
        let text = e.text.trim().to_string();
        let mut row = e.row;
        if row >= self.n() {
            if text.is_empty() {
                return row;
            }
            // Typing in an empty row creates the task (at the end of the list).
            let name = if e.col == Col::Name { text.clone() } else { String::new() };
            match self.doc.insert_task(self.n(), &name, 1.0) {
                Ok(i) => row = i,
                Err(err) => {
                    self.message = Some(err);
                    return e.row;
                }
            }
        }
        let r = match e.col {
            Col::Name => {
                self.doc.rename(row, &text);
                Ok(())
            }
            Col::Dur => match fmt::parse_days(&text) {
                Some(d) => self.doc.set_duration_days(row, d),
                None => Err(format!("Duração inválida: {text}")),
            },
            Col::Start => match fmt::parse_date(&text) {
                Some(d) => self.doc.set_start(row, d),
                None => Err(format!("Data inválida: {text} (use dd/mm/aa)")),
            },
            Col::Pred => self.doc.set_predecessors(row, &text),
            Col::Res => self.doc.set_resources(row, &text),
            _ => Ok(()),
        };
        self.report(r);
        row
    }

    fn col_x(&self, col: Col) -> (f32, f32) {
        let mut x = 0.0;
        for (c, _, w) in COLUMNS {
            if c == col {
                return (x, w);
            }
            x += w;
        }
        (x, 0.0)
    }

    fn move_cursor(&mut self, drow: i64, dcol: i64) {
        let max_row = self.n() + 50;
        let row = (self.cursor.0 as i64 + drow).clamp(0, max_row as i64) as usize;
        let idx = COLUMNS.iter().position(|c| c.0 == self.cursor.1).unwrap_or(3) as i64;
        let idx = (idx + dcol).clamp(1, COLUMNS.len() as i64 - 2) as usize;
        self.cursor = (row, COLUMNS[idx].0);
        self.select_only(row);
    }

    fn keyboard(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>, was_editing: bool) {
        if was_editing || self.editing.is_some() || ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let (events, mods) = ctx.input(|i| (i.events.clone(), i.modifiers));
        for ev in events {
            match ev {
                egui::Event::Text(t) if !mods.ctrl => {
                    self.begin_edit(self.cursor.0, self.cursor.1, Some(t));
                    return;
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => match key {
                    Key::ArrowRight if modifiers.alt && modifiers.shift => acts.push(Act::Indent),
                    Key::ArrowLeft if modifiers.alt && modifiers.shift => acts.push(Act::Outdent),
                    Key::ArrowDown => self.move_cursor(1, 0),
                    Key::ArrowUp => self.move_cursor(-1, 0),
                    Key::ArrowRight | Key::Tab => self.move_cursor(0, 1),
                    Key::ArrowLeft => self.move_cursor(0, -1),
                    Key::Enter => self.move_cursor(1, 0),
                    Key::F2 if modifiers.shift => acts.push(Act::Info),
                    Key::F2 => self.begin_edit(self.cursor.0, self.cursor.1, None),
                    Key::Delete => acts.push(Act::Delete),
                    Key::Insert => acts.push(Act::InsertTask),
                    Key::S if modifiers.ctrl => acts.push(Act::Save),
                    Key::O if modifiers.ctrl => acts.push(Act::Open),
                    Key::N if modifiers.ctrl => acts.push(Act::New),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn row_click(&mut self, row: usize, col: Col, mods: egui::Modifiers) {
        if mods.ctrl {
            if !self.selected.remove(&row) {
                self.selected.insert(row);
            }
        } else if mods.shift {
            let (a, b) = (self.cursor.0.min(row), self.cursor.0.max(row));
            self.selected = (a..=b).collect();
        } else {
            self.select_only(row);
        }
        self.cursor = (row, if col == Col::Num || col == Col::Add { Col::Name } else { col });
    }

    fn grid(&mut self, ui: &mut egui::Ui, area: Rect) {
        let n = self.n();
        let head = Rect::from_min_max(area.min, Pos2::new(area.right(), area.top() + HEAD));
        let body = Rect::from_min_max(Pos2::new(area.left(), head.bottom()), area.max);
        let total_w: f32 = COLUMNS.iter().map(|c| c.2).sum();
        self.grid_x = self.grid_x.clamp(0.0, (total_w - area.width()).max(0.0));
        let left = area.left() - self.grid_x;
        let p = ui.painter_at(area);
        p.rect_filled(area, 0.0, Color32::WHITE);

        // Header.
        p.rect_filled(head, 0.0, HEADER_BG);
        let mut x = left;
        for (col, title, w) in COLUMNS {
            let r = Rect::from_min_size(Pos2::new(x, head.top()), Vec2::new(w, HEAD));
            p.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0_f32, GRID_LINE));
            let color = if col == Col::Add { Color32::from_gray(110) } else { TEXT };
            let galley = p.layout(title.to_string(), FontId::proportional(12.0), color, w - 6.0);
            let pos = if col == Col::Name { Pos2::new(r.left() + 6.0, r.center().y - galley.size().y / 2.0) } else { r.center() - galley.size() / 2.0 };
            p.galley(pos, galley, color);
            if col == Col::Ind {
                icons::draw(&p, Rect::from_center_size(r.center(), Vec2::splat(14.0)), Icon::Info);
            }
            x += w;
        }
        p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0_f32, Color32::from_gray(170)));

        // Rows: the tasks, then empty rows to type into.
        let visible = (body.height() / ROW).ceil() as usize + 1;
        let first = (self.scroll_y / ROW).floor() as usize;
        let pb = ui.painter_at(body);
        for row in first..first + visible {
            let y = body.top() + row as f32 * ROW - self.scroll_y;
            let line = Rect::from_min_size(Pos2::new(left, y), Vec2::new(total_w, ROW));
            if self.selected.contains(&row) {
                pb.rect_filled(line, 0.0, SELECTED);
            }
            let mut x = left;
            for (col, _, w) in COLUMNS {
                let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, ROW));
                x += w;
                if col == Col::Num {
                    pb.rect_filled(r, 0.0, if self.selected.contains(&row) { Color32::from_rgb(198, 216, 236) } else { HEADER_BG });
                }
                pb.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0_f32, GRID_LINE));
                pb.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0_f32, GRID_LINE));
                if row >= n {
                    continue;
                }
                let t = &self.doc.project.tasks[row];
                let font = FontId::proportional(12.0);
                match col {
                    Col::Num => {
                        pb.text(r.center(), Align2::CENTER_CENTER, t.id.to_string(), font, TEXT);
                    }
                    Col::Ind => {
                        if t.progress.percent_complete >= 100 {
                            let c = Stroke::new(2.0_f32, Color32::from_rgb(60, 150, 60));
                            pb.line_segment([r.center() + Vec2::new(-5.0, 0.0), r.center() + Vec2::new(-1.0, 4.0)], c);
                            pb.line_segment([r.center() + Vec2::new(-1.0, 4.0), r.center() + Vec2::new(6.0, -5.0)], c);
                        } else if !matches!(t.constraint, pjsched::model::Constraint::AsSoonAsPossible) {
                            icons::draw(&pb, Rect::from_center_size(r.center(), Vec2::splat(14.0)), Icon::Calendar);
                        }
                    }
                    Col::Mode => {
                        let icon = if t.manual.is_some() { Icon::Pin } else { Icon::Auto };
                        icons::draw(&pb, Rect::from_center_size(r.center(), Vec2::splat(16.0)), icon);
                    }
                    Col::Name => {
                        let indent = (self.doc.outline_level(row) - 1) as f32 * 14.0;
                        let mut tx = r.left() + 4.0 + indent;
                        if t.summary {
                            let c = Pos2::new(tx + 4.0, r.center().y);
                            pb.add(egui::Shape::convex_polygon(vec![c + Vec2::new(3.0, -3.0), c + Vec2::new(3.0, 3.0), c + Vec2::new(-3.0, 3.0)], TEXT, Stroke::NONE));
                            tx += 12.0;
                        }
                        let clip = pb.with_clip_rect(r.intersect(body));
                        clip.text(Pos2::new(tx, r.center().y), Align2::LEFT_CENTER, &t.name, font.clone(), TEXT);
                        if t.summary {
                            // Summaries in bold: the same text again, half a pixel to the right.
                            clip.text(Pos2::new(tx + 0.6, r.center().y), Align2::LEFT_CENTER, &t.name, font, TEXT);
                        }
                    }
                    Col::Dur | Col::Start | Col::Finish | Col::Pred | Col::Res => {
                        let clip = pb.with_clip_rect(r.shrink(1.0).intersect(body));
                        let align = if col == Col::Dur { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
                        let pos = if col == Col::Dur { Pos2::new(r.right() - 4.0, r.center().y) } else { Pos2::new(r.left() + 4.0, r.center().y) };
                        clip.text(pos, align, self.cell_text(row, col), font, TEXT);
                    }
                    Col::Add => {}
                }
            }
        }
        // The active cell: green frame, as in Project.
        let (cx, cw) = self.col_x(self.cursor.1);
        let cy = body.top() + self.cursor.0 as f32 * ROW - self.scroll_y;
        let cell = Rect::from_min_size(Pos2::new(left + cx, cy), Vec2::new(cw, ROW));
        if self.editing.is_none() {
            pb.rect_stroke(cell.shrink(0.5), 0.0, Stroke::new(2.0_f32, GREEN));
        }

        // Clicks.
        let resp = ui.interact(body, ui.id().with("grid"), Sense::click());
        if let Some(pos) = resp.interact_pointer_pos() {
            if resp.clicked() || resp.double_clicked() {
                let row = ((pos.y - body.top() + self.scroll_y) / ROW).floor().max(0.0) as usize;
                let mut col = Col::Name;
                let mut x = left;
                for (c, _, w) in COLUMNS {
                    if pos.x >= x && pos.x < x + w {
                        col = c;
                    }
                    x += w;
                }
                if let Some(e) = self.editing.take() {
                    self.commit(e);
                }
                let mods = ui.input(|i| i.modifiers);
                self.row_click(row, col, mods);
                if resp.double_clicked() {
                    if col == Col::Mode && row < self.n() {
                        let manual = self.doc.project.tasks[row].manual.is_none();
                        let r = self.doc.set_manual(row, manual);
                        self.report(r);
                    } else {
                        self.begin_edit(row, col, None);
                    }
                }
            }
        }

        // The cell being edited.
        if let Some((erow, ecol)) = self.editing.as_ref().map(|e| (e.row, e.col)) {
            let (ex, ew) = self.col_x(ecol);
            let ey = body.top() + erow as f32 * ROW - self.scroll_y;
            let e = self.editing.as_mut().unwrap();
            let r = Rect::from_min_size(Pos2::new(left + ex, ey), Vec2::new(ew, ROW));
            ui.painter().rect_filled(r, 0.0, Color32::WHITE);
            ui.painter().rect_stroke(r, 0.0, Stroke::new(1.5_f32, GREEN));
            let mut out = None;
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(r.shrink2(Vec2::new(3.0, 1.0))), |ui| {
                out = Some(egui::TextEdit::singleline(&mut e.text).frame(false).font(FontId::proportional(12.0)).desired_width(ew - 6.0).show(ui));
            });
            let out = out.unwrap();
            // Enter, Tab and Escape are read here directly, so a key typed before the field has
            // taken focus still ends the edit.
            let (enter, tab, escape) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Tab), i.key_pressed(Key::Escape)));
            if escape {
                self.editing = None;
            } else if enter || tab || (!e.fresh && out.response.lost_focus()) {
                let e = self.editing.take().unwrap();
                let col = e.col;
                let row = self.commit(e);
                self.cursor = (row, col);
                if tab {
                    self.move_cursor(0, 1);
                } else {
                    self.move_cursor(1, 0);
                }
            } else if e.fresh {
                out.response.request_focus();
                let mut state = out.state;
                let end = egui::text::CCursor::new(e.text.chars().count());
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), out.response.id);
                e.fresh = false;
            }
        }
    }

    fn chart(&mut self, ui: &mut egui::Ui, area: Rect) {
        let head = Rect::from_min_max(area.min, Pos2::new(area.right(), area.top() + HEAD));
        let body = Rect::from_min_max(Pos2::new(area.left(), head.bottom()), area.max);
        let probe = gantt::Scale::new(&self.doc, self.day, 0.0);
        if self.scroll_to_start {
            self.gantt_x = (probe.x(self.doc.project.start) - 3.0 * self.day).max(0.0);
            self.scroll_to_start = false;
        }
        self.gantt_x = self.gantt_x.clamp(0.0, (probe.width(&self.doc) - area.width()).max(0.0));
        let scale = gantt::Scale::new(&self.doc, self.day, area.left() - self.gantt_x);
        gantt::header(&ui.painter_at(head), head, &scale);
        let p = ui.painter_at(body);
        gantt::background(&p, body, &scale, &self.doc);
        let n = self.n();
        for row in self.selected.iter().copied() {
            let y = body.top() + row as f32 * ROW - self.scroll_y;
            p.rect_filled(Rect::from_min_size(Pos2::new(body.left(), y), Vec2::new(body.width(), ROW)), 0.0, SELECTED.gamma_multiply(0.6));
        }
        let mid_of = |i: usize| body.top() + i as f32 * ROW - self.scroll_y + ROW / 2.0;
        gantt::links(&p, &scale, &self.doc, mid_of, ROW);
        for i in 0..n {
            let mid = mid_of(i);
            if mid > body.top() - ROW && mid < body.bottom() + ROW {
                gantt::bar(&p, &scale, &self.doc, i, mid);
            }
        }
        let resp = ui.interact(body, ui.id().with("chart"), Sense::click());
        if let Some(pos) = resp.hover_pos() {
            let row = ((pos.y - body.top() + self.scroll_y) / ROW).floor() as usize;
            if row < n {
                let s = &self.doc.schedule[row];
                let (x0, x1) = (scale.x(self.doc.dt(s.start)), scale.x(self.doc.dt(s.finish)));
                if pos.x >= x0 - 6.0 && pos.x <= x1 + 6.0 {
                    let tip = gantt::tooltip(&self.doc, row);
                    resp.clone().on_hover_text_at_pointer(tip);
                }
            }
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let row = ((pos.y - body.top() + self.scroll_y) / ROW).floor().max(0.0) as usize;
                let mods = ui.input(|i| i.modifiers);
                let col = self.cursor.1;
                self.row_click(row, col, mods);
            }
        }
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        let full = ui.max_rect();
        // The vertical view name on the left edge.
        let label = Rect::from_min_size(full.min, Vec2::new(24.0, full.height()));
        let p = ui.painter();
        p.rect_filled(label, 0.0, Color32::from_rgb(225, 225, 225));
        let galley = p.layout_no_wrap(
            if self.view == View::Gantt { "GRÁFICO DE GANTT" } else { "PLANILHA DE TAREFAS" }.to_string(),
            FontId::proportional(11.0),
            Color32::from_gray(90),
        );
        let pos = Pos2::new(label.center().x - galley.size().y / 2.0, label.top() + 60.0 + galley.size().x);
        p.add(egui::epaint::TextShape::new(pos, galley, Color32::from_gray(90)).with_angle(-std::f32::consts::FRAC_PI_2));
        let area = Rect::from_min_max(Pos2::new(label.right(), full.top()), full.max);

        let n = self.n();
        let content_h = (n + 30) as f32 * ROW;
        let (grid, chart) = if self.view == View::Gantt {
            let split = self.split.clamp(120.0, (area.width() - 120.0).max(120.0));
            let grid = Rect::from_min_max(area.min, Pos2::new(area.left() + split, area.bottom()));
            let handle = Rect::from_min_max(Pos2::new(grid.right(), area.top()), Pos2::new(grid.right() + 5.0, area.bottom()));
            let resp = ui.interact(handle, ui.id().with("split"), Sense::drag());
            if resp.dragged() {
                self.split = split + resp.drag_delta().x;
            }
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            ui.painter().rect_filled(handle, 0.0, Color32::from_rgb(200, 200, 200));
            (grid, Some(Rect::from_min_max(Pos2::new(handle.right(), area.top()), area.max)))
        } else {
            (area, None)
        };

        // Wheel: vertical for both panes; shift (or a horizontal wheel) pans the pane under the mouse.
        let (delta, shift, hover) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.shift, i.pointer.hover_pos()));
        if let Some(h) = hover.filter(|h| area.contains(*h)) {
            let (dx, dy) = if shift { (delta.y + delta.x, 0.0) } else { (delta.x, delta.y) };
            self.scroll_y = (self.scroll_y - dy).clamp(0.0, (content_h - (area.height() - HEAD)).max(0.0));
            if chart.is_some_and(|c| c.contains(h)) {
                self.gantt_x -= dx;
            } else {
                self.grid_x -= dx;
            }
        }
        self.grid(ui, grid);
        if let Some(c) = chart {
            self.chart_w = c.width();
            self.chart(ui, c);
            // Horizontal position hint for the chart.
            let bar = Rect::from_min_max(Pos2::new(c.left(), c.bottom() - 4.0), c.max);
            let total = gantt::Scale::new(&self.doc, self.day, 0.0).width(&self.doc).max(c.width());
            let thumb_w = (c.width() / total * c.width()).max(20.0);
            let thumb_x = c.left() + self.gantt_x / total * c.width();
            ui.painter().rect_filled(bar, 0.0, Color32::from_gray(235));
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(thumb_x, bar.top()), Vec2::new(thumb_w, 4.0)), 2.0, Color32::from_gray(160));
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            ui.label(egui::RichText::new("Pronto").size(12.0));
            ui.label(egui::RichText::new("Novas Tarefas: Agendada Automaticamente").size(12.0));
            if let Some(m) = self.message.clone() {
                ui.label(egui::RichText::new(m).size(12.0).color(Color32::from_rgb(190, 40, 40)));
                if ui.small_button("OK").clicked() {
                    self.message = None;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                let mut zoom = self.day.ln();
                if ui.add(egui::Slider::new(&mut zoom, 1.5f32.ln()..=120f32.ln()).show_value(false)).changed() {
                    self.day = zoom.exp();
                }
                if small_button(ui, Icon::Sheet, "", true) {
                    acts.push(Act::View(View::Sheet));
                }
                if small_button(ui, Icon::Gantt, "", true) {
                    acts.push(Act::View(View::Gantt));
                }
            });
        });
    }

    fn windows(&mut self, ctx: &egui::Context) {
        if let Some(i) = self.info.filter(|&i| i < self.n()) {
            let mut open = true;
            let t = &self.doc.project.tasks[i];
            let s = &self.doc.schedule[i];
            egui::Window::new("Informações da Tarefa").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
                egui::Grid::new("info").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                    let rows = [
                        ("Nome", t.name.clone()),
                        ("Duração", fmt::days(self.doc.shown_days(i))),
                        ("Início", fmt::date(self.doc.dt(s.start))),
                        ("Término", fmt::date(self.doc.dt(s.finish))),
                        ("% concluída", format!("{}%", t.progress.percent_complete)),
                        ("Predecessoras", self.doc.predecessors_text(i)),
                        ("Recursos", self.doc.resources_text(i)),
                        ("Folga total", fmt::days(s.total_slack as f64 / doc::DAY as f64)),
                        ("Crítica", if s.critical { "Sim" } else { "Não" }.into()),
                        ("Custo", format!("R$ {:.2}", self.doc.costs.task[i])),
                        ("Modo", if t.manual.is_some() { "Agendada Manualmente" } else { "Agendada Automaticamente" }.into()),
                    ];
                    for (k, v) in rows {
                        ui.label(egui::RichText::new(k).color(Color32::from_gray(100)));
                        ui.label(v);
                        ui.end_row();
                    }
                });
            });
            if !open {
                self.info = None;
            }
        }
        if let Some(mut text) = self.project_info.take() {
            let mut keep = true;
            egui::Window::new("Informações do Projeto").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Data de início:");
                    ui.text_edit_singleline(&mut text);
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        match fmt::parse_date(&text) {
                            Some(d) => {
                                let r = self.doc.set_project_start(d);
                                self.report(r);
                                self.scroll_to_start = true;
                                keep = false;
                            }
                            None => self.message = Some(format!("Data inválida: {text}")),
                        }
                    }
                    if ui.button("Cancelar").clicked() {
                        keep = false;
                    }
                });
            });
            if keep {
                self.project_info = Some(text);
            }
        }
        if self.overview {
            let mut open = true;
            let doc = &self.doc;
            let n = doc.project.tasks.len();
            let finish = doc.schedule.iter().map(|s| s.finish).max();
            let top: f64 = (0..n).filter(|&i| doc.project.tasks[i].parent.is_none()).map(|i| doc.costs.task[i]).sum();
            let done = doc.project.tasks.iter().filter(|t| t.progress.percent_complete >= 100).count();
            let late = (0..n).filter(|&i| doc.schedule[i].total_slack < 0).count();
            egui::Window::new("Visão Geral do Projeto").open(&mut open).collapsible(false).show(ctx, |ui| {
                egui::Grid::new("overview").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                    let rows = [
                        ("Início", fmt::date(doc.project.start)),
                        ("Término", finish.map(|f| fmt::date(doc.dt(f))).unwrap_or_else(|| "-".into())),
                        ("Tarefas", n.to_string()),
                        ("Concluídas", done.to_string()),
                        ("Com folga negativa", late.to_string()),
                        ("Recursos", doc.project.resources.len().to_string()),
                        ("Custo total", format!("R$ {top:.2}")),
                    ];
                    for (k, v) in rows {
                        ui.label(egui::RichText::new(k).color(Color32::from_gray(100)));
                        ui.label(v);
                        ui.end_row();
                    }
                });
            });
            self.overview = open;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));
        let mut acts = Vec::new();
        let was_editing = self.editing.is_some();
        egui::TopBottomPanel::top("tabs").exact_height(26.0).frame(egui::Frame::none().fill(GREEN)).show(ctx, |ui| self.tabs(ui));
        if self.tab == Tab::File {
            acts.extend(self.backstage(ctx));
        } else {
            egui::TopBottomPanel::top("ribbon")
                .exact_height(96.0)
                .frame(egui::Frame::none().fill(RIBBON_BG).stroke(Stroke::new(1.0_f32, Color32::from_gray(210))))
                .show(ctx, |ui| acts.extend(self.ribbon(ui)));
            if self.timeline {
                egui::TopBottomPanel::top("timeline").exact_height(64.0).frame(egui::Frame::none()).show(ctx, |ui| self.timeline_strip(ui));
            }
            egui::TopBottomPanel::bottom("status")
                .exact_height(24.0)
                .frame(egui::Frame::none().fill(RIBBON_BG).inner_margin(egui::Margin::symmetric(8.0, 2.0)))
                .show(ctx, |ui| self.status_bar(ui, &mut acts));
            egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::WHITE)).show(ctx, |ui| self.central(ui));
            self.windows(ctx);
            self.keyboard(ctx, &mut acts, was_editing);
        }
        for act in acts {
            self.run(act);
        }
    }
}
