use crate::app::{App, Input, Modal, Pane, Panel, Row, array, s};
use crate::theme::current as theme;
use ratatui::{
    Frame,
    layout::{Constraint as C, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use serde_json::Value as V;
pub fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}
fn txt(v: &V, key: &str) -> String {
    clean(s(v, key))
}
fn block(title: impl Into<String>, active: bool) -> Block<'static> {
    Block::default()
        .title(format!(" {} ", title.into()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if active {
            theme().active
        } else {
            theme().inactive
        })
}
fn accent(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}
fn para(text: impl Into<Text<'static>>) -> Paragraph<'static> {
    Paragraph::new(text).style(Style::default().fg(theme().fg))
}
fn shorten(value: &str, width: usize) -> String {
    let value = clean(value);
    if Line::from(value.clone()).width() <= width {
        return value;
    }
    if width == 0 {
        return String::new();
    }
    let mut text = String::new();
    let mut used = 0;
    for c in value.chars() {
        let w = Span::raw(c.to_string()).width();
        if used + w > width - 1 {
            break;
        }
        text.push(c);
        used += w;
    }
    text.push('…');
    text
}

pub fn render(f: &mut Frame, a: &mut App) {
    crate::markdown::set_palette(a.syntax_palette);
    let area = f.area();
    a.review_list.hits.clear();
    a.reader.selection.visible = false;
    f.render_widget(
        Block::default().style(Style::default().bg(theme().bg).fg(theme().fg)),
        area,
    );
    let layout =
        Layout::vertical([C::Length(2), C::Min(4), C::Length(1), C::Length(1)]).split(area);
    let header = Layout::vertical([C::Length(1), C::Length(1)]).split(layout[0]);
    f.render_widget(
        para(Line::from(vec![
            Span::raw(format!(" {}  ", txt(&a.pr, "title"))),
            accent(
                format!("{} → {}", txt(&a.pr, "headRef"), txt(&a.pr, "baseRef")),
                theme().muted,
            ),
        ])),
        header[0],
    );
    review_toolbar(f, a, header[1]);
    let panes = a.panes.calculate(layout[1], a.pane.index());
    let annotations = a.annotations();
    if panes[0].width > 0 {
        tree(f, a, panes[0], &annotations);
    }
    if panes[1].width > 0 {
        diff(f, a, panes[1], &annotations);
    }
    if panes[2].width > 0 {
        panel(f, a, panes[2]);
    }
    let status_area = pane_controls(f, a, layout[2]);
    let status = if a.ai.enabled && !a.is_demo() && a.status.is_empty() {
        format!(" {} · {}", a.ai.name(), a.ai.label())
    } else if a.busy {
        a.spinner = (a.spinner + 1) % 4;
        format!(" {} {}", ["⠋", "⠙", "⠹", "⠸"][a.spinner], a.status)
    } else {
        format!(" {}", a.status)
    };
    f.render_widget(
        para(Line::from(accent(
            shorten(&status, status_area.width as usize),
            if a.busy { Color::Yellow } else { theme().muted },
        ))),
        status_area,
    );
    let identity = format!(
        " {}/{} #{}{}",
        txt(&a.pr, "owner"),
        txt(&a.pr, "repo"),
        a.pr["number"],
        if a.is_demo() { " · sample" } else { "" }
    );
    let hints = if !a.is_demo() {
        format!(
            "g a {} · {}  ? Shortcuts  \\ Panes ",
            a.ai.name(),
            a.ai.label()
        )
    } else {
        "g a Claude  D Description  ? Shortcuts  \\ Panes  r Your review ".into()
    };
    let footer = Layout::horizontal([
        C::Min(0),
        C::Length((hints.len() as u16).min(area.width / 2)),
    ])
    .split(layout[3]);
    f.render_widget(
        para(Line::from(accent(
            shorten(&identity, footer[0].width as usize),
            theme().muted,
        ))),
        footer[0],
    );
    f.render_widget(
        para(Line::from(accent(hints, theme().muted))).right_aligned(),
        footer[1],
    );
    if a.busy {
        crate::activity::render(
            f,
            match a.status.as_str() {
                "draft…" => "Saving comment",
                "submit…" => "Publishing review",
                "load…" => "Refreshing pull request",
                "reply…" => "Sending reply",
                "chat…" => "Thinking",
                _ => "Working",
            },
            a.started,
        );
    }
    if let Some(modal) = a.modal {
        overlay(f, a, modal);
    }
}
// Collapsed panes keep a labelled restore button in the existing status row.
// Expanded panes have a small button on their border, outside selectable text.
fn pane_controls(f: &mut Frame, a: &mut App, status: Rect) -> Rect {
    a.panes.controls = [Rect::default(); 3];
    let panel_name = match a.panel {
        Panel::Ai => a.ai.name(),
        Panel::Overview => "Pull request",
        Panel::Threads => "Threads",
        Panel::Review => "Your review",
    };
    let names = ["Files", "Changes", panel_name];
    let mut right = status.right();
    for i in (0..3).rev() {
        let area = a.panes.areas[i];
        if area.width > 0 {
            if area.width >= 8 && area.height > 0 && a.panes.visible().len() > 1 {
                let rect = Rect::new(area.right().saturating_sub(5), area.y, 3, 1);
                a.panes.controls[i] = rect;
                f.render_widget(
                    Paragraph::new("[−]").style(Style::default().fg(theme().muted).bg(theme().bg)),
                    rect,
                );
            }
        } else {
            let label = format!("[+ {}]", names[i]);
            let width = Line::from(label.as_str()).width() as u16;
            if status.height > 0 && right.saturating_sub(status.x) >= width {
                let rect = Rect::new(right - width, status.y, width, 1);
                a.panes.controls[i] = rect;
                f.render_widget(
                    Paragraph::new(label).style(Style::default().fg(theme().accent).bg(theme().bg)),
                    rect,
                );
                right = rect.x.saturating_sub(1).max(status.x);
            }
        }
    }
    Rect::new(
        status.x,
        status.y,
        right.saturating_sub(status.x),
        status.height,
    )
}
fn annotation_colors() -> [Color; 2] {
    let colors = theme();
    let ai = [colors.purple, colors.green, colors.fg]
        .into_iter()
        .find(|color| *color != colors.accent)
        .unwrap_or(colors.muted);
    [colors.accent, ai]
}
fn annotation_badges(counts: crate::annotations::Counts) -> Vec<Span<'static>> {
    let colors = annotation_colors();
    let mut spans = vec![];
    if counts.github > 0 {
        spans.push(accent(format!("●{}", counts.github), colors[0]));
    }
    if counts.ai > 0 {
        spans.push(accent(format!("◇{}", counts.ai), colors[1]));
    }
    if !spans.is_empty() {
        spans.push(Span::raw(" "));
    }
    spans
}
fn annotation_gutter(
    annotations: &crate::annotations::Annotations,
    line: &V,
    side: Option<crate::annotations::Side>,
    style: Style,
) -> Vec<Span<'static>> {
    if annotations.gutter_width() == 0 {
        return vec![];
    }
    let colors = annotation_colors();
    annotations
        .markers(line, side)
        .into_iter()
        .enumerate()
        .map(|(i, mark)| Span::styled(mark.to_string(), style.fg(colors[i])))
        .collect()
}
fn tree(f: &mut Frame, a: &App, area: Rect, annotations: &crate::annotations::Annotations) {
    let outer = block(
        format!("Files · {}/{}", a.visible().len(), a.files().len()),
        a.pane == Pane::Tree,
    );
    let inner = outer.inner(area);
    f.render_widget(outer, area);
    let layout = Layout::vertical([C::Length(1), C::Min(1)]).split(inner);
    f.render_widget(
        para(Line::from(accent(
            format!(
                "⌕ {}",
                if a.filter.is_empty() {
                    "Files"
                } else {
                    &a.filter
                }
            ),
            theme().muted,
        ))),
        layout[0],
    );
    let rows = a.rows();
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            Row::Category(c) => {
                let count = a.files().iter().filter(|v| s(v, "category") == c).count();
                ListItem::new(Line::from(vec![
                    accent(format!("{c}  "), theme().accent),
                    accent(count.to_string(), theme().muted),
                ]))
            }
            Row::Directory(path, depth) => {
                let name = path
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or(path);
                ListItem::new(Line::from(accent(
                    format!(
                        "{}{} {name}/",
                        "  ".repeat(*depth),
                        if a.collapsed.contains(path) {
                            "▸"
                        } else {
                            "▾"
                        }
                    ),
                    theme().muted,
                )))
            }
            Row::File(i, depth) => {
                let file = &a.files()[*i];
                let name = s(file, "filename").rsplit('/').next().unwrap_or("");
                let (icon, color) = match s(file, "status") {
                    "added" => ("+", theme().green),
                    "removed" => ("−", theme().red),
                    "renamed" => ("→", theme().purple),
                    _ => ("~", theme().muted),
                };
                let plus = format!("+{}", file["additions"].as_u64().unwrap_or(0));
                let minus = format!("−{}", file["deletions"].as_u64().unwrap_or(0));
                let stats =
                    Line::from(plus.as_str()).width() + Line::from(minus.as_str()).width() + 2;
                let counts = annotations.counts(s(file, "filename"));
                // Yield indentation before counts, the filename, and +/- totals
                // when the user shrinks the tree. One cell belongs to selection.
                let available = layout[1].width.saturating_sub(1) as usize;
                let indent = (*depth * 2).min(available.saturating_sub(stats + counts.width() + 4));
                let prefix = format!(
                    "{}{} ",
                    " ".repeat(indent),
                    if file["viewed"] == true { "✓" } else { icon }
                );
                let room = available
                    .saturating_sub(Line::from(prefix.as_str()).width() + counts.width() + stats);
                let name = shorten(name, room);
                let pad = room.saturating_sub(Line::from(name.clone()).width());
                let mut spans = vec![accent(prefix, color)];
                spans.extend(annotation_badges(counts));
                spans.extend([
                    accent(
                        format!("{name}{} ", " ".repeat(pad)),
                        if *i == a.file {
                            theme().accent
                        } else {
                            theme().fg
                        },
                    ),
                    accent(plus, theme().green),
                    Span::raw(" "),
                    accent(minus, theme().red),
                ]);
                ListItem::new(Line::from(spans))
            }
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(a.tree_cursor));
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(theme().selected)
            .highlight_symbol("›"),
        layout[1],
        &mut state,
    );
}
fn line_style(line: &V, selected: bool) -> Style {
    let base = match s(line, "kind") {
        "add" => Style::default().fg(theme().green).bg(theme().add_bg),
        "delete" => Style::default().fg(theme().red).bg(theme().delete_bg),
        "hunk" => Style::default().fg(theme().accent).bg(theme().hunk_bg),
        _ => Style::default().fg(theme().fg),
    };
    if selected {
        base.patch(theme().selected)
    } else {
        base
    }
}
fn diff_style(a: &App, line: &V) -> Style {
    let mut style = line_style(line, false);
    if matches!(s(line, "kind"), "add" | "delete") {
        style = style.bg(a.syntax_palette.diff_bg(s(line, "kind") == "add"));
    }
    if is_selected(a, line) {
        style.patch(theme().selected)
    } else {
        style
    }
}
fn is_selected(a: &App, line: &V) -> bool {
    let mut selections = a.ranges.clone();
    if let Some(sel) = a.selection() {
        selections.push(sel);
    }
    selections.iter().any(|sel| {
        let key = if sel["side"] == "LEFT" { "old" } else { "new" };
        sel["path"] == a.current()["filename"]
            && line["hunk"] == sel["hunk"]
            && line[key].as_u64().is_some_and(|n| {
                n >= sel["start"].as_u64().unwrap_or(0) && n <= sel["end"].as_u64().unwrap_or(0)
            })
    })
}

