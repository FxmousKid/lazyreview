use crate::theme::current as theme;
use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
const KEYS: &[(&str, &str)] = &[
    ("Tab / Shift-Tab", "Next / previous pane"),
    ("j / k · gg / G", "Move in the focused pane"),
    (
        "h / l · Left / Right",
        "Scroll code; recursively fold/unfold the focused subtree",
    ),
    ("Ctrl-d / Ctrl-u", "Half-page through diff or companion"),
    (
        "+ / - · drag border",
        "Grow / shrink focused pane; drag a vertical divider to resize",
    ),
    (
        "\\",
        "Open pane controls; j/k choose, Enter folds/unfolds; border buttons work too",
    ),
    ("h/l (chooser)", "Focus repository / PR pane"),
    ("Z", "Maximize / restore focused pane"),
    (
        "=",
        "Fit file names and the reading column; keep folded panes",
    ),
    ("T", "Cycle themes: Lazygit / original blue / custom"),
    ("H", "Syntax palette: GitHub Dark / transparent Lunar"),
    (
        "D / C / g m / b",
        "Timeline top / first comment / first commit / inline threads",
    ),
    (
        "h/l (PR reading)",
        "Jump timeline entries; from threads return to the timeline",
    ),
    ("s", "Toggle diff layout: inline / side by side"),
    (
        "n / N (Files)",
        "Next / previous file; skip folders and reveal folded files",
    ),
    ("J / K · ]f / [f", "Next / previous file"),
    ("]c / [c", "Next / previous hunk"),
    (
        "Enter (tree)",
        "Expand directory or open file; type headings stay open",
    ),
    (
        "1–5 / 0 (review)",
        "Filter Code / Tests / Docs / Config / Assets / all types",
    ),
    ("/ (Files)", "Filter file paths"),
    (
        "/ (Changes)",
        "Search diff; Tab cycles Both / Original / Modified",
    ),
    (
        "n / N (Changes)",
        "Next / previous search match; without a search, next / previous file",
    ),
    (
        "o / u / z / x",
        "Sort files / unviewed only / show hidden / hide file",
    ),
    (
        "v then j/k",
        "Select a line range on the same side and hunk",
    ),
    ("a / A", "Attach selected range or diff / whole file to AI"),
    ("[a / ]a · d", "Select / remove an AI attachment"),
    (
        "Drag (AI / timeline / threads)",
        "Select text; release copies, including while composing a question",
    ),
    (
        "v then h/j/k/l · y",
        "Select reading-pane text; y copies, Escape clears",
    ),
    (
        "t (Changes / AI)",
        "Add a temporary AI note to selected code or queued attachment",
    ),
    ("m", "Toggle GitHub viewed state"),
    (
        "Space (diff)",
        "Pin/unpin selected range; repeat across hunks or files",
    ),
    (
        "c",
        "Comment on pinned ranges; first is the anchor, others become links",
    ),
    (
        "r (review)",
        "Your review: collapsed files and comments; Enter or h/l unfolds/folds",
    ),
    ("S", "Review changes: Comment / Approve / Request changes"),
    ("p / b", "Cycle companion views / jump to review threads"),
    ("i", "Compose AI question, thread reply or general comment"),
    ("g a", "Open or set up your AI companion"),
    (
        "Ctrl-] (native CLI)",
        "Leave CLI input and focus previous visible pane",
    ),
    (
        "Ctrl-\\ (native CLI)",
        "Use companion controls without leaving its pane",
    ),
    (
        "i / Enter (native navigation)",
        "Return keyboard control to the AI CLI",
    ),
    (
        "g c",
        "Inspect queued code/notes; Enter edits, d removes, i asks",
    ),
    ("g v", "Choose companion style: Ledger / Paper / Studio"),
    ("g o", "Open saved PR artifacts; Enter previews in browser"),
    (
        "F (AI)",
        "Toggle whole PR / focused file context for the next question",
    ),
    (
        "o (AI / timeline)",
        "Conversation questions / PR timeline index; Enter jumps",
    ),
    (
        "[[ / ]] (AI / timeline)",
        "Previous / next question, heading or timeline entry",
    ),
    (
        "G (AI)",
        "Jump to latest message and resume following the stream",
    ),
    (
        "N / X / R (AI)",
        "New PR chat / stop / retry; Ctrl-\\ first in native mode",
    ),
    ("R (threads)", "Resolve / reopen thread"),
    ("f (review)", "Refresh current PR"),
    ("O / Ctrl-p", "Paste PR URL / return to repository chooser"),
    ("e", "Reopen cancelled composer"),
    (
        "Enter (editor)",
        "Save draft / send reply or chat; preview a final review",
    ),
    (
        "Tab / Shift-Tab (review editor)",
        "Choose outcome; own PR allows Comment only",
    ),
    (
        "P (Your review)",
        "Switch Drafts / Posted; AI notes stay separate",
    ),
    (
        "Enter / Escape (preview)",
        "Publish review once / return to composer with summary preserved",
    ),
    (
        "Alt-Enter / Ctrl-n",
        "New line in the composer; pastes preserve newlines",
    ),
    (
        "@ / # (editor)",
        "People / issue and PR suggestions; arrows choose, Tab/Enter accepts",
    ),
    ("/ (chooser)", "Open fuzzy repository and PR history search"),
    (
        "f (chooser)",
        "Filter loaded PRs / search GitHub repository names",
    ),
    (
        "Enter (chooser)",
        "Load repository's PRs / open selected PR",
    ),
    (
        "1–4 (chooser)",
        "My open-PR repositories / review requests / authored PRs / repository search",
    ),
    (
        "n / p (chooser)",
        "Next / previous result page in the focused pane",
    ),
    (
        "Tab / Alt-Left (search)",
        "Toggle PR authors / return to repositories",
    ),
    ("Ctrl-r (search)", "Refresh repository or PR history"),
    ("r / o / d (chooser)", "Refresh / paste PR URL / open demo"),
    ("? / g ? / g !", "Open keyboard shortcuts"),
    ("f (shortcuts)", "Filter shortcut keys and descriptions"),
    (
        "Escape",
        "Leave input, close overlay or return to current review",
    ),
    ("q", "Quit; ordinary text while typing"),
];
#[derive(Default)]
pub struct Help {
    pub open: bool,
    pub filtering: bool,
    pub filter: String,
    pub scroll: usize,
    max_scroll: usize,
}
impl Help {
    pub fn show(&mut self) {
        self.open = true;
        self.filtering = false;
        self.filter.clear();
        self.scroll = 0;
        self.max_scroll = 0;
    }
    pub fn matches(&self) -> Vec<(&str, &str)> {
        let q = self.filter.to_lowercase();
        KEYS.iter()
            .copied()
            .filter(|(k, d)| {
                q.split_whitespace()
                    .all(|word| format!("{k} {d}").to_lowercase().contains(word))
            })
            .collect()
    }
    pub fn key(&mut self, key: KeyEvent) {
        if key.code == K::Esc {
            self.open = false;
            self.filtering = false;
            return;
        }
        if self.filtering {
            match key.code {
                K::Enter => self.filtering = false,
                K::Backspace => {
                    self.filter.pop();
                    self.scroll = 0;
                }
                K::Char('u') if key.modifiers.contains(M::CONTROL) => {
                    self.filter.clear();
                    self.scroll = 0;
                }
                K::Char(c) if !key.modifiers.intersects(M::CONTROL | M::ALT) => {
                    if self.filter.len() < 200 {
                        self.filter.push(c);
                    }
                    self.scroll = 0;
                }
                _ => {}
            }
        } else {
            match key.code {
                K::Esc | K::Char('q' | '?') => self.open = false,
                K::Char('f' | '/') => self.filtering = true,
                K::Char('j') | K::Down => {
                    self.scroll = self.scroll.saturating_add(1).min(self.max_scroll)
                }
                K::Char('k') | K::Up => self.scroll = self.scroll.saturating_sub(1),
                K::Home => self.scroll = 0,
                _ => {}
            }
        }
    }
    pub fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        let width = area.width.saturating_sub(4).min(108);
        let height = area.height.saturating_sub(2);
        let rect = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        f.render_widget(Clear, rect);
        let block = Block::default()
            .title(" Keyboard shortcuts ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(
                ratatui::style::Style::default()
                    .bg(theme().bg)
                    .fg(theme().fg),
            )
            .border_style(if self.filtering {
                theme().searching
            } else {
                theme().active
            });
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);
        let query = if self.filter.is_empty() && !self.filtering {
            "All shortcuts".into()
        } else {
            format!("{}{}", self.filter, if self.filtering { "▏" } else { "" })
        };
        f.render_widget(
            Paragraph::new(format!("⌕ {query}")).style(Style::default().fg(theme().accent)),
            rows[0],
        );
        let lines = shortcut_lines(&self.matches(), rows[2].width);
        self.max_scroll = lines.len().saturating_sub(rows[2].height as usize);
        self.scroll = self.scroll.min(self.max_scroll);
        f.render_widget(
            Paragraph::new(lines)
                .scroll((self.scroll.min(u16::MAX as usize) as u16, 0))
                .style(Style::default().fg(theme().fg)),
            rows[2],
        );
        f.render_widget(
            Paragraph::new("j/k Scroll · f Filter · Esc Close")
                .style(Style::default().fg(theme().muted)),
            rows[3],
        );
    }
}
// Keep wrapped descriptions under their own column instead of under the key.
// Compact terminals stack each key above its description, preserving every binding.
fn shortcut_lines(matches: &[(&str, &str)], width: u16) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![];
    }
    if matches.is_empty() {
        return crate::markdown::wrap(&[Line::from("No matching shortcuts.")], width);
    }
    let stacked = width < 76;
    let key_width = matches
        .iter()
        .map(|(key, _)| Line::from(*key).width())
        .max()
        .unwrap_or(0);
    let indent = if stacked {
        2.min(width as usize - 1)
    } else {
        key_width + 2
    };
    let description_width = width.saturating_sub(indent as u16).max(1);
    let mut lines = vec![];
    for (key, description) in matches {
        if stacked {
            lines.extend(crate::markdown::wrap(
                &[Line::styled(
                    key.to_string(),
                    Style::default()
                        .fg(theme().accent)
                        .add_modifier(Modifier::BOLD),
                )],
                width,
            ));
        }
        for (i, line) in
            crate::markdown::wrap(&[Line::from(description.to_string())], description_width)
                .into_iter()
                .enumerate()
        {
            let mut spans = if !stacked && i == 0 {
                vec![
                    Span::styled(
                        key.to_string(),
                        Style::default()
                            .fg(theme().accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ".repeat(indent - Line::from(*key).width())),
                ]
            } else {
                vec![Span::raw(" ".repeat(indent))]
            };
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        }
        if stacked {
            lines.push(Line::from(""));
        }
    }
    lines
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_matches_keys_and_actions_and_never_triggers_them() {
        let mut h = Help::default();
        h.show();
        h.key(KeyEvent::new(K::Char('f'), M::NONE));
        for c in "side by side".chars() {
            h.key(KeyEvent::new(K::Char(c), M::NONE));
        }
        assert_eq!(h.matches().len(), 1);
        assert_eq!(h.matches()[0].0, "s");
        assert!(h.open);
        h.key(KeyEvent::new(K::Esc, M::NONE));
        assert!(!h.open);
        assert!(!h.filtering);
    }
    #[test]
    fn wrapped_shortcuts_remain_readable_and_scroll_to_the_last_binding() {
        use ratatui::{Terminal, backend::TestBackend};
        for width in [42, 80, 120] {
            let mut help = Help::default();
            help.show();
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            terminal.draw(|f| help.render(f)).unwrap();
            for _ in 0..1000 {
                help.key(KeyEvent::new(K::Char('j'), M::NONE));
            }
            terminal.draw(|f| help.render(f)).unwrap();
            let screen = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                screen.contains("Quit; ordinary text while typing"),
                "width {width}"
            );
            assert_eq!(help.scroll, help.max_scroll);
            // A live filter must reset a scrolled view and expose its matching key.
            help.key(KeyEvent::new(K::Char('f'), M::NONE));
            for c in "side by side".chars() {
                help.key(KeyEvent::new(K::Char(c), M::NONE));
            }
            terminal.draw(|f| help.render(f)).unwrap();
            assert_eq!(help.scroll, 0);
            assert_eq!(
                help.matches(),
                vec![("s", "Toggle diff layout: inline / side by side")]
            );
        }
    }
}
