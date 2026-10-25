//! The ribbon, tab by tab and group by group in the order Microsoft Project uses. Commands
//! Etapix does not have yet are shown greyed out (with a tooltip), so the layout stays the one the
//! team knows.

use eframe::egui;

use super::{big_button, column, group, small_button, small_button_response, soon, soon_big, Act, App, Report, Tab, View};
use crate::icons::Icon;
use crate::rows::{Filter, Sort};

impl App {
    pub(super) fn ribbon(&mut self, ui: &mut egui::Ui) -> Vec<Act> {
        let mut acts = Vec::new();
        let has = !self.targets().is_empty();
        let many = self.targets().len() >= 2;
        let avail = ui.available_width();
        // Wider than the window on small screens: it scrolls sideways (shift + wheel or the bar).
        egui::ScrollArea::horizontal().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded).show(ui, |ui| {
            ui.set_min_height(92.0);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.style_mut().text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(12.0));
            ui.style_mut().text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(12.0));
            let full = self.ribbon_widths.get(&self.tab).copied();
            let compact = full.is_some_and(|w| w > avail);
            super::COMPACT.with(|c| c.set(compact));
            let row = ui.horizontal_top(|ui| {
                ui.add_space(4.0);
                match self.tab {
                    Tab::File => {}
                    Tab::Task => self.task_tab(ui, &mut acts, has, many),
                    Tab::Resource => self.resource_tab(ui, &mut acts, has),
                    Tab::Report => {
                        for (id, rect) in report_tab(ui, &mut acts) {
                            self.target(ui, id, rect);
                        }
                    }
                    Tab::Project => {
                        for (id, rect) in project_tab(ui, &mut acts) {
                            self.target(ui, id, rect);
                        }
                    }
                    Tab::View => self.view_tab(ui, &mut acts, has),
                    Tab::Format => self.format_tab(ui),
                    Tab::Help => help_tab(ui, &mut acts),
                }
                if !matches!(self.tab, Tab::File | Tab::Help) {
                    let rect = group(ui, "Ajuda", |ui| {
                        if big_button(ui, Icon::Help, "Como\nusar", true) {
                            acts.push(Act::HowTo);
                        }
                    });
                    self.target(ui, "ajuda.como_usar", rect);
                }
            });
            super::COMPACT.with(|c| c.set(false));
            if !compact {
                self.ribbon_widths.insert(self.tab, row.response.rect.width());
                if row.response.rect.width() > avail {
                    ui.ctx().request_repaint();
                }
            }
        });
        acts
    }

    fn task_tab(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>, has: bool, many: bool) {
        group(ui, "Exibir", |ui| {
            if big_button(ui, Icon::Gantt, "Gráfico de\nGantt", true) {
                acts.push(Act::View(View::Gantt));
            }
        });
        group(ui, "Área de Transferência", |ui| {
            if big_button(ui, Icon::Paste, "Colar", !self.clipboard.is_empty()) {
                acts.push(Act::Paste);
            }
            column(ui, |ui| {
                if small_button(ui, Icon::Cut, "Recortar", has) {
                    acts.push(Act::Cut);
                }
                if small_button(ui, Icon::Copy, "Copiar", has) {
                    acts.push(Act::Copy);
                }
                soon(ui, Icon::Brush, "Pincel de Formatação");
            });
        });
        group(ui, "Fonte", |ui| {
            column(ui, |ui| {
                self.font_picker(ui);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for icon in [Icon::Bold, Icon::Italic, Icon::Underline] {
                        soon(ui, icon, "");
                    }
                });
            });
        });
        let rect = group(ui, "Cronograma", |ui| {
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
                    ui.spacing_mut().item_spacing.x = 0.0;
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
            column(ui, |ui| {
                if small_button(ui, Icon::OnTrack, "Marcar no Controle", has) {
                    acts.push(Act::Percent(100));
                }
                if small_button(ui, Icon::Respect, "Respeitar Vínculos", has) {
                    acts.push(Act::RespectLinks);
                }
                soon(ui, Icon::Inactive, "Inativar");
            });
        });
        self.target(ui, "tarefa.cronograma", rect);
        let rect = group(ui, "Tarefas", |ui| {
            if big_button(ui, Icon::Pin, "Agendar\nManualmente", has) {
                acts.push(Act::Manual(true));
            }
            if big_button(ui, Icon::Auto, "Agendamento\nAutomático", has) {
                acts.push(Act::Manual(false));
            }
            column(ui, |ui| {
                soon(ui, Icon::Inspect, "Inspecionar");
                let mover = small_button_response(ui, Icon::Move, "Mover ▾", has);
                let id = ui.make_persistent_id("mover");
                if has && mover.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(id));
                }
                egui::popup_below_widget(ui, id, &mover, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(170.0);
                    for (label, days) in [
                        ("1 Dia", 1),
                        ("1 Semana", 7),
                        ("2 Semanas", 14),
                        ("4 Semanas", 28),
                        ("1 Dia para trás", -1),
                        ("1 Semana para trás", -7),
                    ] {
                        if ui.button(label).clicked() {
                            acts.push(Act::Move(days));
                        }
                    }
                });
                soon(ui, Icon::Mode, "Modo");
            });
        });
        self.target(ui, "tarefa.tarefas", rect);
        let rect = group(ui, "Inserir", |ui| {
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
                soon(ui, Icon::Deliverable, "Entregável");
            });
        });
        self.target(ui, "tarefa.inserir", rect);
        let rect = group(ui, "Propriedades", |ui| {
            if big_button(ui, Icon::Info, "Informações", has) {
                acts.push(Act::Info);
            }
            column(ui, |ui| {
                soon(ui, Icon::Notes, "Anotações");
                soon(ui, Icon::Details, "Detalhes");
                if small_button(ui, Icon::TimelineAdd, "Adicionar à Linha do Tempo", has) {
                    acts.push(Act::AddTimeline);
                }
            });
        });
        self.target(ui, "tarefa.propriedades", rect);
        group(ui, "Editar", |ui| {
            if big_button(ui, Icon::Scroll, "Rolar até\na Tarefa", has) {
                acts.push(Act::ScrollTo);
            }
            column(ui, |ui| {
                if small_button(ui, Icon::Find, "Localizar", true) {
                    acts.push(Act::Find);
                }
                if small_button(ui, Icon::Eraser, "Limpar", has) {
                    acts.push(Act::Delete);
                }
                soon(ui, Icon::Fill, "Preencher");
            });
        });
    }

    fn resource_tab(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>, has: bool) {
        group(ui, "Exibir", |ui| {
            soon_big(ui, Icon::Team, "Organizador\nde Equipe");
        });
        let rect = group(ui, "Atribuições", |ui| {
            if big_button(ui, Icon::Assign, "Atribuir\nRecursos", has) {
                acts.push(Act::Assign);
            }
            soon_big(ui, Icon::Pool, "Pool de\nRecursos");
        });
        self.target(ui, "recurso.atribuicoes", rect);
        let rect = group(ui, "Inserir", |ui| {
            if big_button(ui, Icon::AddResource, "Adicionar\nRecursos", true) {
                acts.push(Act::AddResource);
            }
        });
        self.target(ui, "recurso.inserir", rect);
        group(ui, "Propriedades", |ui| {
            soon_big(ui, Icon::Info, "Informações");
            column(ui, |ui| {
                soon(ui, Icon::Notes, "Anotações");
                soon(ui, Icon::Details, "Detalhes");
            });
        });
        let rect = group(ui, "Nível", |ui| {
            if big_button(ui, Icon::Level, "Nivelar\nSeleção", has) {
                acts.push(Act::Level);
            }
            if big_button(ui, Icon::Level, "Nivelar\nRecurso", true) {
                acts.push(Act::Level);
            }
            if big_button(ui, Icon::Level, "Nivelar\nTudo", true) {
                acts.push(Act::Level);
            }
            column(ui, |ui| {
                soon(ui, Icon::Options, "Opções de Nivelamento");
                if small_button(ui, Icon::Clear, "Limpar Nivelamento", true) {
                    acts.push(Act::ClearLevel);
                }
                soon(ui, Icon::NextOver, "Próxima Superalocação");
            });
        });
        self.target(ui, "recurso.nivel", rect);
    }

    fn view_tab(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>, has: bool) {
        group(ui, "Modos de Exibição de Tarefa", |ui| {
            if big_button(ui, Icon::Gantt, "Gráfico de\nGantt", true) {
                acts.push(Act::View(View::Gantt));
            }
            soon_big(ui, Icon::Usage, "Uso da\nTarefa");
            column(ui, |ui| {
                soon(ui, Icon::Network, "Diagrama de Rede");
                soon(ui, Icon::Month, "Calendário");
                if small_button(ui, Icon::Sheet, "Planilha de Tarefas", true) {
                    acts.push(Act::View(View::Sheet));
                }
            });
        });
        group(ui, "Modos de Exibição de Recurso", |ui| {
            soon_big(ui, Icon::Team, "Organizador\nde Equipe");
            soon_big(ui, Icon::Usage, "Uso do\nRecurso");
            column(ui, |ui| {
                if small_button(ui, Icon::Sheet, "Planilha de Recursos", true) {
                    acts.push(Act::Report(Report::Resources));
                }
                soon(ui, Icon::Options, "Outros Modos de Exibição");
            });
        });
        group(ui, "Dados", |ui| {
            column(ui, |ui| {
                self.menu_button(ui, Icon::Sort, "Classificar ▾", "classificar", |app, ui| {
                    for sort in Sort::ALL {
                        if ui.selectable_label(app.display.sort == sort, sort.label()).clicked() {
                            app.display.sort = sort;
                        }
                    }
                });
                self.menu_button(ui, Icon::Outline, "Estrutura de Tópicos ▾", "estrutura", |app, ui| {
                    if ui.button("Todas as Subtarefas").clicked() {
                        app.display.show_level(&app.doc, None);
                    }
                    for level in 1..=9 {
                        if ui.button(format!("Nível {level}")).clicked() {
                            app.display.show_level(&app.doc, Some(level));
                        }
                    }
                });
                soon(ui, Icon::Tables, "Tabelas");
            });
            column(ui, |ui| {
                soon(ui, Icon::Highlight, "[Sem Realce]");
                let label = format!("{} ▾", self.display.filter.label());
                self.menu_button(ui, Icon::Filter, &label, "filtro", |app, ui| {
                    for filter in Filter::ALL {
                        if ui.selectable_label(app.display.filter == filter, filter.label()).clicked() {
                            app.display.filter = filter;
                        }
                    }
                });
                soon(ui, Icon::Group, "[Sem Grupo]");
            });
        });
        let rect = group(ui, "Zoom", |ui| {
            column(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Escala de Tempo:");
                    let current = if self.day >= 40.0 { "Horas" } else if self.day >= 10.0 { "Dias" } else if self.day >= 3.0 { "Semanas" } else { "Meses" };
                    egui::ComboBox::from_id_salt("timescale").selected_text(current).width(90.0).show_ui(ui, |ui| {
                        for (label, day) in [("Horas", 80.0), ("Dias", 22.0), ("Semanas", 6.0), ("Meses", 2.0)] {
                            if ui.selectable_label(current == label, label).clicked() {
                                acts.push(Act::Timescale(day));
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if small_button(ui, Icon::ZoomIn, "Ampliar", true) {
                        acts.push(Act::ZoomIn);
                    }
                    if small_button(ui, Icon::ZoomOut, "Reduzir", true) {
                        acts.push(Act::ZoomOut);
                    }
                });
            });
            if big_button(ui, Icon::Whole, "Projeto\nInteiro", true) {
                acts.push(Act::Whole);
            }
            if big_button(ui, Icon::Find, "Tarefas\nSelecionadas", has) {
                acts.push(Act::ZoomSelected);
            }
        });
        self.target(ui, "exibir.zoom", rect);
        let rect = group(ui, "Modo Divisão", |ui| {
            column(ui, |ui| {
                let mut on = self.timeline;
                if ui.checkbox(&mut on, "Linha do Tempo").changed() {
                    acts.push(Act::Timeline);
                }
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Detalhes"));
            });
        });
        self.target(ui, "exibir.modo_divisao", rect);
        group(ui, "Janela", |ui| {
            soon_big(ui, Icon::NewWindow, "Nova\nJanela");
            column(ui, |ui| {
                soon(ui, Icon::Switch, "Alternar Janelas");
                soon(ui, Icon::Arrange, "Organizar Tudo");
                soon(ui, Icon::Hide, "Ocultar");
            });
        });
        group(ui, "Macros", |ui| {
            soon_big(ui, Icon::Macro, "Macros");
        });
    }

    fn format_tab(&mut self, ui: &mut egui::Ui) {
        group(ui, "Formatar", |ui| {
            soon_big(ui, Icon::Text, "Estilos de\nTexto");
            column(ui, |ui| {
                soon(ui, Icon::Grid, "Linhas de Grade");
                soon(ui, Icon::Layout, "Layout");
            });
        });
        group(ui, "Colunas", |ui| {
            soon_big(ui, Icon::Column, "Inserir\nColuna");
            column(ui, |ui| {
                soon(ui, Icon::Options, "Configurações de Coluna");
                soon(ui, Icon::Fields, "Campos Personalizados");
            });
        });
        let rect = group(ui, "Estilos de Barra", |ui| {
            soon_big(ui, Icon::Format, "Formatar");
            column(ui, |ui| {
                ui.checkbox(&mut self.gantt_opts.critical, "Tarefas Críticas");
                ui.checkbox(&mut self.gantt_opts.slack, "Margem de Atraso");
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Tarefas Atrasadas"));
            });
            column(ui, |ui| {
                ui.checkbox(&mut self.gantt_opts.baseline, "Linha de Base");
            });
        });
        self.target(ui, "formato.estilos_barra", rect);
        group(ui, "Mostrar/Ocultar", |ui| {
            column(ui, |ui| {
                ui.checkbox(&mut self.outline_numbers, "Número da Estrutura de Tópicos");
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Tarefa de Resumo do Projeto"));
                ui.add_enabled(false, egui::Checkbox::new(&mut true, "Tarefas de Resumo"));
            });
        });
        group(ui, "Desenhos", |ui| {
            soon_big(ui, Icon::Drawing, "Desenho");
        });
    }
}

