//! Syntect parses each immutable side from its beginning, off the UI thread.
use crate::{
    app::{App, s},
    backend::Bridge,
};
use ratatui::style::{Color, Modifier, Style};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
};
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, StyleModifier, Theme, ThemeItem, ThemeSettings},
    parsing::SyntaxSet,
};

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum Palette {
    #[default]
    Github,
    Lunar,
}
impl Palette {
    pub fn name(self) -> &'static str {
        match self {
            Self::Github => "GitHub Dark",
            Self::Lunar => "Lunar",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Github => Self::Lunar,
            Self::Lunar => Self::Github,
        }
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "github-dark" => Ok(Self::Github),
            "lunar" => Ok(Self::Lunar),
            _ => Err("Syntax theme must be github-dark or lunar".into()),
        }
    }
    pub fn diff_bg(self, added: bool) -> Color {
        match (self, added) {
            (Self::Github, true) => rgb(0x12261e),
            (Self::Github, false) => rgb(0x301a1e),
            (Self::Lunar, true) => rgb(0x193824),
            (Self::Lunar, false) => rgb(0x2a1717),
        }
    }
}
fn rgb(n: u32) -> Color {
    Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)
}
fn syncolor(n: u32) -> syntect::highlighting::Color {
    syntect::highlighting::Color {
        r: (n >> 16) as u8,
        g: (n >> 8) as u8,
        b: n as u8,
        a: 255,
    }
}
pub(crate) fn textmate(p: Palette) -> Theme {
    // Lunar follows installed lunar.nvim's highlight groups; backgrounds are independent.
    let lunar = p == Palette::Lunar;
    let (fg, comment, keyword, string, number, function, types, operator) = if lunar {
        (
            0xc0caf5, 0x565f89, 0x9d7cd8, 0x9ece6a, 0xff9e64, 0x7aa2f7, 0x2ac3de, 0x89ddff,
        )
    } else {
        (
            0xe6edf3, 0x8b949e, 0xff7b72, 0xa5d6ff, 0x79c0ff, 0xd2a8ff, 0xffa657, 0xff7b72,
        )
    };
    let mut theme = Theme {
        settings: ThemeSettings {
            foreground: Some(syncolor(fg)),
            ..Default::default()
        },
        ..Default::default()
    };
    for (scope, color, italic) in [
        ("comment", comment, true),
        ("keyword, storage", keyword, false),
        ("string", string, false),
        (
            "constant.numeric, constant.language, constant.character",
            number,
            false,
        ),
        ("entity.name.function, support.function", function, false),
        ("entity.name.type, support.type", types, false),
        ("storage.type", if lunar { types } else { keyword }, false),
        ("keyword.operator", operator, false),
        ("punctuation", if lunar { operator } else { fg }, false),
        ("punctuation.definition.string", string, false),
        ("punctuation.definition.comment", comment, lunar),
        (
            "entity.name.tag",
            if lunar { function } else { 0x7ee787 },
            false,
        ),
        (
            "entity.other.attribute-name",
            if lunar { 0xbb9af7 } else { 0x79c0ff },
            false,
        ),
        (
            "keyword.control.conditional, keyword.control.loop",
            if lunar { 0xbb9af7 } else { keyword },
            false,
        ),
        (
            "keyword.control.import, keyword.control.include",
            if lunar { 0x7dcfff } else { keyword },
            false,
        ),
    ] {
        theme.scopes.push(ThemeItem {
            scope: scope.parse().expect("fixed scope"),
            style: StyleModifier {
                foreground: Some(syncolor(color)),
                font_style: italic.then_some(FontStyle::ITALIC),
                ..Default::default()
            },
        });
    }
    theme
}
#[derive(Default)]
pub struct CodeLine {
    pub text: String,
    pub tokens: Vec<(usize, Style)>,
}
impl CodeLine {
    pub fn style_at(&self, byte: usize) -> Style {
        let i = self.tokens.partition_point(|(end, _)| *end <= byte);
        self.tokens
            .get(i)
            .map(|(_, style)| *style)
            .unwrap_or_default()
    }
}
#[derive(Default)]
pub struct FileSyntax {
    left: Option<Vec<CodeLine>>,
    right: Option<Vec<CodeLine>>,
}
impl FileSyntax {
    pub fn partial(&self) -> bool {
        self.left.is_none() || self.right.is_none()
    }
    pub fn line(&self, left: bool, number: u64, text: &str) -> Option<&CodeLine> {
        let lines = if left { &self.left } else { &self.right };
        lines
            .as_ref()?
            .get(number.checked_sub(1)? as usize)
            .filter(|l| l.text == text)
    }
}
#[cfg(test)]
fn highlight(
    syntaxes: &SyntaxSet,
    name: &str,
    source: &str,
    palette: Palette,
) -> Option<Vec<CodeLine>> {
    highlight_with_stop(syntaxes, name, source, palette, None, None)
}
fn highlight_with_stop(
    syntaxes: &SyntaxSet,
    name: &str,
    source: &str,
    palette: Palette,
    stop: Option<&AtomicBool>,
    last_line: Option<usize>,
) -> Option<Vec<CodeLine>> {
    if source.len() > 200_000 || source.contains('\0') || source.lines().any(|l| l.len() > 5000) {
        return None;
    }
    if source.is_empty() {
        return Some(vec![]);
    }
    let filename = name.rsplit('/').next().unwrap_or(name);
    let syntax = syntaxes
        .find_syntax_by_extension(filename)
        .or_else(|| {
            syntaxes.find_syntax_by_extension(filename.rsplit('.').next().unwrap_or(filename))
        })
        .or_else(|| syntaxes.find_syntax_by_first_line(source.lines().next().unwrap_or("")))?;
    let theme = textmate(palette);
    let mut h = HighlightLines::new(syntax, &theme);
    let mut lines = vec![];
    let mut token_count = 0;
    for raw in source
        .split_inclusive('\n')
        .take(last_line.unwrap_or(usize::MAX))
    {
        if stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
            return None;
        }
        let mut line = CodeLine::default();
        for (style, text) in h.highlight_line(raw, syntaxes).ok()? {
            let text = crate::ui::clean(text.trim_end_matches('\n')).replace('\t', "    ");
            if text.is_empty() {
                continue;
            }
            line.text.push_str(&text);
            let c = style.foreground;
            let mut color = Style::default().fg(Color::Rgb(c.r, c.g, c.b));
            if style.font_style.contains(FontStyle::ITALIC) {
                color = color.add_modifier(Modifier::ITALIC);
            }
            if style.font_style.contains(FontStyle::BOLD) {
                color = color.add_modifier(Modifier::BOLD);
            }
            line.tokens.push((line.text.len(), color));
            token_count += 1;
            if token_count > 40_000 {
                return None;
            }
        }
        lines.push(line);
    }
    Some(lines)
}
fn render(
    syntaxes: &SyntaxSet,
    sources: &Value,
    palette: Palette,
    stop: Option<&AtomicBool>,
    limits: Option<[usize; 2]>,
) -> FileSyntax {
    let side = |key, path| {
        sources[key].as_str().and_then(|source| {
            highlight_with_stop(
                syntaxes,
                s(sources, path),
                source,
                palette,
                stop,
                limits.map(|l| l[if key == "left" { 0 } else { 1 }]),
            )
        })
    };
    FileSyntax {
        left: side("left", "leftPath"),
        right: side("right", "rightPath"),
    }
}
struct Request {
    key: String,
    snapshot: String,
    input: Value,
    palette: Palette,
    limits: [usize; 2],
}
pub struct Service {
    child: Arc<Mutex<Child>>,
    tx: SyncSender<Option<Request>>,
    rx: Receiver<(String, Arc<FileSyntax>)>,
    join: Option<thread::JoinHandle<()>>,
    pending: bool,
    stop: Arc<AtomicBool>,
    cache: VecDeque<(String, Arc<FileSyntax>)>,
}
impl Service {
    pub fn start() -> Result<Self, String> {
        let mut bridge = Bridge::new()?;
        let child = bridge.child.clone();
        let (tx, requests) = mpsc::sync_channel::<Option<Request>>(1);
        let (responses, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let join = thread::spawn(move || {
            let syntaxes = two_face::syntax::extra_newlines();
            let mut sources: VecDeque<(String, Value)> = VecDeque::new();
            while let Ok(Some(r)) = requests.recv() {
                let data = if let Some(i) = sources.iter().position(|(k, _)| *k == r.snapshot) {
                    let entry = sources.remove(i).unwrap();
                    let data = entry.1.clone();
                    sources.push_back(entry);
                    data
                } else {
                    let data = bridge.request(&r.input).unwrap_or(Value::Null);
                    sources.push_back((r.snapshot, data.clone()));
                    if sources.len() > 4 {
                        sources.pop_front();
                    }
                    data
                };
                if responses
                    .send((
                        r.key,
                        Arc::new(render(
                            &syntaxes,
                            &data,
                            r.palette,
                            Some(&stopping),
                            Some(r.limits),
                        )),
                    ))
                    .is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            tx,
            rx,
            join: Some(join),
            pending: false,
            stop,
            cache: VecDeque::new(),
        })
    }
    pub fn tick(&mut self, app: &mut App) {
        while let Ok(result) = self.rx.try_recv() {
            self.pending = false;
            self.cache.push_back(result);
            if self.cache.len() > 8 {
                self.cache.pop_front();
            }
        }
        let file = app.current();
        let input = json!({"operation":"syntax", "generation":app.syntax_generation, "headRepo":app.pr["headRepo"], "baseRepo":app.pr["baseRepo"], "url":app.pr["url"], "expectedHead":app.pr["head"], "expectedBase":app.pr["base"], "demo":app.is_demo(), "file":{"filename":file["filename"],"previous_filename":file["previous_filename"],"status":file["status"]}});
        let snapshot = input.to_string();
        let key = format!("{}:{snapshot}", app.syntax_palette.name());
        if app.syntax_key != key {
            app.syntax_key = key.clone();
            app.syntax = None;
        }
        if let Some(i) = self.cache.iter().position(|(k, _)| *k == key) {
            let entry = self.cache.remove(i).unwrap();
            app.syntax = Some(entry.1.clone());
            self.cache.push_back(entry);
        } else if !self.pending && !app.lines().is_empty() {
            let limits = ["old", "new"].map(|key| {
                app.lines()
                    .iter()
                    .filter_map(|l| l[key].as_u64())
                    .max()
                    .unwrap_or(0) as usize
            });
            self.pending = self
                .tx
                .try_send(Some(Request {
                    key,
                    snapshot,
                    input,
                    palette: app.syntax_palette,
                    limits,
                }))
                .is_ok();
        }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.tx.try_send(None);
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "release-mode latency benchmark"]
    fn benchmark_syntax_latency() {
        let start = std::time::Instant::now();
        let syntax = two_face::syntax::extra_newlines();
        let mut results = serde_json::Map::new();
        results.insert(
            "grammar_init_ms".into(),
            json!(start.elapsed().as_secs_f64() * 1000.0),
        );
        for (name, line) in [
            (
                "file.tsx",
                "export function Component() { return <button title=\"hello\">hello</button>; }\n",
            ),
            (
                "file.ts",
                "export function add(x: number, y: number) { return x + y; }\n",
            ),
            (
                "file.rs",
                "pub fn greet(name: &str) -> String { format!(\"hello {}\", name) }\n",
            ),
        ] {
            let source = line.repeat(80_000 / line.len());
            for pass in ["cold", "warm"] {
                let start = std::time::Instant::now();
                let highlighted = highlight(&syntax, name, &source, Palette::Github)
                    .expect("benchmark must produce syntax");
                results.insert(
                    format!("{name}_{pass}_ms"),
                    json!(start.elapsed().as_secs_f64() * 1000.0),
                );
                assert_eq!(highlighted.len(), source.lines().count());
            }
            let start = std::time::Instant::now();
            let prefix =
                highlight_with_stop(&syntax, name, &source, Palette::Github, None, Some(40))
                    .unwrap();
            results.insert(
                format!("{name}_first_40_lines_ms"),
                json!(start.elapsed().as_secs_f64() * 1000.0),
            );
            assert_eq!(prefix.len(), 40);
        }
        println!("{}", Value::Object(results));
    }
    #[test]
    fn async_file_changes_cannot_apply_stale_colors_and_palette_round_trip_hits_cache() {
        let mut bridge = Bridge::new().unwrap();
        let mut app = App::new(crate::backend::initial(&mut bridge, None).unwrap());
        let mut service = Service::start().unwrap();
        service.tick(&mut app); // first file is now loading
        app.file = 1;
        service.tick(&mut app); // change file before that result arrives
        assert!(app.syntax.is_none());
        let wait = |service: &mut Service, app: &mut App| {
            for _ in 0..3000 {
                service.tick(app);
                if app.syntax.is_some() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!("syntax worker did not finish");
        };
        wait(&mut service, &mut app);
        let first = app.syntax.clone().unwrap();
        let expected = s(app.current(), "content").lines().next().unwrap();
        assert!(first.line(false, 1, expected).is_some());
        app.syntax_palette = Palette::Lunar;
        service.tick(&mut app);
        wait(&mut service, &mut app);
        app.syntax_palette = Palette::Github;
        service.tick(&mut app);
        assert!(Arc::ptr_eq(&first, app.syntax.as_ref().unwrap()));
        assert!(!service.pending);
    }
    #[test]
    fn multiline_context_is_separate_per_side_and_tabs_unicode_are_preserved() {
        let syntax = two_face::syntax::extra_newlines();
        let data = json!({"leftPath":"before.rs","rightPath":"after.rs", "left":"/*\nlet café = 1;\n*/\n", "right":"// comment\nlet café = 1;\n\tlet x = \"é\";\n"});
        let result = render(&syntax, &data, Palette::Github, None, None);
        let text = "let café = 1;";
        let left = result.line(true, 2, text).unwrap();
        let right = result.line(false, 2, text).unwrap();
        assert_ne!(left.style_at(0).fg, right.style_at(0).fg);
        assert!(left.style_at(0).add_modifier.contains(Modifier::ITALIC));
        assert!(result.line(false, 3, "    let x = \"é\";").is_some());
        assert!(result.line(false, 2, "different snapshot").is_none());
        assert!(result.line(false, 0, text).is_none());
    }
    #[test]
    fn parsing_stops_at_each_sides_last_diff_line_and_preserves_prior_context() {
        let syntax = two_face::syntax::extra_newlines();
        // The repeated tail would exceed the token budget if it were parsed.
        let source = format!(
            "/* opened before the diff\nlet hidden = 1;\n*/\n{}",
            "let tail = (1 + 2) * 3;\n".repeat(6000)
        );
        let data = json!({"leftPath":"a.rs", "rightPath":"a.rs", "left":source, "right":source});
        let result = render(&syntax, &data, Palette::Github, None, Some([2, 3]));
        assert!(!result.partial());
        assert_eq!(result.left.as_ref().unwrap().len(), 2);
        assert_eq!(result.right.as_ref().unwrap().len(), 3);
        let inside_comment = result.line(true, 2, "let hidden = 1;").unwrap();
        assert!(
            inside_comment
                .style_at(0)
                .add_modifier
                .contains(Modifier::ITALIC)
        );
        assert!(result.line(true, 3, "*/").is_none());
        assert!(result.line(false, 3, "*/").is_some());
        assert!(result.line(false, 4, "let tail = (1 + 2) * 3;").is_none());
    }
    #[test]
    fn lunar_uses_token_colors_without_setting_any_background() {
        let syntax = two_face::syntax::extra_newlines();
        let lunar = highlight(
            &syntax,
            "file.ts",
            "const value = \"green\";",
            Palette::Lunar,
        )
        .unwrap();
        assert!(lunar[0].tokens.iter().all(|(_, s)| s.bg.is_none()));
        assert_eq!(lunar[0].style_at(15).fg, Some(rgb(0x9ece6a)));
        assert_eq!(Palette::Lunar.diff_bg(true), rgb(0x193824));
        assert_eq!(Palette::default(), Palette::Github);
    }
    #[test]
    fn unsupported_binary_and_minified_files_fall_back_without_changing_text() {
        let syntax = two_face::syntax::extra_newlines();
        assert!(highlight(&syntax, "a.unknown", "plain", Palette::Github).is_none());
        assert!(highlight(&syntax, "a.rs", "a\0b", Palette::Github).is_none());
        assert!(highlight(&syntax, "a.rs", &"a".repeat(5001), Palette::Github).is_none());
        assert!(highlight(&syntax, "a.rs", "", Palette::Github).is_some());
    }
}
