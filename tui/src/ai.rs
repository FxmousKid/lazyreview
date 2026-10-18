//! Independent streaming transport; GitHub review work never waits for the AI CLI.
use crate::app::{App, s};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

pub fn config_path() -> PathBuf {
    crate::preferences::write_path()
}
pub fn config() -> Result<Value, String> {
    let path = crate::preferences::read_path();
    let doc: Value = if path.exists() {
        serde_yaml_ng::from_str(
            &std::fs::read_to_string(&path).map_err(|_| "Cannot read LazyReview configuration")?,
        )
        .map_err(|_| "Invalid LazyReview YAML")?
    } else {
        json!({})
    };
    let mut value = json!({"enabled":false,"provider":"claude","profile":"default","command":"claude","model":"sonnet","autoPrepare":true,"maxBudgetUsd":1.0,"interface":"paper"});
    if !doc["ai"].is_null() {
        let fields = doc["ai"].as_object().ok_or("ai must be a YAML mapping")?;
        for (k, v) in fields {
            if value.get(k).is_none() {
                return Err(format!("Unknown ai setting: {k}"));
            }
            value[k] = v.clone();
        }
    }
    let provider = value["provider"].as_str().unwrap_or("").to_owned();
    if !matches!(provider.as_str(), "claude" | "codex" | "grok") {
        return Err("ai.provider must be claude, codex or grok".into());
    }
    if provider != "claude" {
        if doc["ai"]["command"].is_null() {
            value["command"] = json!(provider);
        }
        if doc["ai"]["model"].is_null() {
            value["model"] = json!("default");
        }
        if doc["ai"]["interface"].is_null() {
            value["interface"] = json!("terminal");
        }
        if value["interface"] == "paper" {
            return Err("Codex and Grok require ai.interface: terminal".into());
        }
    }
    if !matches!(value["interface"].as_str(), Some("paper" | "terminal")) {
        return Err("ai.interface must be paper or terminal".into());
    }
    for key in ["enabled", "autoPrepare"] {
        if !value[key].is_boolean() {
            return Err(format!("ai.{key} must be true or false"));
        }
    }
    for key in ["profile", "command", "model"] {
        if value[key]
            .as_str()
            .is_none_or(|s| s.is_empty() || s.len() > 4096)
        {
            return Err(format!("Invalid ai.{key}"));
        }
    }
    if value["maxBudgetUsd"]
        .as_f64()
        .is_none_or(|n| n <= 0.0 || n > 100.0)
    {
        return Err("ai.maxBudgetUsd must be between 0 and 100".into());
    }
    Ok(value)
}
pub fn enable() -> Result<Value, String> {
    let mut cfg = config()?;
    cfg["enabled"] = json!(true);
    let path = config_path();
    let source = crate::preferences::read_path();
    let mut doc: Value = if source.exists() {
        serde_yaml_ng::from_str(
            &std::fs::read_to_string(&source).map_err(|_| "Cannot read configuration")?,
        )
        .map_err(|_| "Invalid YAML")?
    } else {
        json!({})
    };
    if !doc.is_object() {
        return Err("Configuration must be a YAML mapping".into());
    }
    doc["ai"] = cfg.clone();
    std::fs::create_dir_all(path.parent().unwrap())
        .map_err(|_| "Cannot create config directory")?;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())
        .map_err(|_| "Cannot prepare configuration")?;
    temp.write_all(
        serde_yaml_ng::to_string(&doc)
            .map_err(|_| "Cannot encode configuration")?
            .as_bytes(),
    )
    .map_err(|_| "Cannot write configuration")?;
    temp.persist(path)
        .map_err(|_| "Cannot save configuration")?;
    Ok(cfg)
}
#[derive(Default)]
pub struct State {
    pub enabled: bool,
    pub provider_label: String,
    pub binding: String,
    pub phase: String,
    pub error: String,
    pub session: String,
    pub summary: String,
    pub streaming: String,
    pub busy: bool,
    pub follow: bool,
    pub display_truncated: bool,
    pub artifacts: Vec<Value>,
    pub evidence: Value,
    pub native_mode: bool,
    pub native_input: bool,
    pub native_area: ratatui::layout::Rect,
    pub native_view: Option<crate::native::Shared>,
    pub native_frozen: Option<vt100::Screen>,
    pub native_launch: Option<Value>,
}
impl State {
    pub fn name(&self) -> &str {
        match self.provider_label.as_str() {
            "Codex" => "Codex",
            "Grok" => "Grok",
            _ => "Claude",
        }
    }
    pub fn label(&self) -> &str {
        if !self.enabled {
            "Set up Claude"
        } else if self.phase.is_empty() {
            "Connecting"
        } else {
            &self.phase
        }
    }
}
struct Transport {
    child: Child,
    input: Option<ChildStdin>,
    rx: Receiver<Value>,
    join: Option<thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}
