use crate::{theme::current as theme, ui::clean};
use pulldown_cmark::{Options, Parser};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};
use scraper::{ElementRef, Html};

const MAX_CODE_BYTES: usize = 16_384;
const MAX_PREVIEW_BYTES: usize = 32_768;
const MAX_PREVIEW_DEPTH: usize = 2;
const CODE_CACHE_SIZE: usize = 32;
thread_local! {
    static CODE_PALETTE: std::cell::Cell<crate::syntax::Palette> = const { std::cell::Cell::new(crate::syntax::Palette::Github) };
    static CODE_FRAME: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
#[derive(Clone, PartialEq)]
struct CodeKey {
    source: String,
    language: String,
    palette: crate::syntax::Palette,
}
enum CodeResult {
    Pending,
    Ready(Option<Vec<Line<'static>>>),
}
struct CodeCache {
    key: CodeKey,
    result: CodeResult,
    frame: u64,
}
struct CodeWorker {
    tx: std::sync::mpsc::SyncSender<CodeKey>,
    cache: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<CodeCache>>>,
    revision: std::sync::Arc<std::sync::atomic::AtomicU64>,
}
static CODE_WORKER: std::sync::OnceLock<Option<CodeWorker>> = std::sync::OnceLock::new();
/// Called once per frame; the reader includes this palette in its content key.
pub fn set_palette(palette: crate::syntax::Palette) {
    CODE_PALETTE.with(|current| current.set(palette));
    static FRAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    CODE_FRAME.with(|frame| frame.set(FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
}
/// A newly colored code block invalidates cached reader documents, just as a
/// completed Mermaid diagram does. Source text is always available immediately.
pub fn revision() -> u64 {
    CODE_WORKER
        .get()
        .and_then(Option::as_ref)
        .map_or(0, |worker| {
            worker.revision.load(std::sync::atomic::Ordering::Relaxed)
        })
}
/// Export-only callers can wait briefly for deterministic colored artifacts.
/// Interactive readers display the immediate source and redraw on revision.
pub(crate) fn pending() -> bool {
    CODE_WORKER
        .get()
        .and_then(Option::as_ref)
        .is_some_and(|worker| {
            worker.cache.lock().is_ok_and(|cache| {
                cache
                    .iter()
                    .any(|entry| matches!(entry.result, CodeResult::Pending))
            })
        })
}
impl CodeWorker {
    fn start() -> Option<Self> {
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
            mpsc,
        };
        let (tx, rx) = mpsc::sync_channel::<CodeKey>(8);
        let cache = Arc::new(Mutex::new(std::collections::VecDeque::<CodeCache>::new()));
        let revision = Arc::new(AtomicU64::new(0));
        let (worker_cache, worker_revision) = (cache.clone(), revision.clone());
        std::thread::Builder::new()
            .name("markdown-syntax".into())
            .spawn(move || {
                // Grammar decoding and regex work never run on the render thread.
                // This worker owns no child process or files to clean up at exit.
                let syntaxes = two_face::syntax::extra_newlines();
                let github = crate::syntax::textmate(crate::syntax::Palette::Github);
                let lunar = crate::syntax::textmate(crate::syntax::Palette::Lunar);
                while let Ok(key) = rx.recv() {
                    let theme = if key.palette == crate::syntax::Palette::Github {
                        &github
                    } else {
                        &lunar
                    };
                    let lines = color_code(&syntaxes, theme, &key.source, &key.language);
                    let Ok(mut cache) = worker_cache.lock() else {
                        break;
                    };
                    if let Some(entry) = cache.iter_mut().find(|entry| entry.key == key) {
                        entry.result = CodeResult::Ready(lines);
                        worker_revision.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
            .ok()?;
        Some(Self {
            tx,
            cache,
            revision,
        })
    }
    fn request(
        &self,
        source: &str,
        language: &str,
        palette: crate::syntax::Palette,
    ) -> Option<Vec<Line<'static>>> {
        let mut cache = self.cache.lock().ok()?;
        let frame = CODE_FRAME.with(|frame| frame.get());
        if let Some(index) = cache.iter().position(|entry| {
            entry.key.source == source
                && entry.key.language == language
                && entry.key.palette == palette
        }) {
            let mut entry = cache.remove(index)?;
            entry.frame = frame;
            let lines = match &entry.result {
                CodeResult::Pending => None,
                CodeResult::Ready(lines) => lines.clone(),
            };
            cache.push_front(entry);
            return lines;
        }
        // Pin everything used in this render. A 70-block timeline must not
        // evict its own first blocks and requeue them forever on every revision.
        // A later frame may reclaim completed entries from a different view.
        let evict = if cache.len() >= CODE_CACHE_SIZE {
            Some(cache.iter().rposition(|entry| {
                entry.frame != frame && matches!(entry.result, CodeResult::Ready(_))
            })?)
        } else {
            None
        };
        let key = CodeKey {
            source: source.into(),
            language: language.into(),
            palette,
        };
        // Keep the lock until Pending is registered, so a fast response cannot
        // arrive before its cache slot exists. Full queues retry on completion.
        self.tx.try_send(key.clone()).ok()?;
        if let Some(index) = evict {
            cache.remove(index);
        }
        cache.push_front(CodeCache {
            key,
            result: CodeResult::Pending,
            frame,
        });
        None
    }
}
fn highlighted_code(source: &str, language: &str) -> Option<Vec<Line<'static>>> {
    if source.len() > MAX_CODE_BYTES
        || source.lines().count() > 160
        || source.lines().any(|line| line.len() > 2048)
    {
        return None;
    }
    let lower = language.to_ascii_lowercase();
    let language = match lower.as_str() {
        "rs" => "rust",
        "js" => "javascript",
        "ts" => "typescript",
        "py" => "python",
        "sh" | "shell" => "bash",
        "yml" => "yaml",
        "md" => "markdown",
        "c++" => "cpp",
        _ => lower.as_str(),
    };
    let worker = CODE_WORKER.get_or_init(CodeWorker::start).as_ref()?;
    worker.request(source, language, CODE_PALETTE.with(|current| current.get()))
}
fn color_code(
    syntaxes: &syntect::parsing::SyntaxSet,
    theme: &syntect::highlighting::Theme,
    source: &str,
    language: &str,
) -> Option<Vec<Line<'static>>> {
    use syntect::{easy::HighlightLines, highlighting::FontStyle};
    let syntax = syntaxes.find_syntax_by_token(language)?;
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = vec![];
    let mut tokens = 0;
    for raw in source.split_inclusive('\n') {
        let mut spans = vec![];
        for (color, text) in highlighter.highlight_line(raw, syntaxes).ok()? {
            let text = text.strip_suffix('\n').unwrap_or(text);
            if text.is_empty() {
                continue;
            }
            let fg = color.foreground;
            let mut style = Style::default().fg(ratatui::style::Color::Rgb(fg.r, fg.g, fg.b));
            if color.font_style.contains(FontStyle::ITALIC) {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if color.font_style.contains(FontStyle::BOLD) {
                style = style.add_modifier(Modifier::BOLD);
            }
            if color.font_style.contains(FontStyle::UNDERLINE) {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            spans.push(Span::styled(text.to_string(), style));
            tokens += 1;
            if tokens > 20_000 {
                return None;
            }
        }
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    Some(lines)
}
/// Expand tabs at four-cell stops before highlighting; indentation and trailing
/// spaces remain source text. Never use prose whitespace normalization for code.
fn expand_code_tabs(source: &str) -> String {
    source
        .split('\n')
        .map(|raw| {
            let mut text = String::new();
            let mut column = 0;
            // Ratatui's styled_graphemes intentionally omits control characters,
            // including tabs, so expand them before passing text to that iterator.
            for (index, part) in raw.split('\t').enumerate() {
                if index > 0 {
                    let spaces = 4 - column % 4;
                    text.push_str(&" ".repeat(spaces));
                    column += spaces;
                }
                text.push_str(part);
                column += Line::from(part).width();
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// Hard-wrap styled code rather than word-wrap it: splitting at words silently
/// consumes spaces around a wrap boundary. Graphemes and token styles stay whole.
fn wrap_code(line: &Line<'static>, width: u16) -> Vec<Line<'static>> {
    let width = width.max(1) as usize;
    let mut rows = vec![];
    let mut spans = vec![];
    let mut used = 0;
    for grapheme in line.styled_graphemes(Style::default()) {
        let cells = Span::raw(grapheme.symbol).width();
        if used + cells > width && !spans.is_empty() {
            rows.push(Line::from(std::mem::take(&mut spans)));
            used = 0;
        }
        // A two-cell glyph cannot fit a one-cell terminal. Keep the rest visible.
        let symbol = if cells > width {
            "�"
        } else {
            grapheme.symbol
        };
        spans.push(Span::styled(symbol.to_owned(), grapheme.style));
        used += cells.min(width);
    }
    if !spans.is_empty() || rows.is_empty() {
        rows.push(Line::from(spans));
    }
    rows
}
fn clip_cells(value: &str, width: usize) -> String {
    let line = Line::from(value);
    let mut text = String::new();
    let mut used = 0;
    for grapheme in line.styled_graphemes(Style::default()) {
        let cells = Span::raw(grapheme.symbol).width();
        if used + cells > width {
            break;
        }
        text.push_str(grapheme.symbol);
        used += cells;
    }
    text
}

/// Markdown and GitHub's HTML subset become inert, styled terminal text.
/// Parsing never loads images, runs scripts, or opens links.
pub fn render_at(source: &str, width: u16) -> Vec<Line<'static>> {
    document_at(source, false, width).lines
}
#[derive(Default)]
pub struct Document {
    pub lines: Vec<Line<'static>>,
    pub headings: Vec<(usize, String)>,
}
/// Use the actual reading column width so tables never spill into another pane.
pub fn document_at(source: &str, spacious: bool, width: u16) -> Document {
    document_nested(source, spacious, width, 0)
}
fn document_nested(source: &str, spacious: bool, width: u16, nesting: usize) -> Document {
    let source = clean(source);
    let parser = Parser::new_ext(
        &source,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS,
    );
    let mut html = String::with_capacity(source.len());
    pulldown_cmark::html::push_html(&mut html, parser);
    let fragment = Html::parse_fragment(&html);
    let mut renderer = Renderer::new(spacious, width.max(1));
    renderer.nesting = nesting;
    renderer.children(fragment.root_element(), base(), 0);
    renderer.finish()
}
fn base() -> Style {
    Style::default().fg(theme().fg)
}
fn muted() -> Style {
    base().fg(theme().muted)
}
fn blocked(name: &str) -> bool {
    matches!(
        name,
        "script"
            | "style"
            | "iframe"
            | "object"
            | "embed"
            | "template"
            | "svg"
            | "math"
            | "noscript"
    )
}
fn invisible(element: ElementRef<'_>) -> bool {
    blocked(element.value().name())
        || element.value().attr("hidden").is_some()
        || element.value().attr("aria-hidden") == Some("true")
}

struct Renderer {
    doc: Document,
    spans: Vec<Span<'static>>,
    spacious: bool,
    width: u16,
    lists: Vec<Option<usize>>,
    quote: usize,
    nesting: usize,
}
impl Renderer {
    fn new(spacious: bool, width: u16) -> Self {
        Self {
            doc: Document::default(),
            spans: vec![],
            spacious,
            width,
            lists: vec![],
            quote: 0,
            nesting: 0,
        }
    }
    fn markdown_preview(&mut self, document: Document) {
        self.doc.lines.push(Line::styled(
            clip_cells("Markdown preview", self.width as usize),
            muted(),
        ));
        for (index, line) in document.lines.iter().enumerate() {
            for (_, title) in document.headings.iter().filter(|(row, _)| *row == index) {
                self.doc
                    .headings
                    .push((self.doc.lines.len(), title.clone()));
            }
            self.doc
                .lines
                .extend(wrap(std::slice::from_ref(line), self.width));
        }
    }
    fn code_box(&mut self, label: &str, document: Document, suggestion: bool) {
        let bordered = self.width >= 6;
        let inner = if bordered { self.width - 4 } else { self.width };
        let border = if suggestion {
            Style::default().fg(theme().green)
        } else {
            muted()
        };
        let surface = base().bg(theme().hunk_bg);
        if bordered {
            let title = clip_cells(&format!(" {label} "), self.width.saturating_sub(2) as usize);
            let fill = (self.width as usize).saturating_sub(2 + Line::from(title.clone()).width());
            self.doc.lines.push(Line::styled(
                format!("╭{title}{}╮", "─".repeat(fill)),
                border,
            ));
        } else {
            self.doc
                .lines
                .push(Line::styled(clip_cells(label, self.width as usize), border));
        }
        for (index, line) in document.lines.iter().enumerate() {
            for (_, title) in document.headings.iter().filter(|(row, _)| *row == index) {
                self.doc
                    .headings
                    .push((self.doc.lines.len(), title.clone()));
            }
            let rows = wrap_code(line, inner);
            for line in rows {
                let used = line.width();
                let mut spans = if bordered {
                    vec![Span::styled("│ ", border)]
                } else {
                    vec![]
                };
                spans.extend(
                    line.spans
                        .into_iter()
                        .map(|span| Span::styled(span.content, surface.patch(span.style))),
                );
                if bordered {
                    spans.push(Span::styled(
                        " ".repeat((inner as usize).saturating_sub(used)),
                        surface,
                    ));
                    spans.push(Span::styled(" │", border));
                }
                self.doc.lines.push(Line::from(spans));
            }
        }
        if bordered {
            self.doc.lines.push(Line::styled(
                format!("╰{}╯", "─".repeat(self.width.saturating_sub(2) as usize)),
                border,
            ));
        }
    }
    fn flush(&mut self) {
        while let Some(last) = self.spans.last_mut() {
            last.content = last.content.trim_end().to_owned().into();
            if !last.content.is_empty() {
                break;
            }
            self.spans.pop();
        }
        if !self.spans.is_empty() {
            self.doc
                .lines
                .push(Line::from(std::mem::take(&mut self.spans)));
        }
    }
    fn blank(&mut self) {
        self.flush();
        if self.doc.lines.last().is_some_and(|line| line.width() > 0) {
            self.doc.lines.push(Line::from(""));
        }
    }
    fn finish(mut self) -> Document {
        self.flush();
        while self.doc.lines.last().is_some_and(|line| line.width() == 0) {
            self.doc.lines.pop();
        }
        self.doc
    }
    fn text(&mut self, source: &str, style: Style) {
        let text = clean(source);
        let words = text.split_whitespace().collect::<Vec<_>>();
        let leading = text.chars().next().is_some_and(char::is_whitespace);
        let trailing = text.chars().last().is_some_and(char::is_whitespace);
        let preceding_space = self
            .spans
            .last()
            .is_none_or(|span| span.content.ends_with(char::is_whitespace));
        let mut value = String::new();
        if leading && !preceding_space {
            value.push(' ');
        }
        value.push_str(&words.join(" "));
        if trailing && !value.ends_with(' ') && !value.is_empty() {
            value.push(' ');
        }
        if !value.is_empty() {
            self.spans.push(Span::styled(value, style));
        }
    }
    fn children(&mut self, element: ElementRef<'_>, style: Style, depth: usize) {
        if depth > 128 {
            return;
        }
        for child in element.children() {
            if let Some(text) = child.value().as_text() {
                self.text(text, style);
            } else if let Some(element) = ElementRef::wrap(child) {
                self.element(element, style, depth + 1);
            }
        }
    }
    fn element(&mut self, element: ElementRef<'_>, style: Style, depth: usize) {
        if invisible(element) {
            return;
        }
        let name = element.value().name();
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "summary" => {
                self.flush();
                if self.spacious {
                    self.blank();
                }
                let start = self.doc.lines.len();
                if name == "summary" {
                    self.spans.push(Span::styled("▾ ", muted()));
                }
                self.children(
                    element,
                    style.fg(theme().accent).add_modifier(Modifier::BOLD),
                    depth,
                );
                self.flush();
                let title = self.doc.lines[start..]
                    .iter()
                    .map(Line::to_string)
                    .collect::<Vec<_>>()
                    .join(" ");
                self.doc.headings.push((start, title));
            }
            "p" | "div" | "section" | "article" | "details" => {
                self.flush();
                if self.quote > 0 {
                    self.spans
                        .push(Span::styled("│ ".repeat(self.quote), muted()));
                }
                self.children(element, style, depth);
                self.flush();
                if self.spacious && self.lists.is_empty() {
                    self.blank();
                }
            }
            "strong" | "b" => self.children(element, style.add_modifier(Modifier::BOLD), depth),
            "em" | "i" => self.children(element, style.add_modifier(Modifier::ITALIC), depth),
            "del" | "s" | "strike" => {
                self.children(element, style.add_modifier(Modifier::CROSSED_OUT), depth)
            }
            "br" => {
                if self.spans.is_empty() {
                    self.doc.lines.push(Line::from(""));
                } else {
                    self.flush();
                }
            }
            "hr" => {
                self.flush();
                self.doc.lines.push(Line::styled(
                    "─".repeat(self.width.min(24) as usize),
                    muted(),
                ));
            }
            "pre" => {
                self.flush();
                let language = element
                    .child_elements()
                    .find(|child| child.value().name() == "code")
                    .and_then(|child| child.value().attr("class"))
                    .and_then(|classes| {
                        classes
                            .split_whitespace()
                            .find_map(|class| class.strip_prefix("language-"))
                    })
                    .unwrap_or("code");
                let code = safe_text(element, 0);
                let mut label = language;
                if language.eq_ignore_ascii_case("mermaid") {
                    match crate::mermaid::request(code.trim(), self.width) {
                        crate::mermaid::RenderStatus::Ready(lines) => {
                            self.doc.lines.push(Line::styled("Diagram", muted()));
                            self.doc.lines.extend(
                                lines.iter().map(|line| {
                                    Line::styled(line.clone(), style.fg(theme().accent))
                                }),
                            );
                            self.blank();
                            return;
                        }
                        crate::mermaid::RenderStatus::Fallback(
                            crate::mermaid::FallbackReason::TooWide,
                        ) => label = "mermaid · widen pane",
                        crate::mermaid::RenderStatus::Fallback(_) => label = "mermaid · source",
                        crate::mermaid::RenderStatus::Pending => {}
                    }
                }
                let lower = language.to_ascii_lowercase();
                let markdown = matches!(lower.as_str(), "markdown" | "md");
                let suggestion = lower == "suggestion" || lower.starts_with("suggestion:");
                if markdown && self.nesting < MAX_PREVIEW_DEPTH && code.len() <= MAX_PREVIEW_BYTES {
                    let nested = document_nested(&code, false, self.width, self.nesting + 1);
                    self.markdown_preview(nested);
                } else {
                    let expanded = expand_code_tabs(&code);
                    let lines = if suggestion {
                        None
                    } else {
                        highlighted_code(&expanded, language)
                    }
                    .unwrap_or_else(|| {
                        expanded
                            .strip_suffix('\n')
                            .unwrap_or(&expanded)
                            .split('\n')
                            .map(|line| Line::from(line.to_owned()))
                            .collect()
                    });
                    let caption = if suggestion {
                        "Suggested change"
                    } else if markdown {
                        "Markdown source · preview limit"
                    } else {
                        label
                    };
                    self.code_box(
                        caption,
                        Document {
                            lines,
                            headings: vec![],
                        },
                        suggestion,
                    );
                }
                if self.spacious {
                    self.blank();
                }
            }
            "code" | "kbd" | "samp" => self.spans.push(Span::styled(
                clean(&safe_text(element, 0)),
                style.fg(theme().accent).bg(theme().hunk_bg),
            )),
            "ul" | "ol" => {
                self.flush();
                self.lists.push(if name == "ol" {
                    Some(
                        element
                            .value()
                            .attr("start")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1),
                    )
                } else {
                    None
                });
                self.children(element, style, depth);
                self.flush();
                self.lists.pop();
                if self.spacious && self.lists.is_empty() {
                    self.blank();
                }
            }
            "li" => {
                self.flush();
                let indent = "  ".repeat(self.lists.len().saturating_sub(1));
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        let s = format!("{n}. ");
                        *n += 1;
                        s
                    }
                    _ => "• ".to_owned(),
                };
                self.spans
                    .push(Span::styled(format!("{indent}{marker}"), style));
                self.children(element, style, depth);
                self.flush();
            }
            "blockquote" => {
                self.flush();
                self.quote += 1;
                self.children(element, style, depth);
                self.flush();
                self.quote -= 1;
            }
            "a" => {
                self.children(
                    element,
                    style.fg(theme().accent).add_modifier(Modifier::UNDERLINED),
                    depth,
                );
            }
            "img" => {
                if let Some(alt) = element.value().attr("alt").filter(|s| !s.is_empty()) {
                    self.text(alt, muted());
                }
            }
            "input" => {
                if element.value().attr("type") == Some("checkbox") {
                    self.spans.push(Span::styled(
                        if element.value().attr("checked").is_some() {
                            "☑ "
                        } else {
                            "☐ "
                        },
                        style.fg(theme().green),
                    ));
                }
            }
            "table" => {
                self.flush();
                let rows = table_rows(element, self.width);
                self.doc.lines.extend(render_table(&rows, self.width));
                if self.spacious {
                    self.blank();
                }
            }
            _ => self.children(element, style, depth),
        }
    }
}
fn safe_text(element: ElementRef<'_>, depth: usize) -> String {
    if depth > 128 || invisible(element) {
        return String::new();
    }
    let mut output = String::new();
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            output.push_str(text);
        } else if let Some(element) = ElementRef::wrap(child) {
            output.push_str(&safe_text(element, depth + 1));
        }
    }
    output
}
#[derive(Clone, Copy, Default)]
enum Align {
    #[default]
    Left,
    Right,
    Center,
}
struct Cell {
    lines: Vec<Line<'static>>,
    align: Align,
    heading: bool,
}
fn table_rows(table: ElementRef<'_>, width: u16) -> Vec<Vec<Cell>> {
    let mut rows = vec![];
    fn visit(element: ElementRef<'_>, width: u16, rows: &mut Vec<Vec<Cell>>, depth: usize) {
        if depth > 128 || invisible(element) {
            return;
        }
        for child in element.child_elements() {
            match child.value().name() {
                "tr" => {
                    let mut row = vec![];
                    for cell in child
                        .child_elements()
                        .filter(|e| matches!(e.value().name(), "td" | "th"))
                    {
                        let heading = cell.value().name() == "th";
                        let mut renderer = Renderer::new(false, width);
                        renderer.children(
                            cell,
                            if heading {
                                base().add_modifier(Modifier::BOLD)
                            } else {
                                base()
                            },
                            0,
                        );
                        let alignment = cell.value().attr("align").or_else(|| {
                            cell.value()
                                .attr("style")?
                                .split(';')
                                .find_map(|declaration| {
                                    let (property, value) = declaration.split_once(':')?;
                                    (property.trim() == "text-align").then_some(value.trim())
                                })
                        });
                        let align = match alignment {
                            Some("right") => Align::Right,
                            Some("center") => Align::Center,
                            _ => Align::Left,
                        };
                        row.push(Cell {
                            lines: renderer.finish().lines,
                            align,
                            heading,
                        });
                    }
                    if !row.is_empty() {
                        rows.push(row);
                    }
                }
                "thead" | "tbody" | "tfoot" => visit(child, width, rows, depth + 1),
                _ => {}
            }
        }
    }
    visit(table, width, &mut rows, 0);
    rows
}
/// Ratatui handles grapheme boundaries, wide glyphs, and styled wrapping identically
/// to the rest of the reading pane. Reconstruct spans without wide-cell padding.
pub(crate) fn wrap(lines: &[Line<'static>], width: u16) -> Vec<Line<'static>> {
    if lines.is_empty() {
        return vec![Line::from("")];
    }
    let width = width.max(1);
    let paragraph = Paragraph::new(lines.to_vec()).wrap(Wrap { trim: false });
    let height = paragraph.line_count(width).max(1).min(u16::MAX as usize) as u16;
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);
    paragraph.render(area, &mut buffer);
    (0..height)
        .map(|y| {
            let mut spans: Vec<Span<'static>> = vec![];
            let mut x = 0;
            while x < width {
                let cell = &buffer[(x, y)];
                let symbol = cell.symbol();
                spans.push(Span::styled(symbol.to_owned(), cell.style()));
                x += Span::raw(symbol).width().max(1) as u16;
            }
            while let Some(last) = spans.last() {
                if last.content == " " {
                    spans.pop();
                } else {
                    break;
                }
            }
            Line::from(spans)
        })
        .collect()
}
fn render_table(rows: &[Vec<Cell>], width: u16) -> Vec<Line<'static>> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return vec![];
    }
    // Small reading panes use labelled records instead of losing columns off-screen.
    if (width as usize) < columns * 8 + 1 {
        return stacked_table(rows, width);
    }
    let mut widths = (0..columns)
        .map(|i| {
            rows.iter()
                .filter_map(|row| row.get(i))
                .flat_map(|cell| &cell.lines)
                .map(Line::width)
                .max()
                .unwrap_or(1)
                .max(1)
        })
        .collect::<Vec<_>>();
    let inset = usize::from(widths.iter().sum::<usize>() + columns * 3 + 1 <= width as usize);
    let separators = columns * (1 + inset * 2) + 1;
    let available = (width as usize).saturating_sub(separators).max(columns);
    while widths.iter().sum::<usize>() > available {
        let (i, &longest) = widths
            .iter()
            .enumerate()
            .max_by_key(|(_, value)| **value)
            .unwrap();
        if longest <= 1 {
            break;
        }
        widths[i] -= 1;
    }
    let border = |left: &str, join: &str, right: &str| {
        Line::styled(
            format!(
                "{left}{}{right}",
                widths
                    .iter()
                    .map(|width| "─".repeat(width + inset * 2))
                    .collect::<Vec<_>>()
                    .join(join)
            ),
            muted(),
        )
    };
    let mut result = vec![border("┌", "┬", "┐")];
    for (row_index, row) in rows.iter().enumerate() {
        let cells = widths
            .iter()
            .enumerate()
            .map(|(i, width)| {
                row.get(i)
                    .map(|cell| wrap(&cell.lines, *width as u16))
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let height = cells.iter().map(Vec::len).max().unwrap_or(1);
        for y in 0..height {
            let mut spans = vec![Span::styled("│", muted())];
            for (i, width) in widths.iter().enumerate() {
                let line = cells[i].get(y).cloned().unwrap_or_default();
                let padding = width.saturating_sub(line.width());
                let left = match row.get(i).map(|c| c.align).unwrap_or_default() {
                    Align::Left => 0,
                    Align::Right => padding,
                    Align::Center => padding / 2,
                };
                spans.push(Span::raw(" ".repeat(left + inset)));
                spans.extend(line.spans);
                spans.push(Span::raw(" ".repeat(padding - left + inset)));
                spans.push(Span::styled("│", muted()));
            }
            result.push(Line::from(spans));
        }
        if row_index + 1 < rows.len() {
            result.push(border("├", "┼", "┤"));
        }
    }
    result.push(border("└", "┴", "┘"));
    result
}
fn stacked_table(rows: &[Vec<Cell>], width: u16) -> Vec<Line<'static>> {
    let header = rows
        .first()
        .filter(|row| row.iter().any(|cell| cell.heading));
    let mut output = vec![];
    for row in rows.iter().skip(usize::from(header.is_some())) {
        if !output.is_empty() {
            output.push(Line::styled("─".repeat(width.min(24) as usize), muted()));
        }
        for (column, cell) in row.iter().enumerate() {
            let label = header
                .and_then(|head| head.get(column))
                .map(|cell| {
                    cell.lines
                        .iter()
                        .map(Line::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_else(|| format!("Column {}", column + 1));
            output.extend(wrap(
                &[Line::styled(
                    label,
                    base().add_modifier(Modifier::BOLD).fg(theme().accent),
                )],
                width,
            ));
            output.extend(wrap(&cell.lines, width));
        }
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mermaid_renders_in_both_markdown_styles_and_preserves_unsupported_source() {
        let source = "flowchart TD\n  Request[Request] --> Review[Review]";
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match crate::mermaid::request(source, 80) {
                crate::mermaid::RenderStatus::Ready(_) => break,
                crate::mermaid::RenderStatus::Pending if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                other => panic!("Diagram did not render: {other:?}"),
            }
        }
        for spacious in [false, true] {
            let doc = document_at(&format!("```mermaid\n{source}\n```"), spacious, 80);
            let text = doc
                .lines
                .iter()
                .map(Line::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                text.contains("Diagram") && text.contains("Request") && text.contains("Review")
            );
            assert!(text.contains('─') && !text.contains("-->"));
            let fallback = document_at("```mermaid\npie\n  \"Tests\" : 4\n```", spacious, 80);
            assert!(
                fallback
                    .lines
                    .iter()
                    .any(|line| line.to_string().contains("Tests"))
            );
        }
    }
    fn render(source: &str) -> Vec<Line<'static>> {
        render_at(source, 84)
    }
    const COVERAGE_HTML: &str = r#"<h2>Coverage report</h2><table><thead><tr><th>Status</th><th>Category</th><th>Percentage</th><th>Covered / Total</th></tr></thead><tbody><tr><td>🟢</td><td>Lines</td><td>41.35% (🎯 36%)<br/>= <em>±0 pp</em></td><td>6441 / 15575</td></tr><tr><td>🟢</td><td>Statements</td><td>40.46%</td><td>6924 / 17112</td></tr></tbody></table><details><summary>File Coverage</summary>No changed files found.</details><em>Generated in <a href="https://example.org/run/42">#42</a>.</em>"#;
    #[test]
    fn links_show_labels_without_appended_destinations_on_every_surface() {
        for spacious in [false, true] {
            let doc = document_at(
                "Read [**the guide**](https://example.org/very/long/path?token=not-a-secret). <a href='https://example.org/build/42'>Build status</a>\n\n| Result |\n| --- |\n| [Passed](https://example.org/check) |",
                spacious,
                40,
            );
            let text = doc
                .lines
                .iter()
                .map(Line::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            for label in ["the guide", "Build status", "Passed"] {
                assert!(text.contains(label));
            }
            assert!(
                !text.contains("https://"),
                "Destinations must not clutter descriptions, comments, tables or threads: {text}"
            );
            assert!(
                doc.lines
                    .iter()
                    .flat_map(|line| &line.spans)
                    .any(|span| span.style.add_modifier.contains(Modifier::UNDERLINED))
            );
        }
        let literal = render_at("```txt\nhttps://example.org/literal\n```", 40)
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            literal.contains("https://example.org/literal"),
            "Code text remains literal"
        );
    }
    #[test]
    fn coverage_html_is_semantic_text_not_raw_tags() {
        let text = render(COVERAGE_HTML)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Coverage report"));
        assert!(text.contains("41.35%"));
        assert!(text.contains("File Coverage"));
        assert!(
            !text.contains("<table>"),
            "HTML coverage tables should render as cells, not tags: {text}"
        );
        assert!(!text.contains("<em>"));
    }
    #[test]
    fn html_tables_fit_wide_narrow_and_stacked_reading_columns() {
        let table = "<table><tr><th>Status</th><th>Category</th><th>Percentage</th><th>Covered / Total</th></tr><tr><td>🟢</td><td>Lines</td><td>41.35%<br>±0 pp</td><td>6441 / 15575</td></tr></table>";
        for width in [84, 42, 25, 18, 8] {
            let lines = render_at(table, width);
            assert!(
                lines.iter().all(|line| line.width() <= width as usize),
                "table overflow at {width}: {lines:?}"
            );
            let text = lines
                .iter()
                .map(Line::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("🟢"));
            assert!(text.contains("41.35%") || width < 25);
            assert!(!text.contains("<td>"));
        }
        let narrow = render_at(table, 18)
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(narrow.contains("Category\nLines"));
    }
    #[test]
    fn html_entities_inline_styles_links_and_details_are_safe_and_complete() {
        let doc = document_at(
            "Hello <em>world</em> &amp; **friends**.\n\n<details><summary>File coverage</summary><p>A &lt; B</p><a href='https://example.org/build'>build</a><a href='javascript:alert(1)'>dangerous link</a><script>secret()</script><style>.bad {}</style><img src='https://example.org/tracker' alt='Diagram'><span hidden>hidden text</span></details>",
            true,
            50,
        );
        let text = doc
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "Hello world & friends.",
            "File coverage",
            "A < B",
            "build",
            "Diagram",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
        for forbidden in [
            "https://example.org/build",
            "javascript:",
            "secret()",
            ".bad",
            "tracker",
            "hidden text",
            "<details>",
        ] {
            assert!(!text.contains(forbidden), "{forbidden}: {text}");
        }
        assert!(
            doc.headings
                .iter()
                .any(|(_, heading)| heading.contains("File coverage"))
        );
        assert!(
            doc.lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.content == "world"
                    && span.style.add_modifier.contains(Modifier::ITALIC))
        );
    }
    #[test]
    fn literal_html_inside_code_remains_literal_and_gfm_tables_share_layout() {
        let text = render("```html\n<script>example()</script>\n```")
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("<script>example()</script>"));
        let table = render_at(
            "| File | Coverage |\n| --- | ---: |\n| app.ts | **98%** |",
            30,
        );
        assert!(table.iter().any(|line| line.to_string().contains("┌")));
        assert!(table.iter().all(|line| line.width() <= 30));
        assert!(table.iter().flat_map(|line| &line.spans).any(|span| span.content == "9" && span.style.add_modifier.contains(Modifier::BOLD)));
    }
    #[test]
    #[ignore = "manual native-cell visual artifact"]
    fn export_html_report_preview() {
        let mut views = vec![];
        for width in [84, 42, 20] {
            let lines = render_at(COVERAGE_HTML, width);
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
            let height = paragraph.line_count(width).min(100) as u16;
            let rect = Rect::new(0, 0, width, height);
            let mut buffer = Buffer::empty(rect);
            paragraph.render(rect, &mut buffer);
            views.push(serde_json::json!({"width":width,"height":height,"html":crate::preview::buffer_html(&buffer)}));
        }
        std::fs::write(
            "/private/tmp/rw-html-reports.json",
            serde_json::to_vec_pretty(&views).unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn markdown_supports_headings_code_lists_tasks_tables_and_inert_html() {
        let input = "# Heading\n\n**bold** and *emphasis* with `code`\n\n- [x] done\n- [ ] pending\n\n```rs\nlet x = 1;\n```\n\n| One | Two |\n| --- | --- |\n| A | B |\n\n<script>bad()</script>\x1b[31m\n";
        let lines = render(input);
        let text = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        for part in ["Heading", "bold", "☑", "☐", "let x = 1;", "One", "Two"] {
            assert!(text.contains(part), "{part}");
        }
        assert!(!text.contains("<script>"));
        assert!(!text.contains("bad()"));
        assert!(!text.contains("```"));
        assert!(!text.contains('\x1b'));
        assert!(!text.contains("**"));
        assert!(
            lines
                .iter()
                .flat_map(|l| &l.spans)
                .any(|s| s.content == "bold" && s.style.add_modifier.contains(Modifier::BOLD))
        );
    }
    fn settled_document(source: &str, spacious: bool, width: u16) -> Document {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            set_palette(CODE_PALETTE.with(|palette| palette.get()));
            let before = revision();
            let document = document_at(source, spacious, width);
            let pending = CODE_WORKER
                .get()
                .and_then(Option::as_ref)
                .is_some_and(|worker| {
                    worker
                        .cache
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|entry| matches!(entry.result, CodeResult::Pending))
                });
            if !pending && before == revision() {
                return document;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Syntax worker did not settle"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    const NESTED_MARKDOWN: &str = "# Review notes\n\n<details><summary>Prompt To Fix With AI</summary>\n\n`````markdown\n## Suggested documentation\nKeep the explanation near the setting.\n\n```suggestion\n- `needs_device` enables the `device-tests` job for changes in `app/mobile/`.\n  Otherwise it remains off.\n```\n\n```rust\nfn next_index(count: usize) -> Option<usize> {\n    if count == 0 { return None; }\n    Some(0)\n}\n```\n`````\n\n</details>\n";
    #[test]
    fn explicit_markdown_wrapper_previews_nested_suggestion_without_raw_fences() {
        for spacious in [false, true] {
            let doc = settled_document(NESTED_MARKDOWN, spacious, 84);
            let text = doc
                .lines
                .iter()
                .map(Line::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                text.contains("Markdown preview"),
                "An explicit Markdown wrapper should have a labelled preview: {text}"
            );
            assert!(text.contains("Suggested change"));
            assert!(
                text.contains("`needs_device`"),
                "Suggestion replacement remains literal"
            );
            assert!(!text.contains("```suggestion"));
            assert!(!text.contains("```rust"));
            assert!(
                doc.headings
                    .iter()
                    .any(|(_, title)| title == "Suggested documentation")
            );
        }
    }
    #[test]
    fn declared_code_language_uses_existing_syntax_colors_in_both_reader_styles() {
        for spacious in [false, true] {
            let doc = settled_document(
                "```rust\nfn next(count: usize) -> Option<usize> {\n    if count == 0 { return None; }\n    Some(0)\n}\n```",
                spacious,
                84,
            );
            let colors = doc
                .lines
                .iter()
                .flat_map(|l| &l.spans)
                .filter(|s| !s.content.trim().is_empty())
                .filter_map(|s| s.style.fg)
                .collect::<std::collections::HashSet<_>>();
            assert!(
                colors.len() >= 4,
                "Expected syntax tokens, found {colors:?}"
            );
        }
    }
    #[test]
    #[ignore = "manual code-block native-cell artifact"]
    fn export_code_block_preview() {
        let mut views = vec![];
        for spacious in [false, true] {
            for width in [84, 42] {
                let lines = settled_document(NESTED_MARKDOWN, spacious, width).lines;
                let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
                let height = paragraph.line_count(width).min(110) as u16;
                let rect = Rect::new(0, 0, width, height);
                let mut buffer = Buffer::empty(rect);
                paragraph.render(rect, &mut buffer);
                views.push(serde_json::json!({"panel":if spacious {"Paper"} else {"Compact"},"width":width,"height":height,"html":crate::preview::buffer_html(&buffer)}));
            }
        }
        let path = std::env::var("LAZYREVIEW_CODE_PREVIEW")
            .unwrap_or("/private/tmp/lazyreview-code-preview.json".into());
        std::fs::write(path, serde_json::to_vec_pretty(&views).unwrap()).unwrap();
    }

    #[test]
    #[ignore = "manual cold/warm syntax timing"]
    fn benchmark_code_block_rendering() {
        let source=(0..160).map(|index|format!("    let value_{index} = options.get(\"example\").map(|value| value.len()).unwrap_or(0); // data\n")).collect::<String>();
        assert!(source.len() <= MAX_CODE_BYTES);
        let markup = format!("```rust\n{source}```");
        let start = std::time::Instant::now();
        let cold = document_at(&markup, false, 84);
        let cold_ms = start.elapsed().as_secs_f64() * 1000.;
        let ready_start = std::time::Instant::now();
        settled_document(&markup, false, 84);
        let ready_ms = ready_start.elapsed().as_secs_f64() * 1000.;
        let start = std::time::Instant::now();
        for _ in 0..100 {
            std::hint::black_box(document_at(&markup, false, 84));
        }
        println!(
            "code render: bytes={} lines={} cold_ms={cold_ms:.3} ready_after_ms={ready_ms:.3} warm_mean_ms={:.3}",
            source.len(),
            cold.lines.len(),
            start.elapsed().as_secs_f64() * 10.
        );
    }
    #[test]
    fn literal_code_preserves_indentation_tabs_blank_lines_and_wrap_spaces() {
        let raw = "\tlet wide = \"界e\u{301}\";  \n    second\tvalue\n\n";
        let expanded = expand_code_tabs(raw);
        assert_eq!(
            expanded,
            "    let wide = \"界e\u{301}\";  \n    second  value\n\n"
        );
        for width in [2, 7, 16, 60] {
            for source in expanded.strip_suffix('\n').unwrap().split('\n') {
                let line = Line::from(source.to_owned());
                let rows = wrap_code(&line, width);
                assert!(rows.iter().all(|row| row.width() <= width as usize));
                assert_eq!(rows.iter().map(Line::to_string).collect::<String>(), source);
            }
        }
        let doc = settled_document(&format!("```rust\n{raw}```"), false, 84);
        let text = doc.lines.iter().map(Line::to_string).collect::<Vec<_>>();
        assert!(
            text.iter()
                .any(|row| row.starts_with("│     let wide = \"界e\u{301}\";  "))
        );
        assert_eq!(
            text.len(),
            5,
            "Three code rows, including a trailing blank, plus two borders"
        );
        assert_eq!(text[3].trim_matches(|c| c == '│' || c == ' '), "");
    }
    #[test]
    fn nested_preview_is_bounded_and_other_languages_stay_literal() {
        let source = "``````markdown\n`````md\n````markdown\n# stays literal at depth limit\n````\n`````\n``````";
        let doc = document_at(source, false, 84);
        let text = doc
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(text.matches("Markdown preview").count(), 2);
        assert!(text.contains("Markdown source · preview limit"));
        assert!(text.contains("# stays literal"));
        assert!(doc.headings.is_empty());
        let literal = "````rust\nlet doc = r#\"```suggestion\n**literal**\n```\"#;\n````";
        let text = settled_document(literal, false, 84)
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("```suggestion"));
        assert!(text.contains("**literal**"));
        assert!(!text.contains("Markdown preview"));
        let suggestion = document_at("```suggestion:-1+2\n**literal** and `code`\n```", false, 84);
        let text = suggestion
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Suggested change"));
        assert!(text.contains("**literal** and `code`"));
    }
    #[test]
    fn code_boxes_fit_narrow_columns_without_losing_outline_positions() {
        for width in [1, 2, 5, 6, 8, 18, 42, 84] {
            let doc = settled_document(
                "````md\n## Nested title\n\n```rust\nlet answer = \"界\";\n```\n````",
                false,
                width,
            );
            assert!(
                doc.lines.iter().all(|line| line.width() <= width as usize),
                "code overflow at {width}: {:?}",
                doc.lines
            );
            let row = doc
                .headings
                .iter()
                .find(|(_, name)| name == "Nested title")
                .unwrap()
                .0;
            assert!(row < doc.lines.len());
            if width >= 18 {
                assert!(doc.lines[row].to_string().contains("Nested title"));
            }
        }
    }
    #[test]
    fn preview_keeps_html_inert_and_link_destinations_hidden() {
        let source = "````md\n# Safe heading\n<script>never_execute()</script>\n\n[build](https://example.org/private)\n\n```html\n<script>literal()</script>\n```\n````";
        let text = settled_document(source, false, 84)
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Safe heading"));
        assert!(text.contains("build"));
        assert!(!text.contains("https://example.org"));
        assert!(!text.contains("never_execute"));
        assert!(text.contains("<script>literal()</script>"));
    }
    #[test]
    fn syntax_palette_and_language_aliases_share_consistent_cache_entries() {
        let source = "fn cache_palette_probe() { let value = \"palette\"; }\n";
        set_palette(crate::syntax::Palette::Github);
        settled_document(&format!("```rs\n{source}```"), false, 84);
        let github = highlighted_code(source, "RUST").expect("Alias result should be ready");
        assert!(highlighted_code(source, "rs").is_some());
        let worker = CODE_WORKER.get().unwrap().as_ref().unwrap();
        assert_eq!(
            worker
                .cache
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.key.source == source
                    && entry.key.palette == crate::syntax::Palette::Github)
                .count(),
            1
        );
        set_palette(crate::syntax::Palette::Lunar);
        settled_document(&format!("```rs\n{source}```"), false, 84);
        let lunar = highlighted_code(source, "rust").unwrap();
        assert_eq!(
            github.iter().map(Line::to_string).collect::<String>(),
            lunar.iter().map(Line::to_string).collect::<String>()
        );
        assert_ne!(
            github, lunar,
            "Palette changes must invalidate cached token colors"
        );
        set_palette(crate::syntax::Palette::Github);
    }
    #[test]
    fn code_worker_deduplicates_pending_requests_and_bounds_queue() {
        use std::sync::{Arc, Mutex, atomic::AtomicU64, mpsc};
        let (tx, rx) = mpsc::sync_channel(2);
        let worker = CodeWorker {
            tx,
            cache: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            revision: Arc::new(AtomicU64::new(0)),
        };
        for _ in 0..10 {
            assert!(
                worker
                    .request("first", "rust", crate::syntax::Palette::Github)
                    .is_none()
            );
        }
        assert!(
            worker
                .request("second", "rust", crate::syntax::Palette::Github)
                .is_none()
        );
        assert!(
            worker
                .request("third", "rust", crate::syntax::Palette::Github)
                .is_none()
        );
        assert_eq!(
            worker.cache.lock().unwrap().len(),
            2,
            "Queue-full requests must not leave unfinishable Pending entries"
        );
        assert_eq!(rx.try_recv().unwrap().source, "first");
        assert_eq!(rx.try_recv().unwrap().source, "second");
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn unsupported_and_oversized_code_remains_readable_without_repeated_work() {
        let source = "plain_fallback_probe()\n";
        let document = settled_document(&format!("```not-a-real-language\n{source}```"), false, 60);
        assert!(
            document
                .lines
                .iter()
                .any(|line| line.to_string().contains("plain_fallback_probe()"))
        );
        let worker = CODE_WORKER.get().unwrap().as_ref().unwrap();
        assert!(
            worker
                .cache
                .lock()
                .unwrap()
                .iter()
                .any(|entry| entry.key.source == source
                    && matches!(entry.result, CodeResult::Ready(None)))
        );
        for source in [
            "x".repeat(MAX_CODE_BYTES + 1),
            "a\n".repeat(161),
            "x".repeat(2049),
        ] {
            assert!(highlighted_code(&source, "rust").is_none());
            assert!(
                !worker
                    .cache
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|entry| entry.key.source == source)
            );
        }
    }
    #[test]
    fn long_documents_stabilize_at_cache_budget_and_new_views_can_replace_entries() {
        use std::sync::atomic::Ordering;
        let worker = CodeWorker::start().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut passes = 0;
        loop {
            set_palette(crate::syntax::Palette::Github);
            let before = worker.revision.load(Ordering::Relaxed);
            for index in 0..70 {
                worker.request(
                    &format!("let history_{index} = {index};\n"),
                    "rust",
                    crate::syntax::Palette::Github,
                );
            }
            while worker
                .cache
                .lock()
                .unwrap()
                .iter()
                .any(|entry| matches!(entry.result, CodeResult::Pending))
            {
                assert!(
                    std::time::Instant::now() < deadline,
                    "Long document syntax did not settle"
                );
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            passes += 1;
            if before == worker.revision.load(Ordering::Relaxed) {
                break;
            }
            assert!(passes < 15, "Rendering evicted and requeued earlier blocks");
        }
        assert_eq!(worker.cache.lock().unwrap().len(), CODE_CACHE_SIZE);
        let settled = worker.revision.load(Ordering::Relaxed);
        assert!((CODE_CACHE_SIZE as u64..=70).contains(&settled));
        // A fast worker may admit a later block between earlier queue-full
        // requests; it may be replaced once, but settled renders never churn.
        for _ in 0..5 {
            set_palette(crate::syntax::Palette::Github);
            for index in 0..70 {
                worker.request(
                    &format!("let history_{index} = {index};\n"),
                    "rust",
                    crate::syntax::Palette::Github,
                );
            }
            assert_eq!(worker.revision.load(Ordering::Relaxed), settled);
            assert!(
                worker
                    .cache
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|entry| matches!(entry.result, CodeResult::Ready(_)))
            );
        }
        set_palette(crate::syntax::Palette::Github);
        worker.request(
            "let different_view = true;\n",
            "rust",
            crate::syntax::Palette::Github,
        );
        assert!(
            worker
                .cache
                .lock()
                .unwrap()
                .iter()
                .any(|entry| entry.key.source.contains("different_view")),
            "A new view may replace entries not used this frame"
        );
        assert_eq!(worker.cache.lock().unwrap().len(), CODE_CACHE_SIZE);
    }
}
