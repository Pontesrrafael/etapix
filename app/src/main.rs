//! pjapp: a desktop project planner for internal use. It opens and saves Microsoft Project XML
//! (MSPDI), so files go back and forth with an installed Project.

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod doc;
mod gantt;

use doc::Doc;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 760.0]).with_title("pjapp"),
        ..Default::default()
    };
    let path = std::env::args().nth(1);
    eframe::run_native("pjapp", options, Box::new(move |_| Ok(Box::new(App::new(path)))))
}

/// Text being edited in a cell, kept until the user leaves the field.
#[derive(Default)]
struct Editing {
    row: usize,
    column: &'static str,
    text: String,
}

struct App {
    doc: Doc,
    message: Option<String>,
    editing: Option<Editing>,
    selected: Option<usize>,
    day_width: f32,
}

const ROW: f32 = 24.0;
const COLUMNS: [(&str, &str, f32); 9] = [
    ("id", "Id", 34.0),
    ("name", "Nome da tarefa", 220.0),
    ("dur", "Duração (d)", 80.0),
    ("start", "Início", 104.0),
    ("finish", "Término", 104.0),
    ("pred", "Predecessoras", 92.0),
    ("res", "Recursos", 110.0),
    ("pct", "% concl.", 64.0),
    ("cost", "Custo", 90.0),
];

impl App {
    fn new(path: Option<String>) -> Self {
        let (doc, message) = match path.map(|p| Doc::open(std::path::Path::new(&p))) {
            Some(Ok(d)) => (d, None),
            Some(Err(e)) => (Doc::blank(), Some(e)),
            None => (Doc::blank(), None),
        };
        App { doc, message, editing: None, selected: None, day_width: 28.0 }
    }

    fn report(&mut self, r: Result<(), String>) {
        if let Err(e) = r {
            self.message = Some(e);
        }
    }