impl Transport {
    fn start() -> Result<Self, String> {
        let dir = tempfile::tempdir().map_err(|_| "Cannot prepare companion")?;
        for (name, body) in [
            ("ai-session.mjs", include_str!("ai-session.mjs")),
            ("ai-context.mjs", include_str!("ai-context.mjs")),
            ("ai-native-hook.mjs", include_str!("ai-native-hook.mjs")),
            ("ai-terminal.mjs", include_str!("ai-terminal.mjs")),
            ("ai-providers.mjs", include_str!("ai-providers.mjs")),
            ("ai-codex.mjs", include_str!("ai-codex.mjs")),
            ("ai-tools.mjs", include_str!("ai-tools.mjs")),
            ("syntax.mjs", include_str!("syntax.mjs")),
            ("artifacts.mjs", include_str!("artifacts.mjs")),
        ] {
            std::fs::write(dir.path().join(name), body).map_err(|_| "Cannot prepare companion")?;
        }
        let mut child = Command::new("node")
            .arg(dir.path().join("ai-session.mjs"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Node.js is required for AI CLI integration")?;
        let input = child.stdin.take();
        let output = child.stdout.take().ok_or("Cannot read companion")?;
        let (tx, rx) = mpsc::channel();
        let join = thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if let Ok(event) = serde_json::from_str(&line) {
                    if tx.send(event).is_err() {
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            input,
            rx,
            join: Some(join),
            _dir: dir,
        })
    }
    fn send(&mut self, value: Value) -> Result<(), String> {
        writeln!(self.input.as_mut().ok_or("Companion stopped")?, "{value}")
            .map_err(|_| "Companion stopped. Restart LazyReview.".to_string())
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.input.take(); // EOF lets the bridge stop only its own Claude process group.
        let deadline = Instant::now() + Duration::from_secs(4);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
pub struct Service {
    pub config: Value,
    transport: Option<Transport>,
    key: String,
    serial: u64,
    terminal: Option<crate::native::Terminal>,
    native_requested: bool,
    context_key: String,
}
impl Service {
    pub fn new(config: Value) -> Self {
        Self {
            config,
            transport: None,
            key: String::new(),
            serial: 0,
            terminal: None,
            native_requested: false,
            context_key: String::new(),
        }
    }
    fn send(&mut self, value: Value) -> Result<(), String> {
        if self.transport.is_none() {
            self.transport = Some(Transport::start()?);
        }
        self.transport.as_mut().unwrap().send(value)
    }
    pub fn tick(&mut self, app: &mut App) {
        app.ai.enabled = self.config["enabled"] == true;
        if app.is_demo() {
            self.terminal.take();
            if !self.key.is_empty() {
                if let Some(t) = self.transport.as_mut() {
                    let _ = t.send(json!({"operation":"close"}));
                }
                self.key.clear();
            }
            return;
        }
        if !app.ai.enabled {
            return;
        }
        let key = format!(
            "{}:{}:{}:{}:{}",
            s(&app.pr, "url"),
            s(&app.pr, "base"),
            s(&app.pr, "head"),
            self.config,
            app.syntax_generation
        );
        if self.key != key || app.ai.binding.is_empty() {
            self.terminal.take();
            self.native_requested = false;
            self.context_key.clear();
            self.key = key;
            self.serial += 1;
            app.ai = State {
                enabled: true,
                binding: self.serial.to_string(),
                phase: "Connecting".into(),
                provider_label: match self.config["provider"].as_str() {
                    Some("codex") => "Codex",
                    Some("grok") => "Grok",
                    _ => "Claude Code",
                }
                .into(),
                native_mode: self.config["interface"] == "terminal",
                follow: true,
                ..Default::default()
            };
            let mut pr = app.pr.clone();
            if let Some(files) = pr["files"].as_array_mut() {
                for file in files {
                    if let Some(obj) = file.as_object_mut() {
                        obj.remove("lines");
                        obj.remove("splitRows");
                    }
                }
            }
            if let Err(error)=self.send(json!({"operation":"open","binding":app.ai.binding,"config":self.config,"pr":pr,"focus":focus(app)})){app.ai.error=error;app.ai.phase="Error".into();}
        }
        if let Some(t) = self.transport.as_mut() {
            while let Ok(event) = t.rx.try_recv() {
                apply_event(app, event);
            }
            if t.child.try_wait().ok().flatten().is_some() {
                app.ai.busy = false;
                app.ai.phase = "Error".into();
                app.ai.error = "Companion stopped. Restart LazyReview.".into();
            }
        }
        if app.ai.native_mode {
            let context = json!({"context":app.context,"focus":focus(app)});
            let encoded = context.to_string();
            if encoded != self.context_key {
                self.context_key = encoded;
                let _=self.send(json!({"operation":"native_context","binding":app.ai.binding,"context":app.context,"focus":focus(app)}));
            }
            if app.panes.panel && app.panel == crate::app::Panel::Ai && !self.native_requested {
                self.native_requested = true;
                if let Err(e) = self.send(json!({"operation":"terminal","binding":app.ai.binding}))
                {
                    app.ai.error = e;
                }
            }
            if let Some(spec) = app.ai.native_launch.take() {
                self.terminal.take();
                let area = app.ai.native_area;
                match crate::native::Terminal::start(
                    &spec,
                    if area.height > 0 { area.height } else { 24 },
                    if area.width > 0 { area.width } else { 84 },
                ) {
                    Ok(terminal) => {
                        app.ai.native_view = Some(terminal.view.clone());
                        app.ai.native_input = true;
                        app.ai.follow = true;
                        self.terminal = Some(terminal);
                    }
                    Err(e) => {
                        app.ai.error = e;
                        app.ai.phase = "Error".into();
                    }
                }
            }
            if let Some(t) = self.terminal.as_mut() {
                let area = app.ai.native_area;
                if area.width > 0 && area.height > 0 {
                    t.resize(area.height, area.width);
                }
                t.poll();
                if t.ended {
                    app.ai.native_input = false;
                    app.ai.busy = false;
                    app.ai.phase = "Exited · R resumes".into();
                }
            }
        }
    }
    pub fn key(&mut self, app: &mut App, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode as K, KeyModifiers as M};
        if !app.ai.native_mode
            || app.pane != crate::app::Pane::Panel
            || app.panel != crate::app::Panel::Ai
            || app.modal.is_some()
        {
            return false;
        }
        recover_mouse_input(app);
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.cancel_click();
        }
        // Legacy terminals encode Ctrl-] as 0x1d, which Crossterm names Ctrl-5.
        if matches!(key.code, K::Char(']' | '5')) && key.modifiers.contains(M::CONTROL) {
            app.ai.native_input = false;
            app.reader.selection.clear();
            app.ai.follow = true;
            app.pane = crate::app::Pane::from_index(app.panes.cycle(app.pane.index(), false));
            return true;
        }
        // Keep companion-local navigation available when the user wants its
        // controls rather than leaving the pane. Ctrl-\ is Ctrl-4 on legacy TTYs.
        if matches!(key.code, K::Char('\\' | '4')) && key.modifiers.contains(M::CONTROL) {
            app.ai.native_input = false;
            app.reader.selection.clear();
            app.ai.follow = true;
            return true;
        }
        if !app.ai.native_input {
            if matches!(key.code, K::Char('i') | K::Enter) {
                app.reader.selection.clear();
                app.ai.native_input = true;
                app.ai.follow = true;
                return true;
            }
            return false;
        }
        if let Some(t) = self.terminal.as_mut() {
            if let Err(e) = t.key(key) {
                app.status = e;
            }
        }
        true
    }
    pub fn click(&mut self, app: &mut App, event: crossterm::event::MouseEvent) {
        if let Some(terminal) = self.terminal.as_mut() {
            if let Err(error) = terminal.click(
                event.row.saturating_sub(app.ai.native_area.y),
                event.column.saturating_sub(app.ai.native_area.x),
                event.modifiers,
                self.config["provider"].as_str().unwrap_or("claude") == "claude",
                app.ai.native_frozen.as_ref(),
            ) {
                app.status = error;
            }
        }
    }
    pub fn paste(&mut self, app: &mut App, text: &str) -> bool {
        if !app.ai.native_mode
            || app.pane != crate::app::Pane::Panel
            || app.panel != crate::app::Panel::Ai
            || app.modal.is_some()
        {
            return false;
        }
        recover_mouse_input(app);
        if !app.ai.native_input {
            return false;
        }
        if let Some(t) = self.terminal.as_mut() {
            t.cancel_click();
            if let Err(e) = t.paste(text) {
                app.status = e;
            }
        }
        true
    }
    pub fn action(&mut self, app: &mut App, request: Value) {
        let result = (|| {
            if request["operation"] == "ai_setup" {
                let interface = self.config["interface"].clone();
                self.config = enable()?;
                self.config["interface"] = interface;
                self.key.clear();
                self.tick(app);
                return Ok(());
            }
            if request["operation"] == "ai_retry" && app.ai.session.is_empty() {
                self.key.clear();
                self.tick(app);
                return Ok(());
            }
            self.tick(app);
            if app.ai.native_mode {
                match s(&request, "operation") {
                    "ai_new" => {
                        self.terminal.take();
                        app.ai.native_view = None;
                        self.native_requested = false;
                        self.send(json!({"operation":"new","binding":app.ai.binding}))?;
                    }
                    "ai_stop" => {
                        if let Some(t) = &mut self.terminal {
                            t.cancel_click();
                            t.write(vec![3])?;
                        }
                    }
                    "ai_retry" => {
                        self.terminal.take();
                        app.ai.native_view = None;
                        self.native_requested = false;
                    }
                    "ai_chat" => {
                        if let Some(t) = &mut self.terminal {
                            t.paste(s(&request, "prompt"))?;
                            t.write(vec![13])?;
                            app.ai.native_input = true;
                            app.ai.follow = true;
                        } else {
                            return Err("AI terminal is not ready".into());
                        }
                    }
                    _ => {
                        self.send(json!({"operation":"artifact_open","binding":app.ai.binding,"id":request["id"]}))?;
                    }
                }
                return Ok(());
            }
            let operation = match s(&request, "operation") {
                "ai_chat" => "ask",
                "ai_new" => "new",
                "ai_stop" => "stop",
                "ai_open_artifact" => "artifact_open",
                _ => "retry",
            };
            if operation == "ask" {
                app.sent_context = app.context.clone();
                app.ai.streaming.clear();
                app.ai.follow = true;
                app.ai.busy = true;
                app.ai.phase = "Thinking".into();
            }
            self.send(json!({"operation":operation,"binding":app.ai.binding,"prompt":request["prompt"],"id":request["id"],"context":app.context,"focus":focus(app)}))
        })();
        if let Err(error) = result {
            app.ai.error = error;
            app.ai.phase = "Error".into();
            app.ai.busy = false;
        }
    }
}
fn recover_mouse_input(app: &mut App) {
    // Copying or scrolling terminal output never transfers keyboard ownership.
    // The next key/paste resumes the live view and reaches the child immediately,
    // even if a mouse-up was lost. Only explicit navigation/keyboard selection
    // keeps keystrokes in LazyReview; no provider-specific menu parser is needed.
    if app.ai.native_input && !app.reader.selection.keyboard {
        app.reader.selection.clear();
        app.ai.native_frozen = None;
        app.ai.follow = true;
        if app.status == "Selecting text · release to copy" {
            app.status.clear();
        }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.terminal.take();
        self.transport.take();
    }
}
pub fn focus(app: &App) -> Value {
    if !app.reader.focused {
        return json!({"scope":"pr","base":app.pr["base"],"head":app.pr["head"]});
    }
    json!({"scope":"file","path":app.current()["filename"],"line":app.lines().get(app.line).cloned().unwrap_or(Value::Null),"selection":app.selection(),"pinnedRanges":app.ranges,"base":app.pr["base"],"head":app.pr["head"]})
}
pub fn apply_event(app: &mut App, event: Value) {
    if event["binding"].as_str() != Some(&app.ai.binding) {
        return;
    }
    match s(&event, "type") {
        "terminal_launch" => app.ai.native_launch = Some(event),
        "native_status" => match s(&event, "kind") {
            "UserPromptSubmit" => {
                app.ai.busy = true;
                app.ai.phase = "Answering".into();
            }
            "Stop" | "SessionStart" => {
                app.ai.busy = false;
                app.ai.phase = "Ready".into();
            }
            "StopCancelled" | "Interrupt" => {
                app.ai.busy = false;
                app.ai.phase = "Stopped · notes retained".into();
            }
            "StopFailure" => {
                app.ai.busy = false;
                app.ai.phase = "Error · notes retained".into();
            }
            "SessionEnd" => {
                app.ai.busy = false;
                app.ai.phase = "Exited · R resumes".into();
            }
            _ => {}
        },
        "native_complete" => {
            if let Some(items) = event["items"].as_array() {
                app.sent_context = items.clone();
                let ids = items.iter().map(|v| v["id"].clone()).collect::<Vec<_>>();
                app.consume_context(&ids);
            }
        }
        "state" => {
            app.ai.phase = s(&event, "status").into();
            app.ai.busy = matches!(app.ai.phase.as_str(), "Preparing" | "Thinking");
            app.ai.error = s(&event, "error").into();
            app.ai.session = s(&event, "session").into();
            app.ai.summary = s(&event, "summary").into();
            app.ai.streaming.clear();
            app.ai.display_truncated = event["displayTruncated"] == true;
            app.ai.evidence = event["evidence"].clone();
            if let Some(items) = event["artifacts"].as_array() {
                app.ai.artifacts = items.clone();
            }
            if let Some(messages) = event["messages"].as_array() {
                app.messages = messages.clone();
            }
        }
        "delta" => {
            app.ai.phase = "Answering".into();
            app.ai.busy = true;
            app.ai.streaming.push_str(s(&event, "text"));
        }
        "turn_complete" => {
            if let Some(ids) = event["contextIds"].as_array() {
                app.consume_context(ids);
            }
        }
        "evidence" => app.ai.evidence = event["evidence"].clone(),
        "artifacts" => {
            if let Some(items) = event["items"].as_array() {
                app.ai.artifacts = items.clone();
            }
            app.reader.artifact_cursor = app
                .reader
                .artifact_cursor
                .min(app.ai.artifacts.len().saturating_sub(1));
        }
        "artifact_opened" => app.status = format!("Opened {}", s(&event, "title")),
        "artifact_error" => app.status = s(&event, "error").into(),
        "session" => app.ai.session = s(&event, "session").into(),
        "activity" => {
            if app.ai.phase != "Preparing" {
                app.ai.phase = s(&event, "label").into();
            }
        }
        "error" => {
            app.ai.error = s(&event, "error").into();
            app.ai.phase = "Error".into();
            app.ai.busy = false;
        }
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ctrl_bracket_leaves_cli_and_focuses_previous_visible_pane() {
        use crate::app::Pane;
        use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
        for native_input in [true, false] {
            for code in [K::Char(']'), K::Char('5')] {
                for (tree, diff, expected) in [
                    (true, true, Pane::Diff),
                    (true, false, Pane::Tree),
                    (false, false, Pane::Panel),
                ] {
                    let mut app = App::new(json!({"files":[]}));
                    app.pane = Pane::Panel;
                    app.panel = crate::app::Panel::Ai;
                    app.panes.tree = tree;
                    app.panes.diff = diff;
                    app.panes.panel = true;
                    app.ai.native_mode = true;
                    app.ai.native_input = native_input;
                    let mut service = Service::new(json!({}));
                    assert!(service.key(&mut app, KeyEvent::new(code, M::CONTROL)));
                    assert!(!app.ai.native_input);
                    assert_eq!(app.pane, expected);
                }
            }
        }
    }
    #[test]
    fn ctrl_backslash_keeps_companion_controls_in_place() {
        use crate::app::{Pane, Panel};
        use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
        for code in [K::Char('4'), K::Char('\\')] {
            let mut app = App::new(json!({"files":[]}));
            app.pane = Pane::Panel;
            app.panel = Panel::Ai;
            app.ai.native_mode = true;
            app.ai.native_input = true;
            let mut service = Service::new(json!({}));
            assert!(service.key(&mut app, KeyEvent::new(code, M::CONTROL)));
            assert!(!app.ai.native_input);
            assert_eq!(app.pane, Pane::Panel);
        }
    }
    #[test]
    fn native_mouse_reading_preserves_keyboard_ownership() {
        use crossterm::event::{
            KeyCode as K, KeyEvent, KeyModifiers as M, MouseButton, MouseEvent, MouseEventKind,
        };
        let mut a = App::new(json!({"files":[]}));
        a.pane = crate::app::Pane::Panel;
        a.panel = crate::app::Panel::Ai;
        a.ai.native_mode = true;
        a.ai.native_input = true;
        let area = ratatui::layout::Rect::new(0, 0, 40, 10);
        let buffer = ratatui::buffer::Buffer::empty(area);
        a.reader
            .selection
            .capture(&buffer, area, 0, "native:test".into());
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 2,
            modifiers: M::NONE,
        };
        let mut service = Service::new(json!({}));
        // A missing release, an active drag, completed copy, and scroll-only
        // history must all let the first key and first paste reach the child.
        for gesture in 0..4 {
            for paste in [false, true] {
                a.reader.selection.clear();
                a.ai.follow = false;
                if gesture != 3 {
                    a.reader.selection.mouse(down);
                }
                if gesture == 1 || gesture == 2 {
                    a.reader.selection.mouse(MouseEvent {
                        kind: MouseEventKind::Drag(MouseButton::Left),
                        column: 8,
                        ..down
                    });
                }
                if gesture == 2 {
                    a.reader.selection.mouse(MouseEvent {
                        kind: MouseEventKind::Up(MouseButton::Left),
                        column: 8,
                        ..down
                    });
                }
                if paste {
                    assert!(service.paste(&mut a, "unsent text"));
                } else {
                    assert!(service.key(&mut a, KeyEvent::new(K::Char('x'), M::NONE)));
                }
                assert!(a.ai.native_input && a.ai.follow);
                assert!(!a.reader.selection.active());
            }
        }
        // Explicit Ctrl-] still gives arrows and selection keys to LazyReview.
        assert!(service.key(&mut a, KeyEvent::new(K::Char(']'), M::CONTROL)));
        assert!(!a.ai.native_input);
        assert!(!service.key(&mut a, KeyEvent::new(K::Down, M::NONE)));
        a.reader.selection.begin_keyboard();
        assert!(!service.key(&mut a, KeyEvent::new(K::Char('x'), M::NONE)));
        assert!(!service.paste(&mut a, "reading mode"));
        assert!(a.reader.selection.keyboard);
    }
    #[test]
    fn native_capture_handles_legacy_control_alias_and_copy_to_typing() {
        use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
        let mut a = App::new(json!({"files":[]}));
        a.pane = crate::app::Pane::Panel;
        a.panel = crate::app::Panel::Ai;
        a.ai.native_mode = true;
        a.ai.native_input = true;
        let mut service = Service::new(json!({}));
        assert!(service.key(&mut a, KeyEvent::new(K::Char('5'), M::CONTROL)));
        assert!(!a.ai.native_input);
        assert_eq!(a.pane, crate::app::Pane::Diff);
        a.pane = crate::app::Pane::Panel;
        a.reader.selection.begin_keyboard();
        assert!(service.key(&mut a, KeyEvent::new(K::Char('i'), M::NONE)));
        assert!(a.ai.native_input);
        assert!(!a.reader.selection.active());
        assert!(service.key(&mut a, KeyEvent::new(K::Char(']'), M::CONTROL)));
        assert!(!a.ai.native_input);
        assert!(!service.key(&mut a, KeyEvent::new(K::Char('q'), M::NONE)));
    }
    #[test]
    fn whole_pr_is_default_and_focused_context_is_explicit() {
        let mut a = App::new(
            json!({"head":"head","base":"base","files":[{"filename":"src/a.ts","lines":[]}]}),
        );
        assert_eq!(focus(&a)["scope"], "pr");
        assert!(focus(&a).get("path").is_none());
        a.reader.focused = true;
        assert_eq!(focus(&a)["scope"], "file");
        assert_eq!(focus(&a)["path"], "src/a.ts");
        a.ai.binding = "current".into();
        a.ai.busy = true;
        a.ai.phase = "Answering".into();
        apply_event(
            &mut a,
            json!({"binding":"current","type":"artifacts","items":[{"id":"one"}]}),
        );
        apply_event(
            &mut a,
            json!({"binding":"current","type":"artifact_error","error":"Cannot open"}),
        );
        assert!(a.ai.busy);
        assert_eq!(a.ai.phase, "Answering");
        assert_eq!(a.ai.artifacts.len(), 1);
        assert_eq!(a.status, "Cannot open");
    }
    #[test]
    fn streamed_events_are_scoped_to_binding_and_never_block_review() {
        let mut a = App::new(json!({"files":[]}));
        a.ai.binding = "new".into();
        apply_event(
            &mut a,
            json!({"binding":"old","type":"delta","text":"stale"}),
        );
        assert!(a.ai.streaming.is_empty());
        apply_event(
            &mut a,
            json!({"binding":"new","type":"delta","text":"answer"}),
        );
        assert_eq!(a.ai.streaming, "answer");
        assert!(a.ai.busy);
        assert!(!a.busy);
        apply_event(
            &mut a,
            json!({"binding":"new","type":"state","status":"Ready","messages":[{"role":"assistant","content":"answer"}]}),
        );
        assert!(!a.ai.busy);
        assert!(a.ai.streaming.is_empty());
        assert_eq!(a.messages.len(), 1);
    }
    #[test]
    fn live_chat_setup_and_controls_do_not_require_attachments_or_block_navigation() {
        use crate::app::{Input, Modal, Pane, Panel};
        use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
        let mut a = App::new(json!({"url":"https://github.com/a/b/pull/1","files":[]}));
        let key = |c| KeyEvent::new(K::Char(c), M::NONE);
        a.key(key('g'));
        a.key(key('a'));
        assert_eq!(a.modal, Some(Modal::AiSetup));
        assert_eq!(
            a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap()["operation"],
            "ai_setup"
        );
        a.ai.enabled = true;
        a.ai.phase = "Preparing".into();
        a.ai.busy = true;
        a.key(key('i'));
        assert_eq!(a.modal, Some(Modal::Editor(Input::Chat)));
        a.paste("Explain this code");
        assert_eq!(
            a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap()["operation"],
            "ai_chat"
        );
        assert!(a.context.is_empty());
        assert!(!a.busy);
        a.panel = Panel::Ai;
        a.pane = Pane::Panel;
        assert_eq!(a.key(key('X')).unwrap()["operation"], "ai_stop");
        assert_eq!(a.key(key('N')).unwrap()["operation"], "ai_new");
        a.key(key('i'));
        assert!(a.key(key('N')).is_none());
        assert!(a.editor.ends_with('N'));
    }
    #[test]
    fn only_successful_current_turn_consumes_unchanged_context() {
        let mut a = App::new(json!({"files":[]}));
        a.ai.binding = "current".into();
        let deep_id = "nested/".repeat(40);
        a.context = vec![
            json!({"id":deep_id,"note":"sent"}),
            json!({"id":"edited","note":"before"}),
        ];
        a.sent_context = a.context.clone();
        a.context[1]["note"] = json!("after");
        a.context.push(json!({"id":"next","note":"during answer"}));
        for event in [
            json!({"binding":"old","type":"turn_complete","contextIds":[deep_id,"edited"]}),
            json!({"binding":"current","type":"turn_accepted","contextIds":[deep_id,"edited"]}),
            json!({"binding":"current","type":"error","error":"Stopped"}),
        ] {
            apply_event(&mut a, event);
            assert_eq!(a.context.len(), 3);
        }
        apply_event(
            &mut a,
            json!({"binding":"current","type":"turn_complete","contextIds":[deep_id,"edited"]}),
        );
        assert_eq!(a.context.len(), 2);
        assert_eq!(a.context[0]["note"], "after");
        assert_eq!(a.context[1]["id"], "next");
        assert!(a.sent_context.is_empty());
    }
}
