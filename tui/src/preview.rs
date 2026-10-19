//! Explicit, synthetic UI fixtures and a gallery of actual Ratatui cells.
use crate::{
    app::{App, Modal, Pane, Panel},
    reader::Appearance,
    ui,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use serde_json::{Value, json};
use std::io;

pub fn seed(a: &mut App, view: &str) {
    a.pr["body"] = json!(
        r#"## Why

The command menu works with a mouse, but keyboard users cannot move between results or reliably choose a command.

## What changed

Extract keyboard navigation into a hook and keep the active result aligned with the filtered list.

- Arrow keys move through matching commands.
- Enter runs the active command and closes the menu.
- [x] Reset selection when the query changes.
- [ ] Verify the empty-results state with a screen reader.

## Implementation

```tsx
const results = filterCommands(commands, query);
useEffect(() => reset(), [query, reset]);
```

> Focus stays on the search input. The active result is announced through aria-activedescendant.

## How to review

Start with the navigation hook, then follow the selection into the command runner. Check that **filtering**, **selection** and **announced focus** agree.

## Risks

A shorter result list could leave the active index out of bounds. An unstable reset callback could also rerun the effect on every render.

## Verification

- Filter from several results down to one.
- Search for a query with no results, then press Enter.
- Open the menu again and confirm selection resets.

## Rollout

No API or configuration changes. Existing mouse selection remains available.
"#
    );
    a.pr["title"] = json!("Make the command menu work from the keyboard");
    a.messages = vec![
        json!({"role":"user","content":"Why reset the active result whenever the query changes?"}),
        json!({"role":"assistant","content":"The active index belongs to the **current result list**. After filtering, the same index may point at a different command—or no command at all.\n\nResetting selection keeps the highlighted row aligned with what Enter will actually run.\n\n```tsx\nuseEffect(() => {\n  reset();\n}, [query, reset]);\n```"}),
        json!({"role":"user","content":"What would you look at first in this diff?"}),
        json!({"role":"assistant","content":"Start with the **selection boundary**.\n\n1. Filter down to zero results: does Enter do nothing?\n2. Narrow a long list: does the active index stay valid?\n3. Check `reset`: is the callback stable across renders?\n\nThen compare the highlighted row with `aria-activedescendant`. Visual selection and the screen reader should describe the same command."}),
    ];
    if view == "artifacts" {
        a.ai.artifacts = vec![
            json!({"id":"sample-diagram","kind":"svg","title":"Command selection flow","summary":"From a search query to the command that Enter runs.","base":a.pr["base"],"head":a.pr["head"],"created":"2026-10-17"}),
            json!({"id":"sample-page","kind":"html","title":"A reviewer’s guide to this PR","summary":"Purpose, design choices and a small verification plan.","base":a.pr["base"],"head":a.pr["head"],"created":"2026-10-17"}),
        ];
        a.modal = Some(Modal::Artifacts);
    }
    a.ai.follow = false;
    a.panel_scroll = 0;
    a.panel = if view == "description" {
        Panel::Overview
    } else {
        Panel::Ai
    };
    a.pane = Pane::Panel;
    a.panes.panel = true;
    a.panes.right_width = 56;
    a.panes.left_width = 28;
    a.status = "UI preview · synthetic conversation".into();
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn color(c: Color, foreground: bool) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Reset => if foreground { "#e6edf3" } else { "#0d1117" }.into(),
        Color::Indexed(i) => {
            let rgb = if i < 16 {
                let colors = [
                    0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5,
                    0x7f7f7f, 0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff,
                ];
                colors[i as usize]
            } else if i >= 232 {
                let n = 8 + (i as u32 - 232) * 10;
                n * 0x010101
            } else {
                let n = i - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v as u32 };
                (level(n / 36) << 16) | (level((n % 36) / 6) << 8) | level(n % 6)
            };
            format!("#{rgb:06x}")
        }
        _ => {
            let names = [
                (Color::Black, "#000000"),
                (Color::Red, "#cd0000"),
                (Color::Green, "#00cd00"),
                (Color::Yellow, "#cdcd00"),
                (Color::Blue, "#0000ee"),
                (Color::Magenta, "#cd00cd"),
                (Color::Cyan, "#00cdcd"),
                (Color::Gray, "#e5e5e5"),
                (Color::DarkGray, "#7f7f7f"),
                (Color::White, "#ffffff"),
            ];
            names
                .iter()
                .find(|(v, _)| *v == c)
                .map(|(_, s)| *s)
                .unwrap_or("#c9d1d9")
                .into()
        }
    }
}
pub fn gallery(path: &str, sample: Value) -> io::Result<()> {
    let mut source = App::new(sample.clone());
    if let Ok(mut syntax) = crate::syntax::Service::start() {
        for _ in 0..100 {
            syntax.tick(&mut source);
            if source.syntax.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    let mut renders = String::new();
    for appearance in Appearance::ALL {
        for view in ["ai", "description", "artifacts"] {
            for zoom in [false, true] {
                let (width, height) = if zoom { (100, 38) } else { (150, 42) };
                let mut app = App::new(sample.clone());
                seed(&mut app, view);
                app.syntax = source.syntax.clone();
                app.reader.appearance = appearance;
                app.panes.zoom = zoom;
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                render_settled(&mut terminal, &mut app)?;
                let buffer = terminal.backend().buffer();
                renders.push_str(&format!(
                    "<pre data-style='{}' data-view='{view}' data-size='{}' hidden>",
                    appearance.name().to_lowercase(),
                    if zoom { "focus" } else { "workspace" }
                ));
                for y in 0..height {
                    let mut x = 0;
                    while x < width {
                        let cell = &buffer[(x, y)];
                        let (mut fg, mut bg) = (cell.fg, cell.bg);
                        if cell.modifier.contains(Modifier::REVERSED) {
                            std::mem::swap(&mut fg, &mut bg);
                        }
                        let extra = if cell.modifier.contains(Modifier::BOLD) {
                            "font-weight:700;"
                        } else {
                            ""
                        };
                        let mut content = String::new();
                        loop {
                            let current = &buffer[(x, y)];
                            content.push_str(current.symbol());
                            let columns =
                                ratatui::text::Span::raw(current.symbol()).width().max(1) as u16;
                            x = x.saturating_add(columns);
                            if x >= width {
                                break;
                            }
                            let next = &buffer[(x, y)];
                            if next.fg != cell.fg
                                || next.bg != cell.bg
                                || next.modifier != cell.modifier
                            {
                                break;
                            }
                        }
                        renders.push_str(&format!(
                            "<span style='color:{};background:{};{extra}'>{}</span>",
                            color(fg, true),
                            color(bg, false),
                            escape(&content)
                        ));
                    }
                    renders.push('\n');
                }
                renders.push_str("</pre>");
            }
        }
    }
    let template = include_str!("preview.html");
    std::fs::write(path, template.replace("<!-- RENDERS -->", &renders))
}

/// Artifact export waits for async colors; interactive rendering never waits.
pub(crate) fn render_settled(
    terminal: &mut Terminal<TestBackend>,
    app: &mut App,
) -> io::Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let before = crate::markdown::revision();
        terminal.draw(|frame| ui::render(frame, app)).unwrap();
        if !crate::markdown::pending() && before == crate::markdown::revision() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Code colors did not settle for export",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[cfg(test)]
pub fn buffer_html(buffer: &ratatui::buffer::Buffer) -> String {
    buffer_html_with_palette(buffer, &Value::Null)
}

/// Terminal defaults are resolved separately from the application's truecolor theme.
pub fn buffer_html_with_palette(buffer: &ratatui::buffer::Buffer, profile: &Value) -> String {
    let resolve = |c, foreground| terminal_color(c, foreground, profile);
    let mut html = String::new();
    for y in buffer.area.y..buffer.area.bottom() {
        let mut x = buffer.area.x;
        while x < buffer.area.right() {
            let cell = &buffer[(x, y)];
            let (mut fg, mut bg) = (resolve(cell.fg, true), resolve(cell.bg, false));
            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let mut extra = String::new();
            for (modifier, css) in [
                (Modifier::BOLD, "font-weight:700;"),
                (Modifier::ITALIC, "font-style:italic;"),
                (Modifier::UNDERLINED, "text-decoration:underline;"),
            ] {
                if cell.modifier.contains(modifier) {
                    extra.push_str(css);
                }
            }
            let mut content = String::new();
            loop {
                let current = &buffer[(x, y)];
                content.push_str(current.symbol());
                x += ratatui::text::Span::raw(current.symbol()).width().max(1) as u16;
                if x >= buffer.area.right() {
                    break;
                }
                let next = &buffer[(x, y)];
                if next.fg != cell.fg || next.bg != cell.bg || next.modifier != cell.modifier {
                    break;
                }
            }
            html.push_str(&format!(
                "<span style='color:{};background:{};{}'>{}</span>",
                fg,
                bg,
                extra,
                escape(&content)
            ));
        }
        html.push('\n');
    }
    html
}

fn terminal_color(c: Color, foreground: bool, profile: &Value) -> String {
    let colors = &profile["colors"];
    let index = match c {
        Color::Indexed(i) if i < 16 => Some(i as usize),
        Color::Black => Some(0),
        Color::Red => Some(1),
        Color::Green => Some(2),
        Color::Yellow => Some(3),
        Color::Blue => Some(4),
        Color::Magenta => Some(5),
        Color::Cyan => Some(6),
        Color::Gray => Some(7),
        Color::DarkGray => Some(8),
        Color::LightRed => Some(9),
        Color::LightGreen => Some(10),
        Color::LightYellow => Some(11),
        Color::LightBlue => Some(12),
        Color::LightMagenta => Some(13),
        Color::LightCyan => Some(14),
        Color::White => Some(15),
        _ => None,
    };
    let value = if c == Color::Reset {
        &colors[if foreground {
            "foreground"
        } else {
            "background"
        }]
    } else if let Some(i) = index {
        &colors["ansi"][i]
    } else {
        &Value::Null
    };
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| color(c, foreground))
}

#[cfg(test)]
mod terminal_profile_tests {
    use super::*;
    #[test]
    fn defaults_ansi_truecolor_and_reversed_cells_keep_their_meaning() {
        let profile: Value =
            serde_json::from_str(include_str!("../../demo/terminal-profile.json")).unwrap();
        assert_eq!(terminal_color(Color::Reset, false, &profile), "#090300");
        assert_eq!(terminal_color(Color::Reset, true, &profile), "#a5a2a2");
        assert_eq!(terminal_color(Color::Indexed(1), true, &profile), "#db2d20");
        assert_eq!(terminal_color(Color::LightCyan, true, &profile), "#cdab53");
        assert_eq!(
            terminal_color(Color::Rgb(13, 17, 23), false, &profile),
            "#0d1117"
        );
        let mut b = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 1, 1));
        b[(0, 0)].set_symbol("x").set_style(
            ratatui::style::Style::default()
                .add_modifier(Modifier::REVERSED | Modifier::ITALIC | Modifier::UNDERLINED),
        );
        let html = buffer_html_with_palette(&b, &profile);
        assert!(html.contains("color:#090300;background:#a5a2a2;"));
        assert!(html.contains("font-style:italic;text-decoration:underline;"));
    }
}
