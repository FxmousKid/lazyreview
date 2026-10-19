//! Input adapter for the hosted sample. Uses the real App and Ratatui renderer.
//! No provider transport, shell, repository picker, or live GitHub entry point.
use crate::{
    app::{App, Pane},
    backend, demo, preview, syntax,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Modifier};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

pub struct BrowserDemo {
    app: App,
    bridge: backend::Bridge,
    terminal: Terminal<TestBackend>,
    previous: Option<Buffer>,
    syntax: syntax::Service,
}
impl BrowserDemo {
    pub fn new() -> Result<Self, String> {
        demo::apply_profile();
        let mut bridge = backend::Bridge::sample_only()?;
        let sample = backend::initial(&mut bridge, None)?;
        if sample["demo"] != true {
            return Err("Expected synthetic PR".into());
        }
        let mut app = App::new(sample);
        app.messages.clear();
        app.panes.panel = false;
        app.pane = Pane::Tree;
        app.status = "Sample PR · changes stay in this demo".into();
        Ok(Self {
            app,
            bridge,
            terminal: Terminal::new(TestBackend::new(140, 36)).map_err(|e| e.to_string())?,
            previous: None,
            syntax: syntax::Service::start_sample()?,
        })
    }
    fn request(&mut self, request: Value) {
        let op = request["operation"].as_str().unwrap_or("");
        // App-generated requests only. The browser cannot supply bridge operations.
        let result = if matches!(
            op,
            "attach"
                | "chat"
                | "viewed"
                | "draft"
                | "submit"
                | "reply"
                | "resolve"
                | "conversation"
        ) {
            self.bridge.request(&request)
        } else {
            Err(
                "This demo uses the sample PR. Install LazyReview to connect GitHub or an AI CLI."
                    .into(),
            )
        };
        self.app.receive(op, result);
    }
    pub fn update(&mut self, input: &Value) -> Result<Value, String> {
        let events = input["events"].as_array().ok_or("Expected input events")?;
        if events.len() > 64 {
            return Err("Too many input events".into());
        }
        for event in events {
            let request = match event["type"].as_str() {
                Some("resize") => {
                    let cols = event["cols"]
                        .as_u64()
                        .filter(|n| (40..=220).contains(n))
                        .ok_or("Invalid columns")? as u16;
                    let rows = event["rows"]
                        .as_u64()
                        .filter(|n| (16..=70).contains(n))
                        .ok_or("Invalid rows")? as u16;
                    self.terminal.backend_mut().resize(cols, rows);
                    self.terminal
                        .resize(ratatui::layout::Rect::new(0, 0, cols, rows))
                        .map_err(|e| e.to_string())?;
                    self.previous = None;
                    None
                }
                Some("key") => self.app.key(key(event)?),
                Some("paste") => {
                    let text = event["text"]
                        .as_str()
                        .filter(|s| s.len() <= 10_000)
                        .ok_or("Paste too long")?;
                    self.app.paste(text);
                    None
                }
                Some("mouse") => self.app.mouse(mouse(event)?),
                Some("poll") => None,
                _ => return Err("Unknown input event".into()),
            };
            if let Some(request) = request {
                self.request(request);
            }
            // Leaving the sample does not expose a chooser or end the browser session.
            if self.app.quit {
                self.app.quit = false;
                self.app.status = "Use Reset to start a fresh sample.".into();
            }
        }
        // A short initial wait gives the genuine syntax worker time to fill its cache.
        for _ in 0..20 {
            self.syntax.tick(&mut self.app);
            if self.app.syntax.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        preview::render_settled(&mut self.terminal, &mut self.app).map_err(|e| e.to_string())?;
        let buffer = self.terminal.backend().buffer();
        let full = self.previous.is_none();
        let profile = demo::profile();
        let mut cells = Vec::new();
        for y in 0..buffer.area.height {
            let mut x = 0;
            while x < buffer.area.width {
                let cell = &buffer[(x, y)];
                let width = ratatui::text::Span::raw(cell.symbol()).width().max(1) as u16;
                if full
                    || self
                        .previous
                        .as_ref()
                        .and_then(|b| b.cell((x, y)))
                        .is_none_or(|old| old != cell)
                {
                    let mut fg = preview::terminal_color(cell.fg, true, &profile);
                    let mut bg = preview::terminal_color(cell.bg, false, &profile);
                    if cell.modifier.contains(Modifier::REVERSED) {
                        std::mem::swap(&mut fg, &mut bg);
                    }
                    let flags = u8::from(cell.modifier.contains(Modifier::BOLD))
                        | u8::from(cell.modifier.contains(Modifier::ITALIC)) << 1
                        | u8::from(cell.modifier.contains(Modifier::UNDERLINED)) << 2
                        | u8::from(cell.modifier.contains(Modifier::CROSSED_OUT)) << 3;
                    cells.push(json!([x, y, cell.symbol(), width, fg, bg, flags]));
                }
                x += width;
            }
        }
        let response = json!({"ok":true,"full":full,"cols":buffer.area.width,"rows":buffer.area.height,"cells":cells,
            "copy":self.app.copy_pending.take(),
            "state":{"pane":self.app.pane.index(),"category":self.app.category,"file":self.app.current()["filename"],
            "treeCursor":self.app.tree_cursor,"folded":self.app.collapsed.len(),"line":self.app.line,"searchQuery":self.app.search.query,
            "split":self.app.split,"selection":self.app.visual.is_some(),"context":self.app.context.len(),"messages":self.app.messages.len(),
            "areas":self.app.panes.areas.map(|r|[r.x,r.y,r.width,r.height]),"readerArea":[self.app.reader.selection.area.x,self.app.reader.selection.area.y,self.app.reader.selection.area.width,self.app.reader.selection.area.height],
            "drafts":self.app.my_comments().iter().filter(|c|c["pending"]==true).count(),"posted":self.app.my_comments().iter().filter(|c|c["pending"]!=true).count(),"modal":self.app.modal.map(|m|format!("{m:?}")),"editing":matches!(self.app.modal,Some(crate::app::Modal::Editor(_))),
            "panelVisible":self.app.panes.panel,"panel":format!("{:?}",self.app.panel),"status":self.app.status},
            "text":if full {Some(buffer_text(buffer))} else {None}});
        self.previous = Some(buffer.clone());
        Ok(response)
    }
}
fn buffer_text(buffer: &Buffer) -> String {
    let mut text = String::new();
    for y in 0..buffer.area.height {
        let mut x = 0;
        while x < buffer.area.width {
            let c = &buffer[(x, y)];
            text.push_str(c.symbol());
            x += ratatui::text::Span::raw(c.symbol()).width().max(1) as u16;
        }
        text.push('\n');
    }
    text
}
fn modifiers(event: &Value) -> KeyModifiers {
    let mut m = KeyModifiers::NONE;
    for (name, flag) in [
        ("ctrl", KeyModifiers::CONTROL),
        ("shift", KeyModifiers::SHIFT),
        ("alt", KeyModifiers::ALT),
    ] {
        if event[name] == true {
            m |= flag;
        }
    }
    m
}
fn key(event: &Value) -> Result<KeyEvent, String> {
    let name = event["key"].as_str().ok_or("Missing key")?;
    let code = match name {
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Esc,
        "Tab" if event["shift"] == true => KeyCode::BackTab,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "ArrowUp" => KeyCode::Up,
        "ArrowDown" => KeyCode::Down,
        "ArrowLeft" => KeyCode::Left,
        "ArrowRight" => KeyCode::Right,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        _ => {
            let mut c = name.chars();
            let ch = c.next().ok_or("Missing key")?;
            if c.next().is_some() || ch.is_control() {
                return Err("Unsupported key".into());
            }
            KeyCode::Char(ch)
        }
    };
    Ok(KeyEvent::new(code, modifiers(event)))
}
fn mouse(event: &Value) -> Result<MouseEvent, String> {
    let kind = match event["action"].as_str() {
        Some("down") => MouseEventKind::Down(MouseButton::Left),
        Some("up") => MouseEventKind::Up(MouseButton::Left),
        Some("drag") => MouseEventKind::Drag(MouseButton::Left),
        Some("scrollUp") => MouseEventKind::ScrollUp,
        Some("scrollDown") => MouseEventKind::ScrollDown,
        _ => return Err("Unsupported mouse action".into()),
    };
    let x = event["x"]
        .as_u64()
        .filter(|n| *n < 220)
        .ok_or("Invalid mouse column")? as u16;
    let y = event["y"]
        .as_u64()
        .filter(|n| *n < 70)
        .ok_or("Invalid mouse row")? as u16;
    Ok(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: modifiers(event),
    })
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut demo = BrowserDemo::new().map_err(io::Error::other)?;
    let mut stdout = io::stdout().lock();
    // Limit every input before allocating an unbounded line.
    let mut stdin = io::stdin().lock();
    loop {
        let mut line = Vec::new();
        let size = std::io::Read::take(&mut stdin, 32_769).read_until(b'\n', &mut line)?;
        if size == 0 {
            break;
        }
        if size > 32_768 {
            return Err(io::Error::other("Input too large").into());
        }
        let result = serde_json::from_slice::<Value>(&line)
            .map_err(|_| "Invalid JSON".to_owned())
            .and_then(|v| demo.update(&v));
        let output = result.unwrap_or_else(|e| json!({"ok":false,"error":e}));
        writeln!(stdout, "{output}")?;
        stdout.flush()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_uses_real_navigation_selection_notes_and_review_handlers() {
        let mut d = BrowserDemo::new().unwrap();
        let send =
            |d: &mut BrowserDemo, events: Value| d.update(&json!({"events":events})).unwrap();
        let first = send(&mut d, json!([{"type":"resize","cols":140,"rows":38}]));
        assert_eq!(first["full"], true);
        assert!(first["cells"].as_array().unwrap().len() > 1000);
        let f = send(&mut d, json!([{"type":"key","key":"2"}]));
        assert_eq!(f["state"]["category"], "Tests");
        send(
            &mut d,
            json!([{"type":"key","key":"1"},{"type":"key","key":"Tab"},{"type":"key","key":"v"},{"type":"key","key":"j"},{"type":"key","key":"t"}]),
        );
        let f = send(
            &mut d,
            json!([{"type":"paste","text":"Explain this boundary"},{"type":"key","key":"Enter"}]),
        );
        assert_eq!(f["state"]["context"], 1);
        let f = send(
            &mut d,
            json!([{"type":"key","key":"g"},{"type":"key","key":"a"},{"type":"key","key":"i"},{"type":"paste","text":"How does it work?"},{"type":"key","key":"Enter"}]),
        );
        assert_eq!(f["state"]["messages"], 2);
        assert_eq!(f["state"]["context"], 0);
        assert!(
            d.app.messages[1]["content"]
                .as_str()
                .unwrap()
                .contains("SAMPLE")
        );
        assert!(
            send(&mut d, json!([]))["cells"].as_array().unwrap().len()
                < first["cells"].as_array().unwrap().len()
        );
        assert!(
            d.update(&json!({"events":[{"type":"resize","cols":10000,"rows":4}]}))
                .is_err()
        );
        d.request(json!({"operation":"load","url":"https://github.com/private/repo/pull/1"}));
        assert!(d.app.is_demo());
        assert!(d.app.status.contains("sample PR"));
    }
    #[test]
    fn input_supports_shift_tab_chords_and_rejects_non_key_payloads() {
        assert_eq!(
            key(&json!({"key":"Tab","shift":true})).unwrap().code,
            KeyCode::BackTab
        );
        assert!(
            key(&json!({"key":"j","ctrl":true}))
                .unwrap()
                .modifiers
                .contains(KeyModifiers::CONTROL)
        );
        assert!(key(&json!({"key":"shell command"})).is_err());
        assert!(mouse(&json!({"action":"down","x":9999,"y":0})).is_err());
    }
}
