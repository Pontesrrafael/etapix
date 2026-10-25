//! Etapix: a desktop project planner for internal use, laid out like Microsoft Project (ribbon,
//! timeline, task grid next to the Gantt chart, status bar). It opens and saves `.mpp` (through
//! the installed Project) and Project XML.

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod calendar_window;
mod dialogs;
mod doc;
mod export;
mod fmt;
mod fonts;
mod gantt;
mod help;
mod icons;
/// The Etapix mark and palette.
mod logo;
mod mpp;
#[cfg(test)]
mod monkey_tests;
mod ribbon;
mod rows;
mod snapshot;
mod task_usage;

use std::collections::BTreeSet;

use doc::Doc;
use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, Vec2};
use icons::Icon;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1366.0, 800.0])
            .with_title("Etapix")
            .with_icon(egui::IconData { rgba: logo::rgba(64), width: 64, height: 64 }),
        ..Default::default()
    };
    let path = std::env::args().nth(1);
    eframe::run_native(
        "etapix",
        options,
        Box::new(move |cc| {
            let mut app = App::new(path);
            app.apply_font(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

/// The brand typefaces, built into the executable (both under the SIL Open Font License, see
/// `assets/fonts`): Inter for the interface, Plus Jakarta Sans for headings and the wordmark.
const FONTS: [(&str, &[u8]); 5] = [
    ("inter", include_bytes!("../assets/fonts/Inter-Regular.ttf")),
    ("inter-medium", include_bytes!("../assets/fonts/Inter-Medium.ttf")),
    ("inter-semibold", include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
    ("jakarta-semibold", include_bytes!("../assets/fonts/PlusJakartaSans-SemiBold.ttf")),
    ("jakarta-bold", include_bytes!("../assets/fonts/PlusJakartaSans-Bold.ttf")),
];

/// Light theme and the brand fonts. A font installed on the machine, when one is picked, takes
/// the place of Inter for the interface text (its bold face for bold text). Segoe UI backs them
/// up for symbols they lack.
fn setup_style(ctx: &egui::Context, chosen: Option<&fonts::Family>) {
    ctx.set_visuals(egui::Visuals::light());
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in FONTS {
        fonts.font_data.insert(name.into(), egui::FontData::from_static(bytes));
    }
    let fallback: Vec<String> = fonts.families[&egui::FontFamily::Proportional].clone();
    let mut segoe = Vec::new();
    // Windows' UI font, or DejaVu Sans, which Ubuntu and most Linux desktops ship.
    let backup = if cfg!(windows) { r"C:\Windows\Fonts\segoeui.ttf" } else { "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf" };
    if let Ok(bytes) = std::fs::read(backup) {
        fonts.font_data.insert("segoe".into(), egui::FontData::from_owned(bytes));
        segoe.push("segoe".to_string());
    }
    let (mut regular, mut bold) = (None, None);
    if let Some(f) = chosen {
        // Only faces the glyph rasterizer accepts: a font it cannot read would stop the app.
        let readable = |bytes: &Vec<u8>| ab_glyph::FontRef::try_from_slice(bytes).is_ok();
        if let Some(bytes) = std::fs::read(&f.regular).ok().filter(readable) {
            fonts.font_data.insert("chosen".into(), egui::FontData::from_owned(bytes));
            regular = Some("chosen");
            bold = Some("chosen");
        }
        if let Some(b) = f.bold.as_ref().and_then(|b| std::fs::read(b).ok()).filter(|_| regular.is_some()).filter(readable) {
            fonts.font_data.insert("chosen-bold".into(), egui::FontData::from_owned(b));
            bold = Some("chosen-bold");
        }
    }
    for (family, first) in [
        (egui::FontFamily::Proportional, regular.into_iter().chain(["inter"]).collect::<Vec<_>>()),
        (egui::FontFamily::Name(MEDIUM.into()), regular.into_iter().chain(["inter-medium"]).collect()),
        (egui::FontFamily::Name(SEMIBOLD.into()), bold.into_iter().chain(["inter-semibold"]).collect()),
        (egui::FontFamily::Name(HEADING.into()), vec!["jakarta-bold"]),
        (egui::FontFamily::Name(HEADING_SEMIBOLD.into()), vec!["jakarta-semibold"]),
    ] {
        let chain = first.into_iter().map(String::from).chain(segoe.iter().cloned()).chain(fallback.iter().cloned()).collect();
        fonts.families.insert(family, chain);
    }
    ctx.set_fonts(fonts);
}

const MEDIUM: &str = "medium";
const SEMIBOLD: &str = "semibold";
const HEADING: &str = "heading";
const HEADING_SEMIBOLD: &str = "heading-semibold";

fn font(family: &str, size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name(family.into()))
}

/// Same as `font`, for places where a local variable named `font` hides it.
fn font_named(family: &str, size: f32) -> FontId {
    font(family, size)
}

/// The task's outline number ("2.1.3"), as Project's WBS-like "Número da Estrutura de Tópicos".
fn outline_number(doc: &Doc, row: usize) -> String {
    let tasks = &doc.project.tasks;
    let mut parts = Vec::new();
    let mut cur = Some(row);
    while let Some(i) = cur {
        let parent = tasks[i].parent;
        let position = (0..=i).filter(|&k| tasks[k].parent == parent).count();
        parts.push(position.to_string());
        cur = parent;
    }
    parts.reverse();
    parts.join(".")
}

/// Column widths saved from the last session, or the defaults.
fn saved_widths() -> [f32; 10] {
    let mut w = COLUMNS.map(|c| c.2);
    if let Some(saved) = fonts::setting("column_widths") {
        for (slot, v) in w.iter_mut().zip(saved.split(',')) {
            if let Ok(v) = v.trim().parse::<f32>() {
                *slot = v.clamp(16.0, 900.0);
            }
        }
    }
    w
}

/// Cell text at `x`: one line centred vertically, or, in rows tall enough for two lines or
/// more, wrapped to the cell's width (as Project does when you make a row taller).
fn cell_text(p: &egui::Painter, text: &str, font: FontId, x: f32, cell: Rect, row_h: f32, size: f32) {
    let width = (cell.right() - x - 4.0).max(10.0);
    let galley = if row_h >= size * 2.4 {
        p.layout(text.to_string(), font, TEXT, width)
    } else {
        p.layout_no_wrap(text.to_string(), font, TEXT)
    };
    let y = (cell.center().y - galley.size().y / 2.0).max(cell.top() + 2.0);
    p.galley(Pos2::new(x, y), galley, TEXT);
}

// Etapix palette: deep teal (brand), slate navy (text), emerald (accents), soft grays.
const GREEN: Color32 = Color32::from_rgb(0x0B, 0x6B, 0x66);
const GREEN_DARK: Color32 = Color32::from_rgb(0x08, 0x55, 0x51);
const NAVY: Color32 = Color32::from_rgb(0x1E, 0x2F, 0x3E);
const EMERALD: Color32 = Color32::from_rgb(0x14, 0xC3, 0x8E);
const MINT: Color32 = Color32::from_rgb(0x4E, 0xE7, 0xC7);
const RIBBON_BG: Color32 = Color32::from_rgb(0xF4, 0xF8, 0xFA);
const HOVER: Color32 = Color32::from_rgb(0xDF, 0xEE, 0xEC);
const GRID_LINE: Color32 = Color32::from_rgb(0xDC, 0xE5, 0xEA);
const HEADER_BG: Color32 = Color32::from_rgb(0xEE, 0xF3, 0xF6);
const SELECTED: Color32 = Color32::from_rgb(0xDD, 0xF6, 0xEE);
const TEXT: Color32 = NAVY;
/// Thickness of the grid's and chart's scroll bars.
const BAR: f32 = 12.0;

/// A scroll bar in `track`: drag the thumb, or click the track to jump a page, as in Windows.
/// `total` is the content's length, `visible` the window's, `offset` the scroll position.
fn scrollbar(ui: &mut egui::Ui, id: &str, track: Rect, horizontal: bool, total: f32, visible: f32, offset: &mut f32) {
    let p = ui.painter();
    p.rect_filled(track, 0.0, Color32::from_rgb(0xEE, 0xF3, 0xF6));
    let max = (total - visible).max(0.0);
    *offset = offset.clamp(0.0, max);
    if max <= 0.0 || visible <= 0.0 {
        return;
    }
    let len = if horizontal { track.width() } else { track.height() };
    let thumb_len = (visible / total * len).clamp(24.0, len);
    let travel = (len - thumb_len).max(1.0);
    let at = *offset / max * travel;
    let thumb = if horizontal {
        Rect::from_min_size(Pos2::new(track.left() + at, track.top() + 2.0), Vec2::new(thumb_len, track.height() - 4.0))
    } else {
        Rect::from_min_size(Pos2::new(track.left() + 2.0, track.top() + at), Vec2::new(track.width() - 4.0, thumb_len))
    };
    let resp = ui.interact(track, ui.id().with(id), Sense::click_and_drag());
    if resp.dragged() {
        let d = if horizontal { resp.drag_delta().x } else { resp.drag_delta().y };
        *offset = (*offset + d / travel * max).clamp(0.0, max);
    } else if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let before = if horizontal { pos.x < thumb.left() } else { pos.y < thumb.top() };
            let page = visible * 0.9;
            *offset = (*offset + if before { -page } else { page }).clamp(0.0, max);
        }
    }
    let color = if resp.dragged() {
        Color32::from_rgb(0x0B, 0x6B, 0x66)
    } else if resp.hovered() {
        Color32::from_gray(140)
    } else {
        Color32::from_gray(175)
    };
    ui.painter().rect_filled(thumb, 4.0, color);
}

/// Default row height; the user can change it by dragging a row border.
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Tab {
    File,
    Task,
    Resource,
    Report,
    Project,
    View,
    Help,
    Format,
}

const TABS: [(Tab, &str); 8] = [
    (Tab::File, "Arquivo"),
    (Tab::Task, "Tarefa"),
    (Tab::Resource, "Recurso"),
    (Tab::Report, "Relatório"),
    (Tab::Project, "Projeto"),
    (Tab::View, "Exibir"),
    (Tab::Help, "Ajuda"),
    // Project's contextual tab for the Gantt chart ("Ferramentas de Gráfico de Gantt").
    (Tab::Format, "Formato"),
];

/// The reports of Relatório > Exibir Relatórios.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Report {
    Dashboard,
    Resources,
    Costs,
    InProgress,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Gantt,
    Sheet,
    Usage,
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
    ZoomIn,
    ZoomOut,
    Whole,
    Timeline,
    View(View),
    Cut,
    Copy,
    Paste,
    RespectLinks,
    /// Moves the selected tasks by this many calendar days.
    Move(i64),
    AddTimeline,
    Find,
    Assign,
    AddResource,
    Report(Report),
    Calc,
    MoveProject,
    ZoomSelected,
    /// Sets the timescale: the width of a day in pixels.
    Timescale(f32),
    About,
    Undo,
    Redo,
    /// Como usar for the current tab.
    HowTo,
    HelpCenter,
    Tour(&'static str),
}

/// What waits for "Deseja salvar as alterações?".
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Close,
    New,
    Open,
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
    /// Snapshot mode (see `snapshot.rs`): the tab being captured and frames since it opened.
    snap: Option<(usize, u32, bool)>,
    /// The chart's last laid-out width, used by "Projeto Inteiro".
    chart_w: f32,
    logo: Option<egui::TextureHandle>,
    /// Fonts installed on the machine, listed the first time the picker needs them.
    installed_fonts: Option<Vec<fonts::Family>>,
    /// The picked font family ("" for Inter, the Etapix font) and the grid's text size.
    font_name: String,
    text_size: f32,
    /// Row height and column widths, resizable by dragging the grid's borders.
    row_h: f32,
    col_w: [f32; 10],
    /// Copied tasks: name, duration in days and outline level.
    clipboard: Vec<(String, f64, usize)>,
    /// Tasks (by UID) shown on the timeline strip.
    timeline_tasks: BTreeSet<i64>,
    gantt_opts: gantt::Options,
    gantt_interaction: gantt::Interaction,
    usage_first: Option<chrono::NaiveDate>,
    outline_numbers: bool,
    find: Option<String>,
    assign: bool,
    add_resource: Option<String>,
    report: Option<Report>,
    move_project: Option<String>,
    about: bool,
    /// Each ribbon tab's width at full size, to decide when to draw it compact.
    ribbon_widths: std::collections::HashMap<Tab, f32>,
    /// Driven by tests: no file dialogs (they would wait for a person).
    headless: bool,
    /// An action waiting for the save-changes question; `closing` lets the window close.
    confirm: Option<Pending>,
    closing: bool,
    /// Commands from the quick access toolbar, run with the frame's other actions.
    quick: Vec<Act>,
    help: help::Help,
    /// The first-visit suggestion (off in scripted tests, which click at fixed places).
    suggest: bool,
    /// Filter, sort and collapsed summaries; `shown` is the tasks they leave, top to bottom.
    display: rows::Display,
    shown: Vec<usize>,
}

impl App {
    fn new(path: Option<String>) -> Self {
        let (doc, message) = match path.map(|p| Doc::open(std::path::Path::new(&p))) {
            Some(Ok(d)) => (d, None),
            Some(Err(e)) => (Doc::blank(), Some(e)),
            None => (Doc::blank(), None),
        };
        let (font_name, text_size) = fonts::load_choice();
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
            snap: snapshot::dir().map(|_| (0, 0, false)),
            chart_w: 700.0,
            logo: None,
            installed_fonts: None,
            font_name,
            text_size,
            row_h: fonts::setting("row_height").and_then(|v| v.parse().ok()).unwrap_or(ROW).max(text_size + 8.0),
            col_w: saved_widths(),
            clipboard: Vec::new(),
            timeline_tasks: BTreeSet::new(),
            gantt_opts: gantt::Options::default(),
            gantt_interaction: gantt::Interaction::default(),
            usage_first: None,
            outline_numbers: false,
            find: None,
            assign: false,
            add_resource: None,
            report: None,
            move_project: None,
            about: false,
            ribbon_widths: std::collections::HashMap::new(),
            headless: false,
            confirm: None,
            closing: false,
            quick: Vec::new(),
            help: help::Help::default(),
            suggest: true,
            display: rows::Display::default(),
            shown: Vec::new(),
        }
    }

    fn installed_fonts(&mut self) -> &[fonts::Family] {
        self.installed_fonts.get_or_insert_with(fonts::installed)
    }

    /// Loads the picked font (or the Etapix font) into the interface.
    fn apply_font(&mut self, ctx: &egui::Context) {
        let name = self.font_name.clone();
        let chosen = if name.is_empty() { None } else { self.installed_fonts().iter().find(|f| f.name == name).cloned() };
        if chosen.is_none() {
            // A font missing on this machine falls back to Inter.
            self.font_name.clear();
        }
        setup_style(ctx, chosen.as_ref());
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
        format!("{}{} - Etapix", self.file_name(), if self.doc.dirty { " *" } else { "" })
    }

    fn set_doc(&mut self, doc: Doc) {
        self.doc = doc;
        self.gantt_interaction = gantt::Interaction::default();
        self.usage_first = None;
        self.selected.clear();
        self.cursor = (0, Col::Name);
        self.editing = None;
        self.scroll_y = 0.0;
        self.scroll_to_start = true;
    }

    fn open(&mut self) {
        if self.headless {
            return;
        }
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
        if self.headless {
            return;
        }
        let path = match (&self.doc.path, ask) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new()
                .add_filter("Projeto (*.mpp)", &["mpp"])
                .add_filter("Formato XML (*.xml)", &["xml"])
                .set_file_name(format!("{}.mpp", self.file_name().trim_end_matches(".mpp").trim_end_matches(".xml")))
                .save_file(),
        };
        if let Some(p) = path {
            match self.doc.save(&p) {
                Ok(notice) => self.message = notice,
                Err(e) => self.message = Some(e),
            }
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

    /// The task on display row `row`; rows past the shown tasks are the empty rows to type
    /// into, numbered after the last task.
    fn task_at(&self, row: usize) -> usize {
        self.shown.get(row).copied().unwrap_or(self.n() + row.saturating_sub(self.shown.len()))
    }

    /// The display row of task `i`, if it is shown.
    fn row_of(&self, i: usize) -> Option<usize> {
        if i >= self.n() {
            return Some(self.shown.len() + i - self.n());
        }
        self.shown.iter().position(|&k| k == i)
    }

    fn refresh_shown(&mut self) {
        self.shown = self.display.rows(&self.doc, chrono::Local::now().date_naive());
    }

    fn select_only(&mut self, row: usize) {
        self.selected.clear();
        self.selected.insert(row);
    }

    fn run(&mut self, act: Act) {
        match act {
            Act::Undo => {
                let r = self.doc.undo();
                self.after_history();
                return self.report(r);
            }
            Act::Redo => {
                let r = self.doc.redo();
                self.after_history();
                return self.report(r);
            }
            Act::New | Act::Open if self.doc.dirty && !self.headless => {
                self.confirm = Some(if matches!(act, Act::New) { Pending::New } else { Pending::Open });
                return;
            }
            _ => {}
        }
        let version = self.doc.begin();
        self.run_action(act);
        self.doc.end(version);
    }

    /// "Deseja salvar as alterações?" before closing, a new project or opening another file.
    fn save_changes_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.confirm else { return };
        let mut choice = None;
        egui::Window::new("Etapix")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Deseja salvar as alterações em \"{}\"?", self.file_name()));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Salvar").clicked() {
                        choice = Some(0);
                    }
                    if ui.button("Não Salvar").clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Cancelar").clicked() {
                        choice = Some(2);
                    }
                });
            });
        let go_on = match choice {
            None => return,
            Some(0) => {
                self.save(false);
                !self.doc.dirty
            }
            Some(1) => true,
            _ => false,
        };
        self.confirm = None;
        if !go_on {
            return;
        }
        match pending {
            Pending::Close => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Pending::New => {
                self.doc.dirty = false;
                self.run(Act::New);
            }
            Pending::Open => {
                self.doc.dirty = false;
                self.run(Act::Open);
            }
        }
    }

    /// After Desfazer/Refazer the selection may point past the last task.
    fn after_history(&mut self) {
        let n = self.n();
        self.selected.retain(|&r| r < n);
        self.editing = None;
    }

    fn run_action(&mut self, act: Act) {
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
                if self.tab == Tab::File {
                    self.tab = Tab::Task;
                }
                Ok(())
            }
            Act::SaveAs => {
                self.save(true);
                if self.tab == Tab::File {
                    self.tab = Tab::Task;
                }
                Ok(())
            }
            Act::Percent(p) => rows.iter().try_for_each(|&i| if self.doc.project.tasks[i].summary { Ok(()) } else { self.doc.set_percent(i, p) }),
            // As in Project, rows that cannot move a level are left alone instead of failing.
            Act::Indent => {
                rows.iter().for_each(|&i| {
                    let _ = self.doc.indent(i);
                });
                Ok(())
            }
            Act::Outdent => {
                for &i in rows.iter().rev() {
                    if self.doc.project.tasks[i].parent.is_some() {
                        let _ = self.doc.outdent(i);
                    }
                }
                Ok(())
            }
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
                self.gantt_interaction = gantt::Interaction::default();
                Ok(())
            }
            Act::Copy | Act::Cut => {
                self.clipboard = rows
                    .iter()
                    .map(|&i| (self.doc.project.tasks[i].name.clone(), self.doc.days(i), self.doc.outline_level(i)))
                    .collect();
                if matches!(act, Act::Cut) {
                    let r = rows.iter().rev().try_for_each(|&i| self.doc.delete_task(i));
                    self.selected.clear();
                    r
                } else {
                    Ok(())
                }
            }
            Act::Paste => self.paste(),
            Act::RespectLinks => rows.iter().try_for_each(|&i| {
                // A manual task goes back to where its links put it, and stays manual.
                if self.doc.project.tasks[i].manual.is_some() {
                    self.doc.set_manual(i, false)?;
                    self.doc.set_manual(i, true)
                } else {
                    Ok(())
                }
            }),
            Act::Move(days) => rows.iter().try_for_each(|&i| {
                if self.doc.project.tasks[i].summary {
                    return Ok(());
                }
                let start = self.doc.dt(self.doc.schedule[i].start).date() + chrono::Duration::days(days);
                self.doc.set_start(i, start)
            }),
            Act::AddTimeline => {
                for &i in &rows {
                    let uid = self.doc.project.tasks[i].uid;
                    if !self.timeline_tasks.remove(&uid) {
                        self.timeline_tasks.insert(uid);
                    }
                }
                self.timeline = true;
                Ok(())
            }
            Act::Find => {
                self.find = Some(String::new());
                Ok(())
            }
            Act::Assign => {
                self.assign = true;
                Ok(())
            }
            Act::AddResource => {
                self.add_resource = Some(String::new());
                Ok(())
            }
            Act::Report(r) => {
                self.report = Some(r);
                Ok(())
            }
            // Etapix recalculates after every edit; this just redoes it on request.
            Act::Calc => self.doc.apply(|_| Ok(())),
            Act::MoveProject => {
                self.move_project = Some(self.doc.project.start.format("%d/%m/%Y").to_string());
                Ok(())
            }
            Act::ZoomSelected => {
                self.zoom_to(&rows);
                Ok(())
            }
            Act::Timescale(day) => {
                self.day = day;
                Ok(())
            }
            Act::About => {
                self.about = true;
                Ok(())
            }
            Act::Undo | Act::Redo => Ok(()),
            Act::HowTo => {
                let t = help::content::for_tab(self.tab);
                self.open_help(t.id, 0);
                Ok(())
            }
            Act::HelpCenter => {
                self.help.catalog = true;
                if self.tab == Tab::File {
                    self.tab = Tab::Task;
                }
                Ok(())
            }
            Act::Tour(id) => {
                self.start_tour(id);
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

    /// Inserts the copied tasks below the cursor, keeping their outline levels.
    fn paste(&mut self) -> Result<(), String> {
        let mut at = (self.cursor.0 + 1).min(self.n());
        let clip = self.clipboard.clone();
        let base = clip.iter().map(|c| c.2).min().unwrap_or(1);
        let level0 = if at > 0 && at <= self.n() && self.cursor.0 < self.n() { self.doc.outline_level(self.cursor.0) } else { 1 };
        for (name, days, level) in clip {
            let row = self.doc.insert_task(at, &name, days)?;
            let want = level0 + level - base;
            while self.doc.outline_level(row) > want && self.doc.project.tasks[row].parent.is_some() {
                self.doc.outdent(row)?;
            }
            while self.doc.outline_level(row) < want && self.doc.indent(row).is_ok() {}
            at = row + 1;
        }
        Ok(())
    }

    /// Zoom so the given tasks fill the chart (Exibir > Tarefas Selecionadas).
    fn zoom_to(&mut self, rows: &[usize]) {
        let doc = &self.doc;
        let (Some(lo), Some(hi)) = (rows.iter().map(|&i| doc.schedule[i].start).min(), rows.iter().map(|&i| doc.schedule[i].finish).max()) else {
            return;
        };
        let days = ((doc.dt(hi) - doc.dt(lo)).num_minutes() as f32 / 1440.0).max(0.5) + 1.0;
        self.day = (self.chart_w / days).clamp(1.5, 120.0);
        let x = gantt::Scale::new(doc, self.day, 0.0).x(doc.dt(lo));
        self.gantt_x = (x - self.day * 0.5).max(0.0);
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

/// Texture coordinates of a whole image.
const UV: Rect = Rect { min: Pos2::ZERO, max: Pos2 { x: 1.0, y: 1.0 } };

/// "Etapix" with the i in mint (the logo's emerald, lightened for the dark bars it sits on); `pos` is the left end, vertically centred.
fn wordmark(p: &egui::Painter, pos: Pos2, size: f32, color: Color32) {
    let font = font(HEADING, size);
    let mut job = egui::text::LayoutJob::default();
    for (part, c) in [("Etap", color), ("i", MINT), ("x", color)] {
        job.append(part, 0.0, egui::TextFormat::simple(font.clone(), c));
    }
    let galley = p.layout_job(job);
    let at = Pos2::new(pos.x, pos.y - galley.size().y / 2.0);
    p.galley(at, galley, color);
}

impl App {
    fn logo_texture(&mut self, ctx: &egui::Context) -> egui::TextureId {
        self.logo
            .get_or_insert_with(|| {
                let image = egui::ColorImage::from_rgba_unmultiplied([128, 128], &logo::rgba(128));
                ctx.load_texture("etapix-logo", image, egui::TextureOptions::LINEAR)
            })
            .id()
    }
}

/// Lays out `text` one galley per line, never wrapping: line breaks come only from `\n`, so a
/// wider font makes the label wider instead of breaking words. Returns the lines and the size.
fn lines(p: &egui::Painter, text: &str, font: FontId, color: Color32) -> (Vec<std::sync::Arc<egui::Galley>>, Vec2) {
    let lines: Vec<_> = text.split('\n').map(|l| p.layout_no_wrap(l.to_string(), font.clone(), color)).collect();
    let w = lines.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let h = lines.iter().map(|g| g.size().y).sum();
    (lines, Vec2::new(w, h))
}

/// Draws lines from `lines`, each centred on `center_x`, starting at `top`.
fn draw_centered(p: &egui::Painter, lines: Vec<std::sync::Arc<egui::Galley>>, center_x: f32, top: f32, color: Color32) {
    let mut y = top;
    for g in lines {
        let h = g.size().y;
        p.galley_with_override_text_color(Pos2::new(center_x - g.size().x / 2.0, y), g, color);
        y += h;
    }
}

/// Drawn over the icon of a disabled button.
const FADE: Color32 = Color32::from_rgba_premultiplied(150, 153, 154, 155);

const NOT_YET: &str = "Indisponível agora: selecione uma tarefa (ou mais de uma, para vincular).";
/// The tooltip of a command Project has and Etapix does not have yet.
const SOON: &str = "Em breve no Etapix: este comando ainda não existe.";

/// A ribbon button for a command Etapix does not have yet: drawn disabled, saying so.
fn soon(ui: &mut egui::Ui, icon: Icon, label: &str) {
    SOON_TIP.with(|c| c.set(true));
    small_button(ui, icon, label, false);
    SOON_TIP.with(|c| c.set(false));
}

/// [`soon`] for a large button.
fn soon_big(ui: &mut egui::Ui, icon: Icon, label: &str) {
    SOON_TIP.with(|c| c.set(true));
    big_button(ui, icon, label, false);
    SOON_TIP.with(|c| c.set(false));
}

/// The tooltip of a disabled button.
fn disabled_tip() -> &'static str {
    if SOON_TIP.with(|c| c.get()) { SOON } else { NOT_YET }
}

fn big_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> bool {
    let color = if enabled { TEXT } else { Color32::from_gray(160) };
    let (label, size) = lines(ui.painter(), label, FontId::proportional(12.0), color);
    let w = size.x.max(36.0) + 12.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 72.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(rect, 0.0, HOVER);
    }
    icons::draw(p, Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 20.0), Vec2::splat(28.0)), icon);
    // Text sits between the icon and the group title, however many lines it has (at most two).
    draw_centered(p, label, rect.center().x, (rect.top() + 38.0).min(rect.bottom() - size.y), color);
    if !enabled {
        p.rect_filled(Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 20.0), Vec2::splat(30.0)), 0.0, FADE);
        resp.on_hover_text(disabled_tip());
        return false;
    }
    resp.clicked()
}

