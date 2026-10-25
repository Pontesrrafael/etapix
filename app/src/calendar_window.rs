//! Editor de calendário ("Projeto > Alterar Período de Trabalho"): allows choosing the project
//! calendar or a resource's calendar, editing the working shifts of each day of the week and
//! dated exceptions (holidays, time off or special working hours), and saving via [`Doc::apply`].


use std::cell::RefCell;

use eframe::egui::{self, Color32, RichText};
use pjsched::calendar::{Calendar, Exception, Shift};

use crate::doc::Doc;
use crate::fmt;

const MUTED: Color32 = Color32::from_rgb(100, 110, 120);
const ERROR_RED: Color32 = Color32::from_rgb(190, 40, 40);
const TEAL: Color32 = Color32::from_rgb(0x0B, 0x6B, 0x66);

pub const DAY_NAMES: [&str; 7] = [
    "Domingo",
    "Segunda-feira",
    "Terça-feira",
    "Quarta-feira",
    "Quinta-feira",
    "Sexta-feira",
    "Sábado",
];

/// Which calendar the user is editing in "Alterar Período de Trabalho".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The project's base calendar (`project.calendars[project.calendar]`).
    Project,
    /// A specific calendar in `project.calendars`.
    Calendar(usize),
    /// A resource's calendar (`project.resources[r]`).
    Resource(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayDraft {
    pub working: bool,
    pub shifts: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionDraft {
    pub from: String,
    pub to: String,
    pub shifts: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarDraft {
    pub name: String,
    pub weekdays: [DayDraft; 7],
    pub exceptions: Vec<ExceptionDraft>,
}

/// Formats `[from, to)` minute shifts as `"09:00-12:00, 13:00-18:00"`.
pub fn format_shifts(shifts: &[Shift]) -> String {
    shifts
        .iter()
        .map(|&(a, b)| format!("{:02}:{:02}-{:02}:{:02}", a / 60, a % 60, b / 60, b % 60))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_hm(text: &str) -> Option<u32> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let (h, m) = if let Some((hs, ms)) = t.split_once([':', 'h', 'H']) {
        let h: u32 = hs.trim().parse().ok()?;
        let m: u32 = if ms.trim().is_empty() { 0 } else { ms.trim().parse().ok()? };
        (h, m)
    } else {
        (t.parse::<u32>().ok()?, 0)
    };
    if h > 24 || m >= 60 || h * 60 + m > 24 * 60 {
        return None;
    }
    Some(h * 60 + m)
}

/// Parses `"09:00-12:00, 13:00-18:00"` (or empty / `"folga"` for non-working) into shifts.
pub fn parse_shifts(text: &str) -> Result<Vec<Shift>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("folga")
        || trimmed.eq_ignore_ascii_case("feriado")
        || trimmed == "-"
    {
        return Ok(Vec::new());
    }
    let mut shifts = Vec::new();
    for part in trimmed.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
        let Some((a_str, b_str)) = part.split_once(['-', '–']) else {
            return Err(format!("Horário inválido: \"{part}\" (use HH:MM-HH:MM, ex.: 09:00-12:00)"));
        };
        let a = parse_hm(a_str).ok_or_else(|| format!("Hora inicial inválida em \"{part}\""))?;
        let b = parse_hm(b_str).ok_or_else(|| format!("Hora final inválida em \"{part}\""))?;
        if a >= b {
            return Err(format!("O início deve ser anterior ao término em \"{part}\""));
        }
        shifts.push((a, b));
    }
    shifts.sort_unstable_by_key(|s| s.0);
    for w in shifts.windows(2) {
        if w[0].1 > w[1].0 {
            return Err(format!("Turnos sobrepostos: {}", format_shifts(&shifts)));
        }
    }
    Ok(shifts)
}

impl CalendarDraft {
    pub fn from_calendar(cal: &Calendar) -> Self {
        let weekdays = std::array::from_fn(|d| {
            let s = &cal.weekdays[d];
            DayDraft { working: !s.is_empty(), shifts: format_shifts(s) }
        });
        let exceptions = cal
            .exceptions
            .iter()
            .map(|e| ExceptionDraft {
                from: e.from.format("%d/%m/%Y").to_string(),
                to: e.to.format("%d/%m/%Y").to_string(),
                shifts: format_shifts(&e.shifts),
            })
            .collect();
        CalendarDraft { name: cal.name.clone(), weekdays, exceptions }
    }

    pub fn from_target(doc: &Doc, target: Target) -> Self {
        let p = &doc.project;
        let cal = match target {
            Target::Project => &p.calendars[p.calendar],
            Target::Calendar(c) => &p.calendars[c.min(p.calendars.len().saturating_sub(1))],
            Target::Resource(r) => {
                let c = p.resources.get(r).and_then(|res| res.calendar).unwrap_or(p.calendar);
                &p.calendars[c.min(p.calendars.len().saturating_sub(1))]
            }
        };
        Self::from_calendar(cal)
    }

    /// Validates and converts the draft into a [`Calendar`], keeping `base`'s `source_uid`.
    pub fn to_calendar(&self, base: &Calendar) -> Result<Calendar, String> {
        let mut weekdays: [Vec<Shift>; 7] = Default::default();
        for (d, day) in self.weekdays.iter().enumerate() {
            if !day.working {
                weekdays[d] = Vec::new();
                continue;
            }
            let parsed = parse_shifts(&day.shifts).map_err(|e| format!("{}: {e}", DAY_NAMES[d]))?;
            if parsed.is_empty() {
                return Err(format!(
                    "{}: informe o horário (ex.: 09:00-12:00, 13:00-18:00) ou desmarque Dia útil.",
                    DAY_NAMES[d]
                ));
            }
            weekdays[d] = parsed;
        }
        if weekdays.iter().all(Vec::is_empty) {
            return Err("O calendário precisa ter pelo menos um dia útil na semana.".into());
        }

        let mut exceptions = Vec::new();
        for (k, exc) in self.exceptions.iter().enumerate() {
            if exc.from.trim().is_empty() && exc.to.trim().is_empty() && exc.shifts.trim().is_empty() {
                continue;
            }
            let from = fmt::parse_date(&exc.from)
                .ok_or_else(|| format!("Exceção {}: data inicial inválida ({})", k + 1, exc.from.trim()))?;
            let to = if exc.to.trim().is_empty() {
                from
            } else {
                fmt::parse_date(&exc.to)
                    .ok_or_else(|| format!("Exceção {}: data final inválida ({})", k + 1, exc.to.trim()))?
            };
            if to < from {
                return Err(format!(
                    "Exceção {}: a data final ({}) é anterior à inicial ({})",
                    k + 1,
                    exc.to.trim(),
                    exc.from.trim()
                ));
            }
            let shifts = parse_shifts(&exc.shifts).map_err(|e| format!("Exceção {}: {e}", k + 1))?;
            exceptions.push(Exception { from, to, shifts });
        }

        let name = if self.name.trim().is_empty() { base.name.clone() } else { self.name.trim().to_string() };
        Ok(Calendar { name, weekdays, exceptions, source_uid: base.source_uid })
    }

    /// Applies the edited calendar to `target` in `doc` through [`Doc::apply`].
    pub fn apply_to(&self, doc: &mut Doc, target: Target) -> Result<(), String> {
        let draft = self.clone();
        doc.apply(|p| match target {
            Target::Project => {
                let idx = p.calendar;
                let before = p.calendars[idx].clone();
                let updated = draft.to_calendar(&before)?;
                // Derived resource calendars that inherited the project calendar stay in step.
                for (c, cal) in p.calendars.iter_mut().enumerate() {
                    if c != idx && cal.weekdays == before.weekdays && cal.exceptions == before.exceptions {
                        cal.weekdays = updated.weekdays.clone();
                        cal.exceptions = updated.exceptions.clone();
                    }
                }
                p.calendars[idx] = updated;
                Ok(())
            }
            Target::Calendar(idx) => {
                if idx >= p.calendars.len() {
                    return Err("Calendário não encontrado.".into());
                }
                let updated = draft.to_calendar(&p.calendars[idx])?;
                p.calendars[idx] = updated;
                Ok(())
            }
            Target::Resource(r) => {
                if r >= p.resources.len() {
                    return Err("Recurso não encontrado.".into());
                }
                let current = p.resources[r].calendar;
                let dedicated = current.filter(|&c| {
                    c != p.calendar
                        && p.resources.iter().enumerate().all(|(k, res)| k == r || res.calendar != Some(c))
                        && p.tasks.iter().all(|t| t.calendar != Some(c))
                });
                if let Some(c) = dedicated {
                    let mut updated = draft.to_calendar(&p.calendars[c])?;
                    updated.name = p.calendars[c].name.clone();
                    p.calendars[c] = updated;
                } else {
                    let base_idx = current.unwrap_or(p.calendar);
                    let mut updated = draft.to_calendar(&p.calendars[base_idx])?;
                    updated.name = p.resources[r].name.clone();
                    updated.source_uid = None;
                    p.calendars.push(updated);
                    p.resources[r].calendar = Some(p.calendars.len() - 1);
                }
                Ok(())
            }
        })
    }
}

/// State of the "Alterar Período de Trabalho" dialog.
pub struct CalendarWindow {
    pub target: Target,
    pub draft: CalendarDraft,
    pub error: Option<String>,
}

impl CalendarWindow {
    pub fn new(doc: &Doc) -> Self {
        let target = Target::Project;
        let draft = CalendarDraft::from_target(doc, target);
        CalendarWindow { target, draft, error: None }
    }

    fn target_label(doc: &Doc, target: Target) -> String {
        let p = &doc.project;
        match target {
            Target::Project => format!("Projeto ({})", p.calendars[p.calendar].name),
            Target::Calendar(c) => p
                .calendars
                .get(c)
                .map(|cal| format!("Calendário: {}", cal.name))
                .unwrap_or_else(|| "Calendário".into()),
            Target::Resource(r) => p
                .resources
                .get(r)
                .map(|res| format!("Recurso: {}", res.name))
                .unwrap_or_else(|| "Recurso".into()),
        }
    }

    fn available_targets(doc: &Doc) -> Vec<(Target, String)> {
        let p = &doc.project;
        let mut out = vec![(Target::Project, Self::target_label(doc, Target::Project))];
        for (c, cal) in p.calendars.iter().enumerate() {
            let is_res_cal = p.resources.iter().any(|r| r.calendar == Some(c));
            if c != p.calendar && !is_res_cal {
                out.push((Target::Calendar(c), format!("Calendário: {}", cal.name)));
            }
        }
        for (r, res) in p.resources.iter().enumerate() {
            out.push((Target::Resource(r), format!("Recurso: {}", res.name)));
        }
        out
    }

    /// Renders the window; returns `true` while it remains open.
    pub fn show(&mut self, ctx: &egui::Context, doc: &mut Doc) -> bool {
        let mut open = true;
        let mut close_requested = false;

        egui::Window::new("Alterar Período de Trabalho")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(540.0)
            .show(ctx, |ui| {
                // Target selector
                let targets = Self::available_targets(doc);
                if !targets.iter().any(|(t, _)| *t == self.target) {
                    self.target = Target::Project;
                    self.draft = CalendarDraft::from_target(doc, self.target);
                    self.error = None;
                }
                let mut picked_target = None;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Para o calendário:").strong());
                    egui::ComboBox::from_id_salt("calendar-target-combo")
                        .selected_text(Self::target_label(doc, self.target))
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            for (t, label) in &targets {
                                if ui.selectable_label(self.target == *t, label).clicked() {
                                    picked_target = Some(*t);
                                }
                            }
                        });
                });
                if let Some(next) = picked_target.filter(|&t| t != self.target) {
                    self.target = next;
                    self.draft = CalendarDraft::from_target(doc, next);
                    self.error = None;
                }

                ui.separator();
                ui.label(RichText::new("Horário padrão da semana").strong().color(TEAL));
                ui.label(
                    RichText::new("Separe os turnos por vírgula (ex.: 09:00-12:00, 13:00-18:00).")
                        .size(11.0)
                        .color(MUTED),
                );
                ui.add_space(4.0);

                egui::Grid::new("calendar-weekdays-grid")
                    .striped(true)
                    .num_columns(3)
                    .spacing([12.0, 5.0])
                    .show(ui, |ui| {
                        for (d, name) in DAY_NAMES.iter().enumerate() {
                            let day = &mut self.draft.weekdays[d];
                            ui.label(*name);
                            if ui.checkbox(&mut day.working, "Dia útil").changed() && day.working && day.shifts.trim().is_empty() {
                                day.shifts = "09:00-12:00, 13:00-18:00".into();
                            }
                            if day.working {
                                ui.add(
                                    egui::TextEdit::singleline(&mut day.shifts)
                                        .desired_width(250.0)
                                        .hint_text("09:00-12:00, 13:00-18:00"),
                                );
                            } else {
                                ui.label(RichText::new("Folga").color(MUTED));
                            }
                            ui.end_row();
                        }
                    });

                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Exceções (feriados, folgas e horários especiais)").strong().color(TEAL));
                    if ui.small_button("+ Adicionar exceção").clicked() {
                        let date = doc.project.start.format("%d/%m/%Y").to_string();
                        self.draft.exceptions.push(ExceptionDraft { from: date.clone(), to: date, shifts: String::new() });
                    }
                });
                ui.label(
                    RichText::new("Deixe o horário em branco para feriado/folga, ou informe os turnos trabalhados na data.")
                        .size(11.0)
                        .color(MUTED),
                );
                ui.add_space(4.0);

                let mut remove_idx = None;
                if self.draft.exceptions.is_empty() {
                    ui.label(RichText::new("Nenhuma exceção cadastrada.").color(MUTED));
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .id_salt("calendar-exceptions-scroll")
                        .show(ui, |ui| {
                            egui::Grid::new("calendar-exceptions-grid")
                                .striped(true)
                                .num_columns(4)
                                .spacing([8.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Início").strong());
                                    ui.label(RichText::new("Término").strong());
                                    ui.label(RichText::new("Horário (vazio = folga)").strong());
                                    ui.label("");
                                    ui.end_row();

                                    for (k, exc) in self.draft.exceptions.iter_mut().enumerate() {
                                        ui.add(egui::TextEdit::singleline(&mut exc.from).desired_width(90.0).hint_text("dd/mm/aaaa"));
                                        ui.add(egui::TextEdit::singleline(&mut exc.to).desired_width(90.0).hint_text("dd/mm/aaaa"));
                                        ui.add(
                                            egui::TextEdit::singleline(&mut exc.shifts)
                                                .desired_width(190.0)
                                                .hint_text("Folga / feriado"),
                                        );
                                        if ui.small_button("Remover").clicked() {
                                            remove_idx = Some(k);
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                }
                if let Some(k) = remove_idx {
                    self.draft.exceptions.remove(k);
                }

                if let Some(err) = &self.error {
                    ui.add_space(6.0);
                    ui.label(RichText::new(err).color(ERROR_RED));
                }

                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        let v = doc.begin();
                        match self.draft.apply_to(doc, self.target) {
                            Ok(()) => {
                                doc.end(v);
                                self.error = None;
                                close_requested = true;
                            }
                            Err(e) => {
                                doc.end(v);
                                self.error = Some(e);
                            }
                        }
                    }
                    if ui.button("Aplicar").clicked() {
                        let v = doc.begin();
                        match self.draft.apply_to(doc, self.target) {
                            Ok(()) => {
                                doc.end(v);
                                self.draft = CalendarDraft::from_target(doc, self.target);
                                self.error = None;
                            }
                            Err(e) => {
                                doc.end(v);
                                self.error = Some(e);
                            }
                        }
                    }
                    if ui.button("Restaurar padrão").clicked() {
                        self.draft = CalendarDraft::from_calendar(&Calendar::standard());
                        self.error = None;
                    }
                    if ui.button("Cancelar").clicked() {
                        close_requested = true;
                    }
                });
            });

        open && !close_requested
    }
}

