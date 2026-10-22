//! A simulated user: runs the whole interface without a window and throws thousands of clicks,
//! drags, keys and typed text at it, on real projects. After every frame the document must still
//! be consistent (ids, outline, links, schedule) and nothing may panic.

use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

use super::{setup_style, App, Tab};

/// A small deterministic generator, so a failure can be replayed with the same seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.below(10_000) as f32 / 10_000.0) * (hi - lo)
    }
}

const SIZE: Vec2 = Vec2::new(1366.0, 800.0);

fn check(app: &App, step: usize, seed: u64) {
    let p = &app.doc.project;
    let n = p.tasks.len();
    let at = format!("seed {seed}, frame {step}");
    assert_eq!(app.doc.schedule.len(), n, "schedule out of step ({at})");
    for (i, t) in p.tasks.iter().enumerate() {
        assert_eq!(t.id, i as i64 + 1, "ids not renumbered ({at})");
        if let Some(parent) = t.parent {
            assert!(parent < i, "parent below its child ({at})");
            assert!(p.tasks[parent].summary, "parent not marked summary ({at})");
        }
        for l in &t.preds {
            assert!(l.pred < n && l.pred != i, "bad link ({at})");
        }
        let s = &app.doc.schedule[i];
        assert!(s.finish >= s.start, "finish before start ({at})");
    }
    for a in &p.assignments {
        assert!(a.task < n && a.resource < p.resources.len(), "bad assignment ({at})");
    }
}

/// Where a person would click: mostly the ribbon, the grid and the chart.
fn spot(rng: &mut Rng) -> Pos2 {
    match rng.below(10) {
        0 => Pos2::new(rng.f(0.0, 900.0), rng.f(0.0, 26.0)),      // tabs
        1..=3 => Pos2::new(rng.f(0.0, SIZE.x), rng.f(26.0, 130.0)), // ribbon
        4..=7 => Pos2::new(rng.f(24.0, 640.0), rng.f(194.0, 760.0)), // grid
        8 => Pos2::new(rng.f(640.0, SIZE.x), rng.f(194.0, 760.0)), // chart
        _ => Pos2::new(rng.f(0.0, SIZE.x), rng.f(0.0, SIZE.y)),
    }
}

fn input(events: Vec<Event>, time: f64) -> RawInput {
    RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), events, time: Some(time), ..Default::default() }
}

fn run(file: Option<&str>, seed: u64, frames: usize) {
    let ctx = egui::Context::default();
    setup_style(&ctx, None);
    let mut app = App::new(file.map(String::from));
    app.headless = true;
    let mut rng = Rng(seed);
    let keys = [Key::Enter, Key::Tab, Key::Escape, Key::Delete, Key::Insert, Key::ArrowDown, Key::ArrowUp, Key::ArrowLeft, Key::ArrowRight, Key::F2];
    let words = ["Fundação", "3", "2,5", "0", "1;2", "Ana, Bruno", "22/10/26", "x", "", "10d", "99", "-1"];
    let mut pos = Pos2::new(300.0, 300.0);
    let mut t = 0.0;
    let (mut max_tasks, mut messages, mut tabs) = (0, 0, std::collections::HashSet::new());
    for step in 0..frames {
        t += 0.05;
        let mut ev = Vec::new();
        match rng.below(12) {
            0..=4 => {
                // A click (sometimes a double click) somewhere.
                pos = spot(&mut rng);
                let clicks = if rng.below(4) == 0 { 2 } else { 1 };
                ev.push(Event::PointerMoved(pos));
                for _ in 0..clicks {
                    for pressed in [true, false] {
                        ev.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::default() });
                    }
                }
            }
            5 => {
                // A drag, as on a column or row border or the split.
                let from = spot(&mut rng);
                let to = from + Vec2::new(rng.f(-120.0, 120.0), rng.f(-40.0, 40.0));
                ev.push(Event::PointerMoved(from));
                ev.push(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::default() });
                ev.push(Event::PointerMoved(to));
                ev.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::default() });
                pos = to;
            }
            6..=8 => {
                let key = keys[rng.below(keys.len() as u64) as usize];
                let modifiers = match rng.below(8) {
                    0 => Modifiers::SHIFT,
                    1 => Modifiers::ALT | Modifiers::SHIFT,
                    _ => Modifiers::default(),
                };
                // Desfazer and Refazer now and then.
                if rng.below(5) == 0 {
                    let key = if rng.below(2) == 0 { Key::Z } else { Key::Y };
                    ev.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::CTRL });
                }
                ev.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
                ev.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
            }
            9..=10 => ev.push(Event::Text(words[rng.below(words.len() as u64) as usize].to_string())),
            _ => ev.push(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, rng.f(-200.0, 200.0)),
                modifiers: Modifiers::default(),
            }),
        }
        // The pointer stays where it was, as a real mouse does.
        if ev.is_empty() {
            ev.push(Event::PointerMoved(pos));
        }
        let _ = ctx.run(input(ev, t), |ctx| app.frame_ui(ctx));
        // An empty frame lets clicks and popups settle.
        let _ = ctx.run(input(vec![], t + 0.01), |ctx| app.frame_ui(ctx));
        check(&app, step, seed);
        max_tasks = max_tasks.max(app.doc.project.tasks.len());
        tabs.insert(app.tab as u8);
        if let Some(m) = app.message.take() {
            messages += 1;
            if std::env::var_os("ETAPIX_MONKEY_VERBOSE").is_some() {
                eprintln!("  message: {m}");
            }
        }
        // Leave the backstage now and then, or the run would mostly sit there.
        if app.tab == Tab::File && rng.below(3) == 0 {
            app.tab = Tab::Task;
        }
    }
    eprintln!(
        "seed {seed}: {frames} frames, up to {max_tasks} tasks, {} at the end, {} resources, {messages} error messages, {} tabs visited",
        app.doc.project.tasks.len(),
        app.doc.project.resources.len(),
        tabs.len()
    );
}