fn diff(f: &mut Frame, a: &mut App, area: Rect, annotations: &crate::annotations::Annotations) {
    let outer = block(
        if a.split {
            "Changes · side by side [s]"
        } else {
            "Changes · inline [s]"
        },
        a.pane == Pane::Diff,
    );
    let inner = outer.inner(area);
    f.render_widget(outer, area);
    let layout = Layout::vertical([C::Length(2), C::Min(1), C::Length(1)]).split(inner);
    f.render_widget(
        para(vec![
            Line::from(accent(txt(a.current(), "filename"), theme().fg)),
            Line::from(vec![
                accent(format!(" +{} ", a.current()["additions"]), theme().green),
                accent(format!("-{}  ", a.current()["deletions"]), Color::Red),
                accent(
                    if a.current()["viewed"] == true {
                        "✓ viewed"
                    } else {
                        "m mark viewed"
                    },
                    theme().muted,
                ),
                accent(
                    format!(
                        " · {}{}",
                        a.syntax_palette.name(),
                        match &a.syntax {
                            None => " …",
                            Some(s) if s.partial() => " · plain fallback",
                            _ => "",
                        }
                    ),
                    theme().muted,
                ),
            ]),
        ]),
        layout[0],
    );
    if a.visible().is_empty() {
        f.render_widget(
            para("No files match.\n0 all types · / edit filter · z hidden"),
            layout[1],
        );
        return;
    }
    if a.lines().is_empty() {
        f.render_widget(para("Diff unavailable.\nGitHub omitted this patch (binary or large file).\nOpen the PR on GitHub to inspect it.").wrap(Wrap{trim:false}),layout[1]);
        return;
    }
    let mut lines = vec![];
    let mut cursor = a.line;
    if a.split {
        let width = (layout[1].width.saturating_sub(3) / 2) as usize;
        let selected = a.lines().get(a.line);
        cursor = 0;
        for (i, row) in array(a.current(), "splitRows").iter().enumerate() {
            if !row["hunk"].is_null() {
                lines.push(Line::styled(
                    clean(s(&row["hunk"], "text")),
                    line_style(&row["hunk"], false),
                ));
                if selected == Some(&row["hunk"]) {
                    cursor = i;
                }
                continue;
            }
            let mut parts = vec![];
            for (side, key) in [("left", "old"), ("right", "new")] {
                let line = &row[side];
                if selected == Some(line) {
                    cursor = i;
                }
                let style = diff_style(a, line);
                let mut cell = vec![];
                if !line.is_null() {
                    cell.push(Span::styled(
                        format!(
                            "{:>4} {} ",
                            line[key]
                                .as_u64()
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                            match s(line, "kind") {
                                "add" => "+",
                                "delete" => "-",
                                _ => " ",
                            }
                        ),
                        style,
                    ));
                    cell.extend(annotation_gutter(
                        annotations,
                        line,
                        Some(if side == "left" {
                            crate::annotations::Side::Left
                        } else {
                            crate::annotations::Side::Right
                        }),
                        style,
                    ));
                    cell.extend(code_spans(a, line, style, Some(side)));
                }
                parts.extend(crop_spans(cell, width, style));
                if side == "left" {
                    parts.push(accent(" │ ", theme().border));
                }
            }
            lines.push(Line::from(parts));
        }
    } else {
        for (i, line) in a.lines().iter().enumerate() {
            let old = line["old"]
                .as_u64()
                .map(|n| n.to_string())
                .unwrap_or_default();
            let new = line["new"]
                .as_u64()
                .map(|n| n.to_string())
                .unwrap_or_default();
            let sign = match s(line, "kind") {
                "add" => "+",
                "delete" => "-",
                "hunk" => "@",
                _ => " ",
            };
            let mark = if i == a.line { "›" } else { " " };
            let style = diff_style(a, line);
            let mut spans = vec![Span::styled(
                format!("{mark}{old:>4} {new:>4} {sign} "),
                style,
            )];
            spans.extend(annotation_gutter(annotations, line, None, style));
            spans.extend(code_spans(a, line, style, None));
            lines.push(Line::from(spans).style(style));
        }
    }
    let height = layout[1].height as usize;
    if cursor < a.diff_scroll {
        a.diff_scroll = cursor;
    }
    if cursor >= a.diff_scroll + height {
        a.diff_scroll = cursor.saturating_sub(height.saturating_sub(1));
    }
    f.render_widget(
        para(lines).scroll((a.diff_scroll.min(u16::MAX as usize) as u16, 0)),
        layout[1],
    );
    let searching = a.modal == Some(Modal::Editor(Input::DiffSearch));
    let footer = if searching || !a.search.query.is_empty() {
        format!(
            "/{}{}  {} · {}/{}{}",
            a.search.query,
            if searching { "▏" } else { "" },
            a.search.scope.label(),
            a.search.active.map(|i| i + 1).unwrap_or(0),
            a.search.hits.len(),
            if searching { " · Tab side" } else { " · n/N" }
        )
    } else if !a.ranges.is_empty() {
        format!("{} pinned · c comment", a.ranges.len())
    } else {
        "s layout · / search · v range · Space pin · c comment".into()
    };
    let mut footer_spans = vec![accent(
        footer,
        if searching {
            theme().accent
        } else {
            theme().muted
        },
    )];
    if annotations.gutter_width() > 0
        && !searching
        && a.search.query.is_empty()
        && layout[2].width as usize >= Line::from(footer_spans.clone()).width() + 20
    {
        let colors = annotation_colors();
        footer_spans.extend([accent(" · ● GitHub", colors[0]), accent(" ◇ AI", colors[1])]);
    }
    f.render_widget(para(Line::from(footer_spans)), layout[2]);
}
fn pill(f: &mut Frame, label: &str, rect: Rect, surface: Color, fg: Color) {
    if rect.width < 3 {
        return;
    }
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("◖", Style::default().fg(surface).bg(theme().bg)),
            Span::styled(
                shorten(label, rect.width.saturating_sub(2) as usize),
                Style::default().fg(fg).bg(surface),
            ),
            Span::styled("◗", Style::default().fg(surface).bg(theme().bg)),
        ])),
        rect,
    );
}
fn review_toolbar(f: &mut Frame, a: &mut App, area: Rect) {
    let viewed = a.files().iter().filter(|v| v["viewed"] == true).count();
    let labels = [
        format!("{viewed}/{} viewed", a.files().len()),
        format!("Your review · {}", a.my_comments().len()),
        format!("Review changes ({})", a.pending_comments().len()),
    ];
    let width = |label: &str| (Line::from(label).width() + 2) as u16;
    let action_width = width(&labels[2]).min(area.width);
    let action = Rect::new(
        area.right().saturating_sub(action_width),
        area.y,
        action_width,
        1,
    );
    let mut x = area.x;
    a.comments_button = Rect::default();
    let neutral = Color::Rgb(48, 54, 61);
    for (i, label) in labels[..2].iter().enumerate() {
        let w = width(label);
        if x.saturating_add(w + 1) <= action.x {
            let rect = Rect::new(x, area.y, w, 1);
            pill(f, label, rect, neutral, theme().fg);
            if i == 1 {
                a.comments_button = rect;
            }
            x += w + 1;
        }
    }
    pill(f, &labels[2], action, Color::Rgb(35, 134, 54), Color::White);
    a.review_button = action;
}