fn small_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> bool {
    let resp = small_button_response(ui, icon, label, enabled);
    enabled && resp.clicked()
}

/// A small ribbon button; `▾` after the label when it opens a menu.
fn small_button_response(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> egui::Response {
    let font = FontId::proportional(12.0);
    // A trailing "▾" is drawn as a small triangle (not every font has the glyph).
    let (label, menu) = match label.strip_suffix(" ▾") {
        Some(l) => (l, true),
        None => (label, false),
    };
    // A narrow window shows small buttons as icons only, with the name as a tooltip (as Project
    // does when its ribbon does not fit).
    let compact = COMPACT.with(|c| c.get()) && !label.is_empty();
    let shown = if compact { "" } else { label };
    let galley = ui.painter().layout_no_wrap(shown.to_string(), font, TEXT);
    let w = galley.size().x + if shown.is_empty() { 24.0 } else { 30.0 } + if menu { 12.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    if menu {
        let c = Pos2::new(rect.right() - 8.0, rect.center().y + 1.0);
        let color = if enabled { TEXT } else { Color32::from_gray(160) };
        ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(-3.5, -2.0), c + Vec2::new(3.5, -2.0), c + Vec2::new(0.0, 2.5)], color, Stroke::NONE));
    }
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(rect, 0.0, HOVER);
    }
    let icon_rect = Rect::from_min_size(Pos2::new(rect.left() + 4.0, rect.top() + 3.0), Vec2::splat(16.0));
    icons::draw(p, icon_rect, icon);
    if !enabled {
        p.rect_filled(icon_rect.expand(1.0), 0.0, FADE);
    }
    let color = if enabled { TEXT } else { Color32::from_gray(160) };
    p.galley_with_override_text_color(Pos2::new(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0), galley, color);
    let resp = if compact { resp.on_hover_text(label) } else { resp };
    if enabled { resp } else { resp.on_hover_text(disabled_tip()) }
}

