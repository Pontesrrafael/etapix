//! Checks writing into `.mpp` files against Project itself, in two steps around Project:
//!
//! 1. `prepare OUT FILE.mpp...`: edits each project the way a user would (names, durations,
//!    priorities, constraints, deadlines), saves the edits into a copy of the file
//!    ([`pjsched::mpp::update_mpp`]) as `OUT/NAME.mpp`, and what Etapix expects as
//!    `OUT/NAME.expected.txt`.
//!    `prepare-new OUT FILE.mpp...` instead saves each project, unedited, as a new file
//!    ([`pjsched::mpp::create_mpp`]); `prepare-structure OUT FILE.mpp...` adds and removes tasks,
//!    links and assignments and saves into a copy of the file ([`pjsched::mpp::save_mpp`]).
//! 2. Project opens every `OUT/*.mpp` and saves it as `OUT/NAME.proj.xml`
//!    (`tools/mpp_to_xml.ps1 OUT`).
//! 3. `verify OUT`: Project's reading must match what Etapix expects, schedule included.

use std::path::Path;

use pjsched::calendar::Calendars;
use pjsched::cpm;
use pjsched::model::Project;
use pjsched::mpp::{create_mpp, mpp_to_mspdi, save_mpp, update_mpp};
use pjsched::mspdi::read_project;
use pjsched::mspdi_write::write_project;

#[path = "../tests/support/describe.rs"]
mod describe;
#[path = "../tests/support/edits.rs"]
mod edits;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((cmd, rest)) if cmd == "prepare" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::Edit),
        Some((cmd, rest)) if cmd == "prepare-new" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::New),
        Some((cmd, rest)) if cmd == "prepare-structure" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::Structure),
        Some((cmd, rest)) if cmd == "prepare-progress" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::Progress),
        Some((cmd, rest)) if cmd == "prepare-baseline" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::Baseline),
        Some((cmd, rest)) if cmd == "prepare-cost" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::Cost),
        Some((cmd, rest)) if cmd == "prepare-resources" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::Resources),
        Some((cmd, rest)) if cmd == "prepare-calendars" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::Calendars),
        Some((cmd, rest)) if cmd == "prepare-overtime" && rest.len() >= 2 => prepare(Path::new(&rest[0]), &rest[1..], Mode::Overtime),
        Some((cmd, rest)) if cmd == "verify" && rest.len() == 1 => verify(Path::new(&rest[0])),
        _ => eprintln!("uso: mpp_write_check prepare SAIDA arquivo.mpp... | verify SAIDA"),
    }
}

/// How each project is changed and saved.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Fields edited, saved into the file.
    Edit,
    /// Saved, unedited, as a new file from the template.
    New,
    /// Tasks, links and assignments added and removed, saved into the file.
    Structure,
    /// Progress recorded on tasks, saved into the file.
    Progress,
    /// A baseline saved, saved into the file.
    Baseline,
    /// A cost resource added and charged to a task, saved into the file.
    Cost,
    /// Resources renamed, with more units and new rates, saved into the file.
    Resources,
    /// Holidays and working hours changed on the calendars, saved into the file.
    Calendars,
    /// An hour of overtime on an assignment, saved into the file.
    Overtime,
}

fn prepare(out: &Path, files: &[String], mode: Mode) {
    std::fs::create_dir_all(out).unwrap();
    for f in files {
        let original = std::fs::read(f).unwrap();
        // Folder and file name: corpus folders reuse file names.
        let path = Path::new(f);
        let folder = path.parent().and_then(|d| d.file_name()).map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
        let name = format!("{folder}-{}", path.file_stem().unwrap().to_string_lossy());
        let mut project = read_project(&mpp_to_mspdi(&original).unwrap()).unwrap();
        match mode {
            Mode::Edit => edits::edit(&mut project),
            Mode::Structure => edits::restructure(&mut project),
            Mode::Progress => edits::track(&mut project),
            Mode::Baseline => edits::baseline(&mut project),
            Mode::Cost => edits::add_cost(&mut project),
            Mode::Resources => edits::edit_resources(&mut project),
            Mode::Calendars => edits::edit_calendars(&mut project),
            Mode::Overtime => edits::add_overtime(&mut project),
            Mode::New => {}
        }
        let cals = Calendars::new(&project).unwrap();
        let schedule = cpm::schedule(&project, &cals, None).unwrap();
        let xml = write_project(&project, &cals, &schedule, &name);
        // Etapix's own reading of what it saves is what Project must read too.
        let expected = read_project(&xml).unwrap();
        let saved = match mode {
            Mode::Edit => update_mpp(&original, &xml),
            Mode::New => create_mpp(&xml),
            Mode::Structure | Mode::Progress | Mode::Baseline | Mode::Cost | Mode::Resources | Mode::Calendars | Mode::Overtime => save_mpp(&original, &xml),
        };
        match saved {
            Ok(bytes) => {
                std::fs::write(out.join(format!("{name}.mpp")), bytes).unwrap();
                std::fs::write(out.join(format!("{name}.expected.txt")), comparable(expected).join("\n")).unwrap();
            }
            Err(e) => println!("{name}: não gravado: {e}"),
        }
    }
}

/// What Project and Etapix must agree on. A summary's duration and progress are left out:
/// Project rolls them up from its subtasks when it opens the file, while Etapix keeps the stored
/// values.
fn comparable(mut p: Project) -> Vec<String> {
    for t in p.tasks.iter_mut().filter(|t| t.summary) {
        t.duration = 0;
        t.progress = Default::default();
    }
    let mut text = describe::describe(&p);
    text.push(format!("schedule {:?}", describe::schedule(&p)));
    text
}

fn verify(out: &Path) {
    let (mut ok, mut failed) = (0, 0);
    let mut names: Vec<_> = std::fs::read_dir(out).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "mpp")).collect();
    names.sort();
    for mpp in names {
        let name = mpp.file_stem().unwrap().to_string_lossy().to_string();
        let expected = std::fs::read_to_string(out.join(format!("{name}.expected.txt"))).unwrap();
        let Ok(xml) = std::fs::read_to_string(out.join(format!("{name}.proj.xml"))) else {
            println!("{name}: o Project não abriu o arquivo");
            failed += 1;
            continue;
        };
        let got = comparable(read_project(&xml).unwrap());
        let expected: Vec<&str> = expected.lines().collect();
        match expected.iter().zip(&got).find(|(e, g)| *e != g) {
            Some((e, g)) => {
                println!("{name}:\n   etapix  {e}\n   project {g}");
                failed += 1;
            }
            None if expected.len() != got.len() => {
                println!("{name}: {} vs {} itens", expected.len(), got.len());
                failed += 1;
            }
            None => ok += 1,
        }
    }
    println!("{ok} conferem, {failed} diferem");
}