fn review_action(
    f: &mut Frame,
    a: &mut App,
    rect: Rect,
    label: &str,
    selected: bool,
    action: crate::review_list::Action,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let style = if selected {
        theme().selected.fg(theme().accent)
    } else {
        Style::default().fg(theme().muted).bg(theme().hunk_bg)
    };
    f.render_widget(
        Paragraph::new(shorten(label, rect.width as usize)).style(style),
        rect,
    );
    a.review_list.hits.push((rect, action));
}
fn review_location(c: &V) -> String {
    match (c["startLine"].as_u64(), c["line"].as_u64()) {
        (Some(start), Some(end)) if start != end => format!("L{start}–{end}"),
        (_, Some(end)) => format!("L{end}"),
        _ => "PR comment".into(),
    }
}
fn review_summary(f: &mut Frame, a: &mut App, area: Rect) {
    use crate::review_list::Action;
    a.review_list.hits.clear();
    if area.width == 0 || area.height == 0 {
        return;
    }
    let comments = a.my_comments();
    let drafts = comments.iter().filter(|c| c["pending"] == true).count();
    let posted = comments.len() - drafts;
    let draft_label = format!(" Drafts {drafts} ");
    let posted_label = format!(" Posted {posted} ");
    let first = (draft_label.len() as u16).min(area.width);
    review_action(
        f,
        a,
        Rect::new(area.x, area.y, first, 1),
        &draft_label,
        !a.review_list.posted,
        Action::Posted(false),
    );
    let x = area.x + first + 1;
    let second = (posted_label.len() as u16).min(area.right().saturating_sub(x));
    review_action(
        f,
        a,
        Rect::new(x, area.y, second, 1),
        &posted_label,
        a.review_list.posted,
        Action::Posted(true),
    );
    if area.height < 3 {
        return;
    }
    let body = Rect::new(
        area.x,
        area.y + 2,
        area.width,
        area.height.saturating_sub(3),
    );
    let rows = a.review_list.rows(comments);
    let mut lines = vec![];
    let mut positions = vec![];
    let mut selected_y = 0;
    for (i, row) in rows.iter().enumerate() {
        let open = a.review_list.expanded.contains(&row.key);
        let selected = i == a.panel_cursor;
        if selected {
            selected_y = lines.len();
        }
        positions.push(lines.len());
        let base = if selected {
            theme().selected
        } else {
            Style::default()
        };
        let arrow = if open { "▾" } else { "▸" };
        if let Some(c) = &row.comment {
            let location = review_location(c);
            let first = s(c, "body")
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("");
            let preview = crate::markdown::render_at(first, body.width)
                .first()
                .map(Line::to_string)
                .unwrap_or_default();
            lines.push(Line::styled(
                shorten(
                    &format!("  {arrow} {location}  {preview}"),
                    body.width as usize,
                ),
                base.fg(theme().fg),
            ));
            if open {
                let inner_width = body.width.saturating_sub(2);
                for line in
                    crate::reader::comment_card("you", "", &location, s(c, "body"), inner_width)
                {
                    let mut spans = vec![Span::raw("  ")];
                    spans.extend(line.spans);
                    lines.push(Line::from(spans));
                }
            }
        } else {
            let path = if row.path.is_empty() {
                "PR conversation"
            } else {
                &row.path
            };
            let count = format!(" {} ", row.count);
            let available = body.width.saturating_sub(count.len() as u16) as usize;
            let label = shorten(&format!("{arrow} {path}"), available);
            let gap = available.saturating_sub(Line::from(label.clone()).width());
            lines.push(Line::styled(
                format!("{label}{}{count}", " ".repeat(gap)),
                base.fg(theme().accent)
                    .bg(if selected {
                        theme().selected.bg.unwrap_or(theme().hunk_bg)
                    } else {
                        theme().hunk_bg
                    })
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    if rows.is_empty() {
        lines.push(Line::from(accent(
            if a.review_list.posted {
                "No posted comments."
            } else {
                "No draft comments."
            },
            theme().muted,
        )));
        if !a.review_list.posted {
            lines.push(Line::from(accent(
                "S to leave a summary or approve.",
                theme().muted,
            )));
        }
    }
    if a.review_list.follow {
        if selected_y < a.panel_scroll || selected_y >= a.panel_scroll + body.height as usize {
            a.panel_scroll = selected_y;
        }
        a.review_list.follow = false;
    }
    a.panel_scroll = a
        .panel_scroll
        .min(lines.len().saturating_sub(body.height as usize));
    for (i, y) in positions.into_iter().enumerate() {
        if y >= a.panel_scroll && y < a.panel_scroll + body.height as usize {
            a.review_list.hits.push((
                Rect::new(body.x, body.y + (y - a.panel_scroll) as u16, body.width, 1),
                Action::Row(i),
            ));
        }
    }
    f.render_widget(
        para(lines).scroll((a.panel_scroll.min(u16::MAX as usize) as u16, 0)),
        body,
    );
    let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    let label = " S Submit ";
    let width = (label.len() as u16).min(footer.width);
    let action = Rect::new(footer.right() - width, footer.y, width, 1);
    f.render_widget(
        para(Line::from(accent(
            shorten(
                "Enter open · P history",
                footer.width.saturating_sub(width + 1) as usize,
            ),
            theme().muted,
        ))),
        footer,
    );
    review_action(f, a, action, label, true, Action::Compose);
}
fn review_counts(a: &App) -> String {
    let comments = a.my_comments();
    let drafts = comments
        .iter()
        .filter(|c| c["pending"] == true)
        .collect::<Vec<_>>();
    let files = drafts
        .iter()
        .map(|c| s(c, "path"))
        .collect::<std::collections::HashSet<_>>()
        .len();
    format!(
        "{} draft comment{} · {} file{}",
        drafts.len(),
        if drafts.len() == 1 { "" } else { "s" },
        files,
        if files == 1 { "" } else { "s" }
    )
}
fn review_editor(f: &mut Frame, a: &mut App) {
    use crate::review_list::Action;
    a.review_list.hits.clear();
    let area = f.area();
    let width = area.width.saturating_sub(4).min(74);
    let stacked = width < 62;
    let outcome_rows = if stacked { 3 } else { 1 };
    let text_width = width.saturating_sub(6).max(1) as usize;
    let editor_lines = a
        .editor
        .lines()
        .map(|l| Line::from(l).width().max(1).div_ceil(text_width))
        .sum::<usize>()
        .clamp(3, 7);
    let height =
        (editor_lines + outcome_rows + 9).min(area.height.saturating_sub(2) as usize) as u16;
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    a.review_list.dialog = rect;
    f.render_widget(Clear, rect);
    let outer = block("Review changes", true).style(Style::default().bg(theme().bg).fg(theme().fg));
    let inner = outer.inner(rect);
    f.render_widget(outer, rect);
    if inner.width == 0 || inner.height < 5 {
        return;
    }
    let parts = Layout::vertical([
        C::Length(1),
        C::Length(outcome_rows as u16),
        C::Length(1),
        C::Min(3),
        C::Length(1),
        C::Length(1),
        C::Length(1),
    ])
    .split(inner);
    f.render_widget(
        para(Line::from(accent(review_counts(a), theme().muted))),
        parts[0],
    );
    let mut x = parts[1].x;
    for (i, label) in ["Comment", "Approve", "Request changes"].iter().enumerate() {
        let disabled = a.own_pr() && i != 0;
        let text = format!(" {} {label} ", if a.review_event == i { "●" } else { "○" });
        let cell = if stacked {
            Rect::new(parts[1].x, parts[1].y + i as u16, parts[1].width, 1)
        } else {
            let w =
                (Line::from(text.clone()).width() as u16).min(parts[1].right().saturating_sub(x));
            let r = Rect::new(x, parts[1].y, w, 1);
            x = x.saturating_add(w + 1);
            r
        };
        if disabled {
            f.render_widget(
                para(Line::from(accent(
                    shorten(&text, cell.width as usize),
                    theme().border,
                ))),
                cell,
            );
        } else {
            review_action(f, a, cell, &text, a.review_event == i, Action::Outcome(i));
        }
    }
    f.render_widget(
        para(Line::from(accent(
            if a.own_pr() {
                "Your PR · Comment only"
            } else {
                "Tab / Shift-Tab outcome"
            },
            theme().muted,
        ))),
        parts[2],
    );
    let optional = a.review_event == 1 || !a.pending_comments().is_empty();
    let summary = block(
        if optional {
            "Summary · optional"
        } else {
            "Summary"
        },
        false,
    );
    let text_area = summary.inner(parts[3]);
    f.render_widget(summary, parts[3]);
    let mut display = clean(&a.editor);
    let cursor = a.editor_cursor.min(display.len());
    if display.is_char_boundary(cursor) {
        display.insert(cursor, '▏');
    } else {
        display.push('▏');
    }
    let before = &a.editor[..a.editor_cursor.min(a.editor.len())];
    let cursor_line = before
        .split('\n')
        .map(|l| {
            Line::from(l)
                .width()
                .max(1)
                .div_ceil(text_area.width.max(1) as usize)
        })
        .sum::<usize>()
        .saturating_sub(1);
    f.render_widget(
        para(display).wrap(Wrap { trim: false }).scroll((
            cursor_line
                .saturating_sub(text_area.height.saturating_sub(1) as usize)
                .min(u16::MAX as usize) as u16,
            0,
        )),
        text_area,
    );
    let suggestion = if a.completion.open {
        a.completion
            .items
            .get(a.completion.selected)
            .map(|v| format!("{}  {}", txt(v, "text"), txt(v, "label")))
            .unwrap_or_else(|| {
                if a.completion.error.is_empty() {
                    "Suggestions…".into()
                } else {
                    a.completion.error.clone()
                }
            })
    } else {
        a.status.clone()
    };
    f.render_widget(
        para(Line::from(accent(
            shorten(&suggestion, parts[4].width as usize),
            if a.completion.open {
                theme().accent
            } else {
                theme().red
            },
        ))),
        parts[4],
    );
    f.render_widget(
        para(Line::from(accent(
            if a.completion.open {
                "↑↓ choose · Enter/Tab insert · Esc dismiss"
            } else {
                "Enter preview · Alt-Enter newline"
            },
            theme().muted,
        ))),
        parts[5],
    );
    let back = Rect::new(parts[6].x, parts[6].y, 10.min(parts[6].width), 1);
    review_action(f, a, back, " Esc Back ", false, Action::Back);
    let w = 14.min(parts[6].width);
    review_action(
        f,
        a,
        Rect::new(parts[6].right() - w, parts[6].y, w, 1),
        " Continue → ",
        true,
        Action::Continue,
    );
}
fn review_confirmation(f: &mut Frame, a: &mut App) {
    use crate::review_list::Action;
    a.review_list.hits.clear();
    let value = a.pending.clone().unwrap_or(V::Null);
    let area = f.area();
    let width = area.width.saturating_sub(4).min(68);
    let body_width = width.saturating_sub(4).max(1);
    let mut lines = vec![];
    if value["operation"] == "submit" {
        let event = match s(&value, "event") {
            "APPROVE" => "Approve",
            "REQUEST_CHANGES" => "Request changes",
            _ => "Comment",
        };
        lines.push(Line::from(accent(
            format!(
                "{event} · {}/{} #{}",
                s(&a.pr, "owner"),
                s(&a.pr, "repo"),
                a.pr["number"]
            ),
            theme().accent,
        )));
        lines.push(Line::from(accent(review_counts(a), theme().muted)));
        lines.push(Line::from(""));
        if s(&value, "body").is_empty() {
            lines.push(Line::from(accent("No summary.", theme().muted)));
        } else {
            lines.extend(crate::markdown::wrap(
                &crate::markdown::document_at(s(&value, "body"), false, body_width).lines,
                body_width,
            ));
        }
    } else {
        lines.push(Line::from(if value["resolved"] == true {
            "Resolve this thread?"
        } else {
            "Reopen this thread?"
        }));
        lines.push(Line::from(accent(
            txt(a.selected_thread(), "path"),
            theme().muted,
        )));
    }
    let lines = crate::markdown::wrap(&lines, body_width);
    let height = (lines.len() + 5)
        .clamp(8, 17)
        .min(area.height.saturating_sub(2) as usize) as u16;
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    a.review_list.dialog = rect;
    f.render_widget(Clear, rect);
    let outer = block(
        if value["operation"] == "submit" {
            "Publish review"
        } else {
            "Thread state"
        },
        true,
    )
    .style(Style::default().bg(theme().bg).fg(theme().fg));
    let inner = outer.inner(rect);
    f.render_widget(outer, rect);
    if inner.height < 3 || inner.width == 0 {
        return;
    }
    let parts = Layout::vertical([C::Min(1), C::Length(1), C::Length(1)]).split(inner);
    a.panel_scroll = a
        .panel_scroll
        .min(lines.len().saturating_sub(parts[0].height as usize));
    let scrollable = lines.len() > parts[0].height as usize;
    f.render_widget(
        para(lines).scroll((a.panel_scroll.min(u16::MAX as usize) as u16, 0)),
        parts[0],
    );
    f.render_widget(
        para(Line::from(accent(
            if scrollable {
                "j/k scroll · Enter publishes · Esc edits"
            } else if value["operation"] == "submit" {
                "Enter publishes · Esc edits"
            } else {
                "Enter confirms · Esc cancels"
            },
            theme().muted,
        ))),
        parts[1],
    );
    review_action(
        f,
        a,
        Rect::new(parts[2].x, parts[2].y, 10.min(parts[2].width), 1),
        " Esc Back ",
        false,
        Action::Back,
    );
    let label = if value["operation"] == "submit" {
        " Publish review "
    } else {
        " Confirm "
    };
    let w = (label.len() as u16).min(parts[2].width);
    review_action(
        f,
        a,
        Rect::new(parts[2].right() - w, parts[2].y, w, 1),
        label,
        true,
        Action::Publish,
    );
}

fn code_spans(a: &App, line: &V, style: Style, side: Option<&str>) -> Vec<Span<'static>> {
    let text = crate::search::text(line);
    let allowed = s(line, "kind") != "hunk"
        && a.search.scope.allows(line)
        && !matches!(
            (a.search.scope, side),
            (crate::search::Scope::Original, Some("right"))
                | (crate::search::Scope::Modified, Some("left"))
        );
    let ranges = if allowed {
        a.search
            .regex
            .as_ref()
            .map(|r| {
                r.find_iter(&text)
                    .map(|m| (m.start(), m.end()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        vec![]
    };
    let active = a
        .search
        .current()
        .filter(|hit| a.lines().get(hit.line) == Some(line));
    let left = side == Some("left") || (side.is_none() && s(line, "kind") == "delete");
    let syntax = a.syntax.as_ref().and_then(|syntax| {
        syntax.line(
            left,
            line[if left { "old" } else { "new" }].as_u64().unwrap_or(0),
            &text,
        )
    });
    let mut spans = vec![];
    for (byte, c) in text.char_indices().skip(a.horizontal as usize) {
        let highlight = ranges
            .iter()
            .find(|&&(start, end)| byte >= start && byte < end);
        let color = if let Some(&(start, _)) = highlight {
            style
                .fg(Color::Black)
                .bg(if active.is_some_and(|h| h.start == start) {
                    Color::Yellow
                } else {
                    theme().accent
                })
                .add_modifier(Modifier::BOLD)
        } else {
            syntax
                .map(|line| style.patch(line.style_at(byte)))
                .unwrap_or(style)
        };
        if let Some(last) = spans
            .last_mut()
            .filter(|last: &&mut Span<'static>| last.style == color)
        {
            last.content.to_mut().push(c);
        } else {
            spans.push(Span::styled(c.to_string(), color));
        }
    }
    spans
}

fn crop_spans(spans: Vec<Span<'static>>, width: usize, style: Style) -> Vec<Span<'static>> {
    let mut out = vec![];
    let mut used = 0;
    let mut truncated = false;
    for span in spans {
        let mut text = String::new();
        for c in span.content.chars() {
            let w = Span::raw(c.to_string()).width();
            if used + w > width {
                truncated = true;
                break;
            }
            text.push(c);
            used += w;
        }
        if !text.is_empty() {
            out.push(Span::styled(text, span.style));
        }
        if used >= width || truncated {
            break;
        }
    }
    if used < width {
        out.push(Span::styled(" ".repeat(width - used), style));
    }
    out
}

fn panel(f: &mut Frame, a: &mut App, area: Rect) {
    if a.panel == Panel::Ai && a.ai.native_mode {
        crate::native::render(f, a, area);
        return;
    }
    if a.panel != Panel::Review {
        crate::reader::render(f, a, area);
        return;
    }
    let outer = block("Your review", a.pane == Pane::Panel);
    let inner = outer.inner(area);
    f.render_widget(outer, area);
    review_summary(f, a, inner);
}

fn overlay(f: &mut Frame, a: &mut App, modal: Modal) {
    if modal == Modal::Editor(Input::Review) {
        review_editor(f, a);
        return;
    }
    if modal == Modal::Confirm {
        review_confirmation(f, a);
        return;
    }
    if modal == Modal::Context {
        crate::reader::context(f, a);
        return;
    }
    if modal == Modal::Artifacts {
        crate::reader::artifacts(f, a);
        return;
    }
    if matches!(modal, Modal::Appearance | Modal::Outline) {
        crate::reader::chooser(f, a, modal == Modal::Outline);
        return;
    }
    if modal == Modal::Editor(Input::Chat) {
        return;
    }
    if modal == Modal::Editor(Input::DiffSearch) {
        return;
    }
    if modal == Modal::Help {
        a.help.render(f);
        return;
    }
    let area = f.area();
    if modal == Modal::AiSetup {
        let width = area.width.saturating_sub(4).min(68);
        let height = area.height.saturating_sub(2).min(10);
        let rect = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        f.render_widget(Clear, rect);
        let outer = block("Connect Claude Code", true).style(Style::default().bg(theme().bg));
        let inner = outer.inner(rect);
        f.render_widget(outer, rect);
        f.render_widget(para("Use your existing Claude Code login.\n\nOpening a PR will prepare a persistent chat in the background. PR code is sent through your configured Claude account.\n\nEnter Enable · Esc Cancel\nSettings: ~/.config/lazyreview/config.yml").wrap(Wrap{trim:false}),inner);
        return;
    }
    if modal == Modal::Panes {
        let w = area.width.saturating_sub(4).min(30);
        let h = area.height.saturating_sub(2).min(7);
        let rect = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        );
        f.render_widget(Clear, rect);
        let outer = block("Panes", true).style(Style::default().bg(theme().bg).fg(theme().fg));
        let inner = outer.inner(rect);
        f.render_widget(outer, rect);
        let items: Vec<ListItem> = ["Files", "Changes", "Companion"]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                ListItem::new(format!(
                    "{} {name}",
                    if a.panes.visible().contains(&i) {
                        "●"
                    } else {
                        "○"
                    }
                ))
            })
            .collect();
        f.render_stateful_widget(
            List::new(items)
                .highlight_symbol("› ")
                .highlight_style(theme().selected),
            Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(1),
            ),
            &mut ListState::default().with_selected(Some(a.pane_choice)),
        );
        f.render_widget(
            para("Enter toggle · Esc close"),
            Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
        );
        return;
    }
    if let Modal::Editor(input) = modal {
        let width = area.width.saturating_sub(6).min(78);
        let mut meta = vec![];
        let title = match input {
            Input::Filter => "Filter files",
            Input::DiffSearch => "Search diff",
            Input::Open => "Open PR",
            Input::Comment => "Comment · draft",
            Input::Review => "Review changes",
            Input::Reply => "Reply",
            Input::Conversation => "PR comment",
            Input::Chat => "Ask AI",
            Input::AiNote => "Private note · next AI turn",
        };
        if input == Input::AiNote {
            if let Some(item) = &a.note_target {
                meta.push(Line::from(accent(
                    format!("{} · {}", s(item, "path"), s(item, "label")),
                    theme().muted,
                )));
            }
        }
        if input == Input::Comment {
            if let Some(range) = a.comment_ranges.first() {
                meta.push(Line::from(accent(
                    format!(
                        "{} · {}:{}–{}",
                        txt(range, "path"),
                        txt(range, "side"),
                        range["start"],
                        range["end"]
                    ),
                    theme().muted,
                )));
                if a.comment_ranges.len() > 1 {
                    meta.push(Line::from(accent(
                        format!("+ {} linked ranges", a.comment_ranges.len() - 1),
                        theme().muted,
                    )));
                }
            }
        }
        if input == Input::Review {
            for (i, label) in ["Comment", "Approve", "Request changes"].iter().enumerate() {
                meta.push(Line::styled(
                    format!(" {} {label}", if i == a.review_event { "●" } else { "○" }),
                    if i == a.review_event {
                        theme().selected.fg(theme().accent)
                    } else {
                        Style::default().fg(theme().muted)
                    },
                ));
            }
            meta.push(Line::from(""));
        }
        let choices = if a.completion.open {
            a.completion.items.len().clamp(1, 5)
        } else {
            0
        };
        let mut display = clean(&a.editor);
        let cursor = a.editor_cursor.min(display.len());
        if display.is_char_boundary(cursor) {
            display.insert(cursor, '▏');
        } else {
            display.push('▏');
        }
        let text_height = display
            .lines()
            .map(|l| {
                Line::from(l)
                    .width()
                    .max(1)
                    .div_ceil(width.saturating_sub(4).max(1) as usize)
            })
            .sum::<usize>()
            .clamp(3, 9);
        let height = (meta.len() + text_height + choices + 5)
            .min(area.height.saturating_sub(4) as usize) as u16;
        let rect = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        f.render_widget(Clear, rect);
        let outer = block(title, true).style(Style::default().bg(theme().bg).fg(theme().fg));
        let inner = outer.inner(rect);
        f.render_widget(outer, rect);
        let rows = Layout::vertical([
            C::Length(meta.len() as u16),
            C::Min(2),
            C::Length(choices as u16),
            C::Length(2),
        ])
        .split(inner);
        f.render_widget(para(meta), rows[0]);
        let before = &a.editor[..a.editor_cursor.min(a.editor.len())];
        let cursor_line = before
            .split('\n')
            .map(|l| {
                Line::from(l)
                    .width()
                    .max(1)
                    .div_ceil(rows[1].width.max(1) as usize)
            })
            .sum::<usize>()
            .saturating_sub(1);
        f.render_widget(
            para(display).wrap(Wrap { trim: false }).scroll((
                cursor_line
                    .saturating_sub(rows[1].height.saturating_sub(1) as usize)
                    .min(u16::MAX as usize) as u16,
                0,
            )),
            rows[1],
        );
        if choices > 0 {
            if a.completion.items.is_empty() {
                f.render_widget(
                    para(Line::from(accent(
                        if a.completion.error.is_empty() {
                            "…"
                        } else {
                            &a.completion.error
                        },
                        theme().muted,
                    ))),
                    rows[2],
                );
            } else {
                let items: Vec<ListItem> = a
                    .completion
                    .items
                    .iter()
                    .map(|v| {
                        ListItem::new(format!(
                            "{}  {}  {}",
                            txt(v, "text"),
                            txt(v, "label"),
                            txt(v, "kind")
                        ))
                    })
                    .collect();
                f.render_stateful_widget(
                    List::new(items)
                        .highlight_style(theme().selected)
                        .highlight_symbol("› "),
                    rows[2],
                    &mut ListState::default().with_selected(Some(a.completion.selected)),
                );
            }
        }
        let hint = if a.completion.open && !a.completion.items.is_empty() {
            "↑↓ choose · Enter/Tab insert · Esc dismiss"
        } else {
            match input {
                Input::AiNote => "Enter queue note · Ctrl-n newline · Esc cancel",
                Input::Comment => "Enter save draft · Alt-Enter newline · Esc cancel",
                Input::Review => "Enter preview · Tab outcome · Alt-Enter newline",
                Input::Filter | Input::DiffSearch | Input::Open => "Enter apply · Esc cancel",
                _ => "Enter send · Alt-Enter newline · Esc cancel",
            }
        };
        f.render_widget(
            para(vec![
                Line::from(accent(hint, theme().muted)),
                Line::from(accent(clean(&a.status), theme().red)),
            ]),
            rows[3],
        );
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_is_closed_until_expanded_and_toolbar_opens_outcome_form() {
        use crossterm::event::{
            KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
        };
        let mut a = App::new(
            serde_json::json!({"demo":true,"files":[],"threads":[{"path":"src/example.rs","line":9,"comments":[{"id":7,"author":"you","pending":true,"body":"**Only after opening**\n\nFull detail after selecting."}]}]}),
        );
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(150, 40)).unwrap();
        let text = |t: &ratatui::Terminal<ratatui::backend::TestBackend>| {
            t.backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        };
        a.key(key('r'));
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(text(&terminal).contains("src/example.rs"));
        assert!(!text(&terminal).contains("Only after opening"));
        a.key(key('l'));
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(text(&terminal).contains("Only after opening"));
        assert!(!text(&terminal).contains("Full detail after selecting."));
        a.key(key('j'));
        a.key(key('l'));
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(text(&terminal).contains("Full detail after selecting."));
        let r = a.review_button;
        a.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: r.x + 1,
            row: r.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(a.modal, Some(Modal::Editor(Input::Review)));
        assert!(a.pending.is_none());
        terminal.draw(|f| render(f, &mut a)).unwrap();
        for label in ["Comment", "Approve", "Request changes"] {
            assert!(text(&terminal).contains(label));
        }
    }
    #[test]
    fn search_highlights_match_bytes_and_preserves_diff_background_around_them() {
        let line = serde_json::json!({"kind":"add","text":"let café = café;","new":1});
        let mut a = App::new(serde_json::json!({"files":[{"filename":"a.rs","lines":[line]}]}));
        a.search.query = "café".into();
        let file = a.current().clone();
        a.search.rebuild(&file, false);
        a.search.first_from(0);
        let style = line_style(&line, false);
        let spans = code_spans(&a, &line, style, None);
        assert_eq!(
            spans.iter().map(|s| s.content.as_ref()).collect::<String>(),
            "let café = café;"
        );
        assert!(
            spans
                .iter()
                .any(|s| s.content == "café" && s.style.bg == Some(Color::Yellow))
        );
        assert!(
            spans
                .iter()
                .any(|s| s.content == "café" && s.style.bg == Some(theme().accent))
        );
        assert_eq!(spans[0].style.bg, style.bg);
    }
    #[test]
    fn fit_renders_full_deep_name_and_counts_and_restore_controls_work() {
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let pr = serde_json::json!({"demo":true,"files":[{
            "filename":"src/one/two/three/four/five/six/longest_component_filename.rs",
            "category":"Code","additions":12345,"deletions":6789,"status":"modified","lines":[]
        }]});
        let mut a = App::new(pr);
        a.panes.panel = true;
        a.fit_panes();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(250, 45)).unwrap();
        terminal.draw(|f| render(f, &mut a)).unwrap();
        let text = |t: &ratatui::Terminal<ratatui::backend::TestBackend>| {
            t.backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        };
        let rendered = text(&terminal);
        assert!(rendered.contains("longest_component_filename.rs"));
        assert!(rendered.contains("+12345 −6789"));
        assert_eq!(a.panes.areas[2].width, crate::reader::READING_WIDTH + 5);
        let click = |rect: Rect| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x + 1,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        };
        let fold = a.panes.controls[0];
        assert_eq!(fold.y, a.panes.areas[0].y);
        a.mouse(click(fold));
        assert!(!a.panes.tree);
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(text(&terminal).contains("[+ Files]"));
        a.mouse(click(a.panes.controls[0]));
        assert!(a.panes.tree);
        assert_eq!(a.pane, Pane::Tree);
        terminal.draw(|f| render(f, &mut a)).unwrap();
        a.mouse(click(a.panes.controls[2]));
        assert!(!a.panes.panel);
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(text(&terminal).contains("[+ Claude]"));
        a.mouse(click(a.panes.controls[2]));
        assert!(a.panes.panel);
        assert_eq!(a.pane, Pane::Panel);
    }
    #[test]
    fn layouts_render_both_diff_modes_and_folded_companion_at_small_sizes() {
        let mut bridge = crate::backend::Bridge::new().unwrap();
        let mut a = App::new(crate::backend::initial(&mut bridge, None).unwrap());
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(180, 45)).unwrap();
        terminal.draw(|f| render(f, &mut a)).unwrap();
        let text = |t: &ratatui::Terminal<ratatui::backend::TestBackend>| {
            t.backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        };
        assert!(text(&terminal).contains("Changes · inline"));
        a.split = true;
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(text(&terminal).contains("side by side"));
        a.panes.panel = false;
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert_eq!(a.panes.areas[2].width, 0);
        for (width, height) in [(40, 12), (80, 24), (1, 1), (0, 0)] {
            let mut small =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            small.draw(|f| render(f, &mut a)).unwrap();
        }
    }
    #[test]
    fn large_review_and_compact_composer_render_without_losing_tree_statistics() {
        let mut bridge = crate::backend::Bridge::new().unwrap();
        let mut pr = crate::backend::initial(&mut bridge, None).unwrap();
        let source = pr["files"][0].clone();
        pr["files"] = serde_json::json!(
            (0..85)
                .map(|i| {
                    let mut f = source.clone();
                    f["filename"] = serde_json::json!(format!("src/group{}/file{i}.tsx", i % 5));
                    f["category"] = serde_json::json!(if i % 5 == 0 { "Tests" } else { "Code" });
                    f["status"] =
                        serde_json::json!(["added", "modified", "removed", "renamed"][i % 4]);
                    f
                })
                .collect::<Vec<_>>()
        );
        let mut a = App::new(pr);
        a.category = None;
        assert_eq!(a.visible().len(), 85);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(150, 45)).unwrap();
        terminal.draw(|f| render(f, &mut a)).unwrap();
        let before = terminal.backend().buffer().clone();
        a.pane = Pane::Diff;
        a.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('c'),
            crossterm::event::KeyModifiers::NONE,
        ));
        terminal.draw(|f| render(f, &mut a)).unwrap();
        let after = terminal.backend().buffer();
        let changed = before
            .content
            .iter()
            .zip(&after.content)
            .filter(|(a, b)| a.symbol() != b.symbol())
            .count();
        assert!(changed < 150 * 45 / 2);
        let text = after.content.iter().map(|c| c.symbol()).collect::<String>();
        assert!(text.contains("Files · 85/85"));
        assert!(text.contains("+41"));
        assert!(text.contains("−10"));
        assert!(text.contains("Comment · draft"));
        for (width, height) in [(30, 12), (1, 1), (0, 0)] {
            let mut t =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            t.draw(|f| render(f, &mut a)).unwrap();
        }
    }
    #[test]
    fn loading_card_changes_frames_and_contains_elapsed_time() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
        let now = std::time::Instant::now();
        terminal
            .draw(|f| crate::activity::render(f, "Opening pull request", now))
            .unwrap();
        let first = terminal.backend().buffer().clone();
        terminal
            .draw(|f| {
                crate::activity::render(
                    f,
                    "Opening pull request",
                    now - std::time::Duration::from_millis(400),
                )
            })
            .unwrap();
        assert_ne!(first, terminal.backend().buffer().clone());
    }
    #[test]
    fn strips_terminal_escape_and_control_bytes() {
        assert_eq!(clean("hello\x1b[31m\x07world\n"), "hello[31mworld\n");
    }
    #[test]
    fn compact_controls_have_independent_fills_and_identity_moves_to_footer() {
        let mut a = App::new(
            serde_json::json!({"owner":"owner","repo":"repo","number":42,"title":"PR title","files":[],"threads":[]}),
        );
        let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 30)).unwrap();
        render_test(&mut t, &mut a);
        let buf = t.backend().buffer();
        let row = |y| (0..120).map(|x| buf[(x, y)].symbol()).collect::<String>();
        assert!(row(29).starts_with(" owner/repo #42"));
        assert!(row(29).contains("? Shortcuts"));
        for removed in ["LazyReview", "LIVE GITHUB", "NORMAL"] {
            assert!(
                !buf.content
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>()
                    .contains(removed)
            );
        }
        assert_eq!(a.review_button.height, 1);
        assert_eq!(a.comments_button.height, 1);
        let r = a.comments_button;
        assert_eq!(buf[(r.x - 1, r.y)].bg, theme().bg);
        assert_eq!(buf[(r.right(), r.y)].bg, theme().bg);
        assert_ne!(buf[(r.x + 1, r.y)].bg, theme().bg);
        assert_eq!(buf[(r.x, r.y)].bg, theme().bg); // rounded cap has no square fill
        a.mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: r.x + 1,
            row: r.y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        });
        assert_eq!(a.panel, Panel::Review);
    }
    fn render_test(t: &mut ratatui::Terminal<ratatui::backend::TestBackend>, a: &mut App) {
        t.draw(|f| render(f, a)).unwrap();
    }
    fn review_flow_fixture() -> App {
        let mut a = App::new(
            serde_json::json!({"demo":true,"owner":"modem-dev","repo":"command-menu","number":42,"title":"Make keyboard navigation predictable","head":"a","base":"b","viewer":"you","author":"sarah","files":[],"threads":[
            {"path":"src/hooks/useNavigation.ts","line":24,"comments":[{"id":1,"author":"you","pending":true,"body":"Could we reset the selected index when the filtered list shrinks?\n\n```ts\nuseEffect(() => reset(), [query, reset]);\n```\n\nThis avoids Enter running a command that is no longer visible."}]},
            {"path":"src/hooks/useNavigation.ts","line":41,"comments":[{"id":2,"author":"you","pending":true,"body":"**Empty results:** please guard modulo by zero before changing the active index.\n\n- Return early for an empty result list.\n- Cover it with a regression test."}]},
            {"path":"src/components/CommandMenu.tsx","line":88,"comments":[{"id":3,"author":"you","pending":true,"body":"Can the active result keep its stable ID for aria-activedescendant?"}]},
            {"path":"tests/command-menu.test.tsx","line":12,"comments":[{"id":4,"author":"you","pending":true,"body":"Please add a keyboard-only test for clearing the filter."}]},
            {"path":"src/components/CommandMenu.tsx","line":19,"comments":[{"id":5,"author":"you","pending":false,"body":"The extracted component boundary looks right."}]}
        ],"conversation":[{"id":6,"author":"you","body":"I will focus on keyboard and screen reader behavior."}],"commits":[]}),
        );
        a.panel = Panel::Review;
        a.pane = Pane::Panel;
        a.panes.panel = true;
        a.panes.zoom = true;
        a
    }
    #[test]
    #[ignore = "manual review-flow native-cell artifact"]
    fn export_review_flow_preview() {
        let mut screens = vec![];
        for (w, h) in [(100, 36), (58, 28)] {
            for state in ["closed", "expanded", "editor", "confirm"] {
                let mut a = review_flow_fixture();
                if state != "closed" {
                    for row in a.review_list.rows(a.my_comments()) {
                        a.review_list.expanded.insert(row.key);
                    }
                    if let Some(row) = a
                        .review_list
                        .rows(a.my_comments())
                        .into_iter()
                        .find(|row| row.comment.is_some())
                    {
                        a.review_list.toggle(&row.key, true, Some(true));
                    }
                    a.panel_cursor = 1;
                }
                if state == "editor" || state == "confirm" {
                    a.open_editor(Input::Review);
                    a.review_event = 2;
                    a.editor="The structure looks good. Please handle the empty-list boundary and keep keyboard focus stable before merging.".into();
                    a.editor_cursor = a.editor.len();
                }
                if state == "confirm" {
                    a.key(crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Enter,
                        crossterm::event::KeyModifiers::NONE,
                    ));
                }
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
                terminal.draw(|f| render(f, &mut a)).unwrap();
                screens.push(serde_json::json!({"panel":state,"width":w,"height":h,"html":crate::preview::buffer_html(terminal.backend().buffer())}));
            }
        }
        let file = std::env::var("LAZYREVIEW_REVIEW_PREVIEW")
            .unwrap_or("/private/tmp/lazyreview-review-preview.json".into());
        std::fs::write(file, serde_json::to_vec_pretty(&screens).unwrap()).unwrap();
    }

    #[test]
    fn review_confirmation_is_compact_and_mouse_controls_preserve_summary() {
        use crate::review_list::Action;
        use crossterm::event::{
            KeyCode as K, KeyEvent, KeyModifiers as M, MouseButton as B, MouseEvent,
            MouseEventKind as E,
        };
        let mut a = review_flow_fixture();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 36)).unwrap();
        let hit = |a: &App, find: fn(Action) -> bool| {
            a.review_list
                .hits
                .iter()
                .find(|(_, action)| find(*action))
                .unwrap()
                .0
        };
        let click = |r: Rect, kind| MouseEvent {
            kind,
            column: r.x,
            row: r.y,
            modifiers: M::NONE,
        };
        a.open_editor(Input::Review);
        terminal.draw(|f| render(f, &mut a)).unwrap();
        let outcome = hit(&a, |a| matches!(a, Action::Outcome(2)));
        a.mouse(click(outcome, E::Down(B::Left)));
        assert_eq!(a.review_event, 2);
        a.paste("Please guard the empty-list boundary.");
        let next = hit(&a, |a| matches!(a, Action::Continue));
        assert!(a.mouse(click(next, E::Down(B::Left))).is_none());
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(a.review_list.dialog.height <= 12);
        let rect = a.review_list.dialog;
        let mut text = String::new();
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                text.push_str(terminal.backend().buffer()[(x, y)].symbol());
            }
        }
        assert!(text.contains("4 draft comments"));
        assert!(text.contains("Please guard"));
        assert!(!text.contains("useEffect"));
        let back = hit(&a, |a| matches!(a, Action::Back));
        a.mouse(click(back, E::Down(B::Left)));
        assert_eq!(a.review_event, 2);
        assert_eq!(a.editor, "Please guard the empty-list boundary.");
        a.key(KeyEvent::new(K::Enter, M::NONE));
        terminal.draw(|f| render(f, &mut a)).unwrap();
        let publish = hit(&a, |a| matches!(a, Action::Publish));
        assert!(a.mouse(click(publish, E::Down(B::Left))).is_none());
        let request = a.mouse(click(publish, E::Up(B::Left))).unwrap();
        assert_eq!(request["operation"], "submit");
        assert_eq!(request["event"], "REQUEST_CHANGES");
        assert!(a.mouse(click(publish, E::Up(B::Left))).is_none());
    }
    #[test]
    fn review_list_and_sheets_render_at_narrow_and_zero_sizes() {
        for (w, h) in [(150, 45), (58, 28), (32, 16), (12, 8), (1, 1), (0, 0)] {
            let mut a = review_flow_fixture();
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            for row in a.review_list.rows(a.my_comments()) {
                a.review_list.expanded.insert(row.key);
            }
            if let Some(row) = a
                .review_list
                .rows(a.my_comments())
                .into_iter()
                .find(|r| r.comment.is_some())
            {
                a.review_list.toggle(&row.key, true, Some(true));
            }
            terminal.draw(|f| render(f, &mut a)).unwrap();
            a.open_editor(Input::Review);
            a.editor = "Summary with Unicode café 界 and a long line.".into();
            a.editor_cursor = a.editor.len();
            terminal.draw(|f| render(f, &mut a)).unwrap();
            a.key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ));
            terminal.draw(|f| render(f, &mut a)).unwrap();
        }
    }

    #[test]
    fn review_hitboxes_disappear_with_the_hidden_pane() {
        let mut a = review_flow_fixture();
        a.panes.zoom = false;
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(150, 40)).unwrap();
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(!a.review_list.hits.is_empty());
        a.panes.panel = false;
        terminal.draw(|f| render(f, &mut a)).unwrap();
        assert!(a.review_list.hits.is_empty());
    }
}
