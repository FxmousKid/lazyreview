use crate::{app::s, help::Help, layout::Panes, theme::current as theme, ui::clean};
use crossterm::event::{
    KeyCode as K, KeyEvent, KeyModifiers as M, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    Frame,
    layout::{Constraint as C, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use serde_json::{Value as V, json};
use std::time::{Duration, Instant};
const SCOPES: [&str; 4] = ["repository", "requested", "authored", "repository"];
#[derive(Clone, Copy, PartialEq, Debug)]
enum Editing {
    Filter,
    Repository,
    Url,
}
#[derive(Default)]
struct Search {
    open: bool,
    loaded: bool,
    busy: bool,
    query: String,
    selected: usize,
    repository: Option<String>,
    all_authors: bool,
    repos: Vec<V>,
    own: Vec<V>,
    external: Vec<V>,
    external_repo: String,
    warning: String,
    error: String,
    area: Rect,
    list_area: Rect,
    visible_rows: Vec<Rect>,
    offset: usize,
}
// Match each whitespace-separated word as a subsequence. Keep the original
// recency order rather than promoting a fuzzy score above the newest work.
fn fuzzy(query: &str, text: &str) -> bool {
    let text = text.to_lowercase();
    query.to_lowercase().split_whitespace().all(|word| {
        let mut chars = text.chars();
        word.chars().all(|c| chars.by_ref().any(|v| v == c))
    })
}
fn repository_name(text: &str) -> bool {
    let mut parts = text.split('/');
    let valid = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
    };
    matches!((parts.next(), parts.next(), parts.next()), (Some(a),Some(b),None) if valid(a)&&valid(b))
}
pub struct Picker {
    pub items: Vec<V>,
    pub repos: Vec<V>,
    pub repo_cursor: usize,
    pub focus: usize,
    pub scope: usize,
    pub repository: String,
    pub filter: String,
    pub selected: usize,
    pub page: usize,
    pub total: usize,
    pub has_next: bool,
    pub incomplete: bool,
    pub capped: bool,
    pub viewer: String,
    pub busy: bool,
    pub started: Instant,
    pub status: String,
    pub loading_pr: bool,
    pub quit: bool,
    pub back: bool,
    pub has_review: bool,
    pub panes: Panes,
    pub help: Help,
    query: String,
    repo_page: usize,
    repo_next: bool,
    repo_total: usize,
    repo_warning: String,
    editing: Option<Editing>,
    buffer: String,
    previous_g: Option<Instant>,
    search: Search,
    search_button: Rect,
}
impl Picker {
    pub fn new() -> Self {
        Self {
            items: vec![],
            repos: vec![],
            repo_cursor: 0,
            focus: 0,
            scope: 0,
            repository: String::new(),
            filter: String::new(),
            selected: 0,
            page: 1,
            total: 0,
            has_next: false,
            incomplete: false,
            capped: false,
            viewer: String::new(),
            busy: false,
            started: Instant::now(),
            status: String::new(),
            loading_pr: false,
            quit: false,
            back: false,
            has_review: false,
            panes: Panes::chooser(),
            help: Help::default(),
            query: String::new(),
            repo_page: 1,
            repo_next: false,
            repo_total: 0,
            repo_warning: String::new(),
            editing: None,
            buffer: String::new(),
            previous_g: None,
            search: Search::default(),
            search_button: Rect::default(),
        }
    }
    fn open_search(&mut self) -> Option<V> {
        self.search.open = true;
        self.search.repository = None;
        self.search.query.clear();
        self.search.selected = 0;
        self.search.error.clear();
        if self.search.loaded || self.search.busy {
            return None;
        }
        self.search.busy = true;
        Some(json!({"operation":"history"}))
    }
    pub fn receive_history(&mut self, result: Result<V, String>) {
        self.search.busy = false;
        match result {
            Err(e) => self.search.error = e,
            Ok(data) => {
                self.search.error.clear();
                self.search.warning = s(&data, "warning").into();
                if data["kind"] == "pulls" {
                    self.search.external_repo = s(&data, "repository").into();
                    self.search.external = data["items"].as_array().cloned().unwrap_or_default();
                } else {
                    self.search.loaded = true;
                    self.search.repos =
                        data["repositories"].as_array().cloned().unwrap_or_default();
                    self.search.own = data["items"].as_array().cloned().unwrap_or_default();
                }
                self.search.selected = 0;
            }
        }
    }
    fn search_items(&self) -> Vec<V> {
        let q = self.search.query.trim();
        if let Some(repo) = &self.search.repository {
            let items = if self.search.all_authors {
                &self.search.external
            } else {
                &self.search.own
            };
            items
                .iter()
                .filter(|v| {
                    s(v, "repository").eq_ignore_ascii_case(repo)
                        && fuzzy(
                            q,
                            &format!("#{} {} {}", v["number"], s(v, "title"), s(v, "author")),
                        )
                })
                .cloned()
                .collect()
        } else {
            let mut items: Vec<V> = self
                .search
                .repos
                .iter()
                .filter(|v| fuzzy(q, s(v, "name")))
                .cloned()
                .collect();
            if repository_name(q) && !items.iter().any(|v| s(v, "name").eq_ignore_ascii_case(q)) {
                items.insert(0, json!({"name":q,"external":true}));
            }
            items
        }
    }
    fn search_choose(&mut self) -> Option<V> {
        if self.search.busy {
            return None;
        }
        let item = self.search_items().get(self.search.selected)?.clone();
        if self.search.repository.is_some() {
            self.search.open = false;
            return Some(self.open(s(&item, "url")));
        }
        let repo = s(&item, "name").to_string();
        self.search.repository = Some(repo.clone());
        self.search.query.clear();
        self.search.selected = 0;
        self.search.error.clear();
        self.search.all_authors = item["external"] == true;
        if self.search.all_authors && self.search.external_repo != repo {
            self.search.busy = true;
            self.search.external.clear();
            return Some(json!({"operation":"history","repository":repo}));
        }
        None
    }
    fn search_key(&mut self, key: KeyEvent) -> Option<V> {
        let count = self.search_items().len();
        match key.code {
            K::Esc => self.search.open = false,
            K::Up | K::Char('k') if key.code == K::Up || key.modifiers.contains(M::CONTROL) => {
                self.search.selected = self.search.selected.saturating_sub(1)
            }
            K::Down | K::Char('j') if key.code == K::Down || key.modifiers.contains(M::CONTROL) => {
                self.search.selected = (self.search.selected + 1).min(count.saturating_sub(1))
            }
            K::PageDown => {
                self.search.selected = (self.search.selected + 8).min(count.saturating_sub(1))
            }
            K::PageUp => self.search.selected = self.search.selected.saturating_sub(8),
            K::Enter => return self.search_choose(),
            K::Left if key.modifiers.contains(M::ALT) && !self.search.busy => {
                self.search.repository = None;
                self.search.query.clear();
                self.search.selected = 0;
                self.search.error.clear();
            }
            K::Backspace => {
                self.search.query.pop();
                self.search.selected = 0;
            }
            K::Char('u') if key.modifiers.contains(M::CONTROL) => {
                self.search.query.clear();
                self.search.selected = 0;
            }
            K::Char('r') if key.modifiers.contains(M::CONTROL) && !self.search.busy => {
                self.search.busy = true;
                self.search.error.clear();
                return Some(
                    json!({"operation":"history","repository":if self.search.all_authors {self.search.repository.clone()} else {None}}),
                );
            }
            K::Tab if self.search.repository.is_some() && !self.search.busy => {
                self.search.all_authors = !self.search.all_authors;
                self.search.selected = 0;
                self.search.error.clear();
                let repo = self.search.repository.as_ref().unwrap();
                if self.search.all_authors && self.search.external_repo != *repo {
                    self.search.busy = true;
                    self.search.external.clear();
                    return Some(json!({"operation":"history","repository":repo}));
                }
            }
            K::Char(c) if !key.modifiers.intersects(M::CONTROL | M::ALT) => {
                if self.search.query.len() < 200 {
                    self.search.query.push(c);
                    self.search.selected = 0;
                }
            }
            _ => {}
        }
        None
    }
    pub fn paste(&mut self, text: &str) {
        let value: String = clean(text).chars().filter(|c| !c.is_control()).collect();
        if self.search.open {
            self.search.query.extend(
                value
                    .chars()
                    .take(200usize.saturating_sub(self.search.query.chars().count())),
            );
            self.search.selected = 0;
        } else if self.editing.is_some() {
            self.buffer.extend(
                value
                    .chars()
                    .take(500usize.saturating_sub(self.buffer.chars().count())),
            );
        }
    }
    pub fn home(&mut self) -> V {
        self.query.clear();
        self.repo_page = 1;
        self.repo_request()
    }
    fn repo_request(&mut self) -> V {
        self.busy = true;
        self.started = Instant::now();
        self.loading_pr = false;
        self.focus = 0;
        self.repos.clear();
        self.items.clear();
        self.repo_cursor = 0;
        self.selected = 0;
        self.repository.clear();
        self.repo_warning.clear();
        self.repo_next = false;
        self.filter.clear();
        self.status = if self.query.is_empty() {
            "Finding repositories with your open PRs…"
        } else {
            "Searching repositories…"
        }
        .into();
        json!({"operation":"repositories","query":self.query,"page":self.repo_page})
    }
    pub fn request(&mut self) -> V {
        self.busy = true;
        self.started = Instant::now();
        self.loading_pr = false;
        self.items.clear();
        self.selected = 0;
        self.has_next = false;
        self.status = "Fetching open pull requests…".into();
        json!({"operation":"discover","scope":SCOPES[self.scope],"repository":self.repository,"page":self.page})
    }
    pub fn open(&mut self, url: &str) -> V {
        self.busy = true;
        self.started = Instant::now();
        self.loading_pr = true;
        self.status = "Opening pull request…".into();
        json!({"operation":"load","url":url})
    }
    pub fn receive_repos(&mut self, result: Result<V, String>) {
        self.busy = false;
        self.loading_pr = false;
        match result {
            Err(e) => self.status = e,
            Ok(data) => {
                self.repos = data["repositories"].as_array().cloned().unwrap_or_default();
                if data["viewer"].is_string() {
                    self.viewer = s(&data, "viewer").into();
                }
                self.repo_next = data["hasNext"] == true;
                self.repo_total = data["total"].as_u64().unwrap_or(0) as usize;
                self.repo_warning = s(&data, "warning").into();
                if data["incomplete"] == true {
                    self.repo_warning
                        .push_str(" GitHub returned partial search results.");
                }
                if data["capped"] == true {
                    self.repo_warning
                        .push_str(" First 1,000 repositories; narrow your search.");
                }
                self.status = if self.repos.is_empty() {
                    "No repositories found. Press / to search by name or owner/repo."
                } else {
                    "Select a repository and Enter to see its open PRs."
                }
                .into();
            }
        }
    }
    pub fn receive(&mut self, result: Result<V, String>) {
        self.busy = false;
        self.loading_pr = false;
        match result {
            Err(e) => self.status = e,
            Ok(data) => {
                self.items = data["items"].as_array().cloned().unwrap_or_default();
                if data["viewer"].is_string() {
                    self.viewer = s(&data, "viewer").into();
                }
                self.page = data["page"].as_u64().unwrap_or(1) as usize;
                self.total = data["total"].as_u64().unwrap_or(0) as usize;
                self.has_next = data["hasNext"] == true;
                self.incomplete = data["incomplete"] == true;
                self.capped = data["capped"] == true;
                self.selected = 0;
                self.status = if self.items.is_empty() {
                    "No open PRs here. Press / to search your full history."
                } else {
                    "Enter opens the selected PR · newest updates first"
                }
                .into();
            }
        }
    }
    pub fn filtered(&self) -> Vec<usize> {
        let q = self.filter.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, v)| {
                fuzzy(
                    &q,
                    &format!(
                        "{} {} #{} {}",
                        s(v, "title"),
                        s(v, "repository"),
                        v["number"],
                        s(v, "author")
                    ),
                )
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn edit(&mut self, kind: Editing) {
        self.editing = Some(kind);
        self.buffer = match kind {
            Editing::Filter => self.filter.clone(),
            Editing::Repository => self.query.clone(),
            Editing::Url => String::new(),
        };
    }
    pub fn key(&mut self, key: KeyEvent) -> Option<V> {
        if key.modifiers.contains(M::CONTROL) && key.code == K::Char('c') {
            self.quit = true;
            return None;
        }
        if self.search.open {
            return self.search_key(key);
        }
        if self.help.open {
            self.help.key(key);
            return None;
        }
        if let Some(kind) = self.editing {
            match key.code {
                K::Esc => self.editing = None,
                K::Enter => {
                    self.editing = None;
                    match kind {
                        Editing::Filter => {
                            self.filter = self.buffer.clone();
                            self.selected = 0;
                        }
                        Editing::Repository => {
                            self.query = self.buffer.trim().into();
                            self.repo_page = 1;
                            return Some(self.repo_request());
                        }
                        Editing::Url => {
                            let url = self.buffer.trim().to_string();
                            return Some(self.open(&url));
                        }
                    }
                }
                K::Backspace => {
                    self.buffer.pop();
                }
                K::Char('u') if key.modifiers.contains(M::CONTROL) => self.buffer.clear(),
                K::Char(c) if !key.modifiers.intersects(M::CONTROL | M::ALT) => {
                    if self.buffer.len() < 500 {
                        self.buffer.push(c);
                    }
                }
                _ => {}
            }
            return None;
        }
        if key.code == K::Char('/') && self.editing.is_none() {
            return self.open_search();
        }
        // Help, pane focus and sizing stay usable while network work is running.
        match key.code {
            K::Char('?') => {
                self.help.show();
                return None;
            }
            K::Char('h') => {
                self.focus = 0;
                return None;
            }
            K::Char('l') => {
                self.focus = 1;
                return None;
            }
            K::Tab | K::BackTab => {
                self.focus = 1 - self.focus;
                return None;
            }
            K::Char('+' | '-') => {
                self.panes
                    .resize(self.focus, if key.code == K::Char('+') { 4 } else { -4 });
                return None;
            }
            K::Char('Z') => {
                self.panes.zoom = !self.panes.zoom;
                return None;
            }
            K::Char('=') => {
                self.panes = Panes::chooser();
                return None;
            }
            K::Char('T') => {
                self.status = match crate::theme::cycle() {
                    Ok(name) => format!("Theme: {name}"),
                    Err(e) => e,
                };
                return None;
            }
            K::Char('q') => {
                self.quit = true;
                return None;
            }
            _ => {}
        }
        if self.busy {
            return None;
        }
        let n = if self.focus == 0 {
            self.repos.len()
        } else {
            self.filtered().len()
        };
        let current = if self.focus == 0 {
            self.repo_cursor
        } else {
            self.selected
        };
        let mut selected = current;
        match key.code {
            K::Esc if self.has_review => self.back = true,
            K::Char('j') | K::Down => selected = (current + 1).min(n.saturating_sub(1)),
            K::Char('k') | K::Up => selected = current.saturating_sub(1),
            K::Char('d') if key.modifiers.contains(M::CONTROL) => {
                selected = (current + 10).min(n.saturating_sub(1))
            }
            K::Char('u') if key.modifiers.contains(M::CONTROL) => {
                selected = current.saturating_sub(10)
            }
            K::Char('G') | K::End => selected = n.saturating_sub(1),
            K::Home => selected = 0,
            K::Char('g') => {
                if self
                    .previous_g
                    .is_some_and(|at| at.elapsed() < Duration::from_millis(900))
                {
                    selected = 0;
                    self.previous_g = None;
                } else {
                    self.previous_g = Some(Instant::now());
                }
            }
            K::Char('f') => self.edit(if self.focus == 0 {
                Editing::Repository
            } else {
                Editing::Filter
            }),
            K::Char('o') => self.edit(Editing::Url),
            K::Char('1') => return Some(self.home()),
            K::Char(c @ '2'..='3') => {
                self.scope = c as usize - '1' as usize;
                self.page = 1;
                self.filter.clear();
                self.repository.clear();
                self.focus = 1;
                return Some(self.request());
            }
            K::Char('4') => {
                self.focus = 0;
                self.edit(Editing::Repository);
            }
            K::Char('r') => {
                return Some(
                    if self.focus == 0 || self.repository.is_empty() && self.scope == 0 {
                        self.repo_request()
                    } else {
                        self.request()
                    },
                );
            }
            K::Char('n' | 'p') => {
                let next = key.code == K::Char('n');
                if self.focus == 0 {
                    if !self.query.is_empty()
                        && ((next && self.repo_next) || (!next && self.repo_page > 1))
                    {
                        self.repo_page = if next {
                            self.repo_page + 1
                        } else {
                            self.repo_page - 1
                        };
                        return Some(self.repo_request());
                    }
                } else if (next && self.has_next) || (!next && self.page > 1) {
                    self.page = if next { self.page + 1 } else { self.page - 1 };
                    return Some(self.request());
                }
            }
            K::Enter => {
                if self.focus == 0 {
                    if let Some(repo) = self.repos.get(self.repo_cursor) {
                        self.repository = s(repo, "name").into();
                        self.scope = 0;
                        self.page = 1;
                        self.filter.clear();
                        self.focus = 1;
                        return Some(self.request());
                    }
                } else if let Some(&i) = self.filtered().get(self.selected) {
                    let url = s(&self.items[i], "url").to_string();
                    return Some(self.open(&url));
                }
            }
            K::Char('d') => {
                self.busy = true;
                self.started = Instant::now();
                self.loading_pr = true;
                self.status = "Opening sample…".into();
                return Some(json!({"operation":"sample"}));
            }
            _ => {}
        }
        if self.focus == 0 {
            self.repo_cursor = selected;
        } else {
            self.selected = selected;
        }
        None
    }
    pub fn mouse(&mut self, event: MouseEvent) -> Option<V> {
        let point = (event.column, event.row).into();
        if self.search.open {
            match event.kind {
                MouseEventKind::ScrollDown => {
                    self.search.selected =
                        (self.search.selected + 1).min(self.search_items().len().saturating_sub(1));
                }
                MouseEventKind::ScrollUp => {
                    self.search.selected = self.search.selected.saturating_sub(1);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if !self.search.area.contains(point) {
                        self.search.open = false;
                    } else if let Some(i) = self
                        .search
                        .visible_rows
                        .iter()
                        .position(|r| r.contains(point))
                    {
                        self.search.selected = self.search.offset + i;
                        return self.search_choose();
                    }
                }
                _ => {}
            }
            return None;
        }
        if self.editing.is_none() && !self.help.open {
            if event.kind == MouseEventKind::Down(MouseButton::Left)
                && self.search_button.contains(point)
            {
                return self.open_search();
            }
            if let Some(i) = self.panes.mouse(event) {
                self.focus = i.min(1);
            }
        }
        None
    }
    pub fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        let t = theme();
        f.render_widget(
            Block::default().style(Style::default().bg(t.bg).fg(t.fg)),
            area,
        );
        let rows = Layout::vertical([
            C::Length(1),
            C::Length(2),
            C::Min(4),
            C::Length(1),
            C::Length(2),
        ])
        .split(area);
        f.render_widget(
            Paragraph::new(" ◉ LazyReview").style(Style::default().fg(t.accent)),
            rows[0],
        );
        self.search_button = Rect::new(
            rows[0].right().saturating_sub(15),
            rows[0].y,
            15.min(rows[0].width),
            1,
        );
        f.render_widget(
            Paragraph::new(" ⌕ Search / ").style(theme().selected),
            self.search_button,
        );
        let label = match self.editing {
            Some(Editing::Repository) => "Search repositories",
            Some(Editing::Url) => "GitHub PR URL",
            Some(Editing::Filter) => "Filter PRs",
            None if self.focus == 0 => "⌕ Repository",
            None => "⌕ Pull request",
        };
        let value = if self.editing.is_some() {
            &self.buffer
        } else if self.focus == 0 {
            &self.query
        } else {
            &self.filter
        };
        f.render_widget(
            Paragraph::new(format!(
                " {label}: {}{}",
                clean(value),
                if self.editing.is_some() { "▏" } else { "" }
            ))
            .style(Style::default().fg(t.accent)),
            rows[1],
        );
        let areas = self.panes.calculate(rows[2], self.focus);
        if areas[0].width > 0 {
            self.repo_view(f, areas[0]);
        }
        if areas[1].width > 0 {
            self.pr_view(f, areas[1]);
        }
        let warning = format!(
            " {}{}{} {}",
            clean(&self.status),
            if self.incomplete {
                " · Partial PR results."
            } else {
                ""
            },
            if self.capped {
                " · First 1,000 PRs; narrow by repository."
            } else {
                ""
            },
            clean(&self.repo_warning)
        );
        f.render_widget(
            Paragraph::new(warning)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(t.muted)),
            rows[3],
        );
        f.render_widget(
            Paragraph::new(" h/l Panes · j/k Select · Enter Open · / Search · f Filter · ? Keys")
                .style(Style::default().fg(t.muted)),
            rows[4],
        );
        if self.busy {
            crate::activity::render(
                f,
                if self.loading_pr {
                    "Opening pull request"
                } else if self.focus == 0 {
                    "Loading repositories"
                } else {
                    "Loading pull requests"
                },
                self.started,
            );
        }
        if self.help.open {
            self.help.render(f);
        }
        if self.search.open {
            self.render_search(f);
        }
    }
    fn render_search(&mut self, f: &mut Frame) {
        let area = f.area();
        let width = area.width.saturating_sub(4).min(108);
        let height = area.height.saturating_sub(4).min(30);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        self.search.area = popup;
        f.render_widget(Clear, popup);
        let title = if let Some(repo) = &self.search.repository {
            format!(
                " {} · {} ",
                clean(repo),
                if self.search.all_authors {
                    "All PRs"
                } else {
                    "Your PRs"
                }
            )
        } else {
            " Search repositories ".into()
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme().active)
            .style(Style::default().bg(theme().bg).fg(theme().fg));
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        let rows =
            Layout::vertical([C::Length(2), C::Min(1), C::Length(2), C::Length(1)]).split(inner);
        f.render_widget(
            Paragraph::new(format!(" ⌕ {}▏", clean(&self.search.query)))
                .style(Style::default().fg(theme().accent)),
            rows[0],
        );
        let items = self.search_items();
        let visible = (rows[1].height as usize / 2).max(1);
        self.search.selected = self.search.selected.min(items.len().saturating_sub(1));
        if self.search.selected < self.search.offset {
            self.search.offset = self.search.selected;
        }
        if self.search.selected >= self.search.offset + visible {
            self.search.offset = self.search.selected + 1 - visible;
        }
        self.search.offset = self.search.offset.min(items.len().saturating_sub(visible));
        self.search.list_area = rows[1];
        self.search.visible_rows.clear();
        if self.search.busy {
            let glyph = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
                [(self.started.elapsed().as_millis() / 100) as usize % 10];
            f.render_widget(
                Paragraph::new(format!(
                    " {glyph} Loading {}…",
                    if self.search.repository.is_some() {
                        "pull requests"
                    } else {
                        "your repositories and PR history"
                    }
                )),
                rows[1],
            );
        } else if items.is_empty() {
            let text = if self.search.repository.is_none() {
                " No matches · Enter owner/repo to browse."
            } else if self.search.all_authors {
                " No matching PRs."
            } else {
                " No matching PRs by you · Tab All authors"
            };
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), rows[1]);
        } else {
            for (row, (index, v)) in items
                .iter()
                .enumerate()
                .skip(self.search.offset)
                .take(visible)
                .enumerate()
            {
                let rect = Rect::new(
                    rows[1].x,
                    rows[1].y + row as u16 * 2,
                    rows[1].width,
                    2.min(rows[1].height.saturating_sub(row as u16 * 2)),
                );
                self.search.visible_rows.push(rect);
                let (label, meta) = if self.search.repository.is_none() {
                    (
                        clean(s(v, "name")),
                        if v["external"] == true {
                            "Browse this repository · all authors".into()
                        } else {
                            format!(
                                "{} PRs · {}",
                                v["prs"].as_u64().unwrap_or(0),
                                s(v, "lastContribution")
                                    .get(..10)
                                    .unwrap_or("date unavailable")
                            )
                        },
                    )
                } else {
                    (
                        format!("#{}  {}", v["number"], clean(s(v, "title"))),
                        format!(
                            "{} · @{} · {}",
                            s(v, "state").to_lowercase(),
                            clean(s(v, "author")),
                            s(v, "created").get(..10).unwrap_or("")
                        ),
                    )
                };
                let style = if index == self.search.selected {
                    theme().selected
                } else {
                    Style::default().fg(theme().fg)
                };
                f.render_widget(
                    Paragraph::new(vec![
                        Line::from(format!(
                            "{}{}",
                            if index == self.search.selected {
                                "› "
                            } else {
                                "  "
                            },
                            label
                        )),
                        Line::from(format!("  {meta}")),
                    ])
                    .style(style),
                    rect,
                );
            }
        }
        let message = if self.search.busy {
            " Loading all pages…".into()
        } else if !self.search.error.is_empty() {
            self.search.error.clone()
        } else if !self.search.warning.is_empty() {
            self.search.warning.clone()
        } else if self.search.repository.is_some() {
            format!(
                " {} matches · newest first · open, merged and closed",
                items.len()
            )
        } else {
            format!(" {} repositories · recent contributions first", items.len())
        };
        f.render_widget(
            Paragraph::new(clean(&message))
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(theme().muted)),
            rows[2],
        );
        let hint = if self.search.repository.is_some() {
            " ↑↓ Select · Enter Open · Tab Authors · Alt-← Repos · Esc Close"
        } else {
            " ↑↓ Select · Enter Open · owner/repo Browse · Esc Close"
        };
        f.render_widget(
            Paragraph::new(hint).style(Style::default().fg(theme().muted)),
            rows[3],
        );
    }
    fn border(&self, title: String, index: usize) -> Block<'static> {
        Block::default()
            .title(format!(" {title} "))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if self.focus == index {
                theme().active
            } else {
                theme().inactive
            })
    }
    fn repo_view(&self, f: &mut Frame, area: Rect) {
        let title = if self.query.is_empty() {
            "Repositories".into()
        } else {
            format!(
                "Repository search · page {} / {} results",
                self.repo_page, self.repo_total
            )
        };
        let block = self.border(title, 0);
        let inner = block.inner(area);
        f.render_widget(block, area);
        let rows = Layout::vertical([C::Length(2), C::Min(1)]).split(inner);
        f.render_widget(
            Paragraph::new(if self.query.is_empty() {
                "Recent contributions ↓"
            } else {
                "Search results"
            })
            .style(Style::default().fg(theme().muted))
            .wrap(Wrap { trim: false }),
            rows[0],
        );
        let items: Vec<ListItem> = self
            .repos
            .iter()
            .map(|repo| {
                ListItem::new(vec![
                    Line::styled(clean(s(repo, "name")), Style::default().fg(theme().accent)),
                    Line::styled(
                        if self.query.is_empty() {
                            format!(
                                "  {} commits · {} open PRs",
                                repo["commits"]
                                    .as_u64()
                                    .map(|n| n.to_string())
                                    .unwrap_or("?".into()),
                                repo["openPrs"]
                            )
                        } else {
                            clean(s(repo, "description"))
                        },
                        Style::default().fg(theme().muted),
                    ),
                    Line::from(""),
                ])
            })
            .collect();
        if items.is_empty() {
            f.render_widget(
                Paragraph::new(if self.busy {
                    "Loading repositories…"
                } else {
                    "/ Search any repository\nEnter a name or owner/repo"
                })
                .wrap(Wrap { trim: false }),
                rows[1],
            );
        } else {
            f.render_stateful_widget(
                List::new(items)
                    .highlight_symbol("› ")
                    .highlight_style(theme().selected),
                rows[1],
                &mut ListState::default().with_selected(Some(self.repo_cursor)),
            );
        }
    }
    fn pr_view(&self, f: &mut Frame, area: Rect) {
        let title = if self.repository.is_empty() {
            match self.scope {
                1 => "Review requests".into(),
                2 => "Your PRs".into(),
                _ => "Open pull requests".into(),
            }
        } else {
            format!("{} · all authors", clean(&self.repository))
        };
        let block = self.border(title, 1);
        let inner = block.inner(area);
        f.render_widget(block, area);
        let rows = Layout::vertical([C::Length(2), C::Min(1)]).split(inner);
        let indices = self.filtered();
        f.render_widget(
            Paragraph::new(format!(
                "{} on page {} / {} total",
                indices.len(),
                self.page,
                self.total
            ))
            .style(Style::default().fg(theme().muted)),
            rows[0],
        );
        if indices.is_empty() {
            f.render_widget(
                Paragraph::new(if self.busy {
                    clean(&self.status)
                } else if !self.filter.is_empty() {
                    "No matches · f Edit filter".into()
                } else if self.repository.is_empty() && self.scope == 0 {
                    "Choose a repository · Enter Open".into()
                } else {
                    clean(&self.status)
                })
                .wrap(Wrap { trim: false }),
                rows[1],
            );
            return;
        }
        let items: Vec<ListItem> = indices
            .iter()
            .map(|i| {
                let v = &self.items[*i];
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!("#{} ", v["number"]),
                            Style::default().fg(theme().accent),
                        ),
                        Span::raw(clean(s(v, "title"))),
                    ]),
                    Line::styled(
                        format!(
                            "  {} · @{}{}",
                            clean(s(v, "repository")),
                            clean(s(v, "author")),
                            if v["draft"] == true { " · draft" } else { "" }
                        ),
                        Style::default().fg(theme().muted),
                    ),
                    Line::from(""),
                ])
            })
            .collect();
        f.render_stateful_widget(
            List::new(items)
                .highlight_symbol("› ")
                .highlight_style(theme().selected),
            rows[1],
            &mut ListState::default().with_selected(Some(self.selected)),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(c: char) -> KeyEvent {
        KeyEvent::new(K::Char(c), M::NONE)
    }
    fn loaded() -> Picker {
        let mut p = Picker::new();
        p.focus = 1;
        p.repository = "a/b".into();
        p.receive(Ok(json!({"items":[{"url":"https://github.com/a/b/pull/1","repository":"a/b","number":1,"title":"First"},{"url":"https://github.com/a/b/pull/2","repository":"a/b","number":2,"title":"Second"}],"page":1,"total":51,"hasNext":true})));
        p
    }
    #[test]
    fn enter_loads_selected_url() {
        let mut p = loaded();
        p.key(key('j'));
        assert_eq!(
            p.key(KeyEvent::new(K::Enter, M::NONE)).unwrap()["url"],
            "https://github.com/a/b/pull/2"
        );
        assert!(p.loading_pr);
    }
    #[test]
    fn filter_typing_is_isolated() {
        let mut p = loaded();
        p.key(key('f'));
        for c in "Secondq".chars() {
            assert!(p.key(key(c)).is_none());
        }
        assert!(!p.quit);
        p.key(KeyEvent::new(K::Backspace, M::NONE));
        p.key(KeyEvent::new(K::Enter, M::NONE));
        assert_eq!(p.filtered(), vec![1]);
    }
    #[test]
    fn page_retains_repository() {
        let mut p = loaded();
        let req = p.key(key('n')).unwrap();
        assert_eq!(req["page"], 2);
        assert_eq!(req["repository"], "a/b");
        assert!(p.items.is_empty());
    }
    #[test]
    fn errors_keep_picker_open() {
        let mut p = loaded();
        p.open("bad");
        p.receive(Err("Unavailable".into()));
        assert!(!p.busy);
        assert!(!p.quit);
        assert_eq!(p.items.len(), 2);
    }
    #[test]
    fn repositories_then_pr_use_the_same_load_operation() {
        let mut p = Picker::new();
        assert_eq!(p.home()["operation"], "repositories");
        p.receive_repos(Ok(
            json!({"repositories":[{"name":"a/b","commits":27,"openPrs":2}]}),
        ));
        let req = p.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        assert_eq!(req["repository"], "a/b");
        assert_eq!(req["scope"], "repository");
        assert_eq!(p.focus, 1);
    }
    #[test]
    fn repo_search_does_not_require_your_own_pr() {
        let mut p = Picker::new();
        p.key(key('4'));
        for c in "someone/project".chars() {
            p.key(key(c));
        }
        let req = p.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        assert_eq!(req["operation"], "repositories");
        assert_eq!(req["query"], "someone/project");
    }
    #[test]
    fn tabs_switch_panes_without_requests_and_help_filters() {
        let mut p = loaded();
        assert!(p.key(KeyEvent::new(K::Tab, M::NONE)).is_none());
        assert_eq!(p.focus, 0);
        p.key(KeyEvent::new(K::BackTab, M::SHIFT));
        assert_eq!(p.focus, 1);
        p.key(key('g'));
        p.key(key('?'));
        p.key(key('f'));
        p.key(key('q'));
        assert!(p.help.open);
        assert!(!p.quit);
    }
    #[test]
    fn hl_focuses_each_chooser_pane_while_jk_only_moves_that_pane() {
        let mut p = loaded();
        p.repos = vec![json!({"name":"a/one"}), json!({"name":"a/two"})];
        p.key(key('h'));
        p.key(key('j'));
        assert_eq!(p.repo_cursor, 1);
        assert_eq!(p.selected, 0);
        p.key(key('l'));
        p.key(key('j'));
        assert_eq!(p.selected, 1);
        assert_eq!(p.repo_cursor, 1);
        p.key(key('k'));
        assert_eq!(p.selected, 0);
        assert_eq!(p.repo_cursor, 1);
    }
    #[test]
    fn demo_and_return_remain_explicit() {
        let mut p = loaded();
        p.has_review = true;
        p.key(KeyEvent::new(K::Esc, M::NONE));
        assert!(p.back);
        assert_eq!(p.key(key('d')).unwrap()["operation"], "sample");
    }
    #[test]
    fn history_popup_preserves_main_selection_and_filters_closed_prs() {
        let mut p = loaded();
        p.selected = 1;
        assert_eq!(p.key(key('/')).unwrap()["operation"], "history");
        p.receive_history(Ok(json!({"kind":"catalog","repositories":[{"name":"old/library"}],"items":[{"repository":"old/library","number":42,"title":"Fuzzy matching fixed","url":"https://github.com/old/library/pull/42","state":"MERGED"},{"repository":"old/library","number":14,"title":"Fix cache","url":"https://github.com/old/library/pull/14","state":"CLOSED"}]})));
        for c in "olib".chars() {
            p.key(key(c));
        }
        assert_eq!(p.search_items().len(), 1);
        p.key(KeyEvent::new(K::Enter, M::NONE));
        for c in "14 fx".chars() {
            p.key(key(c));
        }
        assert_eq!(p.search_items()[0]["number"], 14);
        p.key(KeyEvent::new(K::Esc, M::NONE));
        assert!(!p.search.open);
        assert_eq!(p.selected, 1);
        assert!(!p.back);
    }
    #[test]
    fn history_late_response_is_cached_without_reopening_and_paste_is_inert() {
        let mut p = loaded();
        p.key(key('/'));
        p.key(KeyEvent::new(K::Esc, M::NONE));
        p.receive_history(Ok(json!({"kind":"catalog","repositories":[],"items":[]})));
        assert!(!p.search.open);
        assert!(p.key(key('/')).is_none());
        p.paste("somebody/project\n");
        assert_eq!(p.search.query, "somebody/project");
        let request = p.key(KeyEvent::new(K::Enter, M::NONE)).unwrap();
        assert_eq!(request["operation"], "history");
        assert_eq!(request["repository"], "somebody/project");
        assert!(p.search.all_authors);
        assert!(!p.quit);
    }
    #[test]
    fn fuzzy_queries_are_case_insensitive_subsequences_and_all_tokens_match() {
        assert!(fuzzy("lr pr42", "LazyReview #PR42"));
        assert!(fuzzy("14 cache", "#14 Fix cache eviction"));
        assert!(!fuzzy("41 cache", "#14 Fix cache eviction"));
    }
    #[test]
    fn search_popup_renders_and_clicks_at_narrow_and_wide_sizes() {
        use ratatui::{Terminal, backend::TestBackend};
        for (width, height) in [(55, 18), (145, 42)] {
            let mut p = loaded();
            p.open_search();
            p.receive_history(Ok(json!({"kind":"catalog","repositories":[{"name":"archive/library","prs":2,"lastContribution":"2026-10-01"}],"items":[]})));
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|f| p.render(f)).unwrap();
            assert!(p.search.area.x > 0 && p.search.area.y > 0);
            assert!(p.search.visible_rows.len() == 1);
            let row = p.search.visible_rows[0];
            p.mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: row.x + 2,
                row: row.y,
                modifiers: M::NONE,
            });
            assert_eq!(p.search.repository.as_deref(), Some("archive/library"));
        }
    }
}