thread_local! {
    /// Set while drawing a button for a command Etapix does not have yet.
    static SOON_TIP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Set while drawing a ribbon tab that does not fit the window at full size.
    static COMPACT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A ribbon group: its buttons on top, its name underneath, a separator after it.
fn group(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui)) -> Rect {
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
    Rect::from_min_max(r.min, Pos2::new(r.right(), r.top() + 90.0))
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
        let logo = self.logo_texture(ui.ctx());
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 26.0), Sense::hover());
            // The mark is dark teal like the bar: a light outline keeps its shape visible.
            let mark = Rect::from_center_size(rect.center(), Vec2::splat(18.0));
            ui.painter().rect_stroke(mark.expand(0.5), 4.0, Stroke::new(1.0_f32, Color32::from_white_alpha(140)));
            ui.painter().image(logo, mark, UV, Color32::WHITE);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(54.0, 26.0), Sense::hover());
            wordmark(ui.painter(), Pos2::new(rect.left() + 2.0, rect.center().y), 15.0, Color32::WHITE);
            // Quick access toolbar: Salvar, Desfazer, Refazer (Project keeps them in the title bar).
            ui.add_space(4.0);
            let qat_left = ui.cursor().left();
            for (icon, tip, act, enabled) in [
                (Icon::Save, "Salvar (Ctrl+S)", Act::Save, true),
                (Icon::Undo, "Desfazer (Ctrl+Z)", Act::Undo, self.doc.can_undo()),
                (Icon::Redo, "Refazer (Ctrl+Y)", Act::Redo, self.doc.can_redo()),
            ] {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(24.0, 26.0), if enabled { Sense::click() } else { Sense::hover() });
                if enabled && resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, GREEN_DARK);
                }
                icons::draw(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(15.0)), icon);
                if !enabled {
                    ui.painter().rect_filled(Rect::from_center_size(rect.center(), Vec2::splat(17.0)), 0.0, GREEN.gamma_multiply(0.6));
                }
                if resp.on_hover_text(tip).clicked() && enabled {
                    self.quick.push(act);
                }
            }
            let qat = Rect::from_min_max(Pos2::new(qat_left, ui.max_rect().top()), Pos2::new(ui.cursor().left(), ui.max_rect().top() + 26.0));
            self.target(ui, "acesso_rapido", qat);
            ui.add_space(6.0);
            for (tab, name) in TABS {
                let galley = ui.painter().layout_no_wrap(name.to_string(), font(HEADING_SEMIBOLD, 13.0), Color32::WHITE);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 22.0, 26.0), Sense::click());
                let active = self.tab == tab && tab != Tab::File;
                let p = ui.painter();
                if active {
                    p.rect_filled(rect, 0.0, RIBBON_BG);
                } else if resp.hovered() {
                    p.rect_filled(rect, 0.0, GREEN_DARK);
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

    /// Font and size of the interface text (Tarefa > Fonte).
    fn font_picker(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let shown = if self.font_name.is_empty() { "Inter (Etapix)".to_string() } else { self.font_name.clone() };
            let mut picked = None;
            egui::ComboBox::from_id_salt("font")
                .selected_text(shown)
                .width(150.0)
                .height(420.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(self.font_name.is_empty(), "Inter (Etapix)").clicked() {
                        picked = Some(String::new());
                    }
                    let names: Vec<String> = self.installed_fonts().iter().map(|f| f.name.clone()).collect();
                    for name in names {
                        if ui.selectable_label(self.font_name == name, &name).clicked() {
                            picked = Some(name);
                        }
                    }
                });
            ui.horizontal(|ui| {
                let mut size = self.text_size;
                if ui.add(egui::DragValue::new(&mut size).range(8.0..=24.0).speed(0.2).fixed_decimals(0).suffix(" pt")).changed() {
                    self.text_size = size.round();
                    self.row_h = self.row_h.max(self.text_size + 8.0);
                    fonts::save_choice(&self.font_name, self.text_size);
                }
            });
            if let Some(name) = picked {
                self.font_name = name;
                self.apply_font(ui.ctx());
                fonts::save_choice(&self.font_name, self.text_size);
            }
        });
    }

    /// Backstage (the "Arquivo" tab): a green column of commands over the whole window.
    fn backstage(&mut self, ctx: &egui::Context) -> Vec<Act> {
        let mut acts = Vec::new();
        egui::SidePanel::left("backstage").exact_width(170.0).resizable(false).frame(egui::Frame::none().fill(NAVY)).show(ctx, |ui| {
            ui.add_space(14.0);
            let logo = self.logo_texture(ctx);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(170.0, 40.0), Sense::hover());
            ui.painter().image(logo, Rect::from_min_size(rect.min + Vec2::new(18.0, 4.0), Vec2::splat(32.0)), UV, Color32::WHITE);
            wordmark(ui.painter(), Pos2::new(rect.left() + 58.0, rect.center().y), 22.0, Color32::WHITE);
            ui.add_space(8.0);
            let back = ui.add(egui::Label::new(egui::RichText::new("  ←").size(22.0).color(Color32::WHITE)).sense(Sense::click()));
            if back.clicked() {
                self.tab = Tab::Task;
            }
            ui.add_space(16.0);
            for (act, name) in [(Act::New, "Novo"), (Act::Open, "Abrir"), (Act::Save, "Salvar"), (Act::SaveAs, "Salvar como")] {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(170.0, 36.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, GREEN_DARK);
                }
                ui.painter().text(Pos2::new(rect.left() + 24.0, rect.center().y), Align2::LEFT_CENTER, name, font(HEADING_SEMIBOLD, 15.0), Color32::WHITE);
                if resp.clicked() {
                    acts.push(act);
                }
            }
        });
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::WHITE).inner_margin(32.0)).show(ctx, |ui| {
            ui.label(egui::RichText::new("Novo").font(font(HEADING, 30.0)).color(TEXT));
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
            ui.add_space(16.0);
            if ui.add(egui::Button::new(egui::RichText::new("?  Aprender a usar o Etapix (Central de Ajuda)").size(14.0))).clicked() {
                acts.push(Act::HelpCenter);
            }
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Os arquivos .mpp são lidos, criados e salvos pelo próprio Etapix: o Microsoft Project não é necessário.").color(Color32::from_gray(110)));
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
        p.rect_filled(band, 0.0, HEADER_BG);
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
        let shown: Vec<usize> = (0..doc.project.tasks.len()).filter(|&i| self.timeline_tasks.contains(&doc.project.tasks[i].uid)).collect();
        if shown.is_empty() {
            p.text(band.center() + Vec2::new(0.0, 4.0), Align2::CENTER_CENTER, "Adicione tarefas com datas à linha do tempo", FontId::proportional(12.0), Color32::from_gray(140));
        }
        for (k, &i) in shown.iter().enumerate() {
            let s = &doc.schedule[i];
            let (x0, x1) = (x_of(doc.dt(s.start)).max(band.left()), x_of(doc.dt(s.finish)).min(band.right()));
            let lanes = shown.len().min(3) as f32;
            let h = (band.height() - 6.0) / lanes;
            let y = band.top() + 3.0 + (k as f32 % lanes) * h;
            let r = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1.max(x0 + 3.0), y + h - 2.0));
            p.rect_filled(r, 2.0, if k % 2 == 0 { EMERALD } else { MINT });
            p.with_clip_rect(r).text(Pos2::new(r.left() + 4.0, r.center().y), Align2::LEFT_CENTER, &doc.project.tasks[i].name, small.clone(), NAVY);
        }
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
            Col::Dur | Col::Start | Col::Finish | Col::Pred | Col::Res => !summary,
            _ => false,
        }
    }

    fn begin_edit(&mut self, row: usize, col: Col, text: Option<String>) {
        if !self.editable(row, col) {
            return;
        }
        let text = text.unwrap_or_else(|| {
            let t = self.cell_text(row, col);
            if matches!(col, Col::Start | Col::Finish) { t.rsplit(' ').next().unwrap_or("").to_string() } else { t }
        });
        self.editing = Some(Editing { row, col, text, fresh: true });
    }

    /// Applies an edit; returns the row it landed on (a new task typed into an empty row goes
    /// right after the last task).
    fn commit(&mut self, e: Editing) -> usize {
        let version = self.doc.begin();
        let row = self.commit_edit(e);
        self.doc.end(version);
        row
    }

    fn commit_edit(&mut self, e: Editing) -> usize {
        let text = e.text.trim().to_string();
        let mut row = e.row;
        // Clearing a duration or a date keeps the current value, as Project does.
        if text.is_empty() && matches!(e.col, Col::Dur | Col::Start | Col::Finish) {
            return row;
        }
        let invalid = match e.col {
            Col::Dur if fmt::parse_days(&text).is_none() => Some(format!("Duração inválida: {text} (use, por exemplo, 3 ou 2,5 dias)")),
            Col::Start | Col::Finish if fmt::parse_date(&text).is_none() => Some(format!("Data inválida: {text} (use dd/mm/aa)")),
            _ => None,
        };
        if let Some(m) = invalid {
            self.message = Some(m);
            return row;
        }
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
            Col::Finish => match fmt::parse_date(&text) {
                Some(d) => self.doc.set_finish(row, d),
                None => Err(format!("Data inválida: {text} (use dd/mm/aa)")),
            },
            Col::Pred => self.doc.set_predecessors(row, &text),
            Col::Res => self.doc.set_resources(row, &text),
            _ => Ok(()),
        };
        self.report(r);
        row
    }

    /// The grid's columns with their current widths.
    fn columns(&self) -> [(Col, &'static str, f32); 10] {
        let mut cols = COLUMNS;
        for (c, w) in cols.iter_mut().zip(self.col_w) {
            c.2 = w;
        }
        cols
    }

    fn save_layout(&self) {
        let widths = self.col_w.iter().map(|w| format!("{w:.0}")).collect::<Vec<_>>().join(",");
        fonts::save_settings(&[("row_height", format!("{:.0}", self.row_h)), ("column_widths", widths)]);
    }

    /// The width that shows the whole column (header and every task), as a double-click on a
    /// column border does in Project.
    fn fit_width(&self, ui: &egui::Ui, col: Col) -> f32 {
        let font = FontId::proportional(self.text_size);
        let measure = |t: &str, f: FontId| ui.fonts(|fonts| fonts.layout_no_wrap(t.to_string(), f, TEXT).size().x);
        let header = COLUMNS.iter().find(|c| c.0 == col).map(|c| c.1).unwrap_or("");
        let mut w = header.split('\n').map(|l| measure(l, FontId::proportional(self.text_size.min(12.0)))).fold(0.0, f32::max) + 12.0;
        for i in 0..self.n() {
            let t = &self.doc.project.tasks[i];
            let cell = match col {
                Col::Name => {
                    let f = if t.summary { font_named(SEMIBOLD, self.text_size) } else { font.clone() };
                    measure(&t.name, f) + (self.doc.outline_level(i) - 1) as f32 * 14.0 + if t.summary { 12.0 } else { 0.0 } + 12.0
                }
                Col::Num | Col::Dur | Col::Start | Col::Finish | Col::Pred | Col::Res => measure(&self.cell_text(i, col), font.clone()) + 12.0,
                _ => 0.0,
            };
            w = w.max(cell);
        }
        w.clamp(20.0, 900.0)
    }

    fn col_x(&self, col: Col) -> (f32, f32) {
        let mut x = 0.0;
        for (c, _, w) in self.columns() {
            if c == col {
                return (x, w);
            }
            x += w;
        }
        (x, 0.0)
    }

    fn move_cursor(&mut self, drow: i64, dcol: i64) {
        let max_row = self.shown.len() + 50;
        let at = self.row_of(self.cursor.0).unwrap_or(0);
        let row = self.task_at((at as i64 + drow).clamp(0, max_row as i64) as usize);
        let idx = COLUMNS.iter().position(|c| c.0 == self.cursor.1).unwrap_or(3) as i64;
        let idx = (idx + dcol).clamp(1, COLUMNS.len() as i64 - 2) as usize;
        self.cursor = (row, COLUMNS[idx].0);
        self.select_only(row);
    }

    fn keyboard(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>, was_editing: bool) {
        if self.tour_active() {
            return;
        }
        if !was_editing && self.editing.is_none() && self.help.panel.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.help.panel = None;
            return;
        }
        if was_editing || self.editing.is_some() || ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let (events, mods) = ctx.input(|i| (i.events.clone(), i.modifiers));
        for ev in events {
            // Usage has no active editable cell; never type into or delete a hidden grid row.
            if self.view == View::Usage && !matches!(&ev, egui::Event::Key {
                key: Key::S | Key::O | Key::N | Key::Z | Key::Y, modifiers, ..
            } if modifiers.ctrl) {
                continue;
            }
            match ev {
                // egui turns Ctrl+C, Ctrl+X and Ctrl+V into these events on most platforms.
                egui::Event::Copy => acts.push(Act::Copy),
                egui::Event::Cut => acts.push(Act::Cut),
                egui::Event::Paste(_) => acts.push(Act::Paste),
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
                    Key::C if modifiers.ctrl => acts.push(Act::Copy),
                    Key::X if modifiers.ctrl => acts.push(Act::Cut),
                    Key::V if modifiers.ctrl => acts.push(Act::Paste),
                    Key::S if modifiers.ctrl => acts.push(Act::Save),
                    Key::Z if modifiers.ctrl => acts.push(Act::Undo),
                    Key::Y if modifiers.ctrl => acts.push(Act::Redo),
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
            // The range runs over the rows on display (a filter or sort may reorder tasks).
            let (from, to) = (self.row_of(self.cursor.0).unwrap_or(0), self.row_of(row).unwrap_or(0));
            self.selected = (from.min(to)..=from.max(to)).map(|r| self.task_at(r)).collect();
        } else {
            self.select_only(row);
        }
        self.cursor = (row, if col == Col::Num || col == Col::Add { Col::Name } else { col });
    }

    fn grid(&mut self, ui: &mut egui::Ui, area: Rect) {
        let n = self.n();
        let head = Rect::from_min_max(area.min, Pos2::new(area.right(), area.top() + HEAD));
        let body = Rect::from_min_max(Pos2::new(area.left(), head.bottom()), area.max);
        let total_w: f32 = self.col_w.iter().sum();
        self.grid_x = self.grid_x.clamp(0.0, (total_w - area.width()).max(0.0));
        let left = area.left() - self.grid_x;
        let p = ui.painter_at(area);
        p.rect_filled(area, 0.0, Color32::WHITE);

        // Header.
        p.rect_filled(head, 0.0, HEADER_BG);
        let mut x = left;
        for (col, title, w) in self.columns() {
            let r = Rect::from_min_size(Pos2::new(x, head.top()), Vec2::new(w, HEAD));
            p.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0_f32, GRID_LINE));
            let color = if col == Col::Add { Color32::from_gray(110) } else { TEXT };
            // Header text keeps its own size (it has two lines at most in 40 px) and stays in its cell.
            let (title_lines, size) = lines(&p, title, FontId::proportional(self.text_size.min(12.0)), color);
            let clip = p.with_clip_rect(r.shrink(1.0).intersect(head));
            let center_x = if col == Col::Name { r.left() + 6.0 + size.x / 2.0 } else { r.center().x };
            draw_centered(&clip, title_lines, center_x, r.center().y - size.y / 2.0, color);
            if col == Col::Ind {
                icons::draw(&p, Rect::from_center_size(r.center(), Vec2::splat(14.0)), Icon::Info);
            }
            x += w;
        }
        p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0_f32, Color32::from_gray(170)));
        // The "?" in the corner opens the grid's Como usar.
        let (nx0, nw0) = self.col_x(Col::Num);
        let corner = Rect::from_min_size(Pos2::new(left + nx0, head.top()), Vec2::new(nw0, HEAD));
        if help::corner_button(ui, corner).clicked() {
            self.open_help("grade", 0);
        }
        for (col, id) in [(Col::Dur, "grade.duracao"), (Col::Pred, "grade.predecessoras")] {
            let (cx, cw) = self.col_x(col);
            let r = Rect::from_min_size(Pos2::new(left + cx, head.top()), Vec2::new(cw, HEAD));
            let r = r.intersect(area);
            if r.width() >= cw * 0.6 {
                self.target(ui, id, r);
            }
        }
        {
            let (cx, cw) = self.col_x(Col::Name);
            let y = body.top() + self.shown.len() as f32 * self.row_h - self.scroll_y;
            let r = Rect::from_min_size(Pos2::new(left + cx, y), Vec2::new(cw, self.row_h));
            if body.contains_rect(r) {
                self.target(ui, "grade.linha_vazia", r);
            }
        }

        // Rows: the tasks, then empty rows to type into.
        let visible = (body.height() / self.row_h).ceil() as usize + 1;
        let first = (self.scroll_y / self.row_h).floor() as usize;
        let pb = ui.painter_at(body);
        let mut toggles = Vec::new();
        for shown_row in first..first + visible {
            let row = self.task_at(shown_row);
            let y = body.top() + shown_row as f32 * self.row_h - self.scroll_y;
            let line = Rect::from_min_size(Pos2::new(left, y), Vec2::new(total_w, self.row_h));
            if self.selected.contains(&row) {
                pb.rect_filled(line, 0.0, SELECTED);
            }
            let mut x = left;
            for (col, _, w) in self.columns() {
                let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, self.row_h));
                x += w;
                if col == Col::Num {
                    pb.rect_filled(r, 0.0, if self.selected.contains(&row) { Color32::from_rgb(0xC3, 0xEE, 0xDF) } else { HEADER_BG });
                }
                pb.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0_f32, GRID_LINE));
                pb.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0_f32, GRID_LINE));
                if row >= n {
                    continue;
                }
                let t = &self.doc.project.tasks[row];
                let font = FontId::proportional(self.text_size);
                match col {
                    Col::Num => {
                        pb.text(r.center(), Align2::CENTER_CENTER, t.id.to_string(), font, TEXT);
                    }
                    Col::Ind => {
                        if t.progress.percent_complete >= 100 {
                            let c = Stroke::new(2.0_f32, EMERALD);
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
                            // ◢ when its subtasks show, ▷ when collapsed; a click toggles it.
                            let c = Pos2::new(tx + 4.0, r.center().y);
                            let shape = if self.display.collapsed.contains(&t.uid) {
                                vec![c + Vec2::new(-2.0, -4.0), c + Vec2::new(3.0, 0.0), c + Vec2::new(-2.0, 4.0)]
                            } else {
                                vec![c + Vec2::new(3.0, -3.0), c + Vec2::new(3.0, 3.0), c + Vec2::new(-3.0, 3.0)]
                            };
                            pb.add(egui::Shape::convex_polygon(shape, TEXT, Stroke::NONE));
                            toggles.push((Rect::from_center_size(c, Vec2::splat(12.0)).intersect(body), t.uid));
                            tx += 12.0;
                        }
                        let clip = pb.with_clip_rect(r.intersect(body));
                        let name_font = if t.summary { self::font(SEMIBOLD, self.text_size) } else { font };
                        let name = if self.outline_numbers { format!("{} {}", outline_number(&self.doc, row), t.name) } else { t.name.clone() };
                        cell_text(&clip, &name, name_font, tx, r, self.row_h, self.text_size);
                    }
                    Col::Dur | Col::Start | Col::Finish | Col::Pred | Col::Res => {
                        let clip = pb.with_clip_rect(r.shrink(1.0).intersect(body));
                        if matches!(col, Col::Pred | Col::Res) {
                            cell_text(&clip, &self.cell_text(row, col), font, r.left() + 4.0, r, self.row_h, self.text_size);
                        } else {
                            let align = if col == Col::Dur { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
                            let pos = if col == Col::Dur { Pos2::new(r.right() - 4.0, r.center().y) } else { Pos2::new(r.left() + 4.0, r.center().y) };
                            clip.text(pos, align, self.cell_text(row, col), font, TEXT);
                        }
                    }
                    Col::Add => {}
                }
            }
        }
        // The active cell: green frame, as in Project.
        let (cx, cw) = self.col_x(self.cursor.1);
        let cursor_row = self.row_of(self.cursor.0);
        let cy = body.top() + cursor_row.unwrap_or(0) as f32 * self.row_h - self.scroll_y;
        let cell = Rect::from_min_size(Pos2::new(left + cx, cy), Vec2::new(cw, self.row_h));
        if self.editing.is_none() && cursor_row.is_some() {
            pb.rect_stroke(cell.shrink(0.5), 0.0, Stroke::new(2.0_f32, GREEN));
        }

        // Clicks.
        let resp = ui.interact(body, ui.id().with("grid"), Sense::click());
        if let Some(pos) = resp.interact_pointer_pos() {
            // A summary's triangle expands or collapses it.
            if let Some(&(_, uid)) = toggles.iter().find(|(r, _)| resp.clicked() && r.contains(pos)) {
                self.display.toggle(uid);
            } else if resp.clicked() || resp.double_clicked() {
                let row = self.task_at(((pos.y - body.top() + self.scroll_y) / self.row_h).floor().max(0.0) as usize);
                let mut col = Col::Name;
                let mut x = left;
                for (c, _, w) in self.columns() {
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
                        let version = self.doc.begin();
                        let r = self.doc.set_manual(row, manual);
                        self.doc.end(version);
                        self.report(r);
                    } else {
                        self.begin_edit(row, col, None);
                    }
                }
            }
        }

        // Column borders in the header: drag to resize, double-click to fit the contents.
        let mut x = left;
        for (i, &(col, _, _)) in COLUMNS.iter().enumerate() {
            x += self.col_w[i];
            let handle = Rect::from_center_size(Pos2::new(x, head.center().y), Vec2::new(8.0, HEAD));
            if !area.contains(handle.center()) {
                continue;
            }
            let r = ui.interact(handle, ui.id().with(("column-width", i)), Sense::click_and_drag());
            if r.hovered() || r.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if r.dragged() {
                self.col_w[i] = (self.col_w[i] + r.drag_delta().x).clamp(16.0, 900.0);
            }
            if r.double_clicked() {
                self.col_w[i] = self.fit_width(ui, col);
            }
            if r.drag_stopped() || r.double_clicked() {
                self.save_layout();
            }
        }
        // Row borders in the row-number column: drag to change the row height.
        let (nx, nw) = self.col_x(Col::Num);
        for row in first..first + visible {
            let y = body.top() + (row + 1) as f32 * self.row_h - self.scroll_y;
            let handle = Rect::from_center_size(Pos2::new(left + nx + nw / 2.0, y), Vec2::new(nw, 6.0));
            if !body.contains(handle.center()) {
                continue;
            }
            let r = ui.interact(handle, ui.id().with(("row-height", row)), Sense::drag());
            if r.hovered() || r.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if r.dragged() {
                self.row_h = (self.row_h + r.drag_delta().y).clamp(self.text_size + 8.0, 160.0);
            }
            if r.drag_stopped() {
                self.save_layout();
            }
        }

        // The cell being edited.
        if let Some((erow, ecol)) = self.editing.as_ref().map(|e| (e.row, e.col)) {
            let (ex, ew) = self.col_x(ecol);
            let ey = body.top() + self.row_of(erow).unwrap_or(0) as f32 * self.row_h - self.scroll_y;
            let e = self.editing.as_mut().unwrap();
            let r = Rect::from_min_size(Pos2::new(left + ex, ey), Vec2::new(ew, self.row_h));
            ui.painter().rect_filled(r, 0.0, Color32::WHITE);
            ui.painter().rect_stroke(r, 0.0, Stroke::new(1.5_f32, GREEN));
            let mut out = None;
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(r.shrink2(Vec2::new(3.0, 1.0))), |ui| {
                out = Some(egui::TextEdit::singleline(&mut e.text).frame(false).font(FontId::proportional(self.text_size)).desired_width(ew - 6.0).show(ui));
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
        self.target(ui, "gantt", area);
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
        for row in self.selected.iter().filter_map(|&i| self.row_of(i)) {
            let y = body.top() + row as f32 * self.row_h - self.scroll_y;
            p.rect_filled(Rect::from_min_size(Pos2::new(body.left(), y), Vec2::new(body.width(), self.row_h)), 0.0, SELECTED.gamma_multiply(0.6));
        }
        // Tasks a filter or a collapsed summary hides have no row, and their links are not drawn.
        let mut mid = vec![None; n];
        for (row, &i) in self.shown.iter().enumerate() {
            mid[i] = Some(body.top() + row as f32 * self.row_h - self.scroll_y + self.row_h / 2.0);
        }
        gantt::links(&p, &scale, &self.doc, |i| mid[i], self.row_h);
        for &i in &self.shown {
            let m = mid[i].unwrap_or(f32::MIN);
            if m > body.top() - self.row_h && m < body.bottom() + self.row_h {
                gantt::bar(&p, &scale, &self.doc, i, m, self.gantt_opts);
            }
        }
        let resp = ui.interact(body, ui.id().with("chart"), Sense::click_and_drag());
        if let Some(edit) = gantt::interact(ui, body, &scale, &self.doc, &mut self.gantt_interaction, (self.row_h, self.scroll_y, &self.shown), &resp) {
            let version = self.doc.begin();
            let result = edit.and_then(|edit| self.doc.edit_gantt(edit));
            self.doc.end(version);
            self.report(result);
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let row = self.task_at(((pos.y - body.top() + self.scroll_y) / self.row_h).floor().max(0.0) as usize);
                let mods = ui.input(|i| i.modifiers);
                let col = self.cursor.1;
                self.row_click(row, col, mods);
            }
        }
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        if self.view == View::Usage {
            task_usage::show(ui, &self.doc, &mut self.usage_first);
            return;
        }
        let full = ui.max_rect();
        // The vertical view name on the left edge.
        let label = Rect::from_min_size(full.min, Vec2::new(24.0, full.height()));
        let p = ui.painter();
        p.rect_filled(label, 0.0, NAVY);
        let galley = p.layout_no_wrap(
            if self.view == View::Gantt { "GRÁFICO DE GANTT" } else { "PLANILHA DE TAREFAS" }.to_string(),
            FontId::proportional(11.0),
            MINT,
        );
        let pos = Pos2::new(label.center().x - galley.size().y / 2.0, label.top() + 60.0 + galley.size().x);
        p.add(egui::epaint::TextShape::new(pos, galley, MINT).with_angle(-std::f32::consts::FRAC_PI_2));
        let area = Rect::from_min_max(Pos2::new(label.right(), full.top()), full.max);

        self.refresh_shown();
        let content_h = (self.shown.len() + 30) as f32 * self.row_h;
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
            ui.painter().rect_filled(handle, 0.0, Color32::from_rgb(0xC9, 0xD6, 0xDD));
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
        // Scroll bars: horizontal under the grid and under the chart, vertical at the right edge.
        let (grid, chart) = (
            Rect::from_min_max(grid.min, Pos2::new(grid.right(), grid.bottom() - BAR)),
            chart.map(|c| Rect::from_min_max(c.min, Pos2::new(c.right() - BAR, c.bottom() - BAR))),
        );
        let right = chart.unwrap_or(grid);
        let grid = if chart.is_none() { Rect::from_min_max(grid.min, Pos2::new(grid.right() - BAR, grid.bottom())) } else { grid };
        self.grid(ui, grid);
        let grid_total: f32 = self.col_w.iter().sum();
        let track = Rect::from_min_max(Pos2::new(grid.left(), grid.bottom()), Pos2::new(grid.right(), grid.bottom() + BAR));
        scrollbar(ui, "grid-h", track, true, grid_total, grid.width(), &mut self.grid_x);
        if let Some(c) = chart {
            self.chart_w = c.width();
            self.chart(ui, c);
            let total = gantt::Scale::new(&self.doc, self.day, 0.0).width(&self.doc).max(c.width());
            let track = Rect::from_min_max(Pos2::new(c.left(), c.bottom()), Pos2::new(c.right(), c.bottom() + BAR));
            scrollbar(ui, "chart-h", track, true, total, c.width(), &mut self.gantt_x);
        }
        let track = Rect::from_min_max(Pos2::new(right.right(), right.top() + HEAD), Pos2::new(right.right() + BAR, right.bottom()));
        scrollbar(ui, "rows-v", track, false, content_h, right.height() - HEAD, &mut self.scroll_y);
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
                if small_button(ui, Icon::Usage, "Uso da Tarefa", true) {
                    acts.push(Act::View(View::Usage));
                }
                if small_button(ui, Icon::Gantt, "", true) {
                    acts.push(Act::View(View::Gantt));
                }
            });
        });
    }

    fn windows(&mut self, ctx: &egui::Context) {
        self.dialogs(ctx);
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

impl App {
    /// Snapshot mode: show each tab, let it settle, save the window as a PNG, then quit.
    fn snapshot_step(&mut self, ctx: &egui::Context) {
        const STEPS: [(Tab, &str); 8] = [
            (Tab::Task, "tarefa"),
            (Tab::Resource, "recurso"),
            (Tab::Report, "relatorio"),
            (Tab::Project, "projeto"),
            (Tab::View, "exibir"),
            (Tab::Help, "ajuda"),
            (Tab::Format, "formato"),
            (Tab::File, "arquivo"),
        ];
        let (Some((step, frames, asked)), Some(dir)) = (self.snap, snapshot::dir()) else { return };
        ctx.request_repaint();
        // After the tabs: the help panel, the Central de Ajuda and a tour step.
        const EXTRA: [&str; 4] = ["ajuda-painel", "ajuda-central", "ajuda-tour", "uso-tarefa"];
        let (tab, name) = match STEPS.get(step) {
            Some(&s) => s,
            None => match EXTRA.get(step - STEPS.len()) {
                Some(&name) => (Tab::Task, name),
                None => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            },
        };
        if frames == 0 {
            self.view = if name == "uso-tarefa" { View::Usage } else { View::Gantt };
            self.help.panel = None;
            self.help.catalog = false;
            self.help.tour = None;
            match name {
                "ajuda-painel" => self.open_help("tarefa", 0),
                "ajuda-central" => self.help.catalog = true,
                "ajuda-tour" => {
                    self.start_tour("grade");
                    self.tour_next();
                }
                _ => {}
            }
        }
        if self.help.tour.is_none() {
            self.tab = tab;
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let _ = std::fs::create_dir_all(&dir);
            let [w, h] = image.size;
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let _ = std::fs::write(dir.join(format!("{}-{name}.png", step + 1)), snapshot::png(w, h, &rgba));
            self.snap = Some((step + 1, 0, false));
        } else if frames > 15 && !asked {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            self.snap = Some((step, frames + 1, true));
        } else {
            self.snap = Some((step, frames + 1, asked));
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_ui(ctx);
    }
}

impl App {
    /// One frame of the whole interface (also driven by the user-simulation tests).
    fn frame_ui(&mut self, ctx: &egui::Context) {
        self.help_frame_start();
        self.snapshot_step(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && self.doc.dirty && !self.closing && self.snap.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Close);
        }
        self.save_changes_dialog(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));
        let mut acts = Vec::new();
        let was_editing = self.editing.is_some();
        egui::TopBottomPanel::top("tabs").exact_height(26.0).frame(egui::Frame::none().fill(GREEN)).show(ctx, |ui| self.tabs(ui));
        if self.tab == Tab::File {
            acts.extend(self.backstage(ctx));
        } else {
            egui::TopBottomPanel::top("ribbon")
                .exact_height(104.0)
                .frame(egui::Frame::none().fill(RIBBON_BG).stroke(Stroke::new(1.0_f32, Color32::from_gray(210))))
                .show(ctx, |ui| acts.extend(self.ribbon(ui)));
            if self.suggest {
                self.help_banner(ctx);
            }
            if self.timeline {
                egui::TopBottomPanel::top("timeline").exact_height(64.0).frame(egui::Frame::none()).show(ctx, |ui| self.timeline_strip(ui));
            }
            egui::TopBottomPanel::bottom("status")
                .exact_height(24.0)
                .frame(egui::Frame::none().fill(RIBBON_BG).inner_margin(egui::Margin::symmetric(8.0, 2.0)))
                .show(ctx, |ui| self.status_bar(ui, &mut acts));
            self.help_panel(ctx);
            egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::WHITE)).show(ctx, |ui| self.central(ui));
            self.windows(ctx);
            self.help_catalog(ctx);
            self.keyboard(ctx, &mut acts, was_editing);
        }
        self.help_tour(ctx);
        acts.append(&mut self.quick);
        for act in acts {
            self.run(act);
        }
    }
}
