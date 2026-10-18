//! Companion and document reading surfaces. Claude's session state stays in ai.rs.
use crate::{
    app::{App, Input, Modal, Pane, Panel, array, s},
    markdown,
    theme::current as theme,
    ui::clean,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::hash::{Hash, Hasher};
pub const READING_WIDTH: u16 = 84;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Appearance {
    Ledger,
    #[default]
    Paper,
    Studio,
}
impl Appearance {
    pub const ALL: [Self; 3] = [Self::Ledger, Self::Paper, Self::Studio];
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "ledger" => Ok(Self::Ledger),
            "paper" => Ok(Self::Paper),
            "studio" => Ok(Self::Studio),
            _ => Err("companion style must be ledger, paper, or studio".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Ledger => "Ledger",
            Self::Paper => "Paper",
            Self::Studio => "Studio",
        }
    }
    pub fn detail(self) -> &'static str {
        match self {
            Self::Ledger => "Dense transcript · minimal chrome",
            Self::Paper => "Quiet column · generous reading rhythm",
            Self::Studio => "Filled headers · distinct message surfaces",
        }
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(1)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Top,
    Comments,
    Commits,
}
#[derive(Default)]
pub struct Reader {
    pub appearance: Appearance,
    pub choice: usize,
    pub outline_cursor: usize,
    pub artifact_cursor: usize,
    pub focused: bool,
    pub selection: crate::copy_text::Selection,
    pub sections: Vec<(usize, String)>,
    pub max_scroll: usize,
    pub destination: Option<Destination>,
    cache: Cache,
}
#[derive(Default)]
struct Cache {
    key: u64,
    width: u16,
    lines: Vec<Line<'static>>,
    offsets: Vec<usize>,
    height: usize,
}
impl Reader {
    pub fn jump(&self, at: usize, forward: bool) -> usize {
        if forward {
            self.sections
                .iter()
                .find(|(row, _)| *row > at)
                .map(|s| s.0)
                .unwrap_or(self.max_scroll)
        } else {
            self.sections
                .iter()
                .rev()
                .find(|(row, _)| *row < at)
                .map(|s| s.0)
                .unwrap_or(0)
        }
    }
}
fn text(value: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(clean(&value.into()), Style::default().fg(color))
}
fn base() -> Style {
    Style::default().fg(theme().fg)
}
fn paragraph(lines: impl Into<Text<'static>>) -> Paragraph<'static> {
    Paragraph::new(lines)
        .style(base())
        .wrap(Wrap { trim: false })
}
fn height(lines: Vec<Line<'static>>, width: u16) -> usize {
    paragraph(lines).line_count(width)
}
fn row(f: &mut Frame, area: Rect, line: Line<'static>) {
    f.render_widget(Paragraph::new(line).style(base()), area);
}
fn clip(value: &str, width: usize) -> String {
    let value = clean(value);
    if Line::from(value.as_str()).width() <= width {
        return value;
    }
    let mut out = String::new();
    let mut used = 0;
    for c in value.chars() {
        let w = Span::raw(c.to_string()).width();
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    if width > 0 {
        out.push('…');
    }
    out
}
fn header(title: String, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {title} "))
        .border_style(if focused {
            theme().active
        } else {
            theme().inactive
        })
}
fn content_key(a: &App) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.reader.appearance.hash(&mut h);
    a.syntax_palette.name().hash(&mut h);
    crate::mermaid::revision().hash(&mut h);
    crate::markdown::revision().hash(&mut h);
    format!("{:?}", a.panel).hash(&mut h);
    if a.panel == Panel::Threads {
        a.panel_cursor.hash(&mut h);
    }
    format!("{:?}", theme()).hash(&mut h);
    if a.panel == Panel::Ai {
        for message in &a.messages {
            s(message, "role").hash(&mut h);
            s(message, "content").hash(&mut h);
        }
        a.ai.summary.hash(&mut h);
        a.ai.streaming.hash(&mut h);
        a.ai.error.hash(&mut h);
        a.ai.enabled.hash(&mut h);
        a.ai.display_truncated.hash(&mut h);
        a.is_demo().hash(&mut h);
    } else {
        s(&a.pr, "title").hash(&mut h);
        s(&a.pr, "body").hash(&mut h);
        for warning in array(&a.pr, "warnings") {
            warning.as_str().hash(&mut h);
        }
        for key in ["author", "state", "createdAt"] {
            s(&a.pr, key).hash(&mut h);
        }
        for field in ["conversation", "reviews", "commits"] {
            for event in array(&a.pr, field) {
                for key in [
                    "body",
                    "author",
                    "createdAt",
                    "submittedAt",
                    "state",
                    "sha",
                    "message",
                ] {
                    s(event, key).hash(&mut h);
                }
            }
        }
        if a.panel == Panel::Threads {
            let thread = a.selected_thread();
            for key in ["path", "line", "resolved", "outdated", "pending"] {
                thread[key].to_string().hash(&mut h);
            }
            for comment in array(thread, "comments") {
                for key in ["author", "body", "createdAt", "pending"] {
                    comment[key].to_string().hash(&mut h);
                }
            }
        }
    }
    h.finish()
}
fn message(
    lines: &mut Vec<Line<'static>>,
    role: &str,
    body: &str,
    appearance: Appearance,
    width: u16,
) {
    if !lines.is_empty() && appearance != Appearance::Ledger {
        lines.push(Line::from(""));
    }
    let user = role == "user";
    let color = if user { theme().accent } else { theme().purple };
    let name = if user { "You" } else { "Claude" };
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let heading = match appearance {
        Appearance::Ledger => {
            Line::styled(format!("{} {name}", if user { "›" } else { "·" }), style)
        }
        Appearance::Paper => {
            Line::styled(format!("{} {name}", if user { "▎" } else { "✦" }), style)
        }
        Appearance::Studio => Line::styled(format!(" {name} "), style.bg(theme().hunk_bg)),
    };
    lines.push(heading);
    let doc = markdown::document_at(body, appearance != Appearance::Ledger, width);
    lines.extend(doc.lines);
}
fn date(value: &str) -> String {
    if value.len() >= 16
        && value.is_ascii()
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
    {
        format!("{} {}", &value[..10], &value[11..16])
    } else {
        clean(value)
    }
}
/// Pre-wrap the markdown inside the card so a wrapped paragraph never loses its
/// border. The markdown renderer still owns tables, diagrams, links and styling.
pub(crate) fn comment_card(
    author: &str,
    when: &str,
    label: &str,
    source: &str,
    width: u16,
) -> Vec<Line<'static>> {
    if width < 8 {
        let mut lines = vec![Line::styled(
            clip(&format!("@{author} {label}"), width as usize),
            base().fg(theme().accent),
        )];
        lines.extend(markdown::document_at(source, false, width).lines);
        return lines;
    }
    let inner = width.saturating_sub(4);
    let heading = clip(
        &format!(
            "@{}{}{}",
            clean(author),
            if label.is_empty() {
                String::new()
            } else {
                format!(" · {label}")
            },
            if when.is_empty() {
                String::new()
            } else {
                format!(" · {}", date(when))
            }
        ),
        width.saturating_sub(4) as usize,
    );
    let mut lines = vec![Line::from(vec![
        text("╭ ", theme().muted),
        Span::styled(
            heading.clone(),
            base().fg(theme().accent).add_modifier(Modifier::BOLD),
        ),
        text(
            format!(
                " {}╮",
                "─".repeat(width as usize - Line::from(heading).width() - 4)
            ),
            theme().muted,
        ),
    ])];
    let document = markdown::document_at(source, false, inner);
    for mut body in markdown::wrap(&document.lines, inner) {
        let used = body.width();
        let mut spans = vec![text("│ ", theme().muted)];
        spans.append(&mut body.spans);
        spans.push(Span::raw(" ".repeat((inner as usize).saturating_sub(used))));
        spans.push(text(" │", theme().muted));
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(text(
        format!("╰{}╯", "─".repeat(width.saturating_sub(2) as usize)),
        theme().muted,
    )));
    lines
}
fn thread_document(a: &App, width: u16) -> markdown::Document {
    let mut doc = markdown::Document::default();
    let thread = a.selected_thread();
    if thread.is_null() {
        doc.lines
            .push(Line::from(text("No review threads.", theme().muted)));
        return doc;
    }
    let state = if thread["pending"] == true {
        "Pending review"
    } else if thread["resolved"] == true {
        "Resolved"
    } else if thread["outdated"] == true {
        "Outdated"
    } else {
        "Open"
    };
    doc.lines.push(Line::styled(
        format!(
            "{}:{}",
            s(thread, "path"),
            thread["line"]
                .as_u64()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "file".into())
        ),
        base().add_modifier(Modifier::BOLD),
    ));
    doc.lines.push(Line::from(text(
        state,
        if thread["resolved"] == true {
            theme().green
        } else {
            theme().muted
        },
    )));
    for comment in array(thread, "comments") {
        doc.lines.push(Line::from(""));
        doc.headings.push((
            doc.lines.len(),
            format!(
                "@{} · {}",
                s(comment, "author"),
                date(s(comment, "createdAt"))
            ),
        ));
        doc.lines.extend(comment_card(
            s(comment, "author"),
            s(comment, "createdAt"),
            if comment["pending"] == true {
                "pending"
            } else {
                ""
            },
            s(comment, "body"),
            width,
        ));
    }
    doc
}
fn append_timeline(doc: &mut markdown::Document, a: &App, width: u16) {
    let mut events = vec![];
    for (kind, key) in [(0, "commits"), (1, "reviews"), (2, "conversation")] {
        for (index, event) in array(&a.pr, key).iter().enumerate() {
            if kind == 1 && s(event, "state") == "PENDING" {
                continue;
            }
            let when = if s(event, "createdAt").is_empty() {
                s(event, "submittedAt")
            } else {
                s(event, "createdAt")
            };
            events.push((when, kind, index, event));
        }
    }
    // Missing timestamps remain visible at the end; never invent chronology.
    events.sort_by(|a, b| {
        a.0.is_empty()
            .cmp(&b.0.is_empty())
            .then(a.0.cmp(b.0))
            .then(a.1.cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });
    if events.is_empty() {
        return;
    }
    doc.lines.push(Line::from(""));
    doc.lines
        .push(Line::from(text("─".repeat(width as usize), theme().muted)));
    for (when, kind, _, event) in events {
        if kind == 0 {
            let sha = s(event, "sha").chars().take(7).collect::<String>();
            let title = s(event, "message").lines().next().unwrap_or("");
            let suffix = if width >= 72 {
                format!(
                    " · @{}{}",
                    clip(s(event, "author"), 12),
                    if when.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", when.get(..10).unwrap_or(when))
                    }
                )
            } else if width >= 48 && !when.is_empty() {
                format!(" · {}", when.get(..10).unwrap_or(when))
            } else {
                String::new()
            };
            let prefix = clip(&format!("◇ {sha} "), width as usize);
            let available = (width as usize).saturating_sub(
                Line::from(prefix.as_str()).width() + Line::from(suffix.as_str()).width(),
            );
            doc.headings
                .push((doc.lines.len(), format!("Commit · {sha} · {title}")));
            doc.lines.push(Line::from(vec![
                text(prefix, theme().purple),
                text(clip(title, available), theme().fg),
                text(suffix, theme().muted),
            ]));
        } else {
            if doc.lines.last().is_some_and(|line| line.width() > 0) {
                doc.lines.push(Line::from(""));
            }
            let label = if kind == 1 {
                match s(event, "state") {
                    "APPROVED" => "approved",
                    "CHANGES_REQUESTED" => "requested changes",
                    "DISMISSED" => "review dismissed",
                    _ => "reviewed",
                }
            } else {
                ""
            };
            doc.headings.push((
                doc.lines.len(),
                format!(
                    "{} · @{} · {}",
                    if kind == 1 { "Review" } else { "Comment" },
                    s(event, "author"),
                    date(when)
                ),
            ));
            let body = if s(event, "body").is_empty() && kind == 1 {
                label
            } else {
                s(event, "body")
            };
            doc.lines
                .extend(comment_card(s(event, "author"), when, label, body, width));
            doc.lines.push(Line::from(""));
        }
    }
}
fn build(a: &App, width: u16) -> markdown::Document {
    let appearance = a.reader.appearance;
    let mut doc = markdown::Document::default();
    if a.panel == Panel::Threads {
        return thread_document(a, width);
    }
    if a.panel == Panel::Ai {
        if a.ai.display_truncated {
            doc.lines.push(Line::from(text(
                "Earlier messages are in your Claude session.",
                theme().muted,
            )));
        }
        if a.messages.is_empty() {
            if !a.ai.summary.is_empty() {
                message(
                    &mut doc.lines,
                    "assistant",
                    &a.ai.summary,
                    appearance,
                    width,
                );
            } else {
                doc.lines.push(Line::styled(
                    "A second pair of eyes.",
                    base().add_modifier(Modifier::BOLD),
                ));
                doc.lines.push(Line::from(""));
                doc.lines.extend(markdown::document_at(if !a.ai.enabled && !a.is_demo() { "Connect Claude Code to talk through this PR. Your chat stays with the review." } else { "Talk through the whole PR. Explore its purpose, challenge a design, or ask for a diagram.\n\nWhat problem does this solve?\nWhere would you start reviewing?\nMap the flow as a diagram." }, true, width).lines);
            }
        }
        for m in &a.messages {
            if s(m, "role") == "user" {
                let offset = doc.lines.len()
                    + usize::from(!doc.lines.is_empty() && appearance != Appearance::Ledger);
                doc.headings
                    .push((offset, clip(&s(m, "content").replace('\n', " "), 100)));
            }
            message(
                &mut doc.lines,
                s(m, "role"),
                s(m, "content"),
                appearance,
                width,
            );
        }
        if !a.ai.streaming.is_empty() {
            message(
                &mut doc.lines,
                "assistant",
                &a.ai.streaming,
                appearance,
                width,
            );
        }
        if !a.ai.error.is_empty() {
            doc.lines.push(Line::from(""));
            doc.lines.push(Line::from(text(&a.ai.error, theme().red)));
            doc.lines.push(Line::from(text("R Retry", theme().muted)));
        }
    } else {
        let title = clean(s(&a.pr, "title"));
        doc.lines
            .push(Line::styled(title, base().add_modifier(Modifier::BOLD)));
        doc.lines.push(Line::from(text(
            format!(
                "@{}{} · {}",
                s(&a.pr, "author"),
                if s(&a.pr, "createdAt").is_empty() {
                    String::new()
                } else {
                    format!(" · {}", date(s(&a.pr, "createdAt")))
                },
                s(&a.pr, "state")
            ),
            theme().muted,
        )));
        if appearance != Appearance::Ledger {
            doc.lines.push(Line::from(""));
        }
        let mut body =
            markdown::document_at(s(&a.pr, "body"), appearance != Appearance::Ledger, width);
        if body.lines.is_empty() {
            body.lines
                .push(Line::from(text("No description.", theme().muted)));
        }
        let offset = doc.lines.len();
        doc.headings = body
            .headings
            .iter()
            .map(|(i, t)| (i + offset, t.clone()))
            .collect();
        for (i, _) in &body.headings {
            if appearance == Appearance::Studio {
                body.lines[*i] = body.lines[*i].clone().style(base().bg(theme().hunk_bg));
            } else if appearance == Appearance::Ledger {
                body.lines[*i].spans.insert(0, text("§ ", theme().muted));
            }
        }
        doc.lines.extend(body.lines);
        for warning in array(&a.pr, "warnings") {
            doc.lines.push(Line::from(text(
                warning.as_str().unwrap_or(""),
                Color::Yellow,
            )));
        }
        append_timeline(&mut doc, a, width);
    }
    doc
}
fn body(f: &mut Frame, a: &mut App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let appearance = a.reader.appearance;
    let margin = if appearance == Appearance::Ledger || area.width < 24 {
        0
    } else {
        1
    };
    let mut column = Rect::new(
        area.x + margin,
        area.y,
        area.width.saturating_sub(margin * 2 + 1),
        area.height,
    );
    if appearance == Appearance::Paper && column.width > READING_WIDTH {
        column.x += (column.width - READING_WIDTH) / 2;
        column.width = READING_WIDTH;
    }
    let key = content_key(a);
    if key != a.reader.cache.key || column.width != a.reader.cache.width {
        let logical = a
            .reader
            .cache
            .offsets
            .partition_point(|offset| *offset <= a.panel_scroll)
            .saturating_sub(1);
        let resizing = key == a.reader.cache.key && column.width != a.reader.cache.width;
        let mut document = build(a, column.width);
        if appearance == Appearance::Studio {
            for line in &mut document.lines {
                if line.style.bg.is_some() && line.width() < column.width as usize {
                    line.spans.push(Span::styled(
                        " ".repeat(column.width as usize - line.width()),
                        line.style,
                    ));
                }
            }
        }
        let mut offsets = Vec::with_capacity(document.lines.len());
        let mut total = 0;
        for line in &document.lines {
            offsets.push(total);
            total += height(vec![line.clone()], column.width);
        }
        a.reader.sections = document
            .headings
            .into_iter()
            .map(|(i, t)| (offsets.get(i).copied().unwrap_or(0), t))
            .collect();
        if resizing {
            a.panel_scroll = offsets.get(logical).copied().unwrap_or(0);
        }
        a.reader.cache = Cache {
            key,
            width: column.width,
            lines: document.lines,
            offsets,
            height: total,
        };
    }
    a.reader.max_scroll = a.reader.cache.height.saturating_sub(column.height as usize);
    if let Some(destination) = a.reader.destination.take() {
        a.panel_scroll = match destination {
            Destination::Top => 0,
            Destination::Comments => a
                .reader
                .sections
                .iter()
                .find(|(_, title)| title.starts_with("Comment ·") || title.starts_with("Review ·"))
                .map_or(0, |(row, _)| *row),
            Destination::Commits => a
                .reader
                .sections
                .iter()
                .find(|(_, title)| title.starts_with("Commit ·"))
                .map_or(0, |(row, _)| *row),
        };
    }
    if a.panel == Panel::Ai && a.ai.follow {
        a.panel_scroll = a.reader.max_scroll;
    }
    a.panel_scroll = a.panel_scroll.min(a.reader.max_scroll);
    f.render_widget(
        paragraph(a.reader.cache.lines.clone())
            .scroll((a.panel_scroll.min(u16::MAX as usize) as u16, 0)),
        column,
    );
    let token = format!(
        "{:?}:{}:{}:{}",
        a.panel,
        s(&a.pr, "url"),
        s(&a.pr, "head"),
        a.ai.session
    );
    a.reader
        .selection
        .capture(f.buffer_mut(), column, a.panel_scroll, token);
    a.reader
        .selection
        .highlight(f.buffer_mut(), theme().selected);
    if a.reader.max_scroll > 0 {
        let y = area.y
            + ((area.height.saturating_sub(1) as usize * a.panel_scroll) / a.reader.max_scroll)
                as u16;
        row(
            f,
            Rect::new(area.right().saturating_sub(1), y, 1, 1),
            Line::from(text("▐", theme().muted)),
        );
    }
}
fn focus(a: &App, width: u16) -> String {
    let label = if a.reader.focused {
        let range = a
            .selection()
            .map(|v| format!(":{}–{}", v["start"], v["end"]))
            .unwrap_or_default();
        let available = (width as usize).saturating_sub(9);
        format!(
            "↳ {} · F PR",
            clip(&format!("{}{range}", s(a.current(), "filename")), available)
        )
    } else {
        format!(
            "PR #{} · {} files · F Focus",
            a.pr["number"].as_u64().unwrap_or(0),
            a.files().len()
        )
    };
    clip(&label, width as usize)
}
fn composer_height(a: &App, area: Rect) -> u16 {
    if a.modal != Some(Modal::Editor(Input::Chat)) {
        return if area.height >= 10 { 3 } else { 1 };
    }
    let count = paragraph(clean(&a.editor)).line_count(area.width.saturating_sub(2).max(1));
    let choices = if a.completion.open {
        a.completion.items.len().clamp(1, 3)
    } else {
        0
    };
    (count.clamp(2, 6) + 3 + choices).min(area.height.saturating_sub(4).max(1) as usize) as u16
}
fn composer(f: &mut Frame, a: &App, area: Rect) {
    let editing = a.modal == Some(Modal::Editor(Input::Chat));
    let mut outer = Block::default()
        .borders(Borders::TOP)
        .border_style(theme().inactive);
    if editing {
        outer = outer.title(" Ask Claude ").border_style(theme().active);
    }
    let inner = outer.inner(area);
    f.render_widget(outer, area);
    if !editing {
        let label = if a.ai.busy {
            "X Stop"
        } else if !a.ai.enabled && !a.is_demo() {
            "g a Connect Claude"
        } else {
            "i Ask about this PR…"
        };
        row(f, inner, Line::from(text(label, theme().muted)));
        return;
    }
    let choices = if a.completion.open {
        a.completion.items.len().clamp(1, 3) as u16
    } else {
        0
    };
    let parts = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(choices),
        Constraint::Length(1),
    ])
    .split(inner);
    let mut display = clean(&a.editor);
    let cursor = a.editor_cursor.min(display.len());
    if display.is_char_boundary(cursor) {
        display.insert(cursor, '▏');
    } else {
        display.push('▏');
    }
    let before = a.editor.get(..a.editor_cursor).unwrap_or(&a.editor);
    let line = paragraph(clean(before))
        .line_count(parts[0].width)
        .saturating_sub(1);
    f.render_widget(
        paragraph(display).scroll((
            line.saturating_sub(parts[0].height.saturating_sub(1) as usize)
                .min(u16::MAX as usize) as u16,
            0,
        )),
        parts[0],
    );
    if choices > 0 {
        let items: Vec<ListItem> = a
            .completion
            .items
            .iter()
            .map(|v| ListItem::new(clean(&format!("{} {}", s(v, "text"), s(v, "label")))))
            .collect();
        f.render_stateful_widget(
            List::new(items)
                .highlight_style(theme().selected)
                .highlight_symbol("› "),
            parts[1],
            &mut ListState::default().with_selected(Some(a.completion.selected)),
        );
    }
    row(
        f,
        parts[2],
        Line::from(text(
            if inner.width >= 40 {
                "Enter Send · Ctrl-n Newline · Esc Close"
            } else {
                "Enter Send · Esc Close"
            },
            theme().muted,
        )),
    );
}
pub fn render(f: &mut Frame, a: &mut App, area: Rect) {
    let ai = a.panel == Panel::Ai;
    let title = if ai {
        "AI companion"
    } else if a.panel == Panel::Threads {
        "Review threads"
    } else {
        "Pull request"
    };
    let outer = header(title.into(), a.pane == Pane::Panel);
    let inner = outer.inner(area);
    f.render_widget(outer, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let footer = if ai { composer_height(a, inner) } else { 1 };
    let header_height = if inner.height >= 8 {
        if ai {
            if !a.context.is_empty() { 3 } else { 2 }
        } else {
            1
        }
    } else {
        1
    };
    let shelf = u16::from(ai && !a.ai.artifacts.is_empty() && inner.height >= 12);
    let parts = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(1),
        Constraint::Length(shelf),
        Constraint::Length(footer),
    ])
    .split(inner);
    if ai {
        let phase = if a.is_demo() { "Sample" } else { a.ai.label() };
        let glyph = if a.ai.busy {
            ["⠋", "⠙", "⠹", "⠸"][(a.started.elapsed().as_millis() / 120) as usize % 4]
        } else if !a.ai.error.is_empty() {
            "!"
        } else {
            "●"
        };
        let supplied = a.ai.evidence["providedPatchPaths"]
            .as_array()
            .map_or(0, Vec::len);
        let read = a.ai.evidence["readSources"].as_array().map_or(0, Vec::len);
        let evidence = if supplied + read > 0 {
            format!(" · {supplied} supplied / {read} read")
        } else {
            String::new()
        };
        let name = format!("{glyph} Claude · {phase}{evidence}");
        row(
            f,
            Rect::new(parts[0].x, parts[0].y, parts[0].width, 1),
            Line::from(text(
                name,
                if a.ai.error.is_empty() {
                    theme().purple
                } else {
                    theme().red
                },
            )),
        );
        if header_height > 1 {
            row(
                f,
                Rect::new(parts[0].x, parts[0].y + 1, parts[0].width, 1),
                Line::from(text(focus(a, parts[0].width), theme().muted)),
            );
        }
        if header_height > 2 {
            row(
                f,
                Rect::new(parts[0].x, parts[0].y + 2, parts[0].width, 1),
                Line::from(text(
                    format!("{} for next turn · g c Inspect", a.context.len()),
                    theme().accent,
                )),
            );
        }
        body(f, a, parts[1]);
        if shelf > 0 {
            let newest = &a.ai.artifacts[0];
            row(
                f,
                parts[2],
                Line::from(vec![
                    text(format!("◇ {} · g o ", a.ai.artifacts.len()), theme().accent),
                    text(
                        clip(
                            s(newest, "title"),
                            parts[2].width.saturating_sub(14) as usize,
                        ),
                        theme().muted,
                    ),
                ]),
            );
        }
        composer(f, a, parts[3]);
    } else {
        let navigation = if a.panel == Panel::Threads {
            format!(
                "Thread {}/{} · j/k select · i Reply",
                if array(&a.pr, "threads").is_empty() {
                    0
                } else {
                    a.panel_cursor + 1
                },
                array(&a.pr, "threads").len()
            )
        } else {
            "D Description · C Comments · g m Commits".into()
        };
        row(
            f,
            Rect::new(parts[0].x, parts[0].y, parts[0].width, 1),
            Line::from(text(navigation, theme().muted)),
        );
        body(f, a, parts[1]);
        let percent = if a.reader.max_scroll == 0 {
            100
        } else {
            a.panel_scroll * 100 / a.reader.max_scroll
        };
        let label = if a.reader.sections.is_empty() {
            ""
        } else {
            if a.panel == Panel::Threads {
                "Ctrl-d/u Scroll · R Resolve/reopen"
            } else {
                "i Comment · o Timeline · [[ / ]] Jump"
            }
        };
        let cols = Layout::horizontal([Constraint::Min(0), Constraint::Length(5)]).split(parts[3]);
        row(f, cols[0], Line::from(text(label, theme().muted)));
        row(
            f,
            cols[1],
            Line::from(text(format!("{percent:>3}%"), theme().muted)),
        );
    }
    let bottom = if ai && !a.ai.follow && a.panel_scroll < a.reader.max_scroll {
        " G Latest ".to_string()
    } else {
        if ai {
            " o Conversation · g o Artifacts ".into()
        } else {
            format!(" {} · g v Style ", a.reader.appearance.name())
        }
    };
    if area.width > bottom.len() as u16 + 2 {
        row(
            f,
            Rect::new(
                area.right() - bottom.len() as u16 - 2,
                area.bottom().saturating_sub(1),
                bottom.len() as u16,
                1,
            ),
            Line::from(text(bottom, theme().muted)),
        );
    }
}
pub fn chooser(f: &mut Frame, a: &App, outline: bool) {
    let bounds = f.area();
    let width = bounds
        .width
        .saturating_sub(4)
        .min(if outline { 64 } else { 52 });
    let entries = if outline {
        a.reader.sections.len().max(1)
    } else {
        3
    };
    let height = (entries + 4).min(bounds.height.saturating_sub(2) as usize) as u16;
    let area = Rect::new(
        bounds.x + (bounds.width - width) / 2,
        bounds.y + (bounds.height - height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, area);
    let block = header(
        if outline {
            if a.panel == Panel::Ai {
                "Conversation"
            } else {
                "PR timeline"
            }
        } else {
            "Companion style"
        }
        .into(),
        true,
    )
    .style(base().bg(theme().bg));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let parts = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let items: Vec<ListItem> = if outline {
        if a.reader.sections.is_empty() {
            vec![ListItem::new(if a.panel == Panel::Ai {
                "Your questions will appear here."
            } else {
                "No timeline entries."
            })]
        } else {
            a.reader
                .sections
                .iter()
                .map(|(_, title)| ListItem::new(title.clone()))
                .collect()
        }
    } else {
        Appearance::ALL
            .iter()
            .map(|v| ListItem::new(format!("{:<8} {}", v.name(), v.detail())))
            .collect()
    };
    let selected = if outline {
        a.reader.outline_cursor
    } else {
        a.reader.choice
    };
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(theme().selected)
            .highlight_symbol("› "),
        parts[0],
        &mut ListState::default().with_selected(Some(selected)),
    );
    row(
        f,
        parts[1],
        Line::from(text(
            if outline {
                "Enter Jump · Esc Close"
            } else {
                "Enter Apply · Esc Cancel"
            },
            theme().muted,
        )),
    );
}

