//! Differential tests against Microsoft Project.
//!
//! Each case is a project exported by `tools/run_scenarios.ps1` or `tools/run_batch2.ps1`:
//! `<case>-antes.xml` is the input (and the expected CPM result), `<case>-depois.xml` is what
//! Project produced after leveling with the options listed here.

use std::path::Path;

use pjsched::calendar::{Calendars, Clock};
use pjsched::cpm::{self, TaskSchedule};
use pjsched::leveling::{self, LevelOrder, LevelingOptions, PeriodBasis};
use pjsched::mspdi::{read_project, read_results, TaskResult};

const STANDARD: LevelingOptions = LevelingOptions {
    order: LevelOrder::Standard,
    within_slack: false,
    basis: PeriodBasis::Day,
    individual_assignments: true,
    range: None,
};

fn opts(order: LevelOrder, within_slack: bool) -> LevelingOptions {
    LevelingOptions { order, within_slack, ..STANDARD }
}

fn diff(case: &str, clock: &Clock, got: &[TaskSchedule], want: &[TaskResult]) -> Vec<String> {
    let mut out = Vec::new();
    for (g, w) in got.iter().zip(want) {
        let mut check = |field: &str, got: String, want: String| {
            if got != want {
                out.push(format!("{case} / {}: {field} = {got}, Project = {want}", w.name));
            }
        };
        let dt = |at| clock.dt(at).to_string();
        check("start", dt(g.start), w.start.to_string());
        check("finish", dt(g.finish), w.finish.to_string());
        check("early_start", dt(g.early_start), w.early_start.to_string());
        check("late_start", dt(g.late_start), w.late_start.to_string());
        check("total_slack", g.total_slack.to_string(), w.total_slack.to_string());
        check("free_slack", g.free_slack.to_string(), w.free_slack.to_string());
        check("critical", g.critical.to_string(), w.critical.to_string());
        check("leveling_delay", g.leveling_delay.to_string(), w.leveling_delay.to_string());
    }
    if got.len() != want.len() {
        out.push(format!("{case}: {} tasks scheduled, Project has {}", got.len(), want.len()));
    }
    out
}

fn run(dir: &str, case: &str, leveling: Option<LevelingOptions>) -> Vec<String> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(dir);
    let before = std::fs::read_to_string(base.join(format!("{case}-antes.xml"))).unwrap();
    let project = read_project(&before).unwrap_or_else(|e| panic!("{case}: {e}"));
    let cals = Calendars::new(&project).unwrap();
    let clock = &cals.clock;

    let unleveled = cpm::schedule(&project, &cals, None).unwrap_or_else(|e| panic!("{case}: {e:?}"));
    let mut problems = diff(case, clock, &unleveled, &read_results(&before).unwrap());
    if let Some(options) = leveling {
        let after = std::fs::read_to_string(base.join(format!("{case}-depois.xml"))).unwrap();
        let leveled = leveling::level(&project, &cals, options).unwrap();
        problems.extend(diff(&format!("{case} (nivelado)"), clock, &leveled, &read_results(&after).unwrap()));
    }
    problems
}

