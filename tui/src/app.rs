use crate::{completion::Completion, help::Help, layout::Panes};
use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
use serde_json::{Value as V, json};
use std::collections::HashSet;
pub const CATEGORIES: [&str; 5] = ["Code", "Tests", "Docs", "Config", "Assets"];
pub fn s<'a>(v: &'a V, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
pub fn array<'a>(v: &'a V, key: &str) -> &'a [V] {
    v[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pane {
    Tree,
    Diff,
    Panel,
}
impl Pane {
    pub fn index(self) -> usize {
        match self {
            Self::Tree => 0,
            Self::Diff => 1,
            Self::Panel => 2,
        }
    }
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Tree,
            2 => Self::Panel,
            _ => Self::Diff,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Panel {
    Ai,
    Threads,
    Overview,
    Review,
}
#[derive(Clone, PartialEq, Debug)]
pub enum Row {
    Category(String),
    Directory(String, usize),
    File(usize, usize),
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Input {
    Filter,
    DiffSearch,
    Open,
    Comment,
    Review,
    Reply,
    Conversation,
    Chat,
    AiNote,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Modal {
    Help,
    AiSetup,
    Appearance,
    Outline,
    Artifacts,
    Context,
    Panes,
    Editor(Input),
    Confirm,
}
pub struct App {
    pub pr: V,
    pub panes: Panes,
    pub help: Help,
    pub pane: Pane,
    pub panel: Panel,
    pub category: Option<String>,
    pub filter: String,
    pub syntax: Option<std::sync::Arc<crate::syntax::FileSyntax>>,
    pub syntax_key: String,
    pub syntax_generation: u64,
    pub syntax_palette: crate::syntax::Palette,
    pub search: crate::search::Search,
    pub search_origin: Option<(crate::search::Search, usize, u16)>,
    pub review_list: crate::review_list::ReviewList,
    pub review_button: ratatui::layout::Rect,
    pub comments_button: ratatui::layout::Rect,
    pub sort: u8,
    pub hidden: HashSet<String>,
    pub collapsed: HashSet<String>,
    pub show_hidden: bool,
    pub unviewed: bool,
    pub tree_cursor: usize,
    pub file: usize,
    pub line: usize,
    pub diff_scroll: usize,
    pub horizontal: u16,
    pub split: bool,
    pub visual: Option<(usize, String)>,
    pub panel_cursor: usize,
    pub panel_scroll: usize,
    pub context_cursor: usize,
    pub copy_pending: Option<String>,
    pub context: Vec<V>,
    pub sent_context: Vec<V>,
    pub note_target: Option<V>,
    pub messages: Vec<V>,
    pub ai: crate::ai::State,
    pub reader: crate::reader::Reader,
    pub modal: Option<Modal>,
    pub editor: String,
    pub editor_cursor: usize,
    pub completion: Completion,
    pub ranges: Vec<V>,
    pub comment_ranges: Vec<V>,
    pub editor_head: V,
    pub editor_base: V,
    pub reply_id: V,
    pub pane_choice: usize,
    pub started: std::time::Instant,
    pub review_event: usize,
    pub pending: Option<V>,
    pub return_editor: Option<Input>,
    pub status: String,
    pub busy: bool,
    pub write_uncertain: bool,
    pub quit: bool,
    pub prefix: Option<(char, std::time::Instant)>,
    pub spinner: usize,
}
impl App {
    pub fn new(pr: V) -> Self {
        let initial_status = array(&pr, "warnings")
            .first()
            .and_then(V::as_str)
            .unwrap_or("")
            .to_string();
        Self {
            pr,
            panes: {
                let mut p = Panes::default();
                p.panel = false;
                p
            },
            help: Help::default(),
            pane: Pane::Tree,
            panel: Panel::Ai,
            category: Some("Code".into()),
            filter: String::new(),
            syntax: None,
            syntax_key: String::new(),
            syntax_generation: 0,
            syntax_palette: crate::syntax::Palette::default(),
            search: crate::search::Search::default(),
            search_origin: None,
            review_list: crate::review_list::ReviewList::default(),
            review_button: ratatui::layout::Rect::default(),
            comments_button: ratatui::layout::Rect::default(),
            sort: 0,
            hidden: HashSet::new(),
            collapsed: HashSet::new(),
            show_hidden: false,
            unviewed: false,
            tree_cursor: 0,
            file: 0,
            line: 1,
            diff_scroll: 0,
            horizontal: 0,
            split: false,
            visual: None,
            panel_cursor: 0,
            panel_scroll: 0,
            context_cursor: 0,
            copy_pending: None,
            context: vec![],
            sent_context: vec![],
            note_target: None,
            messages: vec![],
            ai: crate::ai::State::default(),
            reader: crate::reader::Reader::default(),
            modal: None,
            editor: String::new(),
            editor_cursor: 0,
            completion: Completion::default(),
            ranges: vec![],
            comment_ranges: vec![],
            editor_head: V::Null,
            editor_base: V::Null,
            reply_id: V::Null,
            pane_choice: 2,
            started: std::time::Instant::now(),
            review_event: 0,
            pending: None,
            return_editor: None,
            status: initial_status,
            busy: false,
            write_uncertain: false,
            quit: false,
            prefix: None,
            spinner: 0,
        }
    }
    pub fn files(&self) -> &[V] {
        array(&self.pr, "files")
    }
    pub fn current(&self) -> &V {
        self.files().get(self.file).unwrap_or(&V::Null)
    }
    pub fn lines(&self) -> &[V] {
        array(self.current(), "lines")
    }
    pub fn is_demo(&self) -> bool {
        self.pr["demo"] == true
    }
    pub fn visible(&self) -> Vec<usize> {
        let query = self.filter.to_lowercase();
        let mut indices: Vec<usize> = self
            .files()
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                self.category.as_ref().is_none_or(|c| s(f, "category") == c)
                    && (query.is_empty() || s(f, "filename").to_lowercase().contains(&query))
                    && (self.show_hidden || !self.hidden.contains(s(f, "filename")))
                    && (!self.unviewed || f["viewed"] != true)
            })
            .map(|(i, _)| i)
            .collect();
        match self.sort {
            1 => indices.sort_by_key(|i| s(&self.files()[*i], "filename")),
            2 => indices.sort_by_key(|i| {
                std::cmp::Reverse(
                    self.files()[*i]["additions"].as_u64().unwrap_or(0)
                        + self.files()[*i]["deletions"].as_u64().unwrap_or(0),
                )
            }),
            3 => indices.sort_by_key(|i| self.files()[*i]["viewed"] == true),
            _ => {}
        }
        indices
    }
    pub fn rows(&self) -> Vec<Row> {
        self.tree_rows(false)
    }
    fn tree_rows(&self, expanded: bool) -> Vec<Row> {
        let visible = self.visible();
        let mut rows = vec![];
        for category in CATEGORIES {
            if self.category.as_ref().is_some_and(|c| c != category) {
                continue;
            }
            rows.push(Row::Category(category.into()));
            let files: Vec<usize> = visible
                .iter()
                .copied()
                .filter(|i| s(&self.files()[*i], "category") == category)
                .collect();
            self.append_tree(&files, "", 0, expanded, &mut rows);
        }
        rows
    }
    fn append_tree(
        &self,
        files: &[usize],
        prefix: &str,
        depth: usize,
        expanded: bool,
        rows: &mut Vec<Row>,
    ) {
        let mut seen = HashSet::new();
        for &i in files {
            let path = s(&self.files()[i], "filename");
            let remaining = path.strip_prefix(prefix).unwrap_or(path);
            if let Some((dir, _)) = remaining.split_once('/') {
                let next = format!("{prefix}{dir}/");
                if !seen.insert(next.clone()) {
                    continue;
                }
                rows.push(Row::Directory(next.clone(), depth));
                if expanded || !self.collapsed.contains(&next) {
                    let children: Vec<usize> = files
                        .iter()
                        .copied()
                        .filter(|j| s(&self.files()[*j], "filename").starts_with(&next))
                        .collect();
                    self.append_tree(&children, &next, depth + 1, expanded, rows);
                }
            } else {
                rows.push(Row::File(i, depth));
            }
        }
    }
    pub fn pending_comments(&self) -> Vec<&V> {
        array(&self.pr, "threads")
            .iter()
            .flat_map(|t| {
                array(t, "comments")
                    .iter()
                    .filter(move |c| c["pending"] == true || t["pending"] == true)
            })
            .collect()
    }
    pub fn ensure(&mut self) {
        let visible = self.visible();
        if !visible.contains(&self.file) {
            if let Some(&i) = visible.first() {
                self.select_file(i);
            }
        }
        self.tree_cursor = self.tree_cursor.min(self.rows().len().saturating_sub(1));
        self.line = self.line.min(self.lines().len().saturating_sub(1));
        self.panel_cursor = self.panel_cursor.min(self.panel_len().saturating_sub(1));
    }
    pub fn select_file(&mut self, i: usize) {
        self.file = i;
        self.line = if array(self.current(), "lines").len() > 1 {
            1
        } else {
            0
        };
        self.diff_scroll = 0;
        self.visual = None;
        self.horizontal = 0;
        self.rebuild_search();
    }
    pub fn open_editor(&mut self, input: Input) {
        if input == Input::Chat {
            self.panel = Panel::Ai;
            self.pane = Pane::Panel;
            self.panes.panel = true;
        }
        self.status.clear();
        self.modal = Some(Modal::Editor(input));
        self.editor = if input == Input::Filter {
            self.filter.clone()
        } else if input == Input::Review {
            if !self.review_list.summary_loaded {
                self.review_list.summary = s(&self.pr["pendingReview"], "body").into();
                self.review_list.summary_loaded = true;
            }
            if self.own_pr() {
                self.review_event = 0;
            }
            self.review_list.summary.clone()
        } else {
            String::new()
        };
        self.editor_cursor = self.editor.len();
        self.completion.reset();
        self.editor_head = self.pr["head"].clone();
        self.editor_base = self.pr["base"].clone();
        self.status.clear();
        if input == Input::Comment {
            self.comment_ranges = if self.ranges.is_empty() {
                self.selection().into_iter().collect()
            } else {
                self.ranges.clone()
            };
        }
        if input == Input::Reply {
            self.reply_id = array(self.selected_thread(), "comments")
                .first()
                .map(|c| c["id"].clone())
                .unwrap_or(V::Null);
        }
    }
    pub fn my_comments(&self) -> Vec<V> {
        let viewer = if self.is_demo() {
            "you"
        } else {
            s(&self.pr, "viewer")
        };
        let mut comments = vec![];
        for thread in array(&self.pr, "threads") {
            for c in array(thread, "comments") {
                if s(c, "author") == viewer {
                    let mut c = c.clone();
                    c["reviewKey"] = json!(format!("line:{}", c["id"]));
                    c["startLine"] = thread["startLine"].clone();
                    c["path"] = thread["path"].clone();
                    c["line"] = thread["line"].clone();
                    c["pending"] = json!(c["pending"] == true || thread["pending"] == true);
                    comments.push(c);
                }
            }
        }
        for c in array(&self.pr, "conversation") {
            if s(c, "author") == viewer {
                let mut c = c.clone();
                c["reviewKey"] = json!(format!("conversation:{}", c["id"]));
                comments.push(c);
            }
        }
        for c in array(&self.pr, "reviews") {
            if s(c, "author") == viewer
                && !s(c, "body").trim().is_empty()
                && s(c, "state") != "PENDING"
            {
                let mut c = c.clone();
                c["reviewKey"] = json!(format!("review:{}", c["id"]));
                c["pending"] = json!(false);
                comments.push(c);
            }
        }
        comments.sort_by_key(|c| c["pending"] != true);
        comments
    }
    fn show_review(&mut self) {
        self.review_list.follow = true;
        self.review_list.posted = false;
        self.panel = Panel::Review;
        self.panes.panel = true;
        self.panes.zoom = false;
        self.pane = Pane::Panel;
        self.panel_scroll = 0;
        self.panel_cursor = 0;
    }
    fn fold_subtree(&mut self, expand: bool) {
        let row = self.rows().get(self.tree_cursor).cloned();
        let prefix = match &row {
            Some(Row::Directory(path, _)) => path.clone(),
            Some(Row::File(i, _)) => s(&self.files()[*i], "filename")
                .rsplit_once('/')
                .map(|(p, _)| format!("{p}/"))
                .unwrap_or_default(),
            Some(Row::Category(_)) => String::new(),
            _ => return,
        };
        let category = match &row {
            Some(Row::Category(c)) => Some(c.clone()),
            _ => None,
        };
        let paths: Vec<String> = self
            .files()
            .iter()
            .filter(|f| {
                s(f, "filename").starts_with(&prefix)
                    && category.as_ref().is_none_or(|c| s(f, "category") == c)
            })
            .map(|f| s(f, "filename").into())
            .collect();
        for path in paths {
            for (i, c) in path.char_indices() {
                if c == '/' {
                    let dir = &path[..=i];
                    if dir.starts_with(&prefix) {
                        if expand {
                            self.collapsed.remove(dir);
                        } else {
                            self.collapsed.insert(dir.into());
                        }
                    }
                }
            }
        }
        if let Some(index) = self
            .rows()
            .iter()
            .position(|r| matches!(r,Row::Directory(p,_) if p==&prefix))
        {
            self.tree_cursor = index;
        }
        self.tree_cursor = self.tree_cursor.min(self.rows().len().saturating_sub(1));
    }
    fn snapshot(&self, operation: &str) -> V {
        json!({"operation":operation,"expectedHead":self.pr["head"],"expectedBase":self.pr["base"]})
    }
    pub fn paste(&mut self, text: &str) {
        if let Some(Modal::Editor(input)) = self.modal {
            let text = crate::ui::clean(text).replace("\r\n", "\n");
            let text = if matches!(input, Input::Filter | Input::DiffSearch | Input::Open) {
                text.replace('\n', "")
            } else {
                text
            };
            let limit = if input == Input::Review {
                240_000
            } else {
                10_000
            };
            if self.editor.len() + text.len() <= limit {
                self.editor.insert_str(self.editor_cursor, &text);
                self.editor_cursor += text.len();
                self.completion.reset();
                if input == Input::DiffSearch {
                    self.update_search();
                }
            }
        }
    }
    fn send(&mut self, value: V) -> Option<V> {
        if self.write_uncertain
            && matches!(
                s(&value, "operation"),
                "draft" | "submit" | "reply" | "resolve" | "viewed" | "conversation"
            )
        {
            self.status =
                "Refresh first; the previous action may already be saved on GitHub.".into();
            return None;
        }
        if self.busy {
            self.status = "A request is already running.".into();
            None
        } else {
            self.busy = true;
            self.started = std::time::Instant::now();
            self.status = format!("{}…", s(&value, "operation"));
            Some(value)
        }
    }
    pub fn panel_len(&self) -> usize {
        match self.panel {
            Panel::Threads => array(&self.pr, "threads").len(),
            Panel::Review => self.review_list.rows(self.my_comments()).len(),
            _ => 1,
        }
    }
    pub fn selected_thread(&self) -> &V {
        array(&self.pr, "threads")
            .get(self.panel_cursor)
            .unwrap_or(&V::Null)
    }
    pub fn selection(&self) -> Option<V> {
        let current = self.lines().get(self.line)?;
        let (anchor, side) = self.visual.clone().unwrap_or((
            self.line,
            if current["new"].is_number()
                && !(self.search.scope == crate::search::Scope::Original
                    && current["old"].is_number())
            {
                "RIGHT"
            } else {
                "LEFT"
            }
            .into(),
        ));
        let first = self.lines().get(anchor)?;
        let key = if side == "LEFT" { "old" } else { "new" };
        let a = first[key].as_u64()?;
        let b = current[key].as_u64()?;
        if first["hunk"] != current["hunk"] {
            return None;
        }
        Some(
            json!({"path":self.current()["filename"],"side":side,"start":a.min(b),"end":a.max(b),"hunk":first["hunk"]}),
        )
    }
    fn move_line(&mut self, delta: isize) {
        let n = self.lines().len();
        if n == 0 {
            return;
        }
        let next = (self.line as isize + delta).clamp(0, n.saturating_sub(1) as isize) as usize;
        if let Some((anchor, ref side)) = self.visual {
            let key = if side == "LEFT" { "old" } else { "new" };
            let start = self.lines().get(anchor);
            let mut candidate = next;
            while candidate < n && !self.lines()[candidate][key].is_number() {
                if delta > 0 {
                    candidate += 1;
                } else if candidate > 0 {
                    candidate -= 1;
                } else {
                    return;
                }
            }
            if candidate >= n || start.map(|l| &l["hunk"]) != Some(&self.lines()[candidate]["hunk"])
            {
                self.status = "Ranges stay on one side and within one hunk.".into();
                return;
            }
            self.line = candidate;
        } else {
            self.line = next;
        }
    }
    fn reveal_file(&mut self, file: usize) {
        let path = s(&self.files()[file], "filename").to_string();
        self.collapsed.retain(|dir| !path.starts_with(dir));
        self.select_file(file);
        if let Some(cursor) = self
            .rows()
            .iter()
            .position(|row| matches!(row, Row::File(i, _) if *i == file))
        {
            self.tree_cursor = cursor;
        }
    }
    fn step_tree_file(&mut self, forward: bool) {
        // Walk the same category/tree ordering the user sees, including children
        // of folded directories, then reveal only the selected file's ancestry.
        let selected = self.rows().get(self.tree_cursor).cloned();
        let rows = self.tree_rows(true);
        let cursor = selected
            .and_then(|row| rows.iter().position(|item| *item == row))
            .unwrap_or(0);
        let next = if forward {
            rows.iter().skip(cursor + 1).find_map(|row| match row {
                Row::File(i, _) => Some(*i),
                _ => None,
            })
        } else {
            rows[..cursor].iter().rev().find_map(|row| match row {
                Row::File(i, _) => Some(*i),
                _ => None,
            })
        };
        if let Some(file) = next {
            self.reveal_file(file);
        }
    }
    fn step_file(&mut self, delta: isize) {
        let visible: Vec<usize> = self
            .tree_rows(true)
            .into_iter()
            .filter_map(|row| match row {
                Row::File(i, _) => Some(i),
                _ => None,
            })
            .collect();
        if visible.is_empty() {
            return;
        }
        let i = visible.iter().position(|i| *i == self.file).unwrap_or(0);
        let next = (i as isize + delta).clamp(0, visible.len() as isize - 1) as usize;
        self.reveal_file(visible[next]);
    }
    fn step_hunk(&mut self, forward: bool) {
        let indices: Vec<usize> = self
            .lines()
            .iter()
            .enumerate()
            .filter(|(_, l)| s(l, "kind") == "hunk")
            .map(|(i, _)| (i + 1).min(self.lines().len() - 1))
            .collect();
        let next = if forward {
            indices.into_iter().find(|i| *i > self.line)
        } else {
            indices.into_iter().rev().find(|i| *i < self.line)
        };
        if let Some(i) = next {
            self.line = i;
            self.visual = None;
        }
    }
    pub fn receive(&mut self, op: &str, result: Result<V, String>) {
        if op == "suggest" {
            if let Ok(data) = result {
                self.completion.receive(data);
            }
            return;
        }
        self.busy = false;
        match result {
            Err(error) => {
                if error.contains("Action completed")
                    || error.contains("HTTP 5")
                    || error.contains("outcome may be uncertain")
                    || error.contains("reconcile")
                {
                    self.write_uncertain = true;
                }
                self.status = error;
                if op == "chat" {
                    if self.messages.last().is_some_and(|m| m["role"] == "user") {
                        self.messages.pop();
                    }
                    self.return_editor = Some(Input::Chat);
                }
                if let Some(input) = self.return_editor.take() {
                    self.modal = Some(Modal::Editor(input));
                }
            }
            Ok(data) => {
                if op == "load" {
                    self.syntax_generation = self.syntax_generation.wrapping_add(1);
                    self.write_uncertain = false;
                }
                self.return_editor = None;
                if op == "submit" {
                    self.review_list.summary.clear();
                    self.review_list.expanded.clear();
                    self.review_list.posted = true;
                    self.review_event = 0;
                    self.editor.clear();
                    self.editor_cursor = 0;
                    self.panel_cursor = 0;
                    self.panel_scroll = 0;
                }
                if op == "draft" {
                    self.ranges.clear();
                    self.comment_ranges.clear();
                    self.visual = None;
                    self.editor.clear();
                    self.editor_cursor = 0;
                }
                self.completion.reset();
                if op == "attach" {
                    if self.context.iter().any(|c| c["id"] == data["id"]) {
                        self.status = "Already attached.".into();
                    } else if self.context.len() >= 20
                        || self
                            .context
                            .iter()
                            .map(|c| s(c, "content").len() + s(c, "note").len())
                            .sum::<usize>()
                            + s(&data, "content").len()
                            > 160000
                    {
                        self.status = "Context limit: 20 attachments / 160,000 characters.".into();
                    } else {
                        self.context.push(data);
                        self.context_cursor = self.context.len() - 1;
                        self.status = "Added to AI context.".into();
                        self.panel = Panel::Ai;
                        self.visual = None;
                    }
                } else if op == "chat" {
                    let ids = self
                        .sent_context
                        .iter()
                        .map(|v| v["id"].clone())
                        .collect::<Vec<_>>();
                    self.consume_context(&ids);
                    self.messages
                        .push(json!({"role":"assistant","content":data["text"]}));
                    self.status = "Answer received. Ctrl-d/u scroll the panel.".into();
                } else {
                    let old_path = s(self.current(), "filename").to_string();
                    let changed = self.pr["url"] != data["url"]
                        || self.pr["head"] != data["head"]
                        || self.pr["base"] != data["base"];
                    self.pr = data;
                    if changed {
                        self.context.clear();
                        self.sent_context.clear();
                        self.note_target = None;
                        self.ranges.clear();
                        self.messages.clear();
                        self.visual = None;
                        self.hidden.clear();
                        self.collapsed.clear();
                        self.review_list = crate::review_list::ReviewList::default();
                        self.status = "Loaded new snapshot; code context cleared.".into();
                    } else {
                        self.status = if self.is_demo() {
                            "Saved in sample only.".into()
                        } else {
                            "GitHub state refreshed.".into()
                        };
                    }
                    if let Some(i) = self
                        .files()
                        .iter()
                        .position(|f| s(f, "filename") == old_path)
                    {
                        self.file = i;
                    }
                    self.ensure();
                    self.rebuild_search();
                    if op == "conversation" {
                        self.panel_cursor = array(&self.pr, "conversation").len().saturating_sub(1);
                        self.panel_scroll = 0;
                    }
                    if op == "draft" {
                        self.status = "Draft saved · r to review".into();
                    }
                    if op == "submit" {
                        self.status = if self.is_demo() {
                            "Review published in sample."
                        } else {
                            "Review published."
                        }
                        .into();
                    }
                }
            }
        }
    }
    pub fn consume_context(&mut self, ids: &[V]) {
        self.context
            .retain(|item| !ids.contains(&item["id"]) || !self.sent_context.contains(item));
        self.sent_context.clear();
        self.context_cursor = self
            .context_cursor
            .min(self.context.len().saturating_sub(1));
    }
    fn open_ai_note(&mut self) {
        let target = if self.pane == Pane::Diff {
            let Some(range) = self.selection() else {
                self.status = "Select changed lines first.".into();
                return;
            };
            let side = s(&range, "side");
            let commit = if side == "LEFT" {
                &self.pr["base"]
            } else {
                &self.pr["head"]
            };
            let key = if side == "LEFT" { "old" } else { "new" };
            let start = range["start"].as_u64().unwrap_or(0);
            let end = range["end"].as_u64().unwrap_or(0);
            let content = self
                .lines()
                .iter()
                .filter_map(|line| {
                    let n = line[key].as_u64()?;
                    (n >= start && n <= end).then(|| format!("{n}: {}", s(line, "text")))
                })
                .collect::<Vec<_>>()
                .join("\n");
            if content.is_empty() {
                self.status = "Select returned patch lines.".into();
                return;
            }
            json!({"id":format!("{}:{}:lines:{start}:{end}",commit.as_str().unwrap_or(""),s(&range,"path")),"path":range["path"],"commit":commit,"side":side,"kind":"lines","start":start,"end":end,"label":format!("L{start}–{end} {side}"),"content":content})
        } else {
            let Some(value) = self.context.get(self.context_cursor) else {
                self.status = "Select code, then press t to leave a private AI note.".into();
                return;
            };
            value.clone()
        };
        self.open_editor(Input::AiNote);
        self.editor = s(&target, "note").into();
        self.editor_cursor = self.editor.len();
        self.note_target = Some(target);
    }
    pub fn own_pr(&self) -> bool {
        let viewer = if self.is_demo() {
            "you"
        } else {
            s(&self.pr, "viewer")
        };
        !viewer.is_empty() && viewer.eq_ignore_ascii_case(s(&self.pr, "author"))
    }
    fn compose_request(&self, input: Input) -> Result<V, String> {
        if self.write_uncertain {
            return Err(
                "Refresh first; the previous action may already be saved on GitHub.".into(),
            );
        }
        let body = self.editor.trim();
        let operation = match input {
            Input::Comment => "draft",
            Input::Review => "submit",
            Input::Reply => "reply",
            Input::Conversation => "conversation",
            _ => return Err("Not a review action".into()),
        };
        let mut request = self.snapshot(operation);
        request["body"] = json!(body);
        if body.is_empty() && input != Input::Review {
            return Err("Write a message first.".into());
        }
        match input {
            Input::Comment => {
                if self.editor_head != self.pr["head"] || self.editor_base != self.pr["base"] {
                    return Err("PR changed. Close this draft and reselect the code.".into());
                }
                let selected = self
                    .comment_ranges
                    .first()
                    .cloned()
                    .or_else(|| self.selection())
                    .ok_or("Select a diff range first")?;
                request["ranges"] = json!(self.comment_ranges);
                request["path"] = selected["path"].clone();
                request["side"] = selected["side"].clone();
                request["startLine"] = selected["start"].clone();
                request["line"] = selected["end"].clone();
            }
            Input::Review => {
                if body.encode_utf16().count() > 60_000 {
                    return Err("Review summary exceeds 60,000 characters.".into());
                }
                if self.editor_head != self.pr["head"] || self.editor_base != self.pr["base"] {
                    return Err(
                        "PR changed. Close this summary and review the new snapshot.".into(),
                    );
                }
                let event = ["COMMENT", "APPROVE", "REQUEST_CHANGES"][self.review_event.min(2)];
                if self.own_pr() && event != "COMMENT" {
                    return Err("Your PR: choose Comment.".into());
                }
                if body.is_empty() && event != "APPROVE" && self.pending_comments().is_empty() {
                    return Err("Write a summary or add pending comments first.".into());
                }
                request["event"] = json!(event);
            }
            Input::Reply => {
                let thread = self.selected_thread();
                if thread["pending"] == true {
                    return Err("Submit the pending thread before replying.".into());
                }
                if self.reply_id.is_null() {
                    return Err("Select a published thread first".into());
                }
                request["commentId"] = self.reply_id.clone();
            }
            _ => {}
        }
        Ok(request)
    }
    pub fn key(&mut self, key: KeyEvent) -> Option<V> {
        if self.modal.is_none()
            && self.pane == Pane::Panel
            && matches!(key.code, K::Char('j' | 'k') | K::Up | K::Down)
        {
            self.ai.follow = false;
        }
        if self.modal.is_none()
            && self.pane == Pane::Panel
            && matches!(self.panel, Panel::Ai | Panel::Overview | Panel::Threads)
            && self.reader.selection.visible
        {
            let selection = &mut self.reader.selection;
            if key.code == K::Char('v')
                && !self
                    .prefix
                    .is_some_and(|(c, when)| c == 'g' && when.elapsed().as_millis() < 1500)
            {
                if selection.active() {
                    selection.clear();
                } else {
                    selection.begin_keyboard();
                    self.ai.follow = false;
                    self.status = "Select text · h/j/k/l Move · y Copy · Esc Cancel".into();
                }
                return None;
            }
            if selection.active() {
                match key.code {
                    K::Esc => {
                        selection.clear();
                        return None;
                    }
                    K::Char('y') => {
                        self.copy_pending = Some(selection.text());
                        self.status = "Copying…".into();
                        return None;
                    }
                    K::Char('c') if key.modifiers.contains(M::CONTROL) => {
                        self.copy_pending = Some(selection.text());
                        return None;
                    }
                    K::Char(c @ ('h' | 'j' | 'k' | 'l' | '0' | '$')) if selection.keyboard => {
                        let max = self.reader.max_scroll
                            + selection.area.height.saturating_sub(1) as usize;
                        let row = selection.step(c, max);
                        self.ai.follow = false;
                        if row < self.panel_scroll {
                            self.panel_scroll = row;
                        } else if row >= self.panel_scroll + selection.area.height as usize {
                            self.panel_scroll = row
                                .saturating_sub(selection.area.height.saturating_sub(1) as usize);
                        }
                        return None;
                    }
                    _ => {}
                }
            }
        }
        if key.modifiers.contains(M::CONTROL) && key.code == K::Char('c') {
            if self.ai.busy && self.pane == Pane::Panel && self.panel == Panel::Ai {
                return Some(json!({"operation":"ai_stop"}));
            }
            self.quit = true;
            return None;
        }
        if self.modal == Some(Modal::Editor(Input::DiffSearch)) {
            match key.code {
                K::Esc => {
                    if let Some((search, line, horizontal)) = self.search_origin.take() {
                        self.search = search;
                        self.line = line;
                        self.horizontal = horizontal;
                    }
                    self.modal = None;
                }
                K::Enter => {
                    self.modal = None;
                    self.search_origin = None;
                }
                K::Tab => {
                    self.search.scope = self.search.scope.next();
                    self.update_search();
                }
                K::Backspace => {
                    self.editor.pop();
                    self.update_search();
                }
                K::Char('u') if key.modifiers.contains(M::CONTROL) => {
                    self.editor.clear();
                    self.update_search();
                }
                K::Char(c)
                    if !key.modifiers.intersects(M::CONTROL | M::ALT)
                        && self.editor.len() < 200 =>
                {
                    self.editor.push(c);
                    self.update_search();
                }
                _ => {}
            }
            return None;
        }
        if let Some(modal) = self.modal {
            match modal {
                Modal::Appearance => {
                    match key.code {
                        K::Char('j') | K::Down => self.reader.choice = (self.reader.choice + 1) % 3,
                        K::Char('k') | K::Up => self.reader.choice = (self.reader.choice + 2) % 3,
                        K::Enter => {
                            self.reader.appearance =
                                crate::reader::Appearance::ALL[self.reader.choice];
                            self.modal = None;
                        }
                        K::Esc => self.modal = None,
                        _ => {}
                    }
                    return None;
                }
                Modal::Context => {
                    match key.code {
                        K::Esc => self.modal = None,
                        K::Char('j') | K::Down => {
                            self.context_cursor =
                                (self.context_cursor + 1).min(self.context.len().saturating_sub(1))
                        }
                        K::Char('k') | K::Up => {
                            self.context_cursor = self.context_cursor.saturating_sub(1)
                        }
                        K::Char('d') => {
                            if self.context_cursor < self.context.len() {
                                self.context.remove(self.context_cursor);
                            }
                            self.context_cursor = self
                                .context_cursor
                                .min(self.context.len().saturating_sub(1));
                        }
                        K::Enter | K::Char('t') => {
                            self.pane = Pane::Panel;
                            self.open_ai_note();
                        }
                        K::Char('i') => self.open_editor(Input::Chat),
                        _ => {}
                    }
                    return None;
                }
                Modal::Artifacts => {
                    let count = self.ai.artifacts.len();
                    match key.code {
                        K::Char('j') | K::Down => {
                            self.reader.artifact_cursor =
                                (self.reader.artifact_cursor + 1).min(count.saturating_sub(1))
                        }
                        K::Char('k') | K::Up => {
                            self.reader.artifact_cursor =
                                self.reader.artifact_cursor.saturating_sub(1)
                        }
                        K::Enter => {
                            if let Some(item) = self.ai.artifacts.get(self.reader.artifact_cursor) {
                                if self.is_demo() {
                                    self.status =
                                        "Sample artifact. Live AI outputs open in your browser."
                                            .into();
                                } else {
                                    return Some(
                                        json!({"operation":"ai_open_artifact","id":item["id"]}),
                                    );
                                }
                            }
                        }
                        K::Esc => self.modal = None,
                        _ => {}
                    }
                    return None;
                }
                Modal::Outline => {
                    let count = self.reader.sections.len();
                    match key.code {
                        K::Char('j') | K::Down => {
                            self.reader.outline_cursor =
                                (self.reader.outline_cursor + 1).min(count.saturating_sub(1))
                        }
                        K::Char('k') | K::Up => {
                            self.reader.outline_cursor =
                                self.reader.outline_cursor.saturating_sub(1)
                        }
                        K::Enter => {
                            if let Some((row, _)) =
                                self.reader.sections.get(self.reader.outline_cursor)
                            {
                                self.panel_scroll = *row;
                                self.ai.follow = false;
                            }
                            self.modal = None;
                        }
                        K::Esc => self.modal = None,
                        _ => {}
                    }
                    return None;
                }
                Modal::AiSetup => {
                    match key.code {
                        K::Enter => {
                            self.modal = None;
                            return Some(json!({"operation":"ai_setup"}));
                        }
                        K::Esc => self.modal = None,
                        _ => {}
                    }
                    return None;
                }
                Modal::Help => {
                    self.help.key(key);
                    if !self.help.open {
                        self.modal = None;
                    }
                    return None;
                }
                Modal::Panes => {
                    match key.code {
                        K::Char('j') | K::Down => self.pane_choice = (self.pane_choice + 1) % 3,
                        K::Char('k') | K::Up => self.pane_choice = (self.pane_choice + 2) % 3,
                        K::Enter | K::Char(' ') => {
                            let restoring = !self.panes.visible().contains(&self.pane_choice);
                            if self.panes.toggle(self.pane_choice) {
                                let visible = self.panes.visible();
                                if restoring {
                                    self.pane = Pane::from_index(self.pane_choice);
                                    self.enter_native_input();
                                } else if !visible.contains(&self.pane.index()) {
                                    self.pane = if visible.contains(&1) {
                                        Pane::Diff
                                    } else {
                                        Pane::from_index(visible[0])
                                    };
                                }
                                self.modal = None;
                            } else {
                                self.status = "Keep at least one pane open.".into();
                            }
                        }
                        K::Esc | K::Char('\\') => self.modal = None,
                        _ => {}
                    }
                    return None;
                }
                Modal::Confirm => {
                    match key.code {
                        K::Char('j') | K::Down => self.panel_scroll += 1,
                        K::Char('k') | K::Up => {
                            self.panel_scroll = self.panel_scroll.saturating_sub(1)
                        }
                        K::Char('y') | K::Enter if !self.busy => {
                            if self.write_uncertain {
                                self.pending = None;
                                self.modal = self.return_editor.map(Modal::Editor);
                                self.status = "Refresh first; the previous action may already be saved on GitHub.".into();
                                return None;
                            }
                            let request = self.pending.as_ref()?;
                            if request["operation"] == "submit"
                                && (request["expectedHead"] != self.pr["head"]
                                    || request["expectedBase"] != self.pr["base"])
                            {
                                self.pending = None;
                                self.modal = Some(Modal::Editor(Input::Review));
                                self.status =
                                    "PR changed. Close this summary and review the new snapshot."
                                        .into();
                                return None;
                            }
                            self.modal = None;
                            let request = self.pending.take()?;
                            return self.send(request);
                        }
                        K::Esc | K::Char('n') => {
                            self.modal = self.return_editor.map(Modal::Editor);
                            self.pending = None;
                        }
                        _ => {}
                    }
                    return None;
                }
                Modal::Editor(input) => {
                    if self.completion.open {
                        match key.code {
                            K::Esc => {
                                self.completion.reset();
                                return None;
                            }
                            K::Down => {
                                self.completion.selected = (self.completion.selected + 1)
                                    .min(self.completion.items.len().saturating_sub(1));
                                return None;
                            }
                            K::Up => {
                                self.completion.selected =
                                    self.completion.selected.saturating_sub(1);
                                return None;
                            }
                            K::Tab | K::Enter
                                if !key.modifiers.intersects(M::ALT | M::SHIFT | M::CONTROL)
                                    && !self.completion.items.is_empty() =>
                            {
                                self.completion
                                    .accept(&mut self.editor, &mut self.editor_cursor);
                                return None;
                            }
                            _ => {}
                        }
                    }
                    if (key.code == K::Enter && key.modifiers.intersects(M::ALT | M::SHIFT))
                        || (key.code == K::Char('n') && key.modifiers.contains(M::CONTROL))
                    {
                        if !matches!(input, Input::Filter | Input::DiffSearch | Input::Open) {
                            self.editor.insert(self.editor_cursor, '\n');
                            self.editor_cursor += 1;
                            self.completion.reset();
                        }
                        return None;
                    }
                    if key.code == K::Esc {
                        if input == Input::Review {
                            self.review_list.summary = self.editor.clone();
                        }
                        self.modal = None;
                        self.status = "Composer closed; press e to reopen its text.".into();
                        self.return_editor = Some(input);
                        return None;
                    }
                    if matches!(key.code, K::Tab | K::BackTab) && input == Input::Review {
                        self.status.clear();
                        self.review_event = if self.own_pr() {
                            0
                        } else {
                            let reverse =
                                key.code == K::BackTab || key.modifiers.contains(M::SHIFT);
                            (self.review_event + if reverse { 2 } else { 1 }) % 3
                        };
                        return None;
                    }
                    if (key.modifiers.contains(M::CONTROL) && key.code == K::Char('s'))
                        || key.code == K::Enter
                    {
                        if self.busy {
                            self.status = "Wait for the current request.".into();
                            return None;
                        }
                        match input {
                            Input::Filter => {
                                self.filter = self.editor.clone();
                                self.modal = None;
                                self.ensure();
                            }
                            Input::Open => {
                                let url = self.editor.trim().to_string();
                                self.modal = None;
                                return self.send(json!({"operation":"load","url":url}));
                            }
                            Input::AiNote => {
                                if let Some(mut item) = self.note_target.clone() {
                                    item["note"] = json!(self.editor.trim());
                                    let duplicate =
                                        self.context.iter().position(|v| v["id"] == item["id"]);
                                    let bytes = self
                                        .context
                                        .iter()
                                        .filter(|v| v["id"] != item["id"])
                                        .map(|v| s(v, "content").len() + s(v, "note").len())
                                        .sum::<usize>()
                                        + s(&item, "content").len()
                                        + s(&item, "note").len();
                                    if bytes > 160000
                                        || (duplicate.is_none() && self.context.len() >= 20)
                                    {
                                        self.status =
                                            "Context limit: 20 attachments / 160,000 characters."
                                                .into();
                                        return None;
                                    }
                                    if let Some(index) = duplicate {
                                        self.context[index] = item;
                                        self.context_cursor = index;
                                    } else {
                                        self.context.push(item);
                                        self.context_cursor = self.context.len() - 1;
                                    }
                                    self.note_target = None;
                                    self.visual = None;
                                    self.modal = None;
                                    self.status = "Queued for the next AI answer".into();
                                }
                            }
                            Input::Chat => {
                                if !self.is_demo() {
                                    if !self.ai.enabled {
                                        self.modal = Some(Modal::AiSetup);
                                        return None;
                                    }
                                    if self.ai.busy && self.ai.phase != "Preparing" {
                                        self.status =
                                            "AI is answering. X in the companion stops it.".into();
                                        return None;
                                    }
                                    if self.editor.trim().is_empty() {
                                        return None;
                                    }
                                    self.modal = None;
                                    self.sent_context = self.context.clone();
                                    return Some(
                                        json!({"operation":"ai_chat","prompt":self.editor.trim()}),
                                    );
                                }
                                if self.editor.trim().is_empty() {
                                    return None;
                                }
                                self.sent_context = self.context.clone();
                                self.messages
                                    .push(json!({"role":"user","content":self.editor.trim()}));
                                let history: Vec<V> = self
                                    .messages
                                    .iter()
                                    .rev()
                                    .take(20)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .into_iter()
                                    .rev()
                                    .collect();
                                self.modal = None;
                                return self.send(json!({"operation":"chat","messages":history,"context":self.context}));
                            }
                            _ => match self.compose_request(input) {
                                Ok(request) => {
                                    self.return_editor = Some(input);
                                    self.panel_scroll = 0;
                                    self.completion.reset();
                                    if input == Input::Review {
                                        self.review_list.summary = self.editor.clone();
                                        self.pending = Some(request);
                                        self.modal = Some(Modal::Confirm);
                                    } else {
                                        self.modal = None;
                                        return self.send(request);
                                    }
                                }
                                Err(error) => self.status = error,
                            },
                        }
                        return None;
                    }
                    match key.code {
                        K::Char('u') if key.modifiers.contains(M::CONTROL) => {
                            self.editor.clear();
                            self.editor_cursor = 0;
                        }
                        K::Char(c) if !key.modifiers.intersects(M::CONTROL | M::ALT) => {
                            if self.editor.len()
                                < if input == Input::Review {
                                    240_000
                                } else {
                                    10_000
                                }
                            {
                                self.editor.insert(self.editor_cursor, c);
                                self.editor_cursor += c.len_utf8();
                            }
                        }
                        K::Backspace if self.editor_cursor > 0 => {
                            let prev = self.editor[..self.editor_cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            self.editor.drain(prev..self.editor_cursor);
                            self.editor_cursor = prev;
                        }
                        K::Delete if self.editor_cursor < self.editor.len() => {
                            let n = self.editor[self.editor_cursor..]
                                .chars()
                                .next()
                                .unwrap()
                                .len_utf8();
                            self.editor
                                .drain(self.editor_cursor..self.editor_cursor + n);
                        }
                        K::Left => {
                            self.editor_cursor = self.editor[..self.editor_cursor]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0)
                        }
                        K::Right => {
                            self.editor_cursor += self.editor[self.editor_cursor..]
                                .chars()
                                .next()
                                .map(char::len_utf8)
                                .unwrap_or(0)
                        }
                        K::Home => {
                            self.editor_cursor = if key.modifiers.contains(M::CONTROL) {
                                0
                            } else {
                                self.editor[..self.editor_cursor]
                                    .rfind('\n')
                                    .map(|i| i + 1)
                                    .unwrap_or(0)
                            }
                        }
                        K::End => {
                            self.editor_cursor = if key.modifiers.contains(M::CONTROL) {
                                self.editor.len()
                            } else {
                                self.editor[self.editor_cursor..]
                                    .find('\n')
                                    .map(|i| self.editor_cursor + i)
                                    .unwrap_or(self.editor.len())
                            }
                        }
                        K::Up | K::Down => {
                            let start = self.editor[..self.editor_cursor]
                                .rfind('\n')
                                .map(|i| i + 1)
                                .unwrap_or(0);
                            let column = self.editor[start..self.editor_cursor].chars().count();
                            let range = if key.code == K::Up {
                                if start > 0 {
                                    let end = start - 1;
                                    Some((
                                        self.editor[..end].rfind('\n').map(|i| i + 1).unwrap_or(0),
                                        end,
                                    ))
                                } else {
                                    None
                                }
                            } else {
                                self.editor[self.editor_cursor..].find('\n').map(|i| {
                                    let next = self.editor_cursor + i + 1;
                                    (
                                        next,
                                        self.editor[next..]
                                            .find('\n')
                                            .map(|j| next + j)
                                            .unwrap_or(self.editor.len()),
                                    )
                                })
                            };
                            if let Some((start, end)) = range {
                                self.editor_cursor = self.editor[start..end]
                                    .char_indices()
                                    .nth(column)
                                    .map(|(i, _)| start + i)
                                    .unwrap_or(end);
                            }
                        }
                        _ => {}
                    }
                    if !matches!(input, Input::Filter | Input::DiffSearch | Input::Open) {
                        self.completion
                            .sync(&self.editor, self.editor_cursor, &self.pr);
                    }
                    return None;
                }
            }
        }
        if key.modifiers.intersects(M::ALT | M::SUPER) {
            return None;
        }
        let prior = self
            .prefix
            .take()
            .filter(|(_, at)| at.elapsed().as_millis() < 900)
            .map(|(c, _)| c);
        if let K::Char(c) = key.code {
            if prior == Some('w') && ['w', 'h', 'l'].contains(&c) {
                self.cycle_pane(c != 'h');
                return None;
            }
            if key.modifiers.contains(M::CONTROL) && c == 'w' {
                self.prefix = Some(('w', std::time::Instant::now()));
                return None;
            }
            if key.modifiers.contains(M::CONTROL) && ['d', 'u'].contains(&c) {
                let delta = if c == 'd' { 10 } else { -10 };
                if self.pane == Pane::Diff {
                    self.move_line(delta);
                } else {
                    self.ai.follow = false;
                    self.panel_scroll = (self.panel_scroll as isize + delta).max(0) as usize;
                }
                return None;
            }
            if key.modifiers.contains(M::CONTROL) {
                return None;
            }
            if prior == Some('g') && c == '!' {
                self.help.show();
                self.modal = Some(Modal::Help);
                return None;
            }
            if prior == Some('g') && c == 'c' {
                self.modal = Some(Modal::Context);
                return None;
            }
            if prior == Some('g') && c == 'o' {
                self.reader.artifact_cursor = self
                    .reader
                    .artifact_cursor
                    .min(self.ai.artifacts.len().saturating_sub(1));
                self.modal = Some(Modal::Artifacts);
                return None;
            }
            if prior == Some('g') && c == 'v' {
                self.reader.choice = self.reader.appearance.index();
                self.modal = Some(Modal::Appearance);
                return None;
            }
            if self.pane == Pane::Panel
                && matches!(self.panel, Panel::Overview | Panel::Ai)
                && ((prior == Some('[') && c == '[') || (prior == Some(']') && c == ']'))
            {
                self.panel_scroll = self.reader.jump(self.panel_scroll, c == ']');
                self.ai.follow = false;
                return None;
            }
            if prior == Some('g') && c == 'm' {
                self.panel = Panel::Overview;
                self.panes.panel = true;
                self.pane = Pane::Panel;
                self.reader.destination = Some(crate::reader::Destination::Commits);
                return None;
            }
            if prior == Some('g') && c == 'a' {
                self.panel = Panel::Ai;
                self.panes.panel = true;
                self.pane = Pane::Panel;
                self.enter_native_input();
                if !self.ai.enabled && !self.is_demo() {
                    self.modal = Some(Modal::AiSetup);
                }
                return None;
            }
            if prior == Some('g') && c == 'g' {
                self.ai.follow = false;
                self.visual = None;
                self.tree_cursor = 0;
                self.line = 0;
                self.panel_cursor = 0;
                self.panel_scroll = 0;
                self.review_list.follow = true;
                return None;
            }
            if matches!(prior, Some('[') | Some(']')) && c == 'c' {
                self.step_hunk(prior == Some(']'));
                return None;
            }
            if matches!(prior, Some('[') | Some(']')) && c == 'f' {
                self.step_file(if prior == Some(']') { 1 } else { -1 });
                return None;
            }
            if matches!(prior, Some('[') | Some(']')) && c == 'a' {
                self.context_cursor = if prior == Some(']') {
                    (self.context_cursor + 1).min(self.context.len().saturating_sub(1))
                } else {
                    self.context_cursor.saturating_sub(1)
                };
                return None;
            }
        }
        match key.code {
            K::Char('N' | 'X' | 'R')
                if self.pane == Pane::Panel
                    && self.panel == Panel::Ai
                    && !self.is_demo()
                    && self.ai.enabled =>
            {
                let operation = match key.code {
                    K::Char('N') => "ai_new",
                    K::Char('X') => "ai_stop",
                    _ => "ai_retry",
                };
                return Some(json!({"operation":operation}));
            }
            K::Char('F') if self.pane == Pane::Panel && self.panel == Panel::Ai => {
                self.reader.focused = !self.reader.focused;
            }
            K::Char('o')
                if self.pane == Pane::Panel
                    && matches!(self.panel, Panel::Overview | Panel::Ai) =>
            {
                self.reader.outline_cursor = self
                    .reader
                    .sections
                    .partition_point(|(row, _)| *row <= self.panel_scroll)
                    .saturating_sub(1);
                self.modal = Some(Modal::Outline);
            }
            K::Char('q') => self.quit = true,
            K::Char('?') => {
                self.panel_scroll = 0;
                self.help.show();
                self.modal = Some(Modal::Help);
            }
            K::Char('g' | '[' | ']') => {
                if let K::Char(c) = key.code {
                    self.prefix = Some((c, std::time::Instant::now()));
                }
            }
            K::Tab => self.cycle_pane(!key.modifiers.contains(M::SHIFT)),
            K::BackTab => self.cycle_pane(false),
            K::Char('h' | 'l') => {
                let expand = key.code == K::Char('l');
                if self.pane == Pane::Tree {
                    self.fold_subtree(expand);
                } else if self.pane == Pane::Panel && self.panel == Panel::Review {
                    self.toggle_review(Some(expand));
                } else if self.pane == Pane::Panel && self.panel == Panel::Overview {
                    self.panel_scroll = self.reader.jump(self.panel_scroll, expand);
                } else if self.pane == Pane::Panel && self.panel == Panel::Threads {
                    self.panel = Panel::Overview;
                    self.reader.destination = Some(crate::reader::Destination::Comments);
                } else if self.pane == Pane::Diff {
                    self.horizontal = if expand {
                        self.horizontal.saturating_add(4)
                    } else {
                        self.horizontal.saturating_sub(4)
                    };
                }
            }
            K::Char('+' | '-') => self.panes.resize(
                self.pane.index(),
                if key.code == K::Char('+') { 4 } else { -4 },
            ),
            K::Char('\\') => {
                if self.panes.visible().contains(&self.pane_choice) {
                    self.pane_choice = self.pane.index();
                }
                self.modal = Some(Modal::Panes);
            }
            K::Char('t')
                if self.pane == Pane::Diff
                    || (self.pane == Pane::Panel && self.panel == Panel::Ai) =>
            {
                self.open_ai_note();
            }
            K::Char('t') => {
                self.panes.toggle(0);
                let visible = self.panes.visible();
                if !visible.contains(&self.pane.index()) {
                    self.pane = Pane::from_index(*visible.first().unwrap_or(&1));
                }
            }
            K::Char('Z') => self.panes.zoom = !self.panes.zoom,
            K::Char('=') => self.fit_panes(),
            K::Char('H') => {
                self.syntax_palette = self.syntax_palette.next();
                self.status = format!("Syntax: {}", self.syntax_palette.name());
            }
            K::Char('T') => {
                self.status = match crate::theme::cycle() {
                    Ok(name) => format!("Theme: {name}"),
                    Err(e) => e,
                };
            }
            K::Char('j') | K::Down => self.move_cursor(1),
            K::Char('k') | K::Up => self.move_cursor(-1),
            K::Char('G') => {
                if self.pane == Pane::Panel && self.panel == Panel::Ai {
                    self.ai.follow = true;
                }
                self.review_list.follow = true;
                match self.pane {
                    Pane::Tree => self.tree_cursor = self.rows().len().saturating_sub(1),
                    Pane::Diff => {
                        self.line = self.lines().len().saturating_sub(1);
                        self.visual = None;
                    }
                    Pane::Panel => {
                        self.panel_cursor = self.panel_len().saturating_sub(1);
                        self.panel_scroll = if matches!(self.panel, Panel::Ai | Panel::Overview) {
                            u16::MAX as usize
                        } else {
                            0
                        };
                    }
                }
            }
            K::Enter => {
                if self.pane == Pane::Panel && self.panel == Panel::Review {
                    self.toggle_review(None);
                    return None;
                }
                if self.pane == Pane::Tree {
                    match self.rows().get(self.tree_cursor).cloned() {
                        Some(Row::Category(_)) => {}
                        Some(Row::Directory(path, _)) => {
                            if !self.collapsed.remove(&path) {
                                self.collapsed.insert(path);
                            }
                        }
                        Some(Row::File(i, _)) => {
                            self.select_file(i);
                            self.panes.diff = true;
                            self.pane = Pane::Diff;
                        }
                        _ => {}
                    }
                }
            }
            K::Char('n' | 'N') if self.pane == Pane::Tree => {
                self.step_tree_file(key.code == K::Char('n'));
            }
            K::Char('n' | 'N') if self.pane == Pane::Diff => {
                if self.search.query.is_empty() {
                    self.step_file(if key.code == K::Char('n') { 1 } else { -1 });
                } else if let Some(hit) = self.search.step(key.code == K::Char('n'), self.line) {
                    self.goto_hit(hit);
                }
            }
            K::Char('J') => self.step_file(1),
            K::Char('K') => self.step_file(-1),
            K::Char('0'..='5') => {
                if let K::Char(c) = key.code {
                    self.category = if c == '0' {
                        None
                    } else {
                        Some(CATEGORIES[c as usize - '1' as usize].into())
                    };
                    self.ensure();
                }
            }
            K::Char('/') => {
                if self.pane == Pane::Diff {
                    self.search_origin = Some((self.search.clone(), self.line, self.horizontal));
                    self.editor.clear();
                    self.editor_cursor = 0;
                    self.search.query.clear();
                    self.rebuild_search();
                    self.modal = Some(Modal::Editor(Input::DiffSearch));
                } else {
                    self.open_editor(Input::Filter);
                }
            }
            K::Char('O') if !self.busy => self.open_editor(Input::Open),
            K::Char('o') => {
                self.sort = (self.sort + 1) % 4;
                self.ensure();
            }
            K::Char('u') => {
                self.unviewed = !self.unviewed;
                self.ensure();
            }
            K::Char('z') => {
                self.show_hidden = !self.show_hidden;
                self.ensure();
            }
            K::Char('x') => {
                let path = s(self.current(), "filename").to_string();
                if !self.hidden.remove(&path) {
                    self.hidden.insert(path);
                }
                self.ensure();
            }
            K::Char('s') => {
                self.split = !self.split;
                self.rebuild_search();
                self.status = format!(
                    "Diff: {} · s to switch",
                    if self.split { "side by side" } else { "inline" }
                );
            }
            K::Left => self.horizontal = self.horizontal.saturating_sub(4),
            K::Right => self.horizontal = self.horizontal.saturating_add(4),
            K::Char(' ') if self.pane == Pane::Diff => {
                if let Some(range) = self.selection() {
                    if let Some(i) = self.ranges.iter().position(|r| *r == range) {
                        self.ranges.remove(i);
                    } else if self.ranges.len() < 20 {
                        self.ranges.push(range);
                    }
                    self.visual = None;
                    self.status = format!("{} ranges selected", self.ranges.len());
                }
            }
            K::Char('v') if self.pane == Pane::Diff => {
                if self.visual.is_some() {
                    self.visual = None;
                } else if let Some(sel) = self.selection() {
                    self.visual = Some((self.line, s(&sel, "side").into()));
                }
            }
            K::Esc => {
                if self.visual.is_some() {
                    self.visual = None;
                } else if !self.search.query.is_empty() {
                    self.search = crate::search::Search::default();
                } else {
                    self.ranges.clear();
                }
            }
            K::Char('a' | 'A') if !self.busy => {
                let kind = if key.code == K::Char('A') {
                    "file"
                } else if self.visual.is_some() {
                    "lines"
                } else {
                    "diff"
                };
                let selection = self.selection();
                let side = if kind == "lines" {
                    selection
                        .as_ref()
                        .and_then(|s| s["side"].as_str())
                        .unwrap_or("RIGHT")
                } else if self.current()["status"] == "removed" {
                    "LEFT"
                } else {
                    "RIGHT"
                };
                return self.send(json!({"operation":"attach","path":self.current()["filename"],"kind":kind,"side":side,"selection":selection}));
            }
            K::Char('d') if self.pane == Pane::Panel && self.panel == Panel::Ai => {
                if self.context_cursor < self.context.len() {
                    self.context.remove(self.context_cursor);
                    self.context_cursor = self
                        .context_cursor
                        .min(self.context.len().saturating_sub(1));
                }
            }
            K::Char('m') if !self.busy => {
                let mut req = self.snapshot("viewed");
                req["path"] = self.current()["filename"].clone();
                req["viewed"] = json!(self.current()["viewed"] != true);
                return self.send(req);
            }
            K::Char('c') if !self.busy => {
                if !self.ranges.is_empty() || self.selection().is_some() {
                    self.open_editor(Input::Comment);
                } else {
                    self.status = "Choose a returned diff line first.".into();
                }
            }
            K::Char('r') if !self.busy => self.show_review(),
            K::Char('S') if !self.busy => self.open_editor(Input::Review),
            K::Char('P') if self.pane == Pane::Panel && self.panel == Panel::Review => {
                self.review_list.posted = !self.review_list.posted;
                self.review_list.expanded.clear();
                self.review_list.follow = true;
                self.panel_cursor = 0;
                self.panel_scroll = 0;
            }
            K::Char('f') if !self.busy => {
                if self.is_demo() {
                    self.status = "Sample is already current; O opens a real PR.".into();
                } else {
                    return self.send(json!({"operation":"load","url":self.pr["url"]}));
                }
            }
            K::Char('p') => {
                self.panel = match self.panel {
                    Panel::Ai => Panel::Threads,
                    Panel::Threads => Panel::Overview,
                    Panel::Overview => Panel::Review,
                    Panel::Review => Panel::Ai,
                };
                self.panel_cursor = 0;
                self.panel_scroll = 0;
                self.panes.panel = true;
                self.pane = Pane::Panel;
            }
            K::Char('D' | 'C' | 'b') => {
                self.panel = match key.code {
                    K::Char('D') => Panel::Overview,
                    K::Char('C') => Panel::Overview,
                    _ => Panel::Threads,
                };
                self.panel_scroll = 0;
                self.panel_cursor = 0;
                self.reader.destination = match key.code {
                    K::Char('D') => Some(crate::reader::Destination::Top),
                    K::Char('C') => Some(crate::reader::Destination::Comments),
                    _ => None,
                };
                self.panes.panel = true;
                self.pane = Pane::Panel;
            }
            K::Char('i') if !self.busy => {
                if self.panel == Panel::Review {
                    self.open_editor(Input::Review);
                    return None;
                }
                if !matches!(self.panel, Panel::Threads | Panel::Overview)
                    && !self.is_demo()
                    && !self.ai.enabled
                {
                    self.panel = Panel::Ai;
                    self.panes.panel = true;
                    self.pane = Pane::Panel;
                    self.modal = Some(Modal::AiSetup);
                    return None;
                }
                let input = match self.panel {
                    Panel::Threads => Input::Reply,
                    Panel::Overview => Input::Conversation,
                    _ => {
                        self.panel = Panel::Ai;
                        Input::Chat
                    }
                };
                if self.panel == Panel::Review {
                    self.open_editor(Input::Review);
                } else {
                    self.open_editor(input);
                }
            }
            K::Char('e') if !self.busy => {
                if let Some(input) = self.return_editor {
                    if input == Input::Chat {
                        self.panel = Panel::Ai;
                        self.pane = Pane::Panel;
                        self.panes.panel = true;
                    }
                    self.modal = Some(Modal::Editor(input));
                }
            }
            K::Char('R') if self.panel == Panel::Threads && !self.busy => {
                let thread = self.selected_thread();
                if thread["pending"] == true || thread["canResolve"] != true {
                    self.status = "This thread cannot be resolved by the current viewer.".into();
                } else {
                    let mut req = self.snapshot("resolve");
                    req["threadId"] = thread["id"].clone();
                    req["resolved"] = json!(thread["resolved"] != true);
                    self.pending = Some(req);
                    self.return_editor = None;
                    self.panel_scroll = 0;
                    self.modal = Some(Modal::Confirm);
                }
            }
            _ => {}
        }
        None
    }
    pub fn fit_panes(&mut self) {
        use ratatui::text::Line;
        let width = |text: &str| Line::from(crate::ui::clean(text)).width();
        let visible = self.visible();
        let annotations = self.annotations();
        // Measure descendants even when their directory is folded. Opening a
        // subtree after fitting should not immediately truncate its names.
        let mut widest = width(&format!("Files · {}/{}", visible.len(), self.files().len())) + 8;
        for category in CATEGORIES {
            if self.category.as_ref().is_some_and(|c| c != category) {
                continue;
            }
            let count = self
                .files()
                .iter()
                .filter(|f| s(f, "category") == category)
                .count();
            widest = widest.max(width(&format!("{category}  {count}")) + 3);
        }
        for index in visible {
            let file = &self.files()[index];
            let parts: Vec<_> = s(file, "filename").split('/').collect();
            let depth = parts.len().saturating_sub(1);
            for (at, directory) in parts.iter().take(depth).enumerate() {
                widest = widest.max(at * 2 + width(directory) + 6);
            }
            let stats = width(&format!(
                "+{} −{}",
                file["additions"].as_u64().unwrap_or(0),
                file["deletions"].as_u64().unwrap_or(0)
            ));
            // Borders (2), selection marker (1), indentation, status (2),
            // basename, gap (1), and the complete + / - statistics.
            widest = widest.max(
                depth * 2
                    + width(parts.last().copied().unwrap_or(""))
                    + stats
                    + 6
                    + annotations.counts(s(file, "filename")).width(),
            );
        }
        self.panes.fit(
            widest.min(u16::MAX as usize) as u16,
            crate::reader::READING_WIDTH + 5,
        );
    }
    fn cycle_pane(&mut self, forward: bool) {
        self.pane = Pane::from_index(self.panes.cycle(self.pane.index(), forward));
        self.enter_native_input();
    }
    fn enter_native_input(&mut self) {
        if self.ai.native_mode && self.pane == Pane::Panel && self.panel == Panel::Ai {
            self.ai.native_input = true;
            self.ai.follow = true;
            self.reader.selection.clear();
        }
    }
    fn rebuild_search(&mut self) {
        // Borrow the PR separately from search so typing never clones its
        // patches, split rows or full-file context.
        let file = array(&self.pr, "files").get(self.file).unwrap_or(&V::Null);
        self.search.rebuild(file, self.split);
    }
    fn update_search(&mut self) {
        self.editor_cursor = self.editor.len();
        self.search.query = self.editor.clone();
        self.rebuild_search();
        let line = self
            .search_origin
            .as_ref()
            .map(|(_, line, _)| *line)
            .unwrap_or(self.line);
        if let Some(hit) = self.search.first_from(line) {
            self.goto_hit(hit);
        }
    }
    fn goto_hit(&mut self, hit: crate::search::Hit) {
        self.line = hit.line;
        self.visual = None;
        let text = crate::search::text(&self.lines()[hit.line]);
        let column = text[..hit.start].chars().count();
        let width = self.panes.areas[1].width as usize;
        let visible = if self.split {
            (width.saturating_sub(5) / 2).saturating_sub(7 + self.annotation_gutter_width())
        } else {
            width.saturating_sub(16 + self.annotation_gutter_width())
        };
        if column < self.horizontal as usize || column >= self.horizontal as usize + visible {
            self.horizontal = column.saturating_sub(8).min(u16::MAX as usize) as u16;
        }
    }
    fn toggle_review(&mut self, expand: Option<bool>) {
        if let Some(row) = self
            .review_list
            .rows(self.my_comments())
            .get(self.panel_cursor)
        {
            let key = row.key.clone();
            let comment = row.comment.is_some();
            self.review_list.toggle(&key, comment, expand);
        }
        self.panel_cursor = self.panel_cursor.min(self.panel_len().saturating_sub(1));
    }
    pub fn mouse(&mut self, event: crossterm::event::MouseEvent) -> Option<V> {
        use crate::review_list::Action;
        use crossterm::event::{MouseButton, MouseEventKind};
        if matches!(event.kind, MouseEventKind::Drag(_)) {
            self.review_list.publish_pressed = None;
        }
        if event.kind == MouseEventKind::Up(MouseButton::Left) {
            if let Some(rect) = self.review_list.publish_pressed.take() {
                if self.modal == Some(Modal::Confirm)
                    && rect.contains((event.column, event.row).into())
                    && self.review_list.hits.iter().any(|(current, action)| {
                        *current == rect && matches!(action, Action::Publish)
                    })
                {
                    return self.key(KeyEvent::new(K::Enter, M::NONE));
                }
                return None;
            }
        }
        if event.kind == crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        {
            let hit = self
                .review_list
                .hits
                .iter()
                .find(|(rect, _)| rect.contains((event.column, event.row).into()))
                .map(|(rect, action)| (*rect, *action));
            self.review_list.publish_pressed = None;
            if let Some((rect, action)) = hit {
                match (self.modal, action) {
                    (None, Action::Row(index)) if self.panel == Panel::Review => {
                        self.pane = Pane::Panel;
                        self.panel_cursor = index;
                        self.toggle_review(None);
                        return None;
                    }
                    (None, Action::Posted(posted)) if self.panel == Panel::Review => {
                        self.pane = Pane::Panel;
                        self.review_list.posted = posted;
                        self.review_list.expanded.clear();
                        self.panel_cursor = 0;
                        self.panel_scroll = 0;
                        return None;
                    }
                    (None, Action::Compose) if self.panel == Panel::Review && !self.busy => {
                        self.open_editor(Input::Review);
                        return None;
                    }
                    (Some(Modal::Editor(Input::Review)), Action::Outcome(outcome)) => {
                        if !self.own_pr() || outcome == 0 {
                            self.review_event = outcome;
                            self.status.clear();
                        }
                        return None;
                    }
                    (Some(Modal::Editor(Input::Review)), Action::Continue) => {
                        self.completion.reset();
                        return self.key(KeyEvent::new(K::Enter, M::NONE));
                    }
                    (Some(Modal::Confirm), Action::Publish) => {
                        self.review_list.publish_pressed = Some(rect);
                        return None;
                    }
                    (Some(Modal::Editor(Input::Review) | Modal::Confirm), Action::Back) => {
                        return self.key(KeyEvent::new(K::Esc, M::NONE));
                    }
                    _ => {}
                }
            }
        }
        if self.modal.is_none()
            && !self.busy
            && event.kind
                == crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        {
            if self
                .review_button
                .contains((event.column, event.row).into())
            {
                self.open_editor(Input::Review);
                return None;
            }
            if self
                .comments_button
                .contains((event.column, event.row).into())
            {
                self.show_review();
                return None;
            }
        }
        if self.modal == Some(Modal::Editor(Input::Chat))
            && self.panel == Panel::Ai
            && self.select_reader_text(event)
        {
            return None;
        }
        if self.modal.is_some() {
            return None;
        }
        if let Some(i) = self.panes.control_at(event) {
            let restoring = self.panes.areas[i].width == 0;
            if self.panes.activate_control(i) {
                let visible = self.panes.visible();
                if restoring {
                    self.pane = Pane::from_index(i);
                    self.enter_native_input();
                } else if !visible.contains(&self.pane.index()) {
                    self.pane = Pane::from_index(if visible.contains(&1) { 1 } else { visible[0] });
                }
            }
            return None;
        }
        if self.select_reader_text(event) {
            return None;
        }
        if let Some(i) = self.panes.mouse(event) {
            self.pane = Pane::from_index(i);
        }
        match event.kind {
            crossterm::event::MouseEventKind::ScrollDown => self.move_cursor(3),
            crossterm::event::MouseEventKind::ScrollUp => self.move_cursor(-3),
            _ => {}
        }
        None
    }
    fn select_reader_text(&mut self, event: crossterm::event::MouseEvent) -> bool {
        if !self.panes.panel || !matches!(self.panel, Panel::Ai | Panel::Overview | Panel::Threads)
        {
            return false;
        }
        let (handled, copy) = self.reader.selection.mouse(event);
        if handled {
            self.pane = Pane::Panel;
            self.ai.follow = false;
            if let Some(text) = copy {
                self.copy_pending = Some(text);
                self.status = "Copying…".into();
            } else if self.reader.selection.active() {
                self.status = "Selecting text · release to copy".into();
            } else if self.status == "Selecting text · release to copy" {
                self.status.clear();
            }
        }
        handled
    }
    fn move_cursor(&mut self, delta: isize) {
        match self.pane {
            Pane::Tree => {
                self.tree_cursor = (self.tree_cursor as isize + delta)
                    .clamp(0, self.rows().len().saturating_sub(1) as isize)
                    as usize;
                if let Some(Row::File(i, _)) = self.rows().get(self.tree_cursor) {
                    self.select_file(*i);
                }
            }
            Pane::Diff => self.move_line(delta),
            Pane::Panel => {
                if matches!(self.panel, Panel::Ai | Panel::Overview) {
                    self.ai.follow = false;
                    self.panel_scroll = (self.panel_scroll as isize + delta).max(0) as usize;
                } else {
                    self.panel_cursor = (self.panel_cursor as isize + delta)
                        .clamp(0, self.panel_len().saturating_sub(1) as isize)
                        as usize;
                    self.panel_scroll = 0;
                    self.review_list.follow = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> V {
        json!({"demo":true,"head":"a","base":"b","url":"sample","files":[{"filename":"src/a.ts","category":"Code","lines":[{"kind":"hunk","hunk":0},{"kind":"context","new":1,"old":1,"hunk":0},{"kind":"add","new":2,"hunk":0},{"kind":"hunk","hunk":1},{"kind":"context","new":20,"old":19,"hunk":1}]},{"filename":"tests/a.test.ts","category":"Tests","lines":[]}],"threads":[],"conversation":[],"commits":[]})
    }
    fn key(c: char) -> KeyEvent {
        KeyEvent::new(K::Char(c), M::NONE)
    }
    #[test]
    fn n_skips_tree_chrome_reveals_folded_files_and_respects_filters() {
        let mut pr = fixture();
        pr["files"]
            .as_array_mut()
            .unwrap()
            .push(json!({"filename":"src/deep/b.ts","category":"Code","lines":[]}));
        let mut a = App::new(pr);
        a.collapsed.insert("src/".into());
        assert_eq!(a.rows().len(), 2);
        a.key(key('n'));
        assert_eq!(a.file, 0);
        assert!(matches!(a.rows()[a.tree_cursor], Row::File(0, _)));
        assert!(!a.collapsed.contains("src/"));
        a.key(key('n'));
        assert_eq!(a.file, 2);
        assert!(matches!(a.rows()[a.tree_cursor], Row::File(2, _)));
        a.key(key('N'));
        assert_eq!(a.file, 0);
        a.filter = "b.ts".into();
        a.tree_cursor = 0;
        a.key(key('n'));
        assert_eq!(a.file, 2);
        a.key(key('n'));
        assert_eq!(a.file, 2, "end of filtered files is stable");
        a.filter.clear();
        a.category = None;
        a.tree_cursor = a
            .rows()
            .iter()
            .position(|row| matches!(row, Row::File(2, _)))
            .unwrap();
        a.key(key('n'));
        assert_eq!(a.file, 1, "category order matches the tree");
        assert_eq!(a.pane, Pane::Tree);
    }
    #[test]
    fn n_in_diff_moves_files_only_when_search_is_empty() {
        let mut a = App::new(fixture());
        a.category = None;
        a.pane = Pane::Diff;
        a.key(key('n'));
        assert_eq!(a.file, 1);
        a.key(key('N'));
        assert_eq!(a.file, 0);
        a.search.query = "no hits".into();
        a.key(key('n'));
        assert_eq!(
            a.file, 0,
            "an active zero-result search must not change files"
        );
        a.pr["demo"] = json!(false);
        a.ai.enabled = true;
        a.panel = Panel::Ai;
        a.pane = Pane::Panel;
        assert_eq!(a.key(key('N')).unwrap()["operation"], "ai_new");
    }
    #[test]
    fn transcript_drag_works_with_docked_composer_and_preserves_draft() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::{
            buffer::Buffer,
            layout::Rect,
            widgets::{Paragraph, Widget},
        };
        let mut a = App::new(fixture());
        a.open_editor(Input::Chat);
        a.paste("Keep this unsent question");
        let area = Rect::new(10, 3, 30, 3);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
        Paragraph::new("Actual Claude answer").render(area, &mut buffer);
        a.reader
            .selection
            .capture(&buffer, area, 0, "session".into());
        for (kind, column) in [
            (MouseEventKind::Down(MouseButton::Left), 10),
            (MouseEventKind::Up(MouseButton::Left), 15),
        ] {
            a.mouse(MouseEvent {
                kind,
                column,
                row: 3,
                modifiers: M::NONE,
            });
        }
        assert_eq!(a.copy_pending.as_deref(), Some("Actual"));
        assert_eq!(a.modal, Some(Modal::Editor(Input::Chat)));
        assert_eq!(a.editor, "Keep this unsent question");
        assert!(!a.ai.follow);
        assert_eq!(a.status, "Copying…");
    }
    #[test]
    fn pr_reading_shortcuts_share_one_timeline_and_preserve_comment_operations() {
        let mut a = App::new(fixture());
        assert!(a.key(key('D')).is_none());
        assert_eq!(a.panel, Panel::Overview);
        assert!(a.panes.panel);
        assert_eq!(a.pane, Pane::Panel);
        assert_eq!(a.reader.destination, Some(crate::reader::Destination::Top));
        a.key(key('C'));
        assert_eq!(a.panel, Panel::Overview);
        assert_eq!(
            a.reader.destination,
            Some(crate::reader::Destination::Comments)
        );
        a.key(key('g'));
        a.key(key('m'));
        assert_eq!(a.panel, Panel::Overview);
        assert_eq!(
            a.reader.destination,
            Some(crate::reader::Destination::Commits)
        );
        a.key(key('i'));
        assert_eq!(a.modal, Some(Modal::Editor(Input::Conversation)));
        a.key(KeyEvent::new(K::Esc, M::NONE));
        a.key(key('b'));
        assert_eq!(a.panel, Panel::Threads);
        assert!(a.pending.is_none());
        assert!(!a.busy);
    }
    #[test]
    fn diff_search_is_incremental_scoped_and_does_not_change_file_filter() {
        let mut a = App::new(fixture());
        a.pr["files"][0]["lines"][1]["text"] = json!("needle needle");
        a.pr["files"][0]["lines"][2]["text"] = json!("new needle");
        a.filter = "a.ts".into();
        a.pane = Pane::Diff;
        a.key(key('/'));
        a.paste("needle");
        assert_eq!(a.search.hits.len(), 3);
        assert_eq!(a.filter, "a.ts");
        assert_eq!(a.line, 1);
        a.key(KeyEvent::new(K::Enter, M::NONE));
        a.key(key('n'));
        assert_eq!(a.search.current().unwrap().start, 7);
        a.key(key('N'));
        assert_eq!(a.search.current().unwrap().start, 0);
        a.key(key('/'));
        a.paste("new");
        a.key(KeyEvent::new(K::Tab, M::NONE));
        assert!(a.search.hits.is_empty());
        a.key(KeyEvent::new(K::Esc, M::NONE));
        assert_eq!(a.search.query, "needle");
        assert_eq!(a.search.scope, crate::search::Scope::Both);
        a.pane = Pane::Tree;
        a.key(key('/'));
        assert_eq!(a.modal, Some(Modal::Editor(Input::Filter)));
    }
    #[test]
    fn review_disclosures_are_closed_and_publication_has_a_separate_action() {
        let mut a = App::new(fixture());
        a.pr["threads"] = json!([{"path":"src/a.ts","line":2,"comments":[{"id":1,"author":"you","body":"Detail","pending":true},{"id":2,"author":"you","body":"Reply"}]}]);
        a.key(key('r'));
        assert_eq!(a.panel_len(), 1);
        a.key(KeyEvent::new(K::Enter, M::NONE));
        assert_eq!(a.panel_len(), 2); // Posted replies stay in their own tab.
        assert!(a.modal.is_none());
        a.key(key('j'));
        a.key(key('l'));
        assert!(a.review_list.expanded.contains("line:1"));
        a.key(key('h'));
        assert!(!a.review_list.expanded.contains("line:1"));
        a.key(key('S'));
        assert_eq!(a.modal, Some(Modal::Editor(Input::Review)));
        a.paste("Summary");
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        assert_eq!(a.modal, Some(Modal::Confirm));
    }
    #[test]
    fn filters_keep_nested_directories() {
        let mut a = App::new(fixture());
        a.key(key('2'));
        assert!(a.rows().contains(&Row::Directory("tests/".into(), 0)));
        assert_eq!(a.visible(), vec![1]);
    }
    #[test]
    fn visual_range_stays_in_one_hunk() {
        let mut a = App::new(fixture());
        a.pane = Pane::Diff;
        a.key(key('v'));
        a.key(key('j'));
        assert_eq!(a.selection().unwrap()["end"], 2);
        a.key(key('j'));
        assert_eq!(a.line, 2);
    }
    #[test]
    fn typed_shortcuts_do_not_mutate() {
        let mut a = App::new(fixture());
        a.open_editor(Input::Comment);
        for c in "mxqr".chars() {
            assert!(a.key(key(c)).is_none());
        }
        assert!(!a.quit);
        assert!(a.hidden.is_empty());
        assert_eq!(a.editor, "mxqr");
    }
    #[test]
    fn review_requires_preview_then_explicit_confirmation() {
        let mut a = App::new(fixture());
        a.open_editor(Input::Review);
        a.editor = "Reviewed".into();
        assert!(a.key(KeyEvent::new(K::Char('s'), M::CONTROL)).is_none());
        assert_eq!(a.modal, Some(Modal::Confirm));
        let req = a.key(key('y')).unwrap();
        assert_eq!(req["operation"], "submit");
        assert_eq!(req["expectedHead"], "a");
    }
    #[test]
    fn hidden_is_not_viewed() {
        let mut a = App::new(fixture());
        a.key(key('x'));
        assert_eq!(a.visible().len(), 0);
        assert_ne!(a.files()[0]["viewed"], true);
        a.key(key('z'));
        assert_eq!(a.visible(), vec![0]);
    }
    #[test]
    fn equal_fits_content_and_keeps_folding_and_zoom() {
        let mut pr = fixture();
        pr["files"][0]["filename"] = json!("src/architecture/deep/longest_component_name.rs");
        pr["files"][0]["additions"] = json!(1234);
        pr["files"][0]["deletions"] = json!(56);
        let mut a = App::new(pr);
        a.panes
            .calculate(ratatui::layout::Rect::new(0, 0, 220, 40), 0);
        a.panes.tree = false;
        a.panes.zoom = true;
        a.key(key('='));
        assert!(
            !a.panes.tree && !a.panes.panel,
            "fit must preserve folded panes"
        );
        assert!(a.panes.zoom, "fit must preserve maximized mode");
        assert_eq!(a.panes.right_width, 89);
        assert!(
            a.panes.left_width > 34,
            "longest entry must determine tree width"
        );
    }
    #[test]
    fn escape_closes_shortcuts_while_filtering() {
        let mut a = App::new(fixture());
        a.key(key('g'));
        a.key(key('?'));
        a.key(key('f'));
        a.key(key('x'));
        a.key(KeyEvent::new(K::Esc, M::NONE));
        assert!(a.modal.is_none());
        assert!(!a.help.open);
    }
    #[test]
    fn fit_includes_collapsed_descendants_and_unicode_display_width() {
        let mut pr = fixture();
        pr["files"] = json!([
            {"filename":"src/deep/nested/変更のとても長い名前.rs","category":"Code","additions":12345,"deletions":6789,"lines":[]},
            {"filename":"test.rs","category":"Tests","lines":[]}
        ]);
        let mut a = App::new(pr);
        a.collapsed.insert("src/".into());
        a.key(key('='));
        let longest = ratatui::text::Line::from("変更のとても長い名前.rs").width();
        assert_eq!(
            a.panes.left_width as usize,
            3 * 2 + longest + "+12345 −6789".chars().count() + 6
        );
        a.category = Some("Tests".into());
        a.key(key('='));
        assert!(a.panes.left_width < 40);
    }
    #[test]
    fn bang_shortcuts_alias_closes_with_a_single_escape() {
        let mut a = App::new(fixture());
        a.key(key('g'));
        a.key(key('!'));
        assert_eq!(a.modal, Some(Modal::Help));
        a.key(key('f'));
        a.key(KeyEvent::new(K::Esc, M::NONE));
        assert!(a.modal.is_none());
        assert!(!a.quit);
    }
    #[test]
    fn tabs_skip_folded_panes_and_hl_do_not_change_focus() {
        let mut a = App::new(fixture());
        a.panes.panel = true;
        a.key(key('l'));
        assert_eq!(a.pane, Pane::Tree);
        a.key(KeyEvent::new(K::Tab, M::NONE));
        assert_eq!(a.pane, Pane::Diff);
        a.key(key('l'));
        assert_eq!(a.pane, Pane::Diff);
        assert_eq!(a.horizontal, 4);
        a.key(KeyEvent::new(K::Tab, M::NONE));
        assert_eq!(a.pane, Pane::Panel);
        a.key(key('\\'));
        a.key(KeyEvent::new(K::Enter, M::NONE));
        assert_eq!(a.pane, Pane::Diff);
        assert!(!a.panes.panel);
        a.key(KeyEvent::new(K::Tab, M::NONE));
        assert_eq!(a.pane, Pane::Tree);
        a.key(KeyEvent::new(K::BackTab, M::SHIFT));
        assert_eq!(a.pane, Pane::Diff);
        a.key(key('\\'));
        a.pane_choice = 0;
        a.key(KeyEvent::new(K::Enter, M::NONE));
        assert!(!a.panes.tree);
        a.key(KeyEvent::new(K::Tab, M::NONE));
        assert_eq!(a.pane, Pane::Diff);
        a.key(key('='));
        assert!(!a.panes.tree && !a.panes.panel);
    }
    #[test]
    fn file_type_headers_cannot_fold_and_help_filter_is_isolated() {
        let mut a = App::new(fixture());
        let rows = a.rows();
        assert!(matches!(rows[0], Row::Category(_)));
        a.key(KeyEvent::new(K::Enter, M::NONE));
        assert_eq!(a.rows(), rows);
        a.key(key('g'));
        a.key(key('?'));
        a.key(key('f'));
        for c in "fold pane".chars() {
            a.key(key(c));
        }
        assert_eq!(a.modal, Some(Modal::Help));
        assert!(!a.help.matches().is_empty());
        assert!(!a.panes.panel && a.panes.tree);
        assert!(a.editor.is_empty());
        assert!(!a.quit);
    }

    #[test]
    fn recursive_tree_actions_and_all_pane_controls_are_consistent() {
        let mut pr = fixture();
        pr["files"] = json!([
            {"filename":"src/a/b/one.rs","category":"Code","lines":[]},
            {"filename":"src/a/c/two.rs","category":"Code","lines":[]}
        ]);
        let mut a = App::new(pr);
        assert!(!a.panes.panel);
        a.tree_cursor = a
            .rows()
            .iter()
            .position(|r| matches!(r,Row::Directory(p,_) if p=="src/a/"))
            .unwrap();
        a.key(key('h'));
        for dir in ["src/a/", "src/a/b/", "src/a/c/"] {
            assert!(a.collapsed.contains(dir));
        }
        a.key(key('l'));
        assert!(a.collapsed.is_empty());
        a.key(key('\\'));
        assert_eq!(a.pane_choice, 2);
        a.key(KeyEvent::new(K::Enter, M::NONE));
        assert!(a.panes.panel);
        for index in [0, 1, 2] {
            a.modal = Some(Modal::Panes);
            a.pane_choice = index;
            a.key(KeyEvent::new(K::Enter, M::NONE));
            assert!(!a.panes.visible().is_empty());
            assert!(a.panes.visible().contains(&a.pane.index()));
        }
    }
    #[test]
    fn enter_submits_a_draft_paste_never_submits_and_uncertain_writes_require_refresh() {
        let mut a = App::new(fixture());
        a.pane = Pane::Diff;
        a.line = 1;
        a.open_editor(Input::Comment);
        a.paste("**Question**\nSecond line @someone #42");
        assert_eq!(a.modal, Some(Modal::Editor(Input::Comment)));
        assert!(!a.busy);
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        assert_eq!(req["operation"], "draft");
        assert!(s(&req, "body").contains('\n'));
        a.receive(
            "draft",
            Err("Action completed, but refresh failed. Press f before another action.".into()),
        );
        assert!(a.write_uncertain);
        assert_eq!(a.modal, Some(Modal::Editor(Input::Comment)));
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        assert!(a.editor.contains("Second line"));
        a.receive("load", Ok(fixture()));
        assert!(!a.write_uncertain);
    }
    #[test]
    fn complex_review_round_trip_through_real_embedded_adapter_uses_sample_only() {
        let mut bridge = crate::backend::Bridge::new().unwrap();
        let mut a = App::new(crate::backend::initial(&mut bridge, None).unwrap());
        a.pane = Pane::Diff;
        let left = a
            .lines()
            .iter()
            .position(|v| v["kind"] == "delete")
            .unwrap();
        let right = a.lines().iter().position(|v| v["kind"] == "add").unwrap();
        for index in [left, right] {
            a.line = index;
            a.key(key(' '));
        }
        assert_eq!(a.ranges.len(), 2);
        a.key(key('c'));
        a.paste("**Check both sides**\n- [ ] Add a regression test");
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        assert_eq!(req["ranges"].as_array().unwrap().len(), 2);
        a.receive("draft", bridge.request(&req));
        assert_eq!(a.pending_comments().len(), 1);
        assert!(s(a.pending_comments()[0], "body").contains("Related code:"));
        assert!(a.ranges.is_empty());
        a.line = right;
        a.key(key('c'));
        a.paste("Second observation");
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        a.receive("draft", bridge.request(&req));
        assert_eq!(a.pending_comments().len(), 2);
        a.key(key('r'));
        assert_eq!(a.panel, Panel::Review);
        assert_eq!(
            a.my_comments()
                .iter()
                .filter(|c| c["pending"] == true)
                .count(),
            2
        );
        a.key(key('S'));
        a.paste("Reviewed both sides and the related code.");
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        assert_eq!(a.modal, Some(Modal::Confirm));
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        a.receive("submit", bridge.request(&req));
        assert!(a.pending_comments().is_empty());
        a.key(key('b'));
        a.panel_cursor = array(&a.pr, "threads").len() - 1;
        a.key(key('i'));
        a.paste("Reply with `code` and **Markdown**.");
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        a.receive("reply", bridge.request(&req));
        assert_eq!(array(a.selected_thread(), "comments").len(), 2);
        a.key(key('R'));
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        a.receive("resolve", bridge.request(&req));
        assert_eq!(a.selected_thread()["resolved"], true);
        a.key(key('R'));
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        a.receive("resolve", bridge.request(&req));
        assert_eq!(a.selected_thread()["resolved"], false);
    }
    #[test]
    fn autocomplete_acceptance_does_not_submit_and_async_results_do_not_unlock_writes() {
        let mut pr = fixture();
        pr["author"] = json!("alice");
        pr["viewer"] = json!("bob");
        let mut a = App::new(pr);
        a.line = 1;
        a.open_editor(Input::Comment);
        for c in "@al".chars() {
            a.key(key(c));
        }
        assert!(a.completion.open);
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        assert_eq!(a.editor, "@alice ");
        assert_eq!(a.modal, Some(Modal::Editor(Input::Comment)));
        assert!(!a.busy);
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        assert_eq!(req["operation"], "draft");
        assert!(a.busy);
        a.receive("suggest", Ok(json!({"id":0,"items":[]})));
        assert!(a.busy);
    }
    #[test]
    fn changed_head_blocks_old_composer_anchors_and_unicode_cursor_editing_works() {
        let mut a = App::new(fixture());
        a.line = 1;
        a.open_editor(Input::Comment);
        a.paste("αβ\nGamma");
        a.key(KeyEvent::new(K::Home, M::NONE));
        a.key(KeyEvent::new(K::Up, M::NONE));
        assert_eq!(a.editor_cursor, 0);
        a.key(KeyEvent::new(K::Right, M::NONE));
        a.key(key('!'));
        assert!(a.editor.starts_with("α!β"));
        a.pr["head"] = json!("new-head");
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        assert!(a.status.contains("PR changed"));
        assert!(a.editor.contains("Gamma"));
    }
    #[test]
    fn failed_write_keeps_composer() {
        let mut a = App::new(fixture());
        a.editor = "unsent".into();
        a.return_editor = Some(Input::Review);
        a.receive("submit", Err("stale head".into()));
        assert_eq!(a.editor, "unsent");
        assert_eq!(a.modal, Some(Modal::Editor(Input::Review)));
    }
    #[test]
    fn temporary_notes_span_files_and_never_create_github_comments() {
        let mut bridge = crate::backend::Bridge::new().unwrap();
        let mut a = App::new(crate::backend::initial(&mut bridge, None).unwrap());
        a.pane = Pane::Diff;
        let pending = a.pending_comments().len();
        for file in [0, 1] {
            a.select_file(file);
            a.line = a.lines().iter().position(|l| l["kind"] == "add").unwrap();
            a.key(key('v'));
            a.key(key('t'));
            assert_eq!(a.modal, Some(Modal::Editor(Input::AiNote)));
            a.paste("How does this relate to the other change?");
            assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        }
        assert_eq!(a.context.len(), 2);
        assert_ne!(a.context[0]["path"], a.context[1]["path"]);
        assert!(
            a.context
                .iter()
                .all(|c| !s(c, "note").is_empty() && !s(c, "content").is_empty())
        );
        assert_eq!(a.pending_comments().len(), pending);
        a.key(key('g'));
        a.key(key('c'));
        assert_eq!(a.modal, Some(Modal::Context));
        a.key(key('i'));
        a.paste("Explain these together");
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        assert_eq!(req["operation"], "chat");
        assert_eq!(req["context"].as_array().unwrap().len(), 2);
        a.receive("chat", Err("Stopped".into()));
        assert_eq!(a.context.len(), 2);
        assert_eq!(a.modal, Some(Modal::Editor(Input::Chat)));
        let req = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        a.receive("chat", bridge.request(&req));
        assert!(a.context.is_empty());
        assert_eq!(a.pending_comments().len(), pending);
    }
    #[test]
    fn review_outcomes_summary_and_snapshot_are_validated_before_publication() {
        let mut a = App::new(fixture());
        a.open_editor(Input::Review);
        assert!(a.compose_request(Input::Review).is_err());
        a.review_event = 1;
        assert_eq!(
            a.compose_request(Input::Review).unwrap()["event"],
            "APPROVE"
        );
        a.editor = "x".repeat(60_001);
        assert!(
            a.compose_request(Input::Review)
                .unwrap_err()
                .contains("60,000")
        );
        a.editor = "😀".repeat(30_001);
        assert!(
            a.compose_request(Input::Review)
                .unwrap_err()
                .contains("60,000")
        );
        a.editor.clear();
        a.review_event = 2;
        assert!(a.compose_request(Input::Review).is_err());
        a.pr["threads"] = json!([{"path":"src/a.ts","comments":[{"author":"you","pending":true,"body":"Fix boundary"}]}]);
        assert_eq!(
            a.compose_request(Input::Review).unwrap()["event"],
            "REQUEST_CHANGES"
        );
        a.pr["author"] = json!("YOU");
        assert!(
            a.compose_request(Input::Review)
                .unwrap_err()
                .contains("Your PR")
        );
        a.review_event = 0;
        assert!(a.compose_request(Input::Review).is_ok());
        a.key(KeyEvent::new(K::Tab, M::NONE));
        assert_eq!(a.review_event, 0);
        a.pr["base"] = json!("changed-base");
        assert!(
            a.compose_request(Input::Review)
                .unwrap_err()
                .contains("PR changed")
        );
        a.pr["base"] = json!("b");
        a.pr["head"] = json!("changed");
        assert!(
            a.compose_request(Input::Review)
                .unwrap_err()
                .contains("PR changed")
        );
    }
    #[test]
    fn review_summary_survives_back_reopen_and_failure_and_clears_only_on_success() {
        let mut a = App::new(fixture());
        a.pr["pendingReview"] = json!({"body":"Summary already saved on GitHub"});
        a.open_editor(Input::Review);
        assert_eq!(a.editor, "Summary already saved on GitHub");
        a.editor = "My local summary".into();
        a.editor_cursor = a.editor.len();
        a.review_event = 2;
        a.key(KeyEvent::new(K::Esc, M::NONE));
        a.open_editor(Input::Review);
        assert_eq!(a.editor, "My local summary");
        assert_eq!(a.review_event, 2);
        a.key(KeyEvent::new(K::Enter, M::NONE));
        assert_eq!(a.modal, Some(Modal::Confirm));
        a.key(KeyEvent::new(K::Esc, M::NONE));
        assert_eq!(a.editor, "My local summary");
        a.key(KeyEvent::new(K::Enter, M::NONE));
        let request = a.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        assert_eq!(request["event"], "REQUEST_CHANGES");
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        a.receive("submit", Err("HTTP 403: permission denied".into()));
        assert_eq!(a.modal, Some(Modal::Editor(Input::Review)));
        assert_eq!(a.editor, "My local summary");
        a.key(KeyEvent::new(K::Enter, M::NONE));
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_some());
        a.receive("submit", Ok(fixture()));
        assert!(a.review_list.summary.is_empty());
        assert!(a.editor.is_empty());
        assert!(a.review_list.posted);
    }
    #[test]
    fn review_confirmation_rechecks_snapshot_and_uncertain_writes_never_retry() {
        let mut a = App::new(fixture());
        a.open_editor(Input::Review);
        a.paste("Summary");
        a.key(KeyEvent::new(K::Enter, M::NONE));
        a.pr["head"] = json!("new-head");
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        assert!(!a.busy);
        assert!(a.pending.is_none());
        assert_eq!(a.modal, Some(Modal::Editor(Input::Review)));
        assert_eq!(a.editor, "Summary");
        a.editor_head = a.pr["head"].clone();
        a.key(KeyEvent::new(K::Enter, M::NONE));
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_some());
        a.receive(
            "submit",
            Err("Action completed, but refresh failed. Press f before another action.".into()),
        );
        assert!(a.write_uncertain);
        assert!(a.key(KeyEvent::new(K::Enter, M::NONE)).is_none());
        assert_eq!(a.editor, "Summary");
        assert!(a.status.contains("Refresh first"));
    }
    #[test]
    fn review_tabs_and_single_detail_keep_history_and_private_notes_out_of_drafts() {
        let mut a = App::new(fixture());
        a.pr["threads"] = json!([{"path":"src/a.ts","comments":[{"id":1,"author":"you","pending":true,"body":"One"},{"id":2,"author":"you","pending":true,"body":"Two"},{"id":3,"author":"you","body":"Posted"}]}]);
        a.context
            .push(json!({"note":"Private AI note","path":"src/a.ts"}));
        a.key(key('r'));
        a.key(key('l'));
        assert_eq!(a.panel_len(), 3);
        a.key(key('j'));
        a.key(key('l'));
        assert!(a.review_list.expanded.contains("line:1"));
        a.key(key('j'));
        a.key(key('l'));
        assert!(a.review_list.expanded.contains("line:2"));
        assert!(!a.review_list.expanded.contains("line:1"));
        a.key(key('P'));
        assert!(a.review_list.posted);
        a.key(key('l'));
        assert_eq!(a.panel_len(), 2);
        assert_eq!(a.pending_comments().len(), 2);
        assert_eq!(a.my_comments().len(), 3);
    }

    #[test]
    fn review_publish_mouse_requires_release_inside_and_never_publishes_a_drag() {
        use crate::review_list::Action;
        use crossterm::event::{MouseButton as B, MouseEvent, MouseEventKind as E};
        let mut a = App::new(fixture());
        a.open_editor(Input::Review);
        a.paste("A review");
        a.key(KeyEvent::new(K::Enter, M::NONE));
        let rect = ratatui::layout::Rect::new(10, 10, 18, 1);
        a.review_list.hits = vec![(rect, Action::Publish)];
        let click = |kind, x| MouseEvent {
            kind,
            column: x,
            row: 10,
            modifiers: M::NONE,
        };
        assert!(a.mouse(click(E::Down(B::Left), 12)).is_none());
        assert!(!a.busy);
        a.mouse(click(E::Drag(B::Left), 13));
        assert!(a.mouse(click(E::Up(B::Left), 13)).is_none());
        a.mouse(click(E::Down(B::Left), 12));
        assert!(a.mouse(click(E::Up(B::Left), 30)).is_none());
        a.mouse(click(E::Down(B::Left), 12));
        a.review_list.hits = vec![(ratatui::layout::Rect::new(11, 10, 18, 1), Action::Publish)];
        assert!(a.mouse(click(E::Up(B::Left), 12)).is_none()); // Resizing cancels an armed button.
        a.review_list.hits = vec![(rect, Action::Publish)];
        a.mouse(click(E::Down(B::Left), 12));
        assert_eq!(
            a.mouse(click(E::Up(B::Left), 12)).unwrap()["operation"],
            "submit"
        );
        a.mouse(click(E::Down(B::Left), 12));
        assert!(a.mouse(click(E::Up(B::Left), 12)).is_none());
    }
}
