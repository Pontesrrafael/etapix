//! The ribbon, tab by tab and group by group in the order Microsoft Project uses. Commands
//! Etapix does not have yet are shown greyed out (with a tooltip), so the layout stays the one the
//! team knows.

use eframe::egui;

use super::{big_button, column, group, small_button, small_button_response, Act, App, Report, Tab, View};
use crate::icons::Icon;

/// A button that is not implemented yet: drawn disabled.
const OFF: bool = false;

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
                    Tab::Report => report_tab(ui, &mut acts),
                    Tab::Project => project_tab(ui, &mut acts),
                    Tab::View => self.view_tab(ui, &mut acts, has),
                    Tab::Format => self.format_tab(ui),
                    Tab::Help => help_tab(ui, &mut acts),
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
                small_button(ui, Icon::Brush, "Pincel de Formatação", OFF);
            });
        });
        group(ui, "Fonte", |ui| {
            column(ui, |ui| {
                self.font_picker(ui);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for icon in [Icon::Bold, Icon::Italic, Icon::Underline] {
                        small_button(ui, icon, "", OFF);
                    }
                });
            });
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
                small_button(ui, Icon::Inactive, "Inativar", OFF);
            });
        });
        group(ui, "Tarefas", |ui| {
            if big_button(ui, Icon::Pin, "Agendar\nManualmente", has) {
                acts.push(Act::Manual(true));
            }
            if big_button(ui, Icon::Auto, "Agendamento\nAutomático", has) {
                acts.push(Act::Manual(false));
            }
            column(ui, |ui| {
                small_button(ui, Icon::Inspect, "Inspecionar", OFF);
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
                small_button(ui, Icon::Mode, "Modo", OFF);
            });
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
                small_button(ui, Icon::Deliverable, "Entregável", OFF);
            });
        });
        group(ui, "Propriedades", |ui| {
            if big_button(ui, Icon::Info, "Informações", has) {
                acts.push(Act::Info);
            }
            column(ui, |ui| {
                small_button(ui, Icon::Notes, "Anotações", OFF);
                small_button(ui, Icon::Details, "Detalhes", OFF);
                if small_button(ui, Icon::TimelineAdd, "Adicionar à Linha do Tempo", has) {
                    acts.push(Act::AddTimeline);
                }
            });
        });
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
                small_button(ui, Icon::Fill, "Preencher", OFF);
            });
        });
    }

    fn resource_tab(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>, has: bool) {
        group(ui, "Exibir", |ui| {
            big_button(ui, Icon::Team, "Organizador\nde Equipe", OFF);
        });
        group(ui, "Atribuições", |ui| {
            if big_button(ui, Icon::Assign, "Atribuir\nRecursos", has) {
                acts.push(Act::Assign);
            }
            big_button(ui, Icon::Pool, "Pool de\nRecursos", OFF);
        });
        group(ui, "Inserir", |ui| {
            if big_button(ui, Icon::AddResource, "Adicionar\nRecursos", true) {
                acts.push(Act::AddResource);
            }
        });
        group(ui, "Propriedades", |ui| {
            big_button(ui, Icon::Info, "Informações", OFF);
            column(ui, |ui| {
                small_button(ui, Icon::Notes, "Anotações", OFF);
                small_button(ui, Icon::Details, "Detalhes", OFF);
            });
        });
        group(ui, "Nível", |ui| {
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
                small_button(ui, Icon::Options, "Opções de Nivelamento", OFF);
                if small_button(ui, Icon::Clear, "Limpar Nivelamento", true) {
                    acts.push(Act::ClearLevel);
                }
                small_button(ui, Icon::NextOver, "Próxima Superalocação", OFF);
            });
        });
    }

    fn view_tab(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>, has: bool) {
        group(ui, "Modos de Exibição de Tarefa", |ui| {
            if big_button(ui, Icon::Gantt, "Gráfico de\nGantt", true) {
                acts.push(Act::View(View::Gantt));
            }
            big_button(ui, Icon::Usage, "Uso da\nTarefa", OFF);
            column(ui, |ui| {
                small_button(ui, Icon::Network, "Diagrama de Rede", OFF);
                small_button(ui, Icon::Month, "Calendário", OFF);
                if small_button(ui, Icon::Sheet, "Planilha de Tarefas", true) {
                    acts.push(Act::View(View::Sheet));
                }
            });
        });
        group(ui, "Modos de Exibição de Recurso", |ui| {
            big_button(ui, Icon::Team, "Organizador\nde Equipe", OFF);
            big_button(ui, Icon::Usage, "Uso do\nRecurso", OFF);
            column(ui, |ui| {
                if small_button(ui, Icon::Sheet, "Planilha de Recursos", true) {
                    acts.push(Act::Report(Report::Resources));
                }
                small_button(ui, Icon::Options, "Outros Modos de Exibição", OFF);
            });
        });
        group(ui, "Dados", |ui| {
            column(ui, |ui| {
                small_button(ui, Icon::Sort, "Classificar", OFF);
                small_button(ui, Icon::Outline, "Estrutura de Tópicos", OFF);
                small_button(ui, Icon::Tables, "Tabelas", OFF);
            });
            column(ui, |ui| {
                small_button(ui, Icon::Highlight, "[Sem Realce]", OFF);
                small_button(ui, Icon::Filter, "[Sem Filtro]", OFF);
                small_button(ui, Icon::Group, "[Sem Grupo]", OFF);
            });
        });
        group(ui, "Zoom", |ui| {
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
        group(ui, "Modo Divisão", |ui| {
            column(ui, |ui| {
                let mut on = self.timeline;
                if ui.checkbox(&mut on, "Linha do Tempo").changed() {
                    acts.push(Act::Timeline);
                }
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Detalhes"));
            });
        });
        group(ui, "Janela", |ui| {
            big_button(ui, Icon::NewWindow, "Nova\nJanela", OFF);
            column(ui, |ui| {
                small_button(ui, Icon::Switch, "Alternar Janelas", OFF);
                small_button(ui, Icon::Arrange, "Organizar Tudo", OFF);
                small_button(ui, Icon::Hide, "Ocultar", OFF);
            });
        });
        group(ui, "Macros", |ui| {
            big_button(ui, Icon::Macro, "Macros", OFF);
        });
    }

    fn format_tab(&mut self, ui: &mut egui::Ui) {
        group(ui, "Formatar", |ui| {
            big_button(ui, Icon::Text, "Estilos de\nTexto", OFF);
            column(ui, |ui| {
                small_button(ui, Icon::Grid, "Linhas de Grade", OFF);
                small_button(ui, Icon::Layout, "Layout", OFF);
            });
        });
        group(ui, "Colunas", |ui| {
            big_button(ui, Icon::Column, "Inserir\nColuna", OFF);
            column(ui, |ui| {
                small_button(ui, Icon::Options, "Configurações de Coluna", OFF);
                small_button(ui, Icon::Fields, "Campos Personalizados", OFF);
            });
        });
        group(ui, "Estilos de Barra", |ui| {
            big_button(ui, Icon::Format, "Formatar", OFF);
            column(ui, |ui| {
                ui.checkbox(&mut self.gantt_opts.critical, "Tarefas Críticas");
                ui.checkbox(&mut self.gantt_opts.slack, "Margem de Atraso");
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Tarefas Atrasadas"));
            });
            column(ui, |ui| {
                ui.checkbox(&mut self.gantt_opts.baseline, "Linha de Base");
            });
        });
        group(ui, "Mostrar/Ocultar", |ui| {
            column(ui, |ui| {
                ui.checkbox(&mut self.outline_numbers, "Número da Estrutura de Tópicos");
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Tarefa de Resumo do Projeto"));
                ui.add_enabled(false, egui::Checkbox::new(&mut true, "Tarefas de Resumo"));
            });
        });
        group(ui, "Desenhos", |ui| {
            big_button(ui, Icon::Drawing, "Desenho", OFF);
        });
    }
}

fn report_tab(ui: &mut egui::Ui, acts: &mut Vec<Act>) {
    group(ui, "Exibir Relatórios", |ui| {
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
        big_button(ui, Icon::Intro, "Introdução", OFF);
        big_button(ui, Icon::Custom, "Personalizado", OFF);
        big_button(ui, Icon::Recent, "Relatórios\nRecentes", OFF);
    });
    group(ui, "Projeto", |ui| {
        big_button(ui, Icon::Compare, "Comparar\nProjetos", OFF);
    });
    group(ui, "Exportar", |ui| {
        big_button(ui, Icon::Export, "Relatórios\nVisuais", OFF);
    });
}

fn project_tab(ui: &mut egui::Ui, acts: &mut Vec<Act>) {
    group(ui, "Inserir", |ui| {
        big_button(ui, Icon::Subproject, "Subprojeto", OFF);
    });
    group(ui, "Suplementos", |ui| {
        big_button(ui, Icon::AddIn, "Obter\nSuplementos", OFF);
    });
    group(ui, "Propriedades", |ui| {
        if big_button(ui, Icon::Info, "Informações\ndo Projeto", true) {
            acts.push(Act::ProjectInfo);
        }
        big_button(ui, Icon::Fields, "Campos\nPersonalizados", OFF);
        big_button(ui, Icon::Links, "Vínculos Entre\nProjetos", OFF);
        column(ui, |ui| {
            small_button(ui, Icon::Wbs, "EAP", OFF);
            small_button(ui, Icon::WorkTime, "Alterar Período de Trabalho", OFF);
        });
    });
    group(ui, "Cronograma", |ui| {
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
            small_button(ui, Icon::StatusDate, "Data de Status: NA", OFF);
            small_button(ui, Icon::Update, "Atualizar Projeto", OFF);
        });
    });
    group(ui, "Revisão", |ui| {
        big_button(ui, Icon::Spelling, "Ortografia", OFF);
    });
}

fn help_tab(ui: &mut egui::Ui, acts: &mut Vec<Act>) {
    group(ui, "Ajuda", |ui| {
        if big_button(ui, Icon::Help, "Sobre o\nEtapix", true) {
            acts.push(Act::About);
        }
    });
}
