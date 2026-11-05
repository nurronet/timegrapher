//! The calibre menu's search, drawn headless.

use super::*;

/// Shapes' words, drawn by one frame.
fn texts(out: &egui::FullOutput) -> Vec<String> {
    let mut t = Vec::new();
    for s in &out.shapes {
        if let egui::Shape::Text(x) = &s.shape {
            t.push(x.galley.text().to_string());
        }
    }
    t
}

/// One frame of a 300 pt wide Calibre menu in a `screen` sized window.
fn frame(
    ctx: &egui::Context,
    screen: egui::Rect,
    c: &mut Calibres,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_width(300.0);
                c.picker(ui, "", None);
            });
        },
    )
}

/// A click on the menu's button, at the top of the panel.
fn click_the_button(ctx: &egui::Context, screen: egui::Rect, c: &mut Calibres) {
    let at = egui::pos2(60.0, 18.0);
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(ctx, screen, c, vec![]);
    frame(
        ctx,
        screen,
        c,
        vec![egui::Event::PointerMoved(at), press(true)],
    );
    frame(ctx, screen, c, vec![press(false)]);
}

#[test]
fn the_calibre_menu_searches_every_calibre() {
    let mut c = Calibres::in_memory();
    // One the short list doesn't hold, so only a search finds it.
    let rare = calibres::all().iter().find(|x| !x.common).unwrap();
    assert!(!c.in_short(rare));
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 800.0));
    let frame = |c: &mut Calibres, events: Vec<egui::Event>| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(300.0);
                    c.picker(ui, "", None);
                });
            },
        )
    };
    frame(&mut c, vec![]);
    // The menu's button, at the top of the panel.
    let at = egui::pos2(60.0, 18.0);
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(&mut c, vec![egui::Event::PointerMoved(at), press(true)]);
    frame(&mut c, vec![press(false)]);
    let open = frame(&mut c, vec![]);
    assert!(
        texts(&open).iter().any(|t| t == "Search every calibre"),
        "{:?}",
        texts(&open)
    );
    frame(&mut c, vec![egui::Event::Text(rare.calibre.clone())]);
    let found = frame(&mut c, vec![]);
    let want = format!("{} · {}", rare.calibre, rare.maker);
    assert!(
        texts(&found).contains(&want),
        "{want} not in {:?}",
        texts(&found)
    );
}

#[test]
fn manage_calibres_stays_in_view_under_a_long_short_list() {
    let mut c = Calibres::in_memory();
    for x in calibres::all() {
        c.set_short(&x.calibre, true);
    }
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 600.0));
    let frame = |c: &mut Calibres, events: Vec<egui::Event>| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(300.0);
                    c.picker(ui, "", None);
                });
            },
        )
    };
    frame(&mut c, vec![]);
    let at = egui::pos2(60.0, 18.0);
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(&mut c, vec![egui::Event::PointerMoved(at), press(true)]);
    frame(&mut c, vec![press(false)]);
    for _ in 0..10 {
        frame(&mut c, vec![]);
    }
    let open = frame(&mut c, vec![]);
    // Drawn, and inside what its clip lets the eye see.
    let seen = open.shapes.iter().any(|s| match &s.shape {
        egui::Shape::Text(x) if x.galley.text() == "Manage Calibres…" => {
            let r = x.visual_bounding_rect();
            s.clip_rect.contains_rect(r) && screen.contains_rect(r)
        }
        _ => false,
    });
    assert!(seen, "Manage Calibres… scrolled out of view");
}

/// Where "Manage Calibres…" sits when its clip lets the eye see it whole.
fn manage_seen_at(out: &egui::FullOutput) -> Option<f32> {
    out.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(x) if x.galley.text() == "Manage Calibres…" => {
            let r = x.visual_bounding_rect();
            s.clip_rect.contains_rect(r).then_some(r.top())
        }
        _ => None,
    })
}