thread_local! {
    static STATE: RefCell<Option<CalendarWindow>> = const { RefCell::new(None) };
}

/// Opens the "Alterar Período de Trabalho" window for `doc`.
pub fn open(doc: &Doc) {
    STATE.with(|s| *s.borrow_mut() = Some(CalendarWindow::new(doc)));
}

/// Draws the "Alterar Período de Trabalho" window when open.
pub fn show(ctx: &egui::Context, doc: &mut Doc) {
    STATE.with(|s| {
        let mut slot = s.borrow_mut();
        if slot.as_mut().is_some_and(|w| !w.show(ctx, doc)) {
            *slot = None;
        }
    });
}

impl crate::App {
    /// Opens the "Alterar Período de Trabalho" window.
    pub(crate) fn open_calendar_window(&mut self) {
        open(&self.doc);
    }

    /// Draws the "Alterar Período de Trabalho" window when open.
    pub(crate) fn calendar_window(&mut self, ctx: &egui::Context) {
        show(ctx, &mut self.doc);
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn shifts_format_parse_and_validate() {
        let std_cal = Calendar::standard();
        let text = format_shifts(&std_cal.weekdays[1]);
        assert_eq!(text, "09:00-12:00, 13:00-18:00");
        assert_eq!(parse_shifts(&text).unwrap(), vec![(9 * 60, 12 * 60), (13 * 60, 18 * 60)]);
        assert_eq!(parse_shifts("8-12; 13h30-17h30").unwrap(), vec![(8 * 60, 12 * 60), (13 * 60 + 30, 17 * 60 + 30)]);
        assert!(parse_shifts("").unwrap().is_empty());
        assert!(parse_shifts("folga").unwrap().is_empty());
        assert!(parse_shifts("18:00-09:00").is_err());
        assert!(parse_shifts("08:00-13:00, 12:00-18:00").is_err());
        assert!(parse_shifts("25:00-26:00").is_err());
    }

    #[test]
    fn calendar_draft_edits_weekdays_exceptions_and_resource_calendar() {
        let mut doc = Doc::blank();
        let start_date = doc.project.start.date();
        let t0 = doc.add_task("Execução", 2.0).unwrap();
        doc.set_resources(t0, "Ana").unwrap();

        let initial_finish = doc.dt(doc.schedule[t0].finish);

        // Add a 1-day holiday on the project start date and make Monday 08:00-12:00, 13:00-17:00.
        let mut draft = CalendarDraft::from_target(&doc, Target::Project);
        draft.weekdays[1].shifts = "08:00-12:00, 13:00-17:00".into();
        draft.exceptions.push(ExceptionDraft {
            from: start_date.format("%d/%m/%Y").to_string(),
            to: start_date.format("%d/%m/%Y").to_string(),
            shifts: String::new(),
        });

        let v = doc.begin();
        draft.apply_to(&mut doc, Target::Project).unwrap();
        doc.end(v);

        assert!(!doc.is_working_day(start_date));
        assert!(doc.dt(doc.schedule[t0].start).date() > start_date);
        assert!(doc.dt(doc.schedule[t0].finish) > initial_finish);

        // Give resource "Ana" her own exception on the next working day (Tuesday).
        let tuesday = start_date + chrono::Duration::days(1);
        let mut res_draft = CalendarDraft::from_target(&doc, Target::Resource(0));
        res_draft.exceptions.push(ExceptionDraft {
            from: tuesday.format("%d/%m/%Y").to_string(),
            to: tuesday.format("%d/%m/%Y").to_string(),
            shifts: String::new(),
        });
        res_draft.apply_to(&mut doc, Target::Resource(0)).unwrap();

        assert!(doc.project.resources[0].calendar.is_some());
        // Project calendar still works on Tuesday, but Ana's task starts on Wednesday.
        assert!(doc.is_working_day(tuesday));
        assert_eq!(doc.dt(doc.schedule[t0].start).date(), start_date + chrono::Duration::days(2));

        // Reject an all-nonworking week or reversed exception dates.
        let mut bad = CalendarDraft::from_calendar(&Calendar::standard());
        for d in &mut bad.weekdays {
            d.working = false;
        }
        assert!(bad.to_calendar(&Calendar::standard()).is_err());

        let mut bad_exc = CalendarDraft::from_calendar(&Calendar::standard());
        bad_exc.exceptions.push(ExceptionDraft {
            from: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap().format("%d/%m/%Y").to_string(),
            to: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap().format("%d/%m/%Y").to_string(),
            shifts: String::new(),
        });
        assert!(bad_exc.to_calendar(&Calendar::standard()).is_err());
    }

    fn is_open() -> bool {
        STATE.with(|s| s.borrow().is_some())
    }

    fn close() {
        STATE.with(|s| *s.borrow_mut() = None);
    }

    #[test]
    fn window_opens_renders_and_closes_in_egui() {
        let mut doc = Doc::blank();
        doc.add_task("Fundação", 2.0).unwrap();
        doc.add_resource("Ana").unwrap();

        open(&doc);
        assert!(is_open());

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show(ctx, &mut doc);
        });
        assert!(is_open());

        close();
        assert!(!is_open());
    }
}