    fn title(&self) -> String {
        let name = self.doc.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string());
        format!("{}{} — pjapp", name.unwrap_or_else(|| "Projeto novo".into()), if self.doc.dirty { " *" } else { "" })
    }

    fn open(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Project XML", &["xml"]).pick_file() {
            match Doc::open(&path) {
                Ok(d) => {
                    self.doc = d;
                    self.selected = None;
                }
                Err(e) => self.message = Some(e),
            }
        }
    }

    fn save(&mut self, ask: bool) {
        let path = match (&self.doc.path, ask) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new().add_filter("Project XML", &["xml"]).set_file_name("projeto.xml").save_file(),
        };
        if let Some(p) = path {
            let r = self.doc.save(&p);
            self.report(r);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Novo").clicked() {
                self.doc = Doc::blank();
                self.selected = None;
            }
            if ui.button("Abrir…").clicked() {
                self.open();
            }
            if ui.button("Salvar").clicked() {
                self.save(false);
            }
            if ui.button("Salvar como…").clicked() {
                self.save(true);
            }
            ui.separator();
            if ui.button("+ Tarefa").clicked() {
                let n = self.doc.project.tasks.len() + 1;
                match self.doc.add_task(&format!("Tarefa {n}"), 1.0) {
                    Ok(i) => self.selected = Some(i),
                    Err(e) => self.message = Some(e),
                }
            }
            let has_selection = self.selected.is_some();
            if ui.add_enabled(has_selection, egui::Button::new("Excluir")).clicked() {
                if let Some(i) = self.selected.take() {
                    let r = self.doc.delete_task(i);
                    self.report(r);
                }
            }
            ui.separator();
            if ui.button("Nivelar recursos").clicked() {
                let r = self.doc.level();
                self.report(r);
            }
            if ui.button("Limpar nivelamento").clicked() {
                let r = self.doc.clear_leveling();
                self.report(r);
            }
            if ui.button("Salvar linha de base").clicked() {
                let r = self.doc.save_baseline();
                self.report(r);
            }
            ui.separator();
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.day_width, 8.0..=80.0).show_value(false));
        });
    }

    /// One editable cell; commits when focus leaves it.
    fn cell(&mut self, ui: &mut egui::Ui, row: usize, column: &'static str, width: f32, shown: String) {
        let editing = self.editing.as_ref().is_some_and(|e| e.row == row && e.column == column);
        if editing {
            let e = self.editing.as_mut().unwrap();
            let r = ui.add_sized([width, ROW - 4.0], egui::TextEdit::singleline(&mut e.text));
            if r.lost_focus() {
                let e = self.editing.take().unwrap();
                self.commit(e);
            } else {
                r.request_focus();
            }
        } else {
            let r = ui.add_sized([width, ROW - 4.0], egui::Label::new(shown.clone()).truncate().sense(egui::Sense::click()));
            if r.clicked() {
                self.selected = Some(row);
            }
            if r.double_clicked() {
                self.editing = Some(Editing { row, column, text: shown });
            }
        }
    }

    fn commit(&mut self, e: Editing) {
        let text = e.text.trim();
        let r = match e.column {
            "name" => {
                self.doc.rename(e.row, text);
                Ok(())
            }
            "dur" => match text.trim_end_matches(['d', 'D']).trim().replace(',', ".").parse::<f64>() {
                Ok(d) => self.doc.set_duration_days(e.row, d),
                Err(_) => Err(format!("duração inválida: {text}")),
            },
            "pred" => self.doc.set_predecessors(e.row, text),
            "pct" => match text.trim_end_matches('%').trim().parse::<u32>() {
                Ok(p) => self.doc.set_percent(e.row, p),
                Err(_) => Err(format!("percentual inválido: {text}")),
            },
            _ => Ok(()),
        };
        self.report(r);
    }

    fn table_and_gantt(&mut self, ui: &mut egui::Ui) {
        let span = gantt::Span::of(&self.doc);
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            // Header.
            ui.horizontal(|ui| {
                for (_, title, w) in COLUMNS {
                    ui.add_sized([w, ROW], egui::Label::new(egui::RichText::new(title).strong()));
                }
                gantt::header(ui, &span, self.day_width);
            });
            ui.separator();
            for i in 0..self.doc.project.tasks.len() {
                let selected = self.selected == Some(i);
                let frame = egui::Frame::none().fill(if selected {
                    ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                } else {
                    egui::Color32::TRANSPARENT
                });
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let t = &self.doc.project.tasks[i];
                        let s = &self.doc.schedule[i];
                        let summary = t.summary;
                        let indent = {
                            let (mut l, mut up) = (0, t.parent);
                            while let Some(p) = up {
                                l += 1;
                                up = self.doc.project.tasks[p].parent;
                            }
                            l
                        };
                        let values = [
                            t.id.to_string(),
                            format!("{}{}", "   ".repeat(indent), t.name),
                            format!("{:.2}", self.doc.days(i)).trim_end_matches('0').trim_end_matches('.').to_string(),
                            self.doc.dt(s.start).format("%d/%m/%y %H:%M").to_string(),
                            self.doc.dt(s.finish).format("%d/%m/%y %H:%M").to_string(),
                            self.doc.predecessors_text(i),
                            self.doc.resources_text(i),
                            format!("{}%", t.progress.percent_complete),
                            format!("R$ {:.2}", self.doc.costs.task[i]),
                        ];
                        let editable = ["name", "dur", "pred", "pct"];
                        for ((key, _, w), value) in COLUMNS.iter().zip(values) {
                            if editable.contains(key) && !(summary && *key != "name") {
                                self.cell(ui, i, key, *w, value);
                            } else {
                                let r = ui.add_sized([*w, ROW - 4.0], egui::Label::new(value).truncate().sense(egui::Sense::click()));
                                if r.clicked() {
                                    self.selected = Some(i);
                                }
                            }
                        }
                        gantt::row(ui, &self.doc, &span, i, self.day_width, ROW);
                    });
                });
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| self.toolbar(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let p = &self.doc.project;
                let finish = self.doc.schedule.iter().map(|s| s.finish).max();
                let total: f64 = (0..p.tasks.len()).filter(|&i| p.tasks[i].parent.is_none()).map(|i| self.doc.costs.task[i]).sum();
                ui.label(format!(
                    "{} tarefas · início {} · término {} · custo R$ {:.2}",
                    p.tasks.len(),
                    p.start.format("%d/%m/%Y"),
                    finish.map(|f| self.doc.dt(f).format("%d/%m/%Y").to_string()).unwrap_or_else(|| "-".into()),
                    total
                ));
                if let Some(m) = &self.message {
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(200, 60, 60), m);
                    if ui.small_button("ok").clicked() {
                        self.message = None;
                    }
                }
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| self.table_and_gantt(ui));
    }
}
