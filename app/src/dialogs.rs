//! Windows opened from the ribbon: Localizar, Atribuir Recursos, Adicionar Recursos, the reports,
//! Mover Projeto and Sobre.

use eframe::egui::{self, Color32, RichText};

use super::{App, Report};
use crate::fmt;

const MUTED: Color32 = Color32::from_rgb(100, 110, 120);

/// A two-column table of label/value rows.
fn table(ui: &mut egui::Ui, id: &str, header: &[&str], rows: Vec<Vec<String>>) {
    egui::ScrollArea::vertical().max_height(420.0).id_salt(id).show(ui, |ui| {
        egui::Grid::new(id).striped(true).num_columns(header.len()).spacing([18.0, 4.0]).show(ui, |ui| {
            for h in header {
                ui.label(RichText::new(*h).strong());
            }
            ui.end_row();
            for row in rows {
                for cell in row {
                    ui.label(cell);
                }
                ui.end_row();
            }
        });
    });
}

fn money(v: f64) -> String {
    let s = format!("{v:.2}").replace('.', ",");
    format!("R$ {s}")
}

impl App {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.find_window(ctx);
        self.assign_window(ctx);
        self.add_resource_window(ctx);
        self.report_window(ctx);
        self.move_project_window(ctx);
        if self.about {
            let mut open = true;
            egui::Window::new("Sobre o Etapix").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(RichText::new(format!("Etapix {}", env!("CARGO_PKG_VERSION"))).strong().size(16.0));
                ui.label("Planejador de projetos compatível com o Microsoft Project.");
                ui.label(RichText::new("Fontes Inter e Plus Jakarta Sans (SIL Open Font License).").color(MUTED));
                ui.hyperlink_to("github.com/Pontesrrafael/etapix", "https://github.com/Pontesrrafael/etapix");
            });
            self.about = open;
        }
    }

    fn find_window(&mut self, ctx: &egui::Context) {
        let Some(mut text) = self.find.take() else { return };
        let mut open = true;
        let mut go = None;
        egui::Window::new("Localizar").open(&mut open).collapsible(false).default_width(380.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Localizar:");
                ui.text_edit_singleline(&mut text).request_focus();
            });
            let needle = text.trim().to_lowercase();
            if needle.is_empty() {
                return;
            }
            ui.separator();
            let hits: Vec<usize> = (0..self.n()).filter(|&i| self.doc.project.tasks[i].name.to_lowercase().contains(&needle)).collect();
            if hits.is_empty() {
                ui.label(RichText::new("Nenhuma tarefa com esse texto.").color(MUTED));
            }
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for i in hits {
                    let t = &self.doc.project.tasks[i];
                    if ui.selectable_label(self.selected.contains(&i), format!("{}  {}", t.id, t.name)).clicked() {
                        go = Some(i);
                    }
                }
            });
        });
        if let Some(i) = go {
            self.select_only(i);
            self.cursor.0 = i;
            self.scroll_y = (i as f32 * self.row_h - 3.0 * self.row_h).max(0.0);
        }
        if open {
            self.find = Some(text);
        }
    }

    fn assign_window(&mut self, ctx: &egui::Context) {
        if !self.assign {
            return;
        }
        let rows = self.targets();
        let mut open = true;
        let mut wanted: Option<String> = None;
        egui::Window::new("Atribuir Recursos").open(&mut open).collapsible(false).default_width(320.0).show(ctx, |ui| {
            let Some(&row) = rows.first() else {
                ui.label("Selecione uma tarefa.");
                return;
            };
            let t = &self.doc.project.tasks[row];
            ui.label(RichText::new(format!("Tarefa: {}", t.name)).strong());
            if t.summary {
                ui.label(RichText::new("Tarefas de resumo não recebem recursos.").color(MUTED));
                return;
            }
            ui.separator();
            let current: Vec<String> = self.doc.resources_text(row).split(", ").filter(|s| !s.is_empty()).map(String::from).collect();
            let names: Vec<String> = self.doc.project.resources.iter().map(|r| r.name.clone()).collect();
            if names.is_empty() {
                ui.label(RichText::new("O projeto ainda não tem recursos: use Recurso > Adicionar Recursos.").color(MUTED));
            }
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for name in &names {
                    let mut on = current.contains(name);
                    if ui.checkbox(&mut on, name).changed() {
                        let mut next: Vec<String> = current.iter().filter(|n| *n != name).cloned().collect();
                        if on {
                            next.push(name.clone());
                        }
                        wanted = Some(next.join(", "));
                    }
                }
            });
        });
        if let (Some(names), Some(&row)) = (wanted, rows.first()) {
            let r = self.doc.set_resources(row, &names);
            self.report(r);
        }
        self.assign = open;
    }

    fn add_resource_window(&mut self, ctx: &egui::Context) {
        let Some(mut name) = self.add_resource.take() else { return };
        let mut keep = true;
        egui::Window::new("Adicionar Recursos").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Nome do recurso:");
                ui.text_edit_singleline(&mut name);
            });
            ui.label(RichText::new("Recurso de trabalho, 100% de unidades máximas.").color(MUTED));
            ui.horizontal(|ui| {
                if ui.button("Adicionar").clicked() && !name.trim().is_empty() {
                    let r = self.doc.add_resource(name.trim());
                    self.report(r);
                    name.clear();
                }
                if ui.button("Fechar").clicked() {
                    keep = false;
                }
            });
            if !self.doc.project.resources.is_empty() {
                ui.separator();
                ui.label(RichText::new("Recursos do projeto:").color(MUTED));
                for r in &self.doc.project.resources {
                    ui.label(format!("• {}", r.name));
                }
            }
        });
        if keep {
            self.add_resource = Some(name);
        }
    }

    fn report_window(&mut self, ctx: &egui::Context) {
        let Some(report) = self.report else { return };
        if report == Report::Dashboard {
            self.overview = true;
            self.report = None;
            return;
        }
        let doc = &self.doc;
        let mut open = true;
        let title = match report {
            Report::Resources => "Relatório: Recursos",
            Report::Costs => "Relatório: Custos",
            _ => "Relatório: Em Andamento",
        };
        egui::Window::new(title).open(&mut open).collapsible(false).default_width(560.0).show(ctx, |ui| match report {
            Report::Resources => {
                let rows = doc
                    .project
                    .resources
                    .iter()
                    .enumerate()
                    .map(|(k, r)| {
                        let tasks = doc.project.assignments.iter().filter(|a| a.resource == k).count();
                        vec![r.name.clone(), format!("{:.0}%", r.max_units * 100.0), tasks.to_string(), money(doc.costs.resource[k])]
                    })
                    .collect();
                table(ui, "resources", &["Recurso", "Unid. máximas", "Tarefas", "Custo"], rows);
            }
            Report::Costs => {
                let rows: Vec<Vec<String>> = (0..doc.project.tasks.len())
                    .filter(|&i| doc.costs.task[i] != 0.0)
                    .map(|i| vec![doc.project.tasks[i].id.to_string(), doc.project.tasks[i].name.clone(), money(doc.costs.task[i])])
                    .collect();
                let total: f64 = (0..doc.project.tasks.len()).filter(|&i| doc.project.tasks[i].parent.is_none()).map(|i| doc.costs.task[i]).sum();
                ui.label(RichText::new(format!("Custo total do projeto: {}", money(total))).strong());
                if rows.is_empty() {
                    ui.label(RichText::new("Nenhuma tarefa tem custo (atribua recursos com taxas ou custos fixos).").color(MUTED));
                } else {
                    table(ui, "costs", &["Id", "Tarefa", "Custo"], rows);
                }
            }
            _ => {
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
                if rows.is_empty() {
                    ui.label(RichText::new("Nenhuma tarefa em andamento ou atrasada.").color(MUTED));
                } else {
                    table(ui, "progress", &["Id", "Tarefa", "% concluída", "Término", "Situação"], rows);
                }
            }
        });
        if !open {
            self.report = None;
        }
    }

    fn move_project_window(&mut self, ctx: &egui::Context) {
        let Some(mut text) = self.move_project.take() else { return };
        let mut keep = true;
        egui::Window::new("Mover Projeto").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(format!("Início atual: {}", fmt::date(self.doc.project.start)));
            ui.horizontal(|ui| {
                ui.label("Novo início:");
                ui.text_edit_singleline(&mut text);
            });
            ui.label(RichText::new("As tarefas automáticas acompanham o novo início.").color(MUTED));
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
            self.move_project = Some(text);
        }
    }
}