#[test]
fn the_calibre_menu_keeps_its_height_after_a_search() {
    // egui sizes a menu from its last frame, so a search listing few or no
    // calibres once left the menu that small, Manage Calibres… cut off,
    // until the app restarted.
    let mut c = Calibres::in_memory();
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0));
    let settle = |c: &mut Calibres, events: Vec<egui::Event>| {
        frame(&ctx, screen, c, events);
        for _ in 0..4 {
            frame(&ctx, screen, c, vec![]);
        }
        frame(&ctx, screen, c, vec![])
    };
    click_the_button(&ctx, screen, &mut c);
    let open = settle(&mut c, vec![]);
    let first = manage_seen_at(&open).expect("Manage Calibres… in view when the menu opens");
    let backspace = |pressed| egui::Event::Key {
        key: egui::Key::Backspace,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    // A few matches, then none at all, each typed and then wiped out.
    for typed in ["289", "zzzq"] {
        for ch in typed.chars() {
            let during = settle(&mut c, vec![egui::Event::Text(ch.to_string())]);
            assert_eq!(
                manage_seen_at(&during),
                Some(first),
                "the menu changed height searching {typed:?}"
            );
        }
        for _ in typed.chars() {
            frame(&ctx, screen, &mut c, vec![backspace(true)]);
            settle(&mut c, vec![backspace(false)]);
        }
        let cleared = settle(&mut c, vec![]);
        assert!(
            texts(&cleared).iter().any(|t| t == "Short List"),
            "the search {typed:?} was not wiped out"
        );
        assert_eq!(
            manage_seen_at(&cleared),
            Some(first),
            "the menu stayed small after searching {typed:?}"
        );
    }
}

/// Where `words` is drawn, if anywhere.
fn drawn_at(out: &egui::FullOutput, words: &str) -> Option<egui::Rect> {
    out.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(x) if x.galley.text() == words => Some(x.visual_bounding_rect()),
        _ => None,
    })
}

fn key(k: egui::Key) -> Vec<egui::Event> {
    let e = |pressed| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    vec![e(true), e(false)]
}

fn click(at: egui::Pos2) -> Vec<egui::Event> {
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    vec![egui::Event::PointerMoved(at), press(true), press(false)]
}

/// The search menu's rules (Peter, 5 Nov; docs/design/design-guide.md,
/// "Search menus"): a click in its field keeps it open with the caret
/// there; Esc clears the text first and only then closes it; each open
/// starts with an empty search; ↓ and Enter pick from the list.
#[test]
fn a_click_in_the_calibre_search_keeps_the_menu_open() {
    let mut c = Calibres::in_memory();
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0));
    let mut picked = None;
    let mut run = |c: &mut Calibres, events: Vec<egui::Event>| {
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(300.0);
                    if let Some(p) = c.picker(ui, "", None) {
                        picked = Some(p);
                    }
                });
            },
        );
        let settled = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(300.0);
                    c.picker(ui, "", None);
                });
            },
        );
        drop(out);
        settled
    };
    let open =
        |c: &mut Calibres,
         run: &mut dyn FnMut(&mut Calibres, Vec<egui::Event>) -> egui::FullOutput| {
            run(c, vec![]);
            run(c, click(egui::pos2(60.0, 18.0)))
        };
    let shown = open(&mut c, &mut run);
    let field = drawn_at(&shown, "Search every calibre").expect("the search field");
    // A click in the field: still open.
    let after = run(&mut c, click(field.center()));
    assert!(
        drawn_at(&after, "Search every calibre").is_some(),
        "closed by a click in its field"
    );
    // Typing goes into the field, still open.
    let typed = run(&mut c, vec![egui::Event::Text("2824".into())]);
    assert!(
        drawn_at(&typed, "Short List").is_none(),
        "a search lists its hits"
    );
    // A ⊗ clears it, keeping the menu open.
    let clear = drawn_at(&typed, "⊗").unwrap_or_else(|| panic!("{:?}", texts(&typed)));
    let cleared = run(&mut c, click(clear.center()));
    assert!(
        drawn_at(&cleared, "Short List").is_some(),
        "⊗ clears the search"
    );
    // Esc clears the text first, then closes.
    run(&mut c, vec![egui::Event::Text("2824".into())]);
    let esc = run(&mut c, key(egui::Key::Escape));
    assert!(
        drawn_at(&esc, "Short List").is_some(),
        "the first Esc clears the text"
    );
    let esc = run(&mut c, key(egui::Key::Escape));
    assert!(
        drawn_at(&esc, "Search every calibre").is_none(),
        "Esc on an empty field closes"
    );
    // Each open starts empty, with the caret in the field.
    run(&mut c, vec![egui::Event::Text("zzz".into())]);
    let again = open(&mut c, &mut run);
    assert!(
        drawn_at(&again, "Short List").is_some(),
        "a new open starts with no search"
    );
    // ↓ then Enter picks the second hit.
    let hits = run(&mut c, vec![egui::Event::Text("2824".into())]);
    let names: Vec<String> = texts(&hits)
        .into_iter()
        .filter(|t| t.contains(" · "))
        .collect();
    assert!(names.len() >= 2, "{names:?}");
    run(&mut c, key(egui::Key::ArrowDown));
    run(&mut c, key(egui::Key::Enter));
    let second = names[1].split(" · ").next().unwrap().to_string();
    assert_eq!(picked.as_deref(), Some(second.as_str()));
}

