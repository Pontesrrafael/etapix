# Etapix

## Instalar o Etapix

| Sistema | Instalador | Como instalar |
|---|---|---|
| **Windows** 10 ou 11 | [⬇ Etapix-Instalador.exe](https://github.com/Pontesrrafael/etapix/raw/master/Etapix-Instalador.exe) | Baixe e dê dois cliques. |
| **Linux** (Ubuntu 22.04 ou mais novo, x86_64) | [⬇ Etapix-Linux-x86_64.tar.gz](https://github.com/Pontesrrafael/etapix/releases/latest/download/Etapix-Linux-x86_64.tar.gz) | `tar -xzf Etapix-Linux-x86_64.tar.gz && cd etapix && ./instalar.sh` |

Os dois instalam o Etapix só para o seu usuário, sem administrador, e registram o conector do Etapix no Claude Code e no Codex, se estiverem instalados. No Windows, o instalador também oferece o conector ao Claude Desktop. Arquivos `.mpp` abrem, são criados e salvos em qualquer máquina, no Windows ou no Linux, sem o Microsoft Project: tarefas, estrutura, vínculos, recursos de trabalho, material e custo, taxas que mudam com o tempo, atribuições, horas extras, calendários, andamento e linha de base vão direto para o próprio `.mpp`, que guarda também o que só o Project usa (modos de exibição, formatações).

Também estão em [Releases](https://github.com/Pontesrrafael/etapix/releases/latest), junto com `Etapix-Claude-Codex-0.1.0.zip`, que instala só o conector (sem o app): descompacte e dê dois cliques em `instalar.cmd`. No Claude Desktop, também basta abrir `etapix.mcpb`.

## pjsched

A clean-room scheduling engine (critical path and resource leveling) written in Rust. Its behavior
comes from black-box experiments against Microsoft Project 16.0.20430. No vendor code was
decompiled or consulted.

## How the behavior is specified

1. `tools/run_scenarios.ps1` and `tools/run_batch2.ps1` build small projects through Project's
   documented COM object model. They export each one as MSPDI XML: `<case>-antes.xml` holds the
   schedule before leveling and `<case>-depois.xml` holds it after `LevelNow`.
2. Those exports live in `tests/golden/`.
3. `tests/golden.rs` reads every `-antes.xml` as input and schedules it. It then compares start,
   finish, early start, late start, total and free slack, the critical flag and the leveling
   delay against the values Project wrote.

4. Round trip: `examples/roundtrip_write.rs` writes every golden input back out as MSPDI.
   `tools/roundtrip.ps1` makes Project reopen and recalculate each file, saving the result under
   `tests/golden/rt`. `tests/roundtrip.rs` checks that Project computes the same start, finish,
   late start and total slack as pjsched.

```bash
cargo test
```

To refresh the round trip after changing the writer (the second command needs Project installed):

```bash
cargo run --example roundtrip_write -- C:\dev\pjsched\target\roundtrip\in
```

```bash
powershell -File tools\roundtrip.ps1 -InDir C:\dev\pjsched\target\roundtrip\in -OutDir C:\dev\pjsched\tests\golden\rt
```

The project pins the `stable-x86_64-pc-windows-gnu` toolchain in `rust-toolchain.toml`, because
that toolchain ships its own linker and this machine has no MSVC Build Tools.

## Desktop app (`app/`) and installer (`setup/`)

Etapix is a desktop planner for internal use, built with egui on top of the engine and laid out
like Project: ribbon (Arquivo, Tarefa, Recurso, Relatório, Projeto, Exibir), timeline, task grid
next to the Gantt chart, and status bar. It leaves `.mpp`/`.xml` file associations alone.

- `.mpp` files open natively, on Windows or Linux, with no Project installed (`src/cfb.rs` reads
  the compound-file container, `src/mpp.rs` decodes Project's tables into Project XML). The reader
  is checked against the XML Project itself saves of 200+ files (`tests/mpp_native.rs`).
- Saving into a `.mpp` is native too (`src/mpp/update.rs`): the changed fields of tasks,
  resources, calendars and assignments, progress (including started tasks at 0% and explicit `ResumeValid` flags on split tasks), baselines, overtime and cost amounts (including cost assignments across started, manual, constrained/ALAP and split rescheduled tasks, with rate-span metadata and task cost rollup), and the
  scheduled dates go into the file's own tables, and everything else (views, formats, custom
  fields) stays as Project left it. Tasks, links, resources and assignments added or removed
  keep the file's own records, retain removed assignments as deleted records so Project preserves rescheduled task costs, and take new ones from the template (`src/mpp/create.rs`,
  `restructure_mpp`). Checked by `tests/mpp_update.rs` and, against Project itself, by
  `examples/mpp_write_check.rs` (nine modes: `prepare`, `prepare-new`, `prepare-structure`, `prepare-progress`, `prepare-baseline`, `prepare-cost`, `prepare-resources`, `prepare-calendars`, `prepare-overtime`, plus `verify`) with `tools/mpp_to_xml.ps1`.
- New `.mpp` files are native as well (`src/mpp/create.rs`): Etapix clones the records of a
  template Project made (`assets/modelo.mpp`, built by `tools/mpp_template.ps1`) for every task,
  link, resource, calendar and assignment, ties them together with their GUIDs, and lets the edit
  writer fill in the fields. Checked by `tests/mpp_create.rs` and by
  `examples/mpp_write_check.rs prepare-new` against Project.
- Microsoft Project is never needed. When it is installed it stays an optional fallback for a
  file the native writer cannot handle (`app/src/mpp.rs`): it converts Etapix's Project XML
  invisibly over COM. A save goes to a temporary file first and replaces the target only when it
  worked.
- Grid: mode, name (indented by outline level), duration ("3 dias"), start and finish
  ("Qui 22/10/26"), predecessors and resource names. Type into an empty row to add a task. Type
  over a cell or press F2 to edit; Enter, Tab and Escape behave as in Project. Every edit
  reschedules, and one that cannot be scheduled (a dependency cycle, say) is undone with a
  message in the status bar. A typed start date becomes a start-no-earlier-than constraint, as
  in Project.
- Tarefa tab: percent complete, indent and outdent (also Alt+Shift+arrows), link and unlink,
  manual or automatic scheduling, insert task, summary and milestone, delete, task information,
  scroll to the task.
- Gantt chart: weeks over days (months over weeks when zoomed out), nonworking days shaded from
  the project calendar, the current date, bars, summaries, milestones, splits, progress,
  resource names and link arrows.
- Recurso: level all (Project's defaults, kept as saved leveling delays) and clear leveling.
  Projeto: project start date and baseline. Relatório: project overview. Exibir: Gantt or task
  sheet, zoom, whole project, timeline on or off.

The interface uses the Etapix typefaces, built into the executable: Inter (text) and Plus Jakarta
Sans (headings and wordmark), both under the SIL Open Font License 1.1 (licences in
`app/assets/fonts`).

To review the layout without touching the screen, run the app with `ETAPIX_SNAPSHOT_DIR` set to
a folder: it opens each ribbon tab, saves a PNG of the window for each, and quits.

## Claude and Codex connector (`integrations/`)

`etapix-mcp` (`app/src/bin/etapix-mcp.rs`) is a Model Context Protocol server over stdio. Its
tools (`criar_projeto`, `ler_projeto`, `adicionar_tarefas`, `editar_tarefa`, `excluir_tarefa`,
`nivelar_recursos`, `definir_linha_de_base`, `salvar_como`) open a project file, apply the same
operations as the app and save it back, so an assistant can build and change schedules that open
in Etapix and Project. `integrations/skill/etapix/SKILL.md` tells the assistant how to use them.

`tools\build-integrations.ps1` packs `dist\etapix.mcpb` (Claude Desktop extension),
`dist\etapix-skill.zip` (for claude.ai) and `dist\Etapix-Claude-Codex-<version>.zip`, whose
`instalar.cmd` registers the server and the skill in Claude Code and Codex and opens the `.mcpb`
in Claude Desktop.

`etapix-setup` is a per-user installer with the app built in: it installs to
`%LOCALAPPDATA%\Programs\etapix` without admin rights, adds a Start Menu shortcut and an entry in
Windows' installed apps; uninstalling removes all three. `/silent` skips every dialog (for IT
deployment): `etapix-setup-0.1.0.exe /silent`, and `desinstalar.exe /uninstall /silent`.

Build both with:

```bash
powershell -ExecutionPolicy Bypass -File tools\build-installer.ps1
```

The script writes `dist\etapix-setup-<version>.exe`. The UI crates need an assembler for
`dlltool`: the script puts the portable WinLibs binutils in `%USERPROFILE%\tools\mingw64` on
`PATH` (the GNU toolchain ships `dlltool` but not `as`).

## Layout

| Module | Responsibility |
|---|---|
| `calendar` | Calendars (weekly shifts and dated exceptions), the absolute clock, and one timeline per task. A timeline is the task's own calendar, its resources' calendar, their intersection, or elapsed 24/7 time; it maps instants to working minutes and back. |
| `model` | Tasks, links, constraints, resources and assignments. |
| `cpm` | Forward and backward passes, total and free slack, critical flag. It also accepts fixed placements produced by leveling. |
| `cost` | Costs per assignment, task, summary and resource: regular and overtime rates, cost per use, rate changes over time (rate table A), material resources (quantity x unit rate) and fixed costs. |
| `tracking` | Baselines (save and read baseline 0), start/finish/work variances and earned value at the status date (BCWS, BCWP, ACWP, CV, SV). |
| `edit` | How Project recalculates duration, units and work when you add or remove a resource or change duration, units or work. It covers the fixed units, fixed duration and fixed work task types, with and without effort-driven scheduling. |
| `leveling` | Serial list scheduling, resource by resource, of assignments (Project's default "level individual assignments") or whole tasks, with the ID, Standard and Priority/Standard orders, day/week/hour bases, splitting (also of a single uneven assignment), priority 1000, a leveling date range and "within available slack". Each assignment occupies its resource only while it works. |
| `cfb` | Reads and writes the Compound File Binary (OLE2) container used by `.mpp` files. |
| `mpp` | Native `.mpp` reader (`src/mpp.rs`), in-place and restructured `.mpp` updater (`src/mpp/update.rs`), and template-based `.mpp` creator (`src/mpp/create.rs`). |
| `mspdi` | Reads MSPDI input and the results Project computed (including dated resource rate periods with independent per-use costs and `ResumeValid` on rescheduled tasks). |
| `mspdi_write` | Writes MSPDI that Project reopens and recalculates to the same schedule. Element order follows the schema, `ProjectExternallyEdited` is 0, and `HonorConstraints` is 0, which is how Project itself writes "honor constraints" on. Tasks in progress get timephased progress, and those without resources get Project's placeholder assignment (resource -65535), which carries the actual and remaining work per day. |

## Covered

The golden tests pass in full: the 87 leveling and CPM cases in `tests/golden.rs` and every step
of the 10 edit series in `tests/edits.rs`. Costs, baseline variances and earned value match Project in every golden file
(`tests/costs.rs`). The round trip agrees on 140 of 141 files; the exception is listed in `tests/roundtrip.rs` (Project's own MSPDI import
adds overtime on top of work, even for its own export).

- Progress: percent complete, actual start (it overrides links and constraints), actual finish
  (it fixes the duration), and work rescheduled past the status date (a split). Completed tasks
  are never critical, and their late dates equal their actual dates. A started task shows its
  actual start as late start and measures slack at the finish. Progress on tasks with resources
  (actual work per assignment), and leveling delays saved in a file, which are kept on reopen.

- Monthly and yearly recurring exceptions (by day or by position, such as the last Friday of
  November; MSPDI months are 0-based). Status date. Timephased work read from MSPDI keeps each
  stretch's own rate.
- "Honor constraints" off (links win in the forward pass; late dates still follow constraints).
  Working-time lags count on the project calendar both ways, between tasks on any calendars.
  Less work on an assignment resets it to its nominal units.
- Leveling period basis. Day (Project's default) and week compare each period's booked work
  with its capacity, so two half-day tasks may share a day; hour and minute need spare units at
  every moment. Critical slack limit and multiple critical paths (each chain end closes its own
  path).
- As-late-as-possible tasks with successors: they move to their late dates and successors
  follow them. A start-no-earlier-than on a summary binds its subtasks; a start-to-start link
  out of a summary reads the summary's (earliest subtask) start.
- Milestones (zero duration). They sit at the exact instant their links ask for (a
  predecessor's 18:00 finish, the project's 08:00 start), with exact late dates.
- Weekly recurring calendar exceptions and multi-day exceptions. Resources on different
  calendars on one task: each assignment works its own resource's hours.
- Manually scheduled tasks. They stay at their manual start whatever their links say; a link
  they violate shows as negative slack on the predecessor, as with a constraint conflict.
- Absolute time. Tasks on different calendars meet at real instants: a half-day predecessor
  that finishes at 10:00 lets a standard-calendar successor start at 10:00.
- Task calendars, resource calendars and their intersection, "ignore resource calendar",
  derived resource calendars with their own exceptions, and exceptions with working hours (a
  working Saturday morning, a short Tuesday).
- Elapsed durations and lags. They start at the exact project start time, count every minute,
  and their slack is in elapsed minutes. Slack is always measured in the task's own calendar.
- All eight constraint types, including conflicts with predecessors. Constraints win, and the
  conflict shows as negative total slack on the task and its predecessors.
- Deadlines. They cap the late finish without moving the task.
- Summary tasks. Links into a summary bind each subtask's start, links out of a summary bind on
  each subtask's finish, and the summary rolls up dates and slack.
- Task types: fixed units, fixed duration and fixed work, with and without effort-driven
  scheduling, across these edits: add or remove a resource, change the duration, change an
  assignment's units, change the task's work, and change an assignment's work.
- Assignment work profiles. Project keeps per-stretch rates. Example: when a fixed-work task is
  stretched and then gets more work, it keeps the slow stretch and adds the extra work at the
  nominal units.
- Tasks with more than one resource during leveling, including partial units that add up and a
  resource that finishes before the task does.

- Finish-to-start, start-to-start, finish-to-finish and start-to-finish links, with lag and lead.
- Weekends, lunch breaks and whole-day calendar exceptions.
- Leveling of a single resource with units at or below 100%.
- Splitting allowed and not allowed.
- All three leveling orders.
- Priority 1000 (never leveled).
- Leveling within available slack.

## Not covered yet (needs new golden experiments)

- Finish-no-later-than on a summary. Project accepts it but its own MSPDI export drops it (the
  summary is written as ASAP and reopening loses the effect), so it cannot travel through MSPDI.
  Project refuses every other constraint on a summary, and finish-to-finish or start-to-finish
  links into one; the reader rejects those too.
- Leveling across very different calendars. With a night-shift task (ignoring its resource
  calendar) and a day task on one resource, Project flags the resource overallocated on a day
  basis (12h booked in an 8h day, peak still 100%) and its leveler stops on a modal dialog, so
  the leveled result cannot be captured by automation. pjsched compares bookings in each task's
  working time and does not reproduce that day-capacity verdict.