/// PR-local outputs, deliberately separate from the conversation scroll position.
pub fn context(f: &mut Frame, a: &App) {
    let bounds = f.area();
    let width = bounds.width.saturating_sub(4).min(86);
    let height = bounds.height.saturating_sub(2).min(18);
    let area = Rect::new(
        bounds.x + (bounds.width - width) / 2,
        bounds.y + (bounds.height - height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, area);
    let block = header(
        format!("For {}’s next answer · {}", a.ai.name(), a.context.len()),
        true,
    )
    .style(base().bg(theme().bg));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if a.context.is_empty() {
        f.render_widget(paragraph("Select code with v, then a to attach or t to leave a note.\nRepeat across files; i asks your companion about the selection.\n\nEsc Close"),inner);
        return;
    }
    let parts = Layout::vertical([
        Constraint::Length(a.context.len().clamp(2, 5) as u16),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(inner);
    let items: Vec<ListItem> = a
        .context
        .iter()
        .map(|item| {
            ListItem::new(format!(
                "{} · {}{}",
                s(item, "path"),
                s(item, "label"),
                if s(item, "note").is_empty() {
                    ""
                } else {
                    " · note"
                }
            ))
        })
        .collect();
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(theme().selected)
            .highlight_symbol("› "),
        parts[0],
        &mut ListState::default().with_selected(Some(a.context_cursor)),
    );
    if let Some(item) = a.context.get(a.context_cursor) {
        let note = s(item, "note");
        let mut lines = vec![
            Line::from(""),
            Line::from(text(
                if note.is_empty() {
                    "t Add a note"
                } else {
                    note
                },
                theme().fg,
            )),
            Line::from(text("Code preview", theme().muted)),
        ];
        let content = s(item, "content");
        lines.extend(
            content
                .lines()
                .take(5)
                .map(|line| Line::from(text(line, theme().fg))),
        );
        if content.lines().count() > 5 {
            lines.push(Line::from(text("… more code attached", theme().muted)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(text(
            "Clears after a successful answer · Private to AI",
            theme().muted,
        )));
        f.render_widget(paragraph(lines), parts[1]);
    }
    row(
        f,
        parts[2],
        Line::from(text(
            "Enter Note · d Remove · i Ask · Esc Close",
            theme().muted,
        )),
    );
}

pub fn artifacts(f: &mut Frame, a: &App) {
    let bounds = f.area();
    let width = bounds.width.saturating_sub(4).min(82);
    let height = bounds
        .height
        .saturating_sub(2)
        .min((a.ai.artifacts.len() + 9).clamp(9, 22) as u16);
    let area = Rect::new(
        bounds.x + (bounds.width - width) / 2,
        bounds.y + (bounds.height - height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, area);
    let block =
        header(format!("Artifacts · {}", a.ai.artifacts.len()), true).style(base().bg(theme().bg));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if a.ai.artifacts.is_empty() {
        f.render_widget(paragraph("Ask your companion for a diagram, HTML explainer, or notes.\nSaved outputs stay with this PR.\n\nEsc Close"), inner);
        return;
    }
    let parts = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(5),
        Constraint::Length(1),
    ])
    .split(inner);
    let items: Vec<ListItem> =
        a.ai.artifacts
            .iter()
            .map(|item| {
                let kind = match s(item, "kind") {
                    "svg" => "Diagram",
                    "html" => "Page",
                    _ => "Notes",
                };
                ListItem::new(Line::from(vec![
                    text(format!("{kind:<8} "), theme().accent),
                    text(s(item, "title"), theme().fg),
                ]))
            })
            .collect();
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(theme().selected)
            .highlight_symbol("› "),
        parts[0],
        &mut ListState::default().with_selected(Some(a.reader.artifact_cursor)),
    );
    if let Some(item) = a.ai.artifacts.get(a.reader.artifact_cursor) {
        let revision = if item["head"] == a.pr["head"] && item["base"] == a.pr["base"] {
            "Current revision"
        } else {
            "Earlier revision"
        };
        let lines = vec![
            Line::from(""),
            Line::from(text(s(item, "summary"), theme().fg)),
            Line::from(text(
                format!(
                    "{revision} · {} · {}",
                    s(item, "head").chars().take(7).collect::<String>(),
                    s(item, "created").chars().take(10).collect::<String>()
                ),
                theme().muted,
            )),
        ];
        f.render_widget(paragraph(lines), parts[1]);
    }
    row(
        f,
        parts[2],
        Line::from(text("Enter Open in browser · Esc Close", theme().muted)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;
    fn sample() -> App {
        let mut a = App::new(
            json!({"demo":true,"title":"Reader test","author":"reviewer","state":"OPEN","files":[],"threads":[],"conversation":[]}),
        );
        crate::preview::seed(&mut a, "ai");
        a
    }
    fn draw(a: &mut App, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| crate::ui::render(f, a)).unwrap();
        let b = terminal.backend().buffer();
        (0..h)
            .map(|y| (0..w).map(|x| b[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn key(a: &mut App, c: char) {
        a.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    #[test]
    fn reading_surfaces_and_docked_composer_survive_small_and_wide_terminals() {
        for appearance in Appearance::ALL {
            for (w, h) in [
                (1, 1),
                (10, 5),
                (24, 10),
                (60, 20),
                (100, 30),
                (150, 45),
                (240, 70),
            ] {
                let mut a = sample();
                a.reader.appearance = appearance;
                for panel in [Panel::Ai, Panel::Overview] {
                    a.panel = panel;
                    draw(&mut a, w, h);
                }
                a.open_editor(Input::Chat);
                a.editor =
                    "Long Unicode draft: 東京 👩‍💻 é with some long words and\na second line".into();
                a.editor_cursor = a.editor.len();
                let screen = draw(&mut a, w, h);
                if w >= 100 && h >= 30 {
                    assert!(screen.contains("Ask Claude"));
                    assert!(screen.contains("Enter Send"));
                }
            }
        }
    }
    #[test]
    fn scrolling_breaks_follow_and_latest_restores_it_for_streaming() {
        let mut a = sample();
        a.ai.follow = true;
        a.ai.streaming = "Streaming paragraph. ".repeat(120);
        draw(&mut a, 110, 25);
        assert!(a.reader.max_scroll > 0);
        assert_eq!(a.panel_scroll, a.reader.max_scroll);
        key(&mut a, 'k');
        let paused = a.panel_scroll;
        a.ai.streaming.push_str(&"More answer. ".repeat(30));
        let screen = draw(&mut a, 110, 25);
        assert_eq!(a.panel_scroll, paused);
        assert!(screen.contains("G Latest"));
        assert!(!a.ai.follow);
        key(&mut a, 'G');
        draw(&mut a, 110, 25);
        assert!(a.ai.follow);
        assert_eq!(a.panel_scroll, a.reader.max_scroll);
        // Count is Ratatui's real word-wrapping, not ceil(string width / columns).
        assert_eq!(
            a.reader.cache.height,
            paragraph(a.reader.cache.lines.clone()).line_count(a.reader.cache.width)
        );
        key(&mut a, 'g');
        key(&mut a, 'g');
        draw(&mut a, 110, 25);
        assert_eq!(a.panel_scroll, 0);
    }
    #[test]
    fn outline_targets_real_wrapped_headings_and_resize_keeps_logical_position() {
        let mut a = sample();
        a.panel = Panel::Overview;
        draw(&mut a, 110, 26);
        assert!(a.reader.sections.len() >= 5);
        key(&mut a, ']');
        key(&mut a, ']');
        draw(&mut a, 110, 26);
        assert_eq!(a.panel_scroll, a.reader.sections[0].0);
        key(&mut a, 'o');
        assert_eq!(a.modal, Some(Modal::Outline));
        key(&mut a, 'j');
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        draw(&mut a, 110, 26);
        assert_eq!(a.panel_scroll, a.reader.sections[1].0);
        let logical = a
            .reader
            .cache
            .offsets
            .partition_point(|o| *o <= a.panel_scroll)
            - 1;
        a.panes.right_width = 70;
        draw(&mut a, 180, 26);
        assert_eq!(
            a.panel_scroll,
            a.reader.cache.offsets[logical].min(a.reader.max_scroll)
        );
        key(&mut a, '[');
        key(&mut a, '[');
        assert!(a.panel_scroll < a.reader.sections[1].0);
    }
    #[test]
    fn conversation_index_scope_and_artifacts_use_real_navigation_without_losing_position() {
        let mut a = sample();
        a.ai.follow = true;
        draw(&mut a, 150, 25);
        assert_eq!(a.reader.sections.len(), 2);
        key(&mut a, 'o');
        assert_eq!(a.modal, Some(Modal::Outline));
        key(&mut a, 'k');
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        draw(&mut a, 150, 25);
        assert!(!a.ai.follow);
        assert_eq!(a.panel_scroll, a.reader.sections[0].0);
        key(&mut a, ']');
        key(&mut a, ']');
        assert_eq!(a.panel_scroll, a.reader.sections[1].0);
        key(&mut a, 'F');
        assert!(a.reader.focused);
        key(&mut a, 'F');
        assert!(!a.reader.focused);
        let scroll = a.panel_scroll;
        a.pr["demo"] = json!(false);
        a.ai.artifacts = vec![
            json!({"id":"first","kind":"svg","title":"Review flow","summary":"The input and output","head":"older"}),
            json!({"id":"second","kind":"html","title":"Architecture note","summary":"Background","head":"older"}),
        ];
        key(&mut a, 'g');
        key(&mut a, 'o');
        assert_eq!(a.modal, Some(Modal::Artifacts));
        let rendered = draw(&mut a, 150, 25);
        assert!(rendered.contains("Review flow"));
        assert!(rendered.contains("Earlier revision"));
        key(&mut a, 'j');
        let request = a
            .key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();
        assert_eq!(request["operation"], "ai_open_artifact");
        assert_eq!(request["id"], "second");
        a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(a.panel_scroll, scroll.min(a.reader.max_scroll));
        for (w, h) in [(1, 1), (24, 10), (60, 20)] {
            a.modal = Some(Modal::Artifacts);
            draw(&mut a, w, h);
        }
    }
    #[test]
    fn style_picker_cancel_is_nonmutating_and_apply_keeps_chat() {
        let mut a = sample();
        let messages = a.messages.clone();
        a.ai.follow = true;
        key(&mut a, 'g');
        key(&mut a, 'v');
        key(&mut a, 'j');
        a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(a.reader.appearance, Appearance::Paper);
        key(&mut a, 'g');
        key(&mut a, 'v');
        key(&mut a, 'j');
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(a.reader.appearance, Appearance::Studio);
        assert_eq!(a.messages, messages);
        assert!(a.ai.follow);
        a.panes.panel = false;
        a.pane = Pane::Tree;
        a.open_editor(Input::Chat);
        assert!(a.panes.panel);
        assert_eq!(a.pane, Pane::Panel);
    }
    fn timeline_fixture() -> App {
        let mut a = App::new(
            json!({"demo":true,"url":"sample","title":"Make keyboard navigation predictable","author":"sarah","state":"open","createdAt":"2026-10-13T08:00:00Z","body":"## Purpose\nKeep the active result aligned with the filtered command list.\n\n- Preserve keyboard focus.\n- Reset when the query changes.","files":[],"threads":[{"path":"src/navigation.ts","line":42,"resolved":false,"comments":[{"author":"alex","createdAt":"2026-10-13T11:00:00Z","body":"Can we cover **wrapping** at the boundary?\n\n```ts\nconst next = (index + 1) % results.length;\n```"},{"author":"sarah","createdAt":"2026-10-13T12:00:00Z","body":"Added the empty-list guard and a regression test."}]}],"conversation":[{"author":"alex","createdAt":"2026-10-13T11:00:00Z","body":"Could you clarify what happens when the result list is empty?"},{"author":"sarah","createdAt":"2026-10-13T09:00:00Z","body":"Ready for review. The navigation hook is the place to start."}],"reviews":[{"author":"pat","createdAt":"2026-10-13T13:00:00Z","state":"APPROVED","body":"Looks good. The empty-list test now covers the edge case."}],"commits":[{"sha":"1234567890","author":"sarah","createdAt":"2026-10-13T10:00:00Z","message":"Guard empty result lists"},{"sha":"abcdef1234","author":"sarah","createdAt":"2026-10-13T12:00:00Z","message":"Add boundary regression tests"}]}),
        );
        a.panel = Panel::Overview;
        a.pane = Pane::Panel;
        a.panes.panel = true;
        a.panes.zoom = true;
        a
    }
    #[test]
    fn timeline_contains_description_comments_reviews_and_compact_commits_in_time_order() {
        let a = timeline_fixture();
        let document = build(&a, 76);
        let text = document
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        for value in [
            "Purpose",
            "Ready for review",
            "Guard empty result lists",
            "Could you clarify",
            "Add boundary regression tests",
            "Looks good",
        ] {
            assert!(text.contains(value), "Missing {value}");
        }
        assert!(
            text.find("Ready for review").unwrap() < text.find("Guard empty result lists").unwrap()
        );
        assert!(
            text.find("Guard empty result lists").unwrap()
                < text.find("Could you clarify").unwrap()
        );
        assert!(
            text.find("Could you clarify").unwrap()
                < text.find("Add boundary regression tests").unwrap()
        );
        assert!(
            text.find("Add boundary regression tests").unwrap() < text.find("Looks good").unwrap()
        );
    }
    #[test]
    fn diagrams_refresh_cached_timeline_and_thread_cards_without_exposing_link_urls() {
        let mut a = timeline_fixture();
        let source = "```mermaid\nflowchart TD\n  Open[Open PR] --> Read[Read changes]\n```\n\n[Build passed](https://example.org/private/build)";
        for panel in [Panel::Overview, Panel::Threads] {
            a.panel = panel;
            if panel == Panel::Overview {
                a.pr["body"] = json!(source);
                a.pr["conversation"][0]["body"] = json!(source);
            } else {
                a.pr["threads"][0]["comments"][0]["body"] = json!(source);
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                draw(&mut a, 100, 42);
                let document = build(&a, 76);
                let text = document
                    .lines
                    .iter()
                    .map(Line::to_string)
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(!text.contains("https://example.org"));
                if text.contains("Diagram") && !text.contains("flowchart TD") {
                    assert!(text.contains("Open PR") && text.contains("Build passed"));
                    for line in document
                        .lines
                        .iter()
                        .filter(|line| line.to_string().starts_with(['╭', '│', '╰']))
                    {
                        assert!(line.width() <= 76, "Diagram or card overflowed the reader");
                    }
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "Diagram never replaced source"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
    #[test]
    fn timeline_cards_and_commits_fit_narrow_columns_and_refresh_when_comments_change() {
        let mut a = timeline_fixture();
        for width in [8, 20, 42, 76] {
            let document = build(&a, width);
            let commit = document
                .lines
                .iter()
                .find(|line| line.to_string().starts_with("◇"))
                .unwrap();
            assert!(commit.width() <= width as usize);
            for line in &document.lines {
                if line.to_string().starts_with(['╭', '│', '╰']) {
                    assert_eq!(
                        line.width(),
                        width as usize,
                        "Card border alignment at {width}: {line}"
                    );
                }
            }
        }
        let old = content_key(&a);
        a.pr["conversation"][0]["body"] = json!("A changed historical comment");
        assert_ne!(old, content_key(&a));
        let old = content_key(&a);
        a.syntax_palette = a.syntax_palette.next();
        assert_ne!(
            old,
            content_key(&a),
            "Syntax palette must refresh cached code blocks"
        );
        a.panel = Panel::Threads;
        let text = build(&a, 42)
            .lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("src/navigation.ts:42"));
        assert_eq!(
            text.lines().filter(|line| line.starts_with("╭ @")).count(),
            2
        );
        // Two comment cards and the first comment's fenced TypeScript block.
        assert_eq!(text.matches('╭').count(), 3);
        assert_eq!(text.matches('╰').count(), 3);
        assert!(text.contains("regression test"));
    }
    #[test]
    fn timeline_shortcuts_jump_in_one_document_and_comment_editor_keeps_review_state() {
        let mut a = timeline_fixture();
        draw(&mut a, 100, 22);
        key(&mut a, 'C');
        draw(&mut a, 100, 22);
        assert_eq!(a.panel, Panel::Overview);
        assert!(a.panel_scroll > 0);
        let discussion = a.panel_scroll;
        key(&mut a, 'g');
        key(&mut a, 'm');
        draw(&mut a, 100, 22);
        assert_eq!(a.panel, Panel::Overview);
        assert!(a.panel_scroll >= discussion);
        key(&mut a, 'D');
        draw(&mut a, 100, 22);
        assert_eq!(a.panel_scroll, 0);
        key(&mut a, 'i');
        assert_eq!(a.modal, Some(Modal::Editor(Input::Conversation)));
        assert!(!a.busy);
    }
    #[test]
    #[ignore = "manual timeline visual artifact"]
    fn export_timeline_preview() {
        let mut screens = vec![];
        for panel in [Panel::Overview, Panel::Threads] {
            for (w, h) in [(100, 42), (46, 35)] {
                let mut a = timeline_fixture();
                a.panel = panel;
                if std::env::var_os("LAZYREVIEW_DIAGRAM_PREVIEW").is_some() {
                    let body = "## Flow\n[Build passed](https://example.org/build)\n\n```mermaid\nflowchart TD\n  Open[Open PR] --> Read[Read changes]\n  Read --> Ask[Ask Claude]\n```";
                    a.pr["body"] = json!(body);
                    a.pr["threads"][0]["comments"][0]["body"] = json!(body);
                }
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|f| crate::ui::render(f, &mut a)).unwrap();
                if std::env::var_os("LAZYREVIEW_DIAGRAM_PREVIEW").is_some() {
                    for _ in 0..100 {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        terminal.draw(|f| crate::ui::render(f, &mut a)).unwrap();
                        if terminal
                            .backend()
                            .buffer()
                            .content()
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>()
                            .contains("Diagram")
                        {
                            break;
                        }
                    }
                }
                screens.push(json!({"panel":format!("{panel:?}"),"width":w,"height":h,"html":crate::preview::buffer_html(terminal.backend().buffer())}));
            }
        }
        let file = std::env::var("LAZYREVIEW_TIMELINE_PREVIEW")
            .unwrap_or("/private/tmp/rw-timeline-preview.json".into());
        std::fs::write(file, serde_json::to_vec_pretty(&screens).unwrap()).unwrap();
    }
}