fn golden(name: &str) -> String {
    format!("{}/../tests/golden/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// `ETAPIX_MONKEY_FRAMES` and `ETAPIX_MONKEY_SEEDS` make a longer run (the defaults keep
/// `cargo test` under a minute).
fn budget() -> (usize, u64) {
    let get = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    (get("ETAPIX_MONKEY_FRAMES", 1500) as usize, get("ETAPIX_MONKEY_SEEDS", 4))
}

#[test]
fn simulated_user_on_a_new_project() {
    let (frames, seeds) = budget();
    for seed in 1..=seeds {
        run(None, seed * 7919, frames);
    }
}

#[test]
fn simulated_user_on_projects_with_outline_links_and_resources() {
    for (k, file) in ["b10/B10-L2-SS-do-resumo-antes.xml", "b1/S1-caminho-critico-antes.xml"].into_iter().enumerate() {
        let path = golden(file);
        if std::path::Path::new(&path).exists() {
            let (frames, seeds) = budget();
            for seed in 0..seeds.div_ceil(2) {
                run(Some(&path), 104_729 + k as u64 + 31 * seed, frames);
            }
        }
    }
}

/// Clicks and keys at fixed places, as a person entering a small schedule would.
#[test]
fn scripted_user_types_tasks_edits_and_undoes() {
    let ctx = egui::Context::default();
    setup_style(&ctx, None);
    let mut app = App::new(None);
    app.headless = true;
    let mut t = 0.0;
    let mut frame = |app: &mut App, events: Vec<Event>| {
        t += 0.1;
        let _ = ctx.run(input(events, t), |ctx| app.frame_ui(ctx));
        let _ = ctx.run(input(vec![], t + 0.01), |ctx| app.frame_ui(ctx));
    };
    let click = |pos: Pos2| {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::default() },
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::default() },
        ]
    };
    let key = |key: Key, modifiers: Modifiers| vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }];
    let text = |s: &str| vec![Event::Text(s.to_string())];
    frame(&mut app, vec![]);
    // The grid's first row starts under the tabs, ribbon, timeline and column header.
    let row = app.row_h;
    let y0 = 26.0 + 104.0 + 64.0 + 40.0 + row / 2.0;
    let w = app.col_w;
    let name_x = 24.0 + w[0] + w[1] + w[2] + 40.0;
    let dur_x = 24.0 + w[0] + w[1] + w[2] + w[3] + 30.0;
    frame(&mut app, click(Pos2::new(name_x, y0)));
    for name in ["Fundação", "Estrutura"] {
        frame(&mut app, text(name));
        frame(&mut app, key(Key::Enter, Modifiers::default()));
    }
    assert_eq!(app.doc.project.tasks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Fundação", "Estrutura"]);
    frame(&mut app, click(Pos2::new(dur_x, y0 + row)));
    frame(&mut app, text("3"));
    frame(&mut app, key(Key::Enter, Modifiers::default()));
    assert_eq!(app.doc.days(1), 3.0);
    // An invalid duration is refused and leaves no task behind in an empty row.
    frame(&mut app, click(Pos2::new(dur_x, y0 + 5.0 * row)));
    frame(&mut app, text("abc"));
    frame(&mut app, key(Key::Enter, Modifiers::default()));
    assert_eq!(app.doc.project.tasks.len(), 2);
    assert!(app.message.take().is_some());
    // Indent the second task under the first (Alt+Shift+Right), then undo it and the duration.
    frame(&mut app, click(Pos2::new(name_x, y0 + row)));
    frame(&mut app, key(Key::ArrowRight, Modifiers::ALT | Modifiers::SHIFT));
    assert!(app.doc.project.tasks[0].summary);
    frame(&mut app, key(Key::Z, Modifiers::CTRL));
    assert!(!app.doc.project.tasks[0].summary);
    frame(&mut app, key(Key::Z, Modifiers::CTRL));
    assert_eq!(app.doc.days(1), 1.0);
    frame(&mut app, key(Key::Y, Modifiers::CTRL));
    assert_eq!(app.doc.days(1), 3.0);
}