fn assert_cases(cases: &[(&str, &str, Option<LevelingOptions>)]) {
    let problems: Vec<String> = cases.iter().flat_map(|&(dir, case, lv)| run(dir, case, lv)).collect();
    assert!(problems.is_empty(), "{} divergences:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn critical_path_and_calendars() {
    assert_cases(&[
        ("b1", "S1-caminho-critico", None),
        ("b1", "S2-tipos-dependencia", None),
        ("b1", "S7-calendario", None),
    ]);
}

#[test]
fn leveling_batch1() {
    assert_cases(&[
        ("b1", "S3-nivelamento-basico", Some(STANDARD)),
        ("b1", "S4-nivelamento-prioridade", Some(STANDARD)),
        ("b1", "S5-nivelamento-divisao", Some(STANDARD)),
        ("b1", "S6-nivelamento-sem-divisao", Some(STANDARD)),
        ("b1", "S8-unidades-50", Some(STANDARD)),
    ]);
}

#[test]
fn leveling_orders() {
    use LevelOrder::*;
    assert_cases(&[
        ("b2", "B2-A-prioridade-vs-folga-ID", Some(opts(Id, false))),
        ("b2", "B2-A-prioridade-vs-folga-Standard", Some(opts(Standard, false))),
        ("b2", "B2-A-prioridade-vs-folga-Prioridade", Some(opts(PriorityStandard, false))),
        ("b2", "B2-B-longa-vs-curta-ID", Some(opts(Id, false))),
        ("b2", "B2-B-longa-vs-curta-Standard", Some(opts(Standard, false))),
        ("b2", "B2-B-longa-vs-curta-Prioridade", Some(opts(PriorityStandard, false))),
    ]);
}

#[test]
fn constraints_deadlines_and_summaries() {
    let cases = [
        "B4-K1-ALAP",
        "B4-K2-MSO-folga",
        "B4-K3-MSO-conflito",
        "B4-K4-MFO",
        "B4-K5-SNLT-conflito",
        "B4-K5b-SNLT-folga",
        "B4-K6-FNET",
        "B4-K7-FNLT-conflito",
        "B4-K7b-FNLT-folga",
        "B4-K8-deadline",
        "B4-K9-resumo",
        "B4-K10-resumo-restricao",
    ];
    let cases: Vec<_> = cases.iter().map(|c| ("b4", *c, None)).collect();
    assert_cases(&cases);
}

#[test]
fn calendars_and_elapsed_time() {
    let cases = [
        "B5-C1-cal-tarefa-7dias",
        "B5-C2-cal-tarefa-meio",
        "B5-C3-recurso-excecao",
        "B5-C4-recurso-meio",
        "B5-C5-tarefa-e-recurso",
        "B5-C6-ignora-recurso",
        "B5-C7-excecao-sabado",
        "B5-C8-excecao-parcial",
        "B5-C9-decorrido",
    ];
    let cases: Vec<_> = cases.iter().map(|c| ("b5", *c, None)).collect();
    assert_cases(&cases);
}

#[test]
fn progress_and_actuals() {
    let cases = [
        "B6-P1-50pct",
        "B6-P2-100pct",
        "B6-P3-inicio-real-atrasado",
        "B6-P4-termino-real-adiantado",
        "B6-P5-termino-real-atrasado",
        "B6-P6-duracao-real",
        "B6-P7-trabalho-real",
        "B6-P8-reagendar",
    ];
    let cases: Vec<_> = cases.iter().map(|c| ("b6", *c, None)).collect();
    assert_cases(&cases);
}

#[test]
fn progress_with_resources() {
    let cases = [
        "B7-R1-dois-recursos-um-trabalhou",
        "B7-R2-inicio-real-atrasado-50pct",
        "B7-R3-reagendar-com-recurso",
        "B7-R4-nivelar-com-andamento",
    ];
    let cases: Vec<_> = cases.iter().map(|c| ("b7", *c, None)).collect();
    assert_cases(&cases);
}

#[test]
fn manually_scheduled_tasks() {
    let cases = [
        "B8-M1-manual-antes-do-predecessor",
        "B8-M2-manual-depois",
        "B8-M3-manual-sem-vinculos",
        "B8-M4-manual-viola-vinculo",
    ];
    let cases: Vec<_> = cases.iter().map(|c| ("b8", *c, None)).collect();
    assert_cases(&cases);
}

#[test]
fn milestones_recurring_exceptions_and_resource_calendars() {
    let cases = [
        "B9-Z1-marcos",
        "B9-Z2-marco-restricao",
        "B9-E1-feriado-recorrente-semanal",
        "B9-E2-excecao-varios-dias",
        "B9-C1-recursos-calendarios-diferentes",
    ];
    let cases: Vec<_> = cases.iter().map(|c| ("b9", *c, None)).collect();
    assert_cases(&cases);
}

#[test]
fn alap_successors_and_summary_rules() {
    let cases = ["B10-A1-ALAP-com-sucessora", "B10-A2-ALAP-cadeia", "B10-S1-resumo-SNET", "B10-L2-SS-do-resumo"];
    let cases: Vec<_> = cases.iter().map(|c| ("b10", *c, None)).collect();
    assert_cases(&cases);
}

/// "Level individual assignments" on and off gave identical results in these cases.
#[test]
fn leveling_multiple_resources() {
    let mut cases = Vec::new();
    for variant in ["indiv", "tarefa"] {
        for case in ["B3-M1-dois-recursos", "B3-M2-unidades-parciais", "B3-M3-atribuicao-curta"] {
            cases.push(("b3", format!("{case}-{variant}"), Some(STANDARD)));
        }
    }
    let cases: Vec<_> = cases.iter().map(|(d, c, o)| (*d, c.as_str(), *o)).collect();
    assert_cases(&cases);
}

#[test]
fn leveling_within_slack() {
    use LevelOrder::*;
    assert_cases(&[
        ("b2", "B2-C1-folga-curta-dentroDaFolga", Some(opts(Standard, true))),
        ("b2", "B2-C1-folga-curta-normal", Some(opts(Standard, false))),
        ("b2", "B2-C2-folga-longa-dentroDaFolga", Some(opts(Standard, true))),
        ("b2", "B2-C2-folga-longa-normal", Some(opts(Standard, false))),
    ]);
}

#[test]
fn critical_path_options() {
    assert_cases(&[("b11", "B11-K1-limite-folga-2d", None), ("b11", "B11-K2-multiplos-caminhos", None)]);
}

#[test]
fn honor_constraints_off_and_cross_calendar_lag() {
    assert_cases(&[("b12", "B12-H1-restricoes-desligadas", None), ("b12", "B12-L1-lag-entre-calendarios", None)]);
}

#[test]
fn monthly_and_yearly_exceptions() {
    let cases = ["B13-E1-mensal-dia-10", "B13-E2-mensal-primeira-segunda", "B13-E3-anual-25-dez", "B13-E4-anual-ultima-sexta-nov"];
    let cases: Vec<_> = cases.iter().map(|c| ("b13", *c, None)).collect();
    assert_cases(&cases);
}

#[test]
fn leveling_assignments_ranges_and_uneven_splits() {
    let range = |a: &str, b: &str| {
        let p = |d: &str| chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M").unwrap();
        Some((p(a), p(b)))
    };
    assert_cases(&[
        ("b15", "B15-I-indiv", Some(STANDARD)),
        ("b15", "B15-I-tarefa", Some(LevelingOptions { individual_assignments: false, ..STANDARD })),
        ("b15", "B15-D-intervalo", Some(LevelingOptions { range: range("2026-11-13T00:00", "2026-11-20T00:00"), ..STANDARD })),
        ("b15", "B15-D-tudo", Some(STANDARD)),
        ("b15", "B15-S-divisao-desigual", Some(STANDARD)),
    ]);
}

/// A night-shift task that ignores its resource calendar next to a day task on the same resource.
/// Leveling it makes Project stop on a modal dialog, so only the unleveled schedule is pinned.
#[test]
fn night_shift_task_schedule() {
    assert_cases(&[("b16", "B16-N-noite-e-dia", None)]);
}

/// Batch 11 saved Project's leveling in the file. Clearing those delays and leveling with the
/// same period basis must give back Project's dates; the saved file must also schedule as is.
#[test]
fn leveling_period_basis() {
    let mut problems = Vec::new();
    for (case, basis) in [("B11-L1-base-semana", PeriodBasis::Week), ("B11-L1-base-dia", PeriodBasis::Day), ("B11-L1-base-hora", PeriodBasis::Hour)] {
        problems.extend(run("b11", case, None));
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/b11").join(format!("{case}-antes.xml"));
        let xml = std::fs::read_to_string(path).unwrap();
        let mut project = read_project(&xml).unwrap();
        project.tasks.iter_mut().for_each(|t| t.leveling_delay = 0);
        let cals = Calendars::new(&project).unwrap();
        let options = LevelingOptions { basis, ..STANDARD };
        let leveled = leveling::level(&project, &cals, options).unwrap();
        problems.extend(diff(&format!("{case} (nivelado do zero)"), &cals.clock, &leveled, &read_results(&xml).unwrap()));
    }
    assert!(problems.is_empty(), "{} divergences:\n{}", problems.len(), problems.join("\n"));
}
