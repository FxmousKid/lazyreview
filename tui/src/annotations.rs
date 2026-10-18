//! One snapshot for file badges and diff gutters. Only authored GitHub comments
//! and queued AI notes count; neither selections nor bare attachments count.
use crate::app::{App, array, s};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub github: usize,
    pub ai: usize,
}
impl Counts {
    pub fn width(self) -> usize {
        let github = if self.github > 0 {
            1 + self.github.to_string().len()
        } else {
            0
        };
        let ai = if self.ai > 0 {
            1 + self.ai.to_string().len()
        } else {
            0
        };
        github + ai + usize::from(github + ai > 0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Left,
    Right,
}
impl Side {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "LEFT" => Some(Self::Left),
            "RIGHT" => Some(Self::Right),
            _ => None,
        }
    }
    fn key(self) -> &'static str {
        if self == Self::Left { "old" } else { "new" }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Github,
    Ai,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Range {
    kind: Kind,
    side: Side,
    start: u64,
    end: u64,
    hunk: Value,
}
#[derive(Default)]
pub struct Annotations {
    counts: HashMap<String, Counts>,
    ranges: Vec<Range>,
    marks: HashMap<(Side, u64), (Value, [char; 2])>,
}
impl App {
    pub fn annotations(&self) -> Annotations {
        Annotations::new(self)
    }
    pub fn annotation_gutter_width(&self) -> usize {
        self.annotations().gutter_width()
    }
}
impl Annotations {
    fn new(app: &App) -> Self {
        let mut result = Self::default();
        let files = app.files();
        // Current paths win over previous names if two changed files collide.
        let mut paths: HashMap<&str, &str> = files
            .iter()
            .map(|f| (s(f, "filename"), s(f, "filename")))
            .collect();
        for file in files {
            let previous = s(file, "previous_filename");
            if !previous.is_empty() {
                paths.entry(previous).or_insert(s(file, "filename"));
            }
        }
        let current = app.current();
        let anchors = Anchors::new(current);
        let viewer = if app.is_demo() {
            "you"
        } else {
            s(&app.pr, "viewer")
        };
        let mut seen = HashSet::new();
        let mut threads: Vec<_> = array(&app.pr, "threads").iter().enumerate().collect();
        // Prefer a current mapping when the same comment is repeated in an
        // outdated thread. IDs, rather than thread counts, define badge totals.
        threads.sort_by_key(|(_, t)| t["outdated"] == true || t["line"].is_null());
        for (thread_index, thread) in threads {
            let Some(&path) = paths.get(s(thread, "path")) else {
                continue;
            };
            let mut authored = false;
            for (comment_index, comment) in array(thread, "comments").iter().enumerate() {
                if viewer.is_empty() || s(comment, "author") != viewer {
                    continue;
                }
                let key = identity(
                    comment,
                    &format!(
                        "thread:{}:{}:{comment_index}",
                        s(thread, "id"),
                        if thread["id"].is_null() {
                            thread_index
                        } else {
                            0
                        }
                    ),
                );
                if !seen.insert(key) {
                    continue;
                }
                result.counts.entry(path.to_owned()).or_default().github += 1;
                authored = true;
            }
            if !authored || path != s(current, "filename") || thread["outdated"] == true {
                continue;
            }
            // Do not resurrect originalLine, old commit positions, or a stale
            // snapshot's coordinates. GitHub supplies the current mapped line.
            if thread
                .get("head")
                .is_some_and(|head| head != &app.pr["head"])
            {
                continue;
            }
            let Some(side) = Side::parse(s(thread, "side")) else {
                continue;
            };
            if thread
                .get("startSide")
                .and_then(Value::as_str)
                .is_some_and(|value| Side::parse(value) != Some(side))
            {
                continue;
            }
            let Some(end) = thread["line"].as_u64() else {
                continue;
            };
            let start = thread["startLine"].as_u64().unwrap_or(end);
            if let Some(range) = anchors.range(Kind::Github, side, start, end) {
                result.ranges.push(range);
            }
        }
        seen.clear();
        // Editing replaces an item in place. In a duplicate snapshot, the last
        // version wins, including an empty note (which removes its annotation).
        for (index, item) in app.context.iter().enumerate().rev() {
            if !seen.insert(identity(item, &format!("context:{index}"))) {
                continue;
            }
            if s(item, "note").trim().is_empty() {
                continue;
            }
            let Some(side) = Side::parse(s(item, "side")) else {
                continue;
            };
            let commit = if side == Side::Left {
                s(&app.pr, "base")
            } else {
                s(&app.pr, "head")
            };
            if commit.is_empty() || s(item, "commit") != commit {
                continue;
            }
            let Some(&path) = paths.get(s(item, "path")) else {
                continue;
            };
            result.counts.entry(path.to_owned()).or_default().ai += 1;
            // Whole-file/diff notes have a badge, but no invented line anchor.
            if path != s(current, "filename") || s(item, "kind") != "lines" {
                continue;
            }
            if let (Some(start), Some(end)) = (item["start"].as_u64(), item["end"].as_u64()) {
                if let Some(range) = anchors.range(Kind::Ai, side, start, end) {
                    result.ranges.push(range);
                }
            }
        }
        result.merge_overlaps();
        result.index_marks();
        result
    }
    pub fn counts(&self, path: &str) -> Counts {
        self.counts.get(path).copied().unwrap_or_default()
    }
    /// Two consistent lanes: GitHub, then AI. Unannotated files keep their space.
    pub fn gutter_width(&self) -> usize {
        if self.ranges.is_empty() { 0 } else { 2 }
    }
    pub fn markers(&self, line: &Value, side: Option<Side>) -> [char; 2] {
        if s(line, "kind") == "hunk" {
            return [' '; 2];
        }
        let mut glyphs = [' '; 2];
        for current_side in [Side::Left, Side::Right] {
            if side.is_some_and(|side| side != current_side) {
                continue;
            }
            let Some(n) = line[current_side.key()].as_u64() else {
                continue;
            };
            let Some((hunk, next)) = self.marks.get(&(current_side, n)) else {
                continue;
            };
            if *hunk != line["hunk"] {
                continue;
            }
            for i in 0..2 {
                if next[i] == ' ' {
                    continue;
                }
                // A context row can belong to both sides. Their union gets a
                // continuous stem rather than a misleading start/end cap.
                glyphs[i] = if glyphs[i] == ' ' || glyphs[i] == next[i] {
                    next[i]
                } else {
                    '│'
                };
            }
        }
        glyphs
    }
    fn index_marks(&mut self) {
        // Ranges of each kind/side are merged first: expansion is bounded by
        // the selected patch's line count, never comments × rendered rows.
        for range in &self.ranges {
            let lane = if range.kind == Kind::Github { 0 } else { 1 };
            for n in range.start..=range.end {
                let glyph = if range.start == range.end {
                    '╶'
                } else if n == range.start {
                    '╭'
                } else if n == range.end {
                    '╰'
                } else {
                    '│'
                };
                let entry = self
                    .marks
                    .entry((range.side, n))
                    .or_insert_with(|| (range.hunk.clone(), [' '; 2]));
                entry.1[lane] = glyph;
            }
        }
    }
    fn merge_overlaps(&mut self) {
        self.ranges
            .sort_by_key(|r| (r.kind, r.side, r.hunk.as_u64().unwrap_or(0), r.start, r.end));
        let mut merged: Vec<Range> = Vec::new();
        for range in self.ranges.drain(..) {
            if let Some(previous) = merged.last_mut() {
                if previous.kind == range.kind
                    && previous.side == range.side
                    && previous.hunk == range.hunk
                    && range.start <= previous.end
                {
                    previous.end = previous.end.max(range.end);
                    continue;
                }
            }
            merged.push(range);
        }
        self.ranges = merged;
    }
}
fn identity(value: &Value, fallback: &str) -> String {
    match &value["id"] {
        Value::String(id) if !id.is_empty() => id.clone(),
        Value::Number(id) => id.to_string(),
        _ => fallback.to_owned(),
    }
}
// At each returned line, record the final consecutive line in the same hunk.
// This validates every anchor in O(1), including a thousand overlapping threads.
struct Anchors(HashMap<(Side, u64), (u64, Value)>);
impl Anchors {
    fn new(file: &Value) -> Self {
        let mut index = HashMap::new();
        for side in [Side::Left, Side::Right] {
            let mut next: Option<(u64, u64, Value)> = None;
            for line in array(file, "lines").iter().rev() {
                let Some(n) = line[side.key()].as_u64() else {
                    continue;
                };
                let end = match &next {
                    Some((next_n, end, hunk))
                        if n.checked_add(1) == Some(*next_n) && *hunk == line["hunk"] =>
                    {
                        *end
                    }
                    _ => n,
                };
                index.insert((side, n), (end, line["hunk"].clone()));
                next = Some((n, end, line["hunk"].clone()));
            }
        }
        Self(index)
    }
    fn range(&self, kind: Kind, side: Side, start: u64, end: u64) -> Option<Range> {
        if start == 0 || end < start {
            return None;
        }
        let (available, hunk) = self.0.get(&(side, start))?;
        (end <= *available).then(|| Range {
            kind,
            side,
            start,
            end,
            hunk: hunk.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> App {
        let mut app = App::new(
            json!({"demo":true,"head":"head","base":"base","url":"sample","title":"Keep comments beside the code","owner":"acme","repo":"atlas","number":128,"files":[{
            "filename":"src/new.rs","previous_filename":"src/old.rs","status":"renamed","category":"Code","additions":2,"deletions":2,
            "lines":[{"kind":"hunk","hunk":0,"text":"@@ -1,4 +1,4 @@"},
                {"kind":"context","old":1,"new":1,"hunk":0,"text":"fn review() {"},
                {"kind":"delete","old":2,"hunk":0,"text":"    old();"},
                {"kind":"delete","old":3,"hunk":0,"text":"    legacy();"},
                {"kind":"add","new":2,"hunk":0,"text":"    new();"},
                {"kind":"add","new":3,"hunk":0,"text":"    safe();"},
                {"kind":"context","old":4,"new":4,"hunk":0,"text":"}"},
                {"kind":"hunk","hunk":1,"text":"@@ -20 +20 @@"},
                {"kind":"context","old":20,"new":20,"hunk":1,"text":"tail();"}],
            "splitRows":[]
        }],"threads":[],"conversation":[]}),
        );
        let lines = app.lines();
        let rows = json!([
            {"hunk":lines[0]}, {"left":lines[1],"right":lines[1]},
            {"left":lines[2],"right":lines[4]}, {"left":lines[3],"right":lines[5]},
            {"left":lines[6],"right":lines[6]}, {"hunk":lines[7]},
            {"left":lines[8],"right":lines[8]}
        ]);
        app.pr["files"][0]["splitRows"] = rows;
        app
    }
    fn thread(id: u64, path: &str, side: &str, start: u64, end: u64) -> Value {
        json!({"id":format!("thread-{id}"),"path":path,"side":side,"startLine":start,"line":end,"comments":[{"id":id,"author":"you","body":"Comment"}]})
    }
    fn note(id: &str, side: &str, start: u64, end: u64) -> Value {
        json!({"id":id,"path":"src/new.rs","kind":"lines","side":side,"commit":if side=="LEFT" {"base"} else {"head"},"start":start,"end":end,"note":"Explain this","content":"Code"})
    }
    #[test]
    fn counts_authored_comments_once_and_only_queued_notes() {
        let mut a = fixture();
        let own = thread(1, "src/new.rs", "RIGHT", 2, 3);
        let mut other = thread(2, "src/new.rs", "RIGHT", 2, 3);
        other["comments"][0]["author"] = json!("someone-else");
        let mut reply = other.clone();
        reply["comments"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":3,"author":"you","body":"My reply"}));
        a.pr["threads"] = json!([own.clone(), own, other, reply]);
        let ai = note("one", "RIGHT", 2, 3);
        let mut bare = note("bare", "RIGHT", 2, 3);
        bare["note"] = json!("");
        a.context = vec![ai.clone(), ai, bare];
        a.ranges
            .push(json!({"path":"src/new.rs","side":"RIGHT","start":2,"end":3}));
        let annotations = a.annotations();
        assert_eq!(
            annotations.counts("src/new.rs"),
            Counts { github: 2, ai: 1 }
        );
        assert_eq!(annotations.counts("absent.rs"), Counts::default());
        assert_eq!(annotations.counts("src/new.rs").width(), 5);
    }
    #[test]
    fn stale_missing_cross_hunk_and_cross_side_anchors_never_mark_new_code() {
        let mut a = fixture();
        let mut outdated = thread(1, "src/new.rs", "RIGHT", 2, 3);
        outdated["outdated"] = json!(true);
        let mut missing = thread(2, "src/new.rs", "RIGHT", 2, 3);
        missing["line"] = Value::Null;
        missing["originalLine"] = json!(3);
        let mut cross = thread(3, "src/new.rs", "RIGHT", 1, 20);
        cross["startSide"] = json!("LEFT");
        let mut old_head = thread(4, "src/new.rs", "RIGHT", 2, 3);
        old_head["head"] = json!("old");
        a.pr["threads"] = json!([
            outdated,
            missing,
            cross,
            old_head,
            thread(5, "src/new.rs", "RIGHT", 3, 20)
        ]);
        let mut stale = note("old", "RIGHT", 2, 3);
        stale["commit"] = json!("old-head");
        a.context.push(stale);
        let annotations = a.annotations();
        assert_eq!(
            annotations.counts("src/new.rs"),
            Counts { github: 5, ai: 0 }
        );
        assert_eq!(annotations.gutter_width(), 0);
    }
    #[test]
    fn renamed_deleted_left_and_overlapping_right_ranges_keep_sides() {
        let mut a = fixture();
        a.pr["threads"] = json!([
            thread(1, "src/old.rs", "LEFT", 1, 3),
            thread(2, "src/new.rs", "RIGHT", 2, 3)
        ]);
        a.context = vec![note("first", "RIGHT", 1, 2), note("second", "RIGHT", 2, 4)];
        let annotations = a.annotations();
        assert_eq!(
            annotations.counts("src/new.rs"),
            Counts { github: 2, ai: 2 }
        );
        assert_eq!(
            annotations.markers(&a.lines()[1], Some(Side::Left)),
            ['╭', ' ']
        );
        assert_eq!(annotations.markers(&a.lines()[2], None), ['│', ' ']);
        assert_eq!(
            annotations.markers(&a.lines()[3], Some(Side::Left)),
            ['╰', ' ']
        );
        assert_eq!(
            annotations.markers(&a.lines()[4], Some(Side::Right)),
            ['╭', '│']
        );
        assert_eq!(annotations.markers(&a.lines()[5], None), ['╰', '│']);
        assert_eq!(
            annotations.markers(&a.lines()[6], Some(Side::Right)),
            [' ', '╰']
        );
        assert_eq!(annotations.markers(&a.lines()[0], None), [' ', ' ']);
        a.pr["files"][0]["status"] = json!("removed");
        a.pr["files"][0]["lines"] = json!([{"kind":"delete","old":1,"hunk":0,"text":"gone"}]);
        a.pr["threads"] = json!([thread(10, "src/new.rs", "LEFT", 1, 1)]);
        assert_eq!(
            a.annotations().markers(&a.lines()[0], Some(Side::Left)),
            ['╶', ' ']
        );
        assert_eq!(
            a.annotations().markers(&a.lines()[0], Some(Side::Right)),
            [' ', ' ']
        );
    }
    #[test]
    fn note_consumption_removal_edits_and_submission_update_without_cache() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut a = fixture();
        a.pr["threads"] = json!([thread(1, "src/new.rs", "RIGHT", 2, 3)]);
        a.pr["threads"][0]["pending"] = json!(true);
        a.context = vec![note("sent", "RIGHT", 1, 2), note("remove", "LEFT", 2, 3)];
        a.sent_context = vec![a.context[0].clone()];
        a.consume_context(&[json!("sent")]);
        assert_eq!(
            a.annotations().counts("src/new.rs"),
            Counts { github: 1, ai: 1 }
        );
        a.context_cursor = 0;
        a.modal = Some(crate::app::Modal::Context);
        a.key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(a.annotations().counts("src/new.rs").ai, 0);
        a.context.push(note("edit", "RIGHT", 2, 2));
        a.sent_context = a.context.clone();
        a.context[0]["note"] = json!("Edited while answering");
        a.consume_context(&[json!("edit")]);
        assert_eq!(a.annotations().counts("src/new.rs").ai, 1);
        let before = a.annotations().markers(&a.lines()[4], None);
        let mut submitted = a.pr.clone();
        submitted["threads"][0]["pending"] = json!(false);
        a.receive("submit", Ok(submitted));
        assert_eq!(a.annotations().counts("src/new.rs").github, 1);
        assert_eq!(a.annotations().markers(&a.lines()[4], None), before);
        let mut refreshed = a.pr.clone();
        refreshed["head"] = json!("new-head");
        refreshed["threads"][0]["outdated"] = json!(true);
        a.receive("load", Ok(refreshed));
        assert_eq!(
            a.annotations().counts("src/new.rs"),
            Counts { github: 1, ai: 0 }
        );
        assert_eq!(a.annotations().gutter_width(), 0);
    }

    fn illustrated() -> App {
        let mut a = fixture();
        a.pr["threads"] = json!([
            thread(1, "src/old.rs", "LEFT", 1, 3),
            thread(2, "src/new.rs", "RIGHT", 2, 3)
        ]);
        a.context = vec![note("first", "RIGHT", 1, 2), note("second", "RIGHT", 2, 4)];
        a.panes.panel = false;
        a.panes.left_width = 30;
        a.pane = crate::app::Pane::Diff;
        a.line = 2;
        a
    }
    #[test]
    fn fit_includes_badges_without_losing_long_names() {
        let mut a = fixture();
        let path = "src/longest_component_filename.rs";
        a.pr["files"][0]["filename"] = json!(path);
        a.fit_panes();
        let before = a.panes.left_width;
        a.pr["threads"] = json!([thread(1, path, "RIGHT", 2, 3)]);
        let mut ai = note("one", "RIGHT", 2, 3);
        ai["path"] = json!(path);
        a.context.push(ai);
        a.fit_panes();
        assert_eq!(a.panes.left_width, before + 5);
    }
    #[test]
    fn actual_ratatui_cells_preserve_counts_colors_and_split_brackets() {
        use ratatui::{Terminal, backend::TestBackend};
        crate::theme::set(crate::theme::Theme::blue());
        for width in [72, 100, 160] {
            for split in [false, true] {
                let mut a = illustrated();
                a.split = split;
                if width == 72 {
                    a.panes.left_width = 18;
                }
                let mut terminal = Terminal::new(TestBackend::new(width, 28)).unwrap();
                terminal
                    .draw(|frame| crate::ui::render(frame, &mut a))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let text = buffer
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(text.contains("●2◇2 "), "badges at {width}, split={split}");
                assert!(
                    text.contains("+2 −2"),
                    "statistics at {width}, split={split}"
                );
                let github = buffer
                    .content
                    .iter()
                    .find(|cell| cell.symbol() == "●")
                    .unwrap();
                let ai = buffer
                    .content
                    .iter()
                    .find(|cell| cell.symbol() == "◇")
                    .unwrap();
                assert_eq!(github.fg, crate::theme::current().accent);
                assert_ne!(github.fg, ai.fg);
                if width >= 100 {
                    assert!(text.contains("╭│    new();"));
                    assert!(text.contains("╰│    safe();"));
                }
            }
        }
    }
    #[test]
    #[ignore = "manual actual-Ratatui visual export; synthetic/offline"]
    fn export_annotation_gallery() {
        use ratatui::{Terminal, backend::TestBackend, style::Color};
        crate::theme::set(crate::theme::Theme::blue());
        let color = |c| match c {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => "#e6edf3".into(),
        };
        let mut html = String::from(
            "<!doctype html><meta charset=utf-8><title>LazyReview annotations</title><style>body{background:#0d1117;color:#e6edf3;font:14px monospace}pre{line-height:1.2}</style>",
        );
        let mut frames = vec![];
        for (title, width, split) in [
            ("Inline", 150, false),
            ("Side by side", 150, true),
            ("Narrow tree", 72, false),
        ] {
            let mut a = illustrated();
            a.split = split;
            if width == 72 {
                a.panes.left_width = 18;
            }
            let mut terminal = Terminal::new(TestBackend::new(width, 45)).unwrap();
            terminal
                .draw(|frame| crate::ui::render(frame, &mut a))
                .unwrap();
            let buffer = terminal.backend().buffer();
            html.push_str(&format!(
                "<h2>{title}</h2>{}",
                crate::preview::buffer_html(buffer)
            ));
            let cells:Vec<_> = buffer.content.iter().map(|cell|json!({"text":cell.symbol(),"fg":color(cell.fg),"bg":if cell.bg==Color::Reset {"#0d1117".into()} else {color(cell.bg)}})).collect();
            frames.push(json!({"title":title,"width":width,"height":45,"cells":cells}));
        }
        let output = std::env::var("LAZYREVIEW_ANNOTATION_EXPORT")
            .unwrap_or_else(|_| "/private/tmp/lazyreview-annotations-gallery".into());
        std::fs::write(format!("{output}.html"), html).unwrap();
        std::fs::write(
            format!("{output}.json"),
            serde_json::to_vec(&frames).unwrap(),
        )
        .unwrap();
        println!("Exported {output}.html and .json from actual Ratatui cells");
    }
    #[test]
    #[ignore = "manual annotation scaling benchmark; synthetic/offline"]
    fn benchmark_large_annotation_snapshot() {
        let mut a = fixture();
        let lines: Vec<_> = (1..=1000)
            .map(|n| json!({"kind":"context","old":n,"new":n,"hunk":0,"text":"let value = 1;"}))
            .collect();
        a.pr["files"][0]["lines"] = json!(lines);
        let mut files = vec![a.pr["files"][0].clone()];
        for i in 1..1000 {
            files.push(json!({"filename":format!("src/file-{i}.rs"),"lines":[]}));
        }
        a.pr["files"] = json!(files);
        a.pr["threads"] = json!(
            (1..=1000)
                .map(|n| thread(n, "src/new.rs", "RIGHT", n, n))
                .collect::<Vec<_>>()
        );
        let start = std::time::Instant::now();
        for _ in 0..20 {
            let annotations = a.annotations();
            assert_eq!(annotations.counts("src/new.rs").github, 1000);
            for line in a.lines() {
                assert_eq!(annotations.markers(line, Some(Side::Right)), ['╶', ' ']);
            }
        }
        println!(
            "1000 files / 1000 comments / 1000 marker lookups: {:?} mean",
            start.elapsed() / 20
        );
    }
}