fn report_tab(ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Vec<(&'static str, egui::Rect)> {
    let reports = group(ui, "Exibir Relatórios", |ui| {
        for (icon, label, report) in [
            (Icon::Dashboard, "Painéis", Report::Dashboard),
            (Icon::Team, "Recursos", Report::Resources),
            (Icon::Cost, "Custos", Report::Costs),
            (Icon::InProgress, "Em\nAndamento", Report::InProgress),
        ] {
            if big_button(ui, icon, label, true) {
                acts.push(Act::Report(report));
            }
        }
        soon_big(ui, Icon::Intro, "Introdução");
        soon_big(ui, Icon::Custom, "Personalizado");
        soon_big(ui, Icon::Recent, "Relatórios\nRecentes");
    });
    group(ui, "Projeto", |ui| {
        soon_big(ui, Icon::Compare, "Comparar\nProjetos");
    });
    group(ui, "Exportar", |ui| {
        soon_big(ui, Icon::Export, "Relatórios\nVisuais");
    });
    vec![("relatorio.exibir", reports)]
}

fn project_tab(ui: &mut egui::Ui, acts: &mut Vec<Act>) -> Vec<(&'static str, egui::Rect)> {
    group(ui, "Inserir", |ui| {
        soon_big(ui, Icon::Subproject, "Subprojeto");
    });
    group(ui, "Suplementos", |ui| {
        soon_big(ui, Icon::AddIn, "Obter\nSuplementos");
    });
    let properties = group(ui, "Propriedades", |ui| {
        if big_button(ui, Icon::Info, "Informações\ndo Projeto", true) {
            acts.push(Act::ProjectInfo);
        }
        soon_big(ui, Icon::Fields, "Campos\nPersonalizados");
        soon_big(ui, Icon::Links, "Vínculos Entre\nProjetos");
        column(ui, |ui| {
            soon(ui, Icon::Wbs, "EAP");
            soon(ui, Icon::WorkTime, "Alterar Período de Trabalho");
        });
    });
    let schedule = group(ui, "Cronograma", |ui| {
        if big_button(ui, Icon::Calc, "Calcular\nProjeto", true) {
            acts.push(Act::Calc);
        }
        if big_button(ui, Icon::Baseline, "Definir Linha\nde Base", true) {
            acts.push(Act::Baseline);
        }
        if big_button(ui, Icon::MoveProject, "Mover\nProjeto", true) {
            acts.push(Act::MoveProject);
        }
    });
    group(ui, "Status", |ui| {
        column(ui, |ui| {
            soon(ui, Icon::StatusDate, "Data de Status: NA");
            soon(ui, Icon::Update, "Atualizar Projeto");
        });
    });
    group(ui, "Revisão", |ui| {
        soon_big(ui, Icon::Spelling, "Ortografia");
    });
    vec![("projeto.propriedades", properties), ("projeto.cronograma", schedule)]
}

fn help_tab(ui: &mut egui::Ui, acts: &mut Vec<Act>) {
    group(ui, "Ajuda", |ui| {
        if big_button(ui, Icon::Dashboard, "Central de\nAjuda", true) {
            acts.push(Act::HelpCenter);
        }
        if big_button(ui, Icon::Help, "Como usar\na Grade", true) {
            acts.push(Act::HowTo);
        }
        if big_button(ui, Icon::Gantt, "Fazer o\nTour", true) {
            acts.push(Act::Tour("grade"));
        }
    });
    group(ui, "Sobre", |ui| {
        if big_button(ui, Icon::Intro, "Sobre o\nEtapix", true) {
            acts.push(Act::About);
        }
    });
}

impl App {
    /// A small ribbon button that opens a menu below it (`label` ends in " ▾").
    fn menu_button(&mut self, ui: &mut egui::Ui, icon: Icon, label: &str, id: &str, items: impl FnOnce(&mut App, &mut egui::Ui)) {
        let button = small_button_response(ui, icon, label, true);
        let id = ui.make_persistent_id(id);
        if button.clicked() {
            ui.memory_mut(|m| m.toggle_popup(id));
        }
        egui::popup_below_widget(ui, id, &button, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(190.0);
            items(self, ui);
        });
    }
}
