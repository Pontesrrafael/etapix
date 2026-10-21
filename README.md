# pjsched

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

## Layout

| Module | Responsibility |
|---|---|
| `calendar` | Calendars (weekly shifts and dated exceptions), the absolute clock, and one timeline per task. A timeline is the task's own calendar, its resources' calendar, their intersection, or elapsed 24/7 time; it maps instants to working minutes and back. |
| `model` | Tasks, links, constraints, resources and assignments. |
| `cpm` | Forward and backward passes, total and free slack, critical flag. It also accepts fixed placements produced by leveling. |
| `edit` | How Project recalculates duration, units and work when you add or remove a resource or change duration, units or work. It covers the fixed units, fixed duration and fixed work task types, with and without effort-driven scheduling. |
| `leveling` | Serial list scheduling with the ID, Standard and Priority/Standard orders, splitting, priority 1000 and "within available slack". Each assignment occupies its resource only while it works. |
| `mspdi` | Reads MSPDI input and the results Project computed. |
| `mspdi_write` | Writes MSPDI that Project reopens and recalculates to the same schedule. Element order follows the schema, `ProjectExternallyEdited` is 0, and `HonorConstraints` is 0, which is how Project itself writes "honor constraints" on. Tasks in progress get timephased progress, and those without resources get Project's placeholder assignment (resource -65535), which carries the actual and remaining work per day. |

## Covered

The golden tests pass in full: the 81 leveling and CPM cases in `tests/golden.rs` and every step
of the 10 edit series in `tests/edits.rs`. The round trip agrees on all 126 files.

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
- Splitting during leveling for tasks whose assignments do not cover the whole task evenly.
- Any case where the "level individual assignments" setting changes the result. It made no
  difference in batch 3.
- Leveling across different calendars. Bookings are compared as absolute stretches, so
  overnight gaps on unusual calendars could overlap.
- "Level individual assignments" and partial date ranges.