/// Tab and Shift+Tab stay within the open search menu: however often they
/// are pressed, the focus never reaches a control in the window behind.
#[test]
fn tab_stays_within_the_calibre_menu() {
    let mut c = Calibres::in_memory();
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0));
    let behind = std::cell::RefCell::new(Vec::new());
    let run = |c: &mut Calibres, events: Vec<egui::Event>| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(300.0);
                    let before = ui.button("Before").id;
                    c.picker(ui, "", None);
                    let after = ui.button("After").id;
                    *behind.borrow_mut() = vec![before, after];
                });
            },
        )
    };
    run(&mut c, vec![]);
    let at = egui::pos2(60.0, 18.0 + 26.0);
    let shown = (0..3).map(|_| run(&mut c, vec![])).last().unwrap();
    // The menu's button, under "Before".
    let button = shown
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(x) if x.galley.text() == "Not Set" => Some(x.visual_bounding_rect()),
            _ => None,
        })
        .map_or(at, |r| r.center());
    run(&mut c, click(button));
    let open = run(&mut c, vec![]);
    assert!(
        drawn_at(&open, "Search every calibre").is_some(),
        "the menu opened"
    );
    let field_focused = ctx.memory(|m| m.focused());
    assert!(field_focused.is_some(), "the caret is in the field");
    // A search with no hit: the field, its ⊗ and Manage Calibres… are all
    // the menu holds, so a few Tabs run past its end.
    run(&mut c, vec![egui::Event::Text("zzzq".into())]);
    for shift in [true, false] {
        for n in 0..8 {
            let tab = |pressed| egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers {
                    shift,
                    ..Default::default()
                },
            };
            run(&mut c, vec![tab(true), tab(false)]);
            let focused = ctx.memory(|m| m.focused());
            assert!(
                focused.is_none_or(|f| !behind.borrow().contains(&f)),
                "Tab {n} (shift {shift}) reached the window behind"
            );
        }
    }
}

/// Enter straight after opening keeps the calibre picked: the highlight
/// starts on it (Walter's walk of #1192: it picked Not Set and wiped the
/// watch's calibre). Esc on the empty field closes the menu alone: a
/// dialog behind it never sees the key.
#[test]
fn enter_on_opening_keeps_the_calibre_and_esc_stays_in_the_menu() {
    let mut c = Calibres::in_memory();
    let current = calibres::all()
        .iter()
        .find(|x| c.in_short(x))
        .expect("a calibre on the short list")
        .calibre
        .clone();
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0));
    let picked = std::cell::RefCell::new(None);
    let esc_behind = std::cell::Cell::new(false);
    let run = |c: &mut Calibres, events: Vec<egui::Event>| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(300.0);
                    if let Some(p) = c.picker(ui, &current, None) {
                        *picked.borrow_mut() = Some(p);
                    }
                    esc_behind.set(ui.input(|i| i.key_pressed(egui::Key::Escape)));
                });
            },
        )
    };
    run(&mut c, vec![]);
    run(&mut c, click(egui::pos2(60.0, 18.0)));
    run(&mut c, vec![]);
    run(&mut c, key(egui::Key::Enter));
    assert_eq!(picked.borrow().as_deref(), Some(current.as_str()));
    // Again, and Esc on the empty field.
    run(&mut c, vec![]);
    run(&mut c, vec![]);
    run(&mut c, click(egui::pos2(60.0, 18.0)));
    let open = run(&mut c, vec![]);
    assert!(
        drawn_at(&open, "Search every calibre").is_some(),
        "open again"
    );
    run(&mut c, key(egui::Key::Escape));
    assert!(!esc_behind.get(), "the window behind saw the menu's Esc");
    let shut = run(&mut c, vec![]);
    assert!(
        drawn_at(&shut, "Search every calibre").is_none(),
        "Esc closed the menu"
    );
}
