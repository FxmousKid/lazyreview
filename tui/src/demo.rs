//! Deterministic, offline product tour. Every frame is an actual app render after real inputs.
use crate::{
    app::{App, Pane, s},
    backend, preview,
};
use crossterm::event::{
    KeyCode as K, KeyEvent, KeyModifiers as M, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::{Value, json};
use std::io;
struct Tour {
    a: App,
    terminal: Terminal<TestBackend>,
    frames: Vec<Value>,
    syntax: Option<crate::syntax::Service>,
}
impl Tour {
    fn key(&mut self, k: K) -> Option<Value> {
        self.a.key(KeyEvent::new(k, M::NONE))
    }
    fn c(&mut self, c: char) -> Option<Value> {
        self.key(K::Char(c))
    }
    fn card(&mut self, chapter: &str, title: &str, caption: &str, duration: u64) -> io::Result<()> {
        self.record(chapter, title, caption, &[], duration, None)?;
        self.frames.last_mut().unwrap()["kind"] = json!("card");
        Ok(())
    }
    fn record(
        &mut self,
        chapter: &str,
        title: &str,
        caption: &str,
        keys: &[&str],
        duration: u64,
        focus: Option<usize>,
    ) -> io::Result<()> {
        if let Some(service) = &mut self.syntax {
            for _ in 0..80 {
                service.tick(&mut self.a);
                if self.a.syntax.is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        preview::render_settled(&mut self.terminal, &mut self.a)?;
        let focus = focus.and_then(|i| {
            let r = self.a.panes.areas[i];
            (r.width > 0).then(|| {
                let (top, height) = match i {
                    1 => (r.y + 6, 22u16),
                    2 => (r.y, 18u16),
                    _ => (r.y, 25u16),
                };
                json!([r.x, top, r.width, height.min(44 - top)])
            })
        });
        self.frames.push(json!({"id":format!("scene-{:02}",self.frames.len()+1),"chapter":chapter,"title":title,"caption":caption,"keys":keys,"duration":duration,"focus":focus,"html":preview::buffer_html_with_palette(self.terminal.backend().buffer(), &profile())}));
        Ok(())
    }
}
pub(crate) fn profile() -> Value {
    serde_json::from_str(include_str!("../../demo/terminal-profile.json"))
        .expect("valid pinned display profile")
}
pub(crate) fn apply_profile() {
    // Pin display-only settings; never read or publish a local AI config/session.
    let mut theme = crate::theme::Theme::blue();
    theme.bg = ratatui::style::Color::Reset;
    theme.add_bg = ratatui::style::Color::Reset;
    theme.delete_bg = ratatui::style::Color::Reset;
    theme.hunk_bg = ratatui::style::Color::Reset;
    theme.name = "lazygit";
    theme.apply(&profile()).expect("valid pinned Lazygit theme");
    crate::theme::set(theme);
}
pub fn export(path: &str, bridge: &mut backend::Bridge, sample: Value) -> io::Result<()> {
    apply_profile();
    let mut a = App::new(sample);
    preview::seed(&mut a, "ai");
    a.messages.clear();
    a.panes.panel = false;
    a.pane = Pane::Tree;
    a.status = "Demo · sample PR and scripted replies".into();
    let mut t = Tour {
        a,
        terminal: Terminal::new(TestBackend::new(180, 44)).unwrap(),
        frames: vec![],
        syntax: crate::syntax::Service::start().ok(),
    };
    // Two workflows, with long reading holds. Every code/text selection below is
    // driven through the real handlers; only chapter setup moves the camera/cursor.
    t.card(
        "Find the code",
        "Review the change. Keep the context.",
        "Start with the files that matter.",
        4000,
    )?;
    t.c('0');
    t.record(
        "Find the code",
        "A whole PR. One workspace.",
        "The directory tree stays beside the diff.",
        &[],
        5000,
        None,
    )?;
    t.c('2');
    t.record(
        "Find the code",
        "Start with the tests.",
        "Narrow the files without losing their structure.",
        &["2"],
        5000,
        Some(0),
    )?;
    assert_eq!(t.a.category.as_deref(), Some("Tests"));
    t.c('1');
    t.record(
        "Find the code",
        "Then follow the implementation.",
        "Code, tests, and documentation each have their own view.",
        &["1"],
        5000,
        Some(0),
    )?;
    assert_eq!(t.a.category.as_deref(), Some("Code"));

    t.card(
        "Ask across files",
        "Two pieces of code. One question.",
        "Bring Claude the parts you want to understand.",
        5000,
    )?;
    // The workflow cut gives the code and companion equal room to read.
    t.a.panes.tree = false;
    t.c('g');
    t.c('a');
    t.c('=');
    t.record(
        "Ask across files",
        "Open your companion.",
        "A conversation alongside the PR.",
        &["g", "a"],
        5000,
        Some(2),
    )?;
    t.a.pane = Pane::Diff;
    t.a.select_file(0);
    t.a.line =
        t.a.lines()
            .iter()
            .position(|l| s(l, "text").contains("const results = filterCommands"))
            .expect("Sample caller evidence");
    t.c('v');
    t.c('j');
    t.record(
        "Ask across files",
        "Point to the code.",
        "Start a selection with v. Extend it with j.",
        &["v", "j"],
        6000,
        Some(1),
    )?;
    assert!(t.a.visual.is_some());
    t.c('t');
    t.a.paste("Why reset selection when the query changes?");
    t.record(
        "Ask across files",
        "Leave a question on those lines.",
        "A temporary note for Claude, separate from GitHub comments.",
        &["t"],
        6000,
        None,
    )?;
    t.frames.last_mut().unwrap()["focus"] = json!([43, 12, 94, 20]);
    t.key(K::Enter);
    assert_eq!(t.a.context.len(), 1);
    t.record(
        "Ask across files",
        "Keep the question with its code.",
        "It is queued for your next message.",
        &["Enter"],
        5000,
        Some(1),
    )?;
    t.c('n');
    t.record(
        "Ask across files",
        "Follow the behavior into another file.",
        "The first selection stays attached.",
        &["n"],
        5000,
        Some(1),
    )?;
    t.a.line =
        t.a.lines()
            .iter()
            .position(|l| s(l, "text").contains("if (count === 0)"))
            .expect("Sample empty-result guard");
    t.c('v');
    t.c('j');
    let req = t.c('a').expect("Attach second selection");
    let data = bridge.request(&req);
    t.a.receive("attach", data);
    assert_eq!(t.a.context.len(), 2);
    assert_ne!(t.a.context[0]["path"], t.a.context[1]["path"]);
    t.record(
        "Ask across files",
        "Bring the related range with you.",
        "Select with v and j, then attach with a.",
        &["v", "j", "a"],
        5000,
        Some(1),
    )?;
    t.c('g');
    t.c('c');
    t.c('k');
    t.record(
        "Ask across files",
        "Both files are in the conversation.",
        "Inspect the selected code and its temporary question.",
        &["g", "c"],
        6000,
        None,
    )?;
    t.frames.last_mut().unwrap()["focus"] = json!([47, 12, 87, 20]);
    t.c('i');
    t.a.paste("How do these two pieces work together when results become empty?");
    t.record(
        "Ask across files",
        "Ask about the relationship.",
        "Your message carries both ranges and the attached note.",
        &["i"],
        7000,
        Some(2),
    )?;
    let composer = t.a.panes.areas[2];
    t.frames.last_mut().unwrap()["focus"] = json!([composer.x, 30, composer.width, 12]);
    let request = t.key(K::Enter).expect("Sample chat request");
    t.record(
        "Ask across files",
        "Give Claude the context.",
        "The question, the selected code, and the PR stay together.",
        &["Enter"],
        2000,
        Some(2),
    )?;
    let data = bridge.request(&request);
    t.a.receive("chat", data);
    assert!(t.a.context.is_empty());
    t.a.pane = Pane::Panel;
    t.a.ai.follow = false;
    t.c('g');
    t.c('g');
    t.record(
        "Ask across files",
        "Understand how the pieces connect.",
        "The answer stays with this PR. Temporary notes are now consumed.",
        &[],
        8000,
        Some(2),
    )?;

    let area = t.a.reader.selection.area;
    let buffer = t.terminal.backend().buffer();
    let y = (area.y..area.bottom())
        .find(|&y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .contains("Filtering produces")
        })
        .expect("Visible explanation text");
    for (kind, x) in [
        (MouseEventKind::Down(MouseButton::Left), area.x),
        (
            MouseEventKind::Drag(MouseButton::Left),
            (area.x + 35).min(area.right() - 1),
        ),
    ] {
        t.a.mouse(MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: M::NONE,
        });
    }
    t.record(
        "Ask across files",
        "Take the useful part with you.",
        "Drag to select. Release to copy.",
        &["Drag to select"],
        5000,
        Some(2),
    )?;
    t.a.mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: (area.x + 35).min(area.right() - 1),
        row: y,
        modifiers: M::NONE,
    });
    assert!(
        t.a.copy_pending
            .take()
            .unwrap()
            .starts_with("Filtering produces")
    );
    t.card(
        "Ask across files",
        "Bring your expertise. Build the context.",
        "LazyReview",
        4000,
    )?;
    std::fs::write(
        path,
        serde_json::to_string(
            &json!({"version":1,"cols":180,"rows":44,"terminalProfile":profile(),"frames":t.frames}),
        )?,
    )?;
    Ok(())
}
