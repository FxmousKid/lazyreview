use crate::app::s;
use ratatui::layout::Rect;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Copy)]
pub enum Action {
    Row(usize),
    Posted(bool),
    Compose,
    Outcome(usize),
    Continue,
    Publish,
    Back,
}
#[derive(Default)]
pub struct ReviewList {
    pub expanded: HashSet<String>,
    pub follow: bool,
    pub posted: bool,
    pub summary: String,
    pub summary_loaded: bool,
    pub hits: Vec<(Rect, Action)>,
    pub publish_pressed: Option<Rect>,
    pub dialog: Rect,
}
pub struct Entry {
    pub key: String,
    pub path: String,
    pub comment: Option<Value>,
    pub count: usize,
}
impl ReviewList {
    pub fn rows(&self, comments: Vec<Value>) -> Vec<Entry> {
        let mut groups: Vec<(String, Vec<Value>)> = vec![];
        for c in comments
            .into_iter()
            .filter(|c| (c["pending"] != true) == self.posted)
        {
            let path = s(&c, "path").to_string();
            if let Some((_, items)) = groups.iter_mut().find(|(p, _)| *p == path) {
                items.push(c);
            } else {
                groups.push((path, vec![c]));
            }
        }
        let mut rows = vec![];
        for (path, comments) in groups {
            let key = format!("file:{path}");
            let open = self.expanded.contains(&key);
            rows.push(Entry {
                key,
                path: path.clone(),
                count: comments.len(),
                comment: None,
            });
            if open {
                for c in comments {
                    rows.push(Entry {
                        key: s(&c, "reviewKey").into(),
                        path: path.clone(),
                        comment: Some(c),
                        count: 0,
                    });
                }
            }
        }
        rows
    }
    pub fn toggle(&mut self, key: &str, comment: bool, expand: Option<bool>) {
        let open = expand.unwrap_or(!self.expanded.contains(key));
        if open {
            if comment {
                self.expanded.retain(|entry| entry.starts_with("file:"));
            }
            self.expanded.insert(key.into());
        } else {
            self.expanded.remove(key);
        }
        self.follow = true;
    }
}
