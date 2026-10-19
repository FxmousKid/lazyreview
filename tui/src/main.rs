mod activity;
mod ai;
mod annotations;
mod app;
mod backend;
mod browser_demo;
mod clipboard;
mod completion;
mod copy_text;
mod demo;
mod help;
mod input_trace;
mod layout;
mod markdown;
mod mermaid;
mod native;
mod picker;
mod preferences;
mod preview;
mod reader;
mod review_list;
mod search;
mod syntax;
mod theme;
mod ui;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde_json::{Value, json};
use std::{io, time::Duration};
#[derive(Debug, PartialEq)]
enum Mode {
    Picker,
    Demo,
    Preview(String),
    Gallery(String),
    Tour(String),
    Review(String),
    Check(String),
    Snapshot,
    Help,
    SetupAi,
}
fn arguments(args: &[String]) -> Result<Mode, String> {
    match args {
        [] => Ok(Mode::Picker),
        [flag] if flag == "--setup-ai" => Ok(Mode::SetupAi),
        [flag] if flag == "--demo" => Ok(Mode::Demo),
        [flag] if flag == "--snapshot" => Ok(Mode::Snapshot),
        [flag, view] if flag == "--preview-ui" && matches!(view.as_str(),"ai"|"description"|"artifacts") => Ok(Mode::Preview(view.clone())),
        [flag, path] if flag == "--export-demo" => Ok(Mode::Tour(path.clone())),
        [flag, path] if flag == "--ui-gallery" => Ok(Mode::Gallery(path.clone())),
        [flag] if flag == "--help" || flag == "-h" => Ok(Mode::Help),
        [flag, url] if flag == "--pr" => Ok(Mode::Review(url.clone())),
        [flag, url] if flag == "--check" => Ok(Mode::Check(url.clone())),
        _ => Err(
            "Usage: lazyreview [--pr URL | --demo | --check URL | --snapshot | --setup-ai | --help]"
                .into(),
        ),
    }
}
struct Session {
    app: Option<app::App>,
    picker: picker::Picker,
    choosing: bool,
    palette: syntax::Palette,
    appearance: reader::Appearance,
    preview: Option<String>,
}
impl Session {
    fn new() -> Self {
        Self {
            app: None,
            picker: picker::Picker::new(),
            choosing: true,
            palette: syntax::Palette::default(),
            appearance: reader::Appearance::default(),
            preview: None,
        }
    }
    fn begin(&mut self, mode: &Mode) -> Value {
        match mode {
            Mode::Review(url) => self.picker.open(url),
            Mode::Demo | Mode::Preview(_) => {
                if let Mode::Preview(view) = mode {
                    self.preview = Some(view.clone());
                }
                self.picker.busy = true;
                self.picker.loading_pr = true;
                json!({"operation":"sample"})
            }
            _ => self.picker.home(),
        }
    }
    fn receive(&mut self, op: &str, result: Result<Value, String>) {
        if op == "history" {
            self.picker.receive_history(result);
            return;
        }
        if op == "suggest" {
            if let Some(a) = self.app.as_mut() {
                a.receive(op, result);
            }
            return;
        }
        if self.choosing {
            if op == "repositories" {
                self.picker.receive_repos(result);
            } else if op == "discover" {
                self.picker.receive(result);
            } else {
                match result {
                    Ok(pr) => {
                        let previous = self.app.as_ref().map(|a| (a.panes.clone(), a.split));
                        let mut app = app::App::new(pr);
                        app.reader.appearance = self
                            .app
                            .as_ref()
                            .map(|a| a.reader.appearance)
                            .unwrap_or(self.appearance);
                        if let Some(view) = self.preview.take() {
                            preview::seed(&mut app, &view);
                        }
                        app.syntax_palette = self
                            .app
                            .as_ref()
                            .map(|a| a.syntax_palette)
                            .unwrap_or(self.palette);
                        if let Some((panes, split)) = previous {
                            app.panes = panes;
                            app.split = split;
                            if !app.panes.visible().contains(&app.pane.index()) {
                                app.pane = app::Pane::from_index(app.panes.visible()[0]);
                            }
                        }
                        self.app = Some(app);
                        self.choosing = false;
                        self.picker.busy = false;
                        self.picker.loading_pr = false;
                        self.picker.has_review = true;
                    }
                    Err(e) => self.picker.receive(Err(e)),
                }
            }
        } else if let Some(app) = self.app.as_mut() {
            app.receive(op, result);
        }
    }
    fn key(&mut self, key: KeyEvent) -> Option<Value> {
        if self.choosing {
            let request = self.picker.key(key);
            if self.picker.back {
                self.choosing = false;
                self.picker.back = false;
            }
            return request;
        }
        let app = self.app.as_mut()?;
        if app.modal.is_none()
            && !app.busy
            && key.code == KeyCode::Char('p')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.choosing = true;
            self.picker.has_review = true;
            if self.picker.repos.is_empty() && self.picker.items.is_empty() {
                return Some(self.picker.home());
            }
            return None;
        }
        app.key(key)
    }
    fn quit(&self) -> bool {
        self.picker.quit || self.app.as_ref().is_some_and(|app| app.quit)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    struct DiagramWorker;
    impl Drop for DiagramWorker {
        fn drop(&mut self) {
            mermaid::shutdown();
        }
    }
    let _diagrams = DiagramWorker;
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let theme_override = if let Some(i) = args.iter().position(|s| s == "--theme") {
        if i + 1 >= args.len() {
            return Err(io::Error::other("--theme requires lazygit, blue, or custom").into());
        }
        let name = args.remove(i + 1);
        args.remove(i);
        Some(name)
    } else {
        None
    };
    let syntax_override = if let Some(i) = args.iter().position(|s| s == "--syntax-theme") {
        if i + 1 >= args.len() {
            return Err(io::Error::other("--syntax-theme requires github-dark or lunar").into());
        }
        let name = args.remove(i + 1);
        args.remove(i);
        Some(name)
    } else {
        None
    };
    let style_override = if let Some(i) = args.iter().position(|s| s == "--companion-style") {
        if i + 1 >= args.len() {
            return Err(
                io::Error::other("--companion-style requires ledger, paper, or studio").into(),
            );
        }
        let name = args.remove(i + 1);
        args.remove(i);
        Some(name)
    } else {
        None
    };
    let ai_override = if let Some(i) = args.iter().position(|s| s == "--ai-interface") {
        if i + 1 >= args.len() {
            return Err(io::Error::other("--ai-interface requires paper or terminal").into());
        }
        let value = args.remove(i + 1);
        args.remove(i);
        if !matches!(value.as_str(), "paper" | "terminal") {
            return Err(io::Error::other("--ai-interface requires paper or terminal").into());
        }
        Some(value)
    } else {
        None
    };
    let provider_override = if let Some(i) = args.iter().position(|s| s == "--ai-provider") {
        if i + 1 >= args.len() {
            return Err(io::Error::other("--ai-provider requires claude, codex, or grok").into());
        }
        let value = args.remove(i + 1);
        args.remove(i);
        if !matches!(value.as_str(), "claude" | "codex" | "grok") {
            return Err(io::Error::other("--ai-provider requires claude, codex, or grok").into());
        }
        Some(value)
    } else {
        None
    };
    if args == ["--browser-demo"] {
        return browser_demo::run();
    }
    let mode = arguments(&args).map_err(io::Error::other)?;
    if mode == Mode::SetupAi {
        if ai_override.is_some() || provider_override.is_some() {
            return Err(io::Error::other("Use --setup-ai alone. Set ai.provider and ai.interface in config.yml, or use provider/interface flags when launching a review.").into());
        }
        ai::enable().map_err(io::Error::other)?;
        println!(
            "AI companion enabled. Sign in with your configured CLI if needed, then ./lazyreview. Settings: {}",
            ai::config_path().display()
        );
        return Ok(());
    }
    if mode == Mode::Help {
        println!(
            "LazyReview\n\nlazyreview                         Choose repository, then an open PR\nlazyreview --pr <GitHub PR URL>    Open a specific PR directly\nlazyreview --demo                  Synthetic sample; no network\nlazyreview --check <GitHub URL>    Read-only integration smoke\nlazyreview --snapshot              Render sample terminal to stdout\n\nChooser: h/l or Tab/Shift-Tab panes, j/k select, Enter open, / fuzzy repository and PR history, f filter loaded PRs.\nRepository history follows your latest contributions; authored PRs are newest first.\nThemes: --theme lazygit|blue|custom; reads Lazygit gui.theme YAML.\nSyntax: --syntax-theme github-dark|lunar; H switches palette.\nPR: D description, C comments, g m commits in one timeline; b review threads.\nPanes: = fits file names and reading width, +/- resize, drag dividers; backslash opens controls for every pane.\nFiles: n/N jumps directly between files, skipping folders.\nDiff: / searches code; Tab chooses side; n/N moves between matches (files when no search). s changes layout.\nSelections: v selects, Space pins ranges, c comments on GitHub. t adds a temporary AI note; g c inspects queued context.\nComments: Enter saves a draft, Alt-Enter or Ctrl-n inserts a newline.\nReview: r opens folded comments; S opens the review outcome form.\nShortcuts: g ? (or g !) then f to filter; Escape closes.\nIn a review: Ctrl-p returns to the chooser; ? shows Vim keys.\nRequires Node.js 22.18+. GitHub authentication: gh auth login.\nNative CLI: --ai-interface terminal; --ai-provider claude|codex|grok. Click to type; Ctrl-] leaves input and focuses the previous pane; Ctrl-\\ keeps companion controls.\nAI: --setup-ai (existing CLI login); g a opens companion. N new chat, X stop, R retry in companion navigation mode.\nCompanion: --companion-style ledger|paper|studio; g v switches while reviewing.\nPreview: --preview-ui ai|description|artifacts (synthetic, offline); --ui-gallery PATH exports actual terminal renders.\nCompanion: F toggles PR/file context; o lists questions; g o opens saved diagrams/pages/notes.\nCopy: drag text in the reader, or v then hjkl and y.\nDemo: --export-demo PATH captures the offline animated tour.\nPR timeline: o index, [[ / ]] entries; G in AI follows the latest reply.\nListing and opening PRs never write to GitHub."
        );
        return Ok(());
    }
    theme::set(theme::load(theme_override.as_deref()).map_err(io::Error::other)?);
    let palette = theme::syntax_palette(syntax_override.as_deref()).map_err(io::Error::other)?;
    let appearance = theme::companion_style(style_override.as_deref()).map_err(io::Error::other)?;
    let mut bridge = backend::Bridge::new().map_err(io::Error::other)?;
    if let Mode::Tour(path) = &mode {
        let sample = backend::initial(&mut bridge, None).map_err(io::Error::other)?;
        demo::export(path, &mut bridge, sample)?;
        println!("Wrote {path}");
        return Ok(());
    }
    if let Mode::Gallery(path) = &mode {
        let sample = backend::initial(&mut bridge, None).map_err(io::Error::other)?;
        preview::gallery(path, sample)?;
        println!("Wrote {path}");
        return Ok(());
    }
    if let Mode::Check(url) = &mode {
        let initial = backend::initial(&mut bridge, Some(url)).map_err(io::Error::other)?;
        println!(
            "Read-only OK: {}/{} #{} — {} files, {} threads, viewer: {}",
            initial["owner"].as_str().unwrap_or(""),
            initial["repo"].as_str().unwrap_or(""),
            initial["number"],
            initial["files"].as_array().map_or(0, Vec::len),
            initial["threads"].as_array().map_or(0, Vec::len),
            initial["viewer"].is_string()
        );
        return Ok(());
    }
    if mode == Mode::Snapshot {
        let initial = backend::initial(&mut bridge, None).map_err(io::Error::other)?;
        let mut app = app::App::new(initial);
        app.syntax_palette = palette;
        app.reader.appearance = appearance;
        let mut syntax = syntax::Service::start().map_err(io::Error::other)?;
        for _ in 0..100 {
            syntax.tick(&mut app);
            if app.syntax.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(150, 45))?;
        terminal.draw(|f| ui::render(f, &mut app))?;
        let buf = terminal.backend().buffer();
        for y in 0..45 {
            let line: String = (0..150).map(|x| buf[(x, y)].symbol()).collect();
            println!("{}", line.trim_end());
        }
        return Ok(());
    }
    let worker = backend::Worker::start(bridge);
    let mut suggestion_worker: Option<backend::Worker> = None;
    let mut suggestion_busy = false;
    let (copy_tx, copy_jobs) = std::sync::mpsc::channel::<String>();
    let (copy_done, copy_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for text in copy_jobs {
            let _ = copy_done.send(clipboard::copy(text));
        }
    });
    let mut ai_config = ai::config().map_err(io::Error::other)?;
    apply_ai_overrides(
        &mut ai_config,
        ai_override.as_deref(),
        provider_override.as_deref(),
    )
    .map_err(io::Error::other)?;
    let mut companion = ai::Service::new(ai_config);
    let mut input_trace = input_trace::InputTrace::from_env();
    let mut session = Session::new();
    session.palette = palette;
    session.appearance = appearance;
    let mut syntax_service = syntax::Service::start().ok();
    worker.tx.send(session.begin(&mode))?;
    ratatui::run(|terminal| -> io::Result<()> {
        struct MouseCapture;
        impl Drop for MouseCapture {
            fn drop(&mut self) {
                let _ = crossterm::execute!(
                    io::stdout(),
                    crossterm::event::DisableMouseCapture,
                    crossterm::event::DisableBracketedPaste
                );
            }
        }
        crossterm::execute!(
            io::stdout(),
            crossterm::event::EnableMouseCapture,
            crossterm::event::EnableBracketedPaste
        )?;
        let _mouse = MouseCapture;
        while !session.quit() {
            if let Some(app) = session.app.as_mut() {
                while let Ok(result) = copy_rx.try_recv() {
                    app.status = match result {
                        Ok(()) => "Copied selection".into(),
                        Err(error) => error,
                    };
                }
                if let Some(text) = app.copy_pending.take() {
                    let _ = copy_tx.send(text);
                }
            }

            while let Ok((op, result)) = worker.rx.try_recv() {
                session.receive(&op, result);
            }
            if let Some(worker) = suggestion_worker.as_ref() {
                while let Ok((op, result)) = worker.rx.try_recv() {
                    suggestion_busy = false;
                    session.receive(&op, result);
                }
            }
            if !suggestion_busy && !session.choosing {
                if let Some(a) = session.app.as_mut() {
                    if let Some(request) = a.completion.request(a.is_demo()) {
                        if suggestion_worker.is_none() {
                            suggestion_worker =
                                backend::Bridge::new().ok().map(backend::Worker::start);
                        }
                        if let Some(worker) = suggestion_worker.as_ref() {
                            suggestion_busy = worker.tx.send(request).is_ok();
                        }
                        if !suggestion_busy {
                            a.completion.error = "Suggestions unavailable; continue typing.".into();
                        }
                    }
                }
            }
            if !session.choosing {
                if let Some(a) = session.app.as_mut() {
                    if let Some(service) = syntax_service.as_mut() {
                        service.tick(a);
                    } else {
                        a.syntax = Some(std::sync::Arc::new(syntax::FileSyntax::default()));
                    }
                }
            }
            if let Some(a) = session.app.as_mut() {
                companion.tick(a);
            }
            terminal.draw(|frame| {
                if session.choosing {
                    session.picker.render(frame);
                } else if let Some(app) = session.app.as_mut() {
                    ui::render(frame, app);
                }
            })?;
            if event::poll(Duration::from_millis(50))? {
                let input = event::read()?;
                input_trace.record(&input, session.app.as_ref(), "received");
                match input {
                    Event::Key(key)
                        if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                    {
                        if !session.choosing {
                            if let Some(app) = session.app.as_mut() {
                                if companion.key(app, key) {
                                    input_trace.record(&Event::Key(key), Some(app), "cli");
                                    continue;
                                }
                            }
                        }
                        if let Some(request) = session.key(key) {
                            if request["operation"]
                                .as_str()
                                .is_some_and(|op| op.starts_with("ai_"))
                            {
                                if let Some(a) = session.app.as_mut() {
                                    companion.action(a, request);
                                }
                                continue;
                            }
                            if worker.tx.send(request).is_err() {
                                session.receive(
                                    "error",
                                    Err("Backend stopped. Restart the app.".into()),
                                );
                            }
                        }
                    }
                    Event::Paste(text) => {
                        if session.choosing {
                            session.picker.paste(&text);
                        } else {
                            if let Some(a) = session.app.as_mut() {
                                if !companion.paste(a, &text) {
                                    a.paste(&text);
                                }
                            }
                        }
                    }
                    Event::Mouse(event) => {
                        if session.choosing {
                            if let Some(request) = session.picker.mouse(event) {
                                if worker.tx.send(request).is_err() {
                                    session.receive(
                                        "error",
                                        Err("Backend stopped. Restart the app.".into()),
                                    );
                                }
                            }
                        } else if let Some(app) = session.app.as_mut() {
                            let (native_click, request) = review_mouse(app, event);
                            if native_click {
                                companion.click(app, event);
                            }
                            input_trace.record(
                                &Event::Mouse(event),
                                Some(app),
                                if native_click {
                                    "cli_click"
                                } else {
                                    "workbench"
                                },
                            );
                            if let Some(request) = request {
                                if worker.tx.send(request).is_err() {
                                    session.receive(
                                        "error",
                                        Err("Backend stopped. Restart the app.".into()),
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}
fn apply_ai_overrides(
    config: &mut Value,
    interface: Option<&str>,
    provider: Option<&str>,
) -> Result<(), String> {
    if let Some(provider) = provider {
        if config["provider"].as_str().unwrap_or("claude") != provider {
            config["command"] = json!(provider);
            config["model"] = json!("default");
        }
        config["provider"] = json!(provider);
        config["enabled"] = json!(true);
        if provider != "claude" {
            config["interface"] = json!("terminal");
        }
    }
    if let Some(interface) = interface {
        config["interface"] = json!(interface);
        if interface == "terminal" {
            config["enabled"] = json!(true);
        }
    }
    if config["provider"].as_str().unwrap_or("claude") != "claude"
        && config["interface"] != "terminal"
    {
        return Err("Codex and Grok require --ai-interface terminal".into());
    }
    Ok(())
}
fn review_mouse(app: &mut app::App, event: event::MouseEvent) -> (bool, Option<Value>) {
    use event::{MouseButton, MouseEventKind};
    let native_body = app.ai.native_mode
        && app.panel == app::Panel::Ai
        && app.modal.is_none()
        && app.reader.selection.visible
        && app
            .ai
            .native_area
            .contains((event.column, event.row).into());
    let selecting = app.reader.selection.active();
    let live = app.panel_scroll == app.reader.max_scroll;
    if native_body && event.kind == MouseEventKind::Down(MouseButton::Left) {
        // Focus on press, but activate child controls only on a completed click.
        // Waiting for release before forwarding keys strands input if a mux
        // drops the release event. The next key cancels that pending gesture.
        app.ai.native_input = true;
    }
    let request = app.mouse(event);
    // Mouse selection freezes the displayed transcript without changing its
    // keyboard owner. Only Ctrl-] explicitly hands keys back to LazyReview.
    // A completed stationary click may also activate a child control.
    if native_body
        && selecting
        && event.kind == MouseEventKind::Up(MouseButton::Left)
        && !app.reader.selection.active()
    {
        app.ai.native_input = true;
        app.ai.follow = true;
        return (live, request);
    }
    (false, request)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_cli_flag_enables_this_run_and_provider_switch_uses_its_defaults() {
        let original = json!({"provider":"claude","command":"custom-claude","model":"sonnet","enabled":false,"interface":"paper"});
        let mut config = original.clone();
        apply_ai_overrides(&mut config, Some("terminal"), None).unwrap();
        assert_eq!(config["enabled"], true);
        assert_eq!(config["command"], "custom-claude");
        assert_eq!(original["enabled"], false);
        apply_ai_overrides(&mut config, None, Some("codex")).unwrap();
        assert_eq!(config["command"], "codex");
        assert_eq!(config["model"], "default");
        assert_eq!(config["interface"], "terminal");
        assert!(apply_ai_overrides(&mut config, Some("paper"), Some("grok")).is_err());
    }
    #[test]
    fn native_click_types_drag_selects_and_motion_does_not_steal_input() {
        use event::{MouseButton, MouseEvent, MouseEventKind};
        let mut app = app::App::new(json!({"files":[],"threads":[]}));
        app.ai.native_mode = true;
        app.ai.native_input = true;
        app.panel = app::Panel::Ai;
        app.panes.panel = true;
        app.ai.native_area = ratatui::layout::Rect::new(10, 2, 40, 20);
        let mut buffer = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 100, 30));
        app.reader
            .selection
            .capture(&mut buffer, app.ai.native_area, 0, "native:test".into());
        let mouse = |kind| MouseEvent {
            kind,
            column: 15,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        for kind in [
            MouseEventKind::Moved,
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Down(MouseButton::Right),
        ] {
            review_mouse(&mut app, mouse(kind));
            assert!(app.ai.native_input);
        }
        assert!(!review_mouse(&mut app, mouse(MouseEventKind::Down(MouseButton::Left))).0);
        assert!(app.ai.native_input);
        assert!(app.reader.selection.mouse_pending());
        assert!(review_mouse(&mut app, mouse(MouseEventKind::Up(MouseButton::Left))).0);
        assert!(app.ai.native_input);
        assert!(!app.reader.selection.active());
        review_mouse(&mut app, mouse(MouseEventKind::Down(MouseButton::Left)));
        let mut release = mouse(MouseEventKind::Up(MouseButton::Left));
        release.column += 4;
        assert!(!review_mouse(&mut app, release).0);
        assert!(app.ai.native_input);
        assert!(app.reader.selection.active());
        app.ai.native_input = true;
        review_mouse(&mut app, mouse(MouseEventKind::ScrollUp));
        assert!(app.ai.native_input);
        app.ai.native_input = true;
        review_mouse(&mut app, mouse(MouseEventKind::ScrollDown));
        assert!(app.ai.native_input);
        app.ai.native_input = true;
        app.reader.selection.visible = false;
        review_mouse(&mut app, mouse(MouseEventKind::Down(MouseButton::Left)));
        assert!(app.ai.native_input);
    }
    #[test]
    fn launch_defaults_to_picker_and_demo_is_explicit() {
        assert_eq!(arguments(&[]).unwrap(), Mode::Picker);
        assert_eq!(arguments(&["--demo".into()]).unwrap(), Mode::Demo);
        assert!(arguments(&["--pr".into()]).is_err());
        assert!(arguments(&["--typo".into()]).is_err());
    }
    #[test]
    fn selection_and_direct_flag_share_review_transition() {
        let pr = json!({"demo":false,"files":[],"threads":[],"title":"Chosen PR","head":"abc"});
        let mut selected = Session::new();
        selected.receive("load", Ok(pr.clone()));
        let mut direct = Session::new();
        let request = direct.begin(&Mode::Review("https://github.com/a/b/pull/1".into()));
        assert_eq!(request["operation"], "load");
        direct.receive("load", Ok(pr));
        assert!(!direct.choosing);
        assert!(!selected.choosing);
        assert_eq!(direct.app.unwrap().pr, selected.app.unwrap().pr);
    }
    #[test]
    fn returning_from_picker_preserves_unsent_review() {
        let mut session = Session::new();
        session.receive("load", Ok(json!({"demo":true,"files":[],"threads":[]})));
        session.app.as_mut().unwrap().editor = "Keep my unsent text".into();
        session.picker.items = vec![json!({"url":"https://github.com/a/b/pull/1"})];
        session.key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert!(session.choosing);
        session.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!session.choosing);
        assert_eq!(session.app.unwrap().editor, "Keep my unsent text");
    }
}
