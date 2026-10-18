use crate::app::{array, s};
use serde_json::{Value as V, json};
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct Completion {
    pub items: Vec<V>,
    pub selected: usize,
    pub open: bool,
    pub error: String,
    token: String,
    pub start: usize,
    end: usize,
    kind: String,
    query: String,
    url: String,
    id: u64,
    requested: bool,
    changed: Option<Instant>,
}
impl Completion {
    pub fn reset(&mut self) {
        self.id += 1;
        self.open = false;
        self.items.clear();
        self.token.clear();
        self.error.clear();
        self.requested = false;
        self.changed = None;
    }
    pub fn sync(&mut self, text: &str, cursor: usize, pr: &V) {
        let prefix = &text[..cursor.min(text.len())];
        let start = prefix
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace() || "([{".contains(*c))
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let token = &prefix[start..];
        let kind = if token.starts_with('@') {
            "user"
        } else if token.starts_with('#') && !token.starts_with("##") {
            "issue"
        } else {
            self.reset();
            return;
        };
        let query = &token[1..];
        if query.len() > 100 || query.contains(['`', '\n']) {
            self.reset();
            return;
        }
        if self.token == token && self.start == start && self.url == s(pr, "url") {
            return;
        }
        self.id += 1;
        self.token = token.into();
        self.start = start;
        self.end = cursor;
        self.kind = kind.into();
        self.query = query.into();
        self.url = s(pr, "url").into();
        self.selected = 0;
        self.requested = false;
        self.changed = Some(Instant::now());
        self.open = true;
        self.error.clear();
        self.items.clear();
        if kind == "user" {
            let mut users = vec![s(pr, "author"), s(pr, "viewer")];
            for t in array(pr, "threads") {
                for c in array(t, "comments") {
                    users.push(s(c, "author"));
                }
            }
            for c in array(pr, "conversation") {
                users.push(s(c, "author"));
            }
            users.sort_unstable();
            users.dedup();
            self.items = users
                .into_iter()
                .filter(|u| !u.is_empty() && u.to_lowercase().contains(&query.to_lowercase()))
                .map(|u| json!({"text":format!("@{u}"),"label":u,"kind":"user"}))
                .take(15)
                .collect();
        } else if pr["demo"] == true {
            self.items = vec![
                json!({"text":format!("#{}",pr["number"]),"label":pr["title"],"kind":"PR"}),
                json!({"text":"#42","label":"Keyboard navigation regression","kind":"issue"}),
            ]
            .into_iter()
            .filter(|v| {
                format!("{} {}", s(v, "text"), s(v, "label"))
                    .to_lowercase()
                    .contains(&query.to_lowercase())
            })
            .collect();
        }
    }
    pub fn request(&mut self, demo: bool) -> Option<V> {
        if demo
            || !self.open
            || self.requested
            || !self
                .changed
                .is_some_and(|t| t.elapsed() >= Duration::from_millis(400))
        {
            return None;
        }
        self.requested = true;
        Some(
            json!({"operation":"suggest","url":self.url,"kind":self.kind,"query":self.query,"id":self.id}),
        )
    }
    pub fn receive(&mut self, data: V) {
        if data["id"].as_u64() != Some(self.id)
            || s(&data, "url") != self.url
            || s(&data, "kind") != self.kind
            || s(&data, "query") != self.query
            || !self.open
        {
            return;
        }
        self.error = s(&data, "error").into();
        for item in array(&data, "items") {
            if !self.items.iter().any(|v| v["text"] == item["text"]) {
                self.items.push(item.clone());
            }
        }
        self.items.truncate(15);
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
    }
    pub fn accept(&mut self, text: &mut String, cursor: &mut usize) -> bool {
        let Some(item) = self.items.get(self.selected) else {
            return false;
        };
        let value = format!("{} ", s(item, "text"));
        if !text.is_char_boundary(self.start) || !text.is_char_boundary(self.end) {
            self.reset();
            return false;
        }
        text.replace_range(self.start..self.end, &value);
        *cursor = self.start + value.len();
        self.reset();
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_respects_word_boundaries_cursor_and_stale_results() {
        let pr = json!({"url":"https://github.com/a/b/pull/1","author":"alice","viewer":"bob","demo":true});
        let mut c = Completion::default();
        c.sync("email@alice", 11, &pr);
        assert!(!c.open);
        let mut text = "Ask @al about this".to_string();
        let mut cursor = 7;
        c.sync(&text, cursor, &pr);
        assert_eq!(c.items[0]["text"], "@alice");
        let old = c.id;
        c.sync("@bo", 3, &pr);
        c.receive(json!({"id":old,"url":pr["url"],"items":[{"text":"@wrong"}]}));
        assert_eq!(c.items[0]["text"], "@bob");
        c.receive(
            json!({"id":c.id,"url":pr["url"],"kind":"issue","query":"bo","items":[{"text":"#42"}]}),
        );
        c.receive(json!({"id":c.id,"url":pr["url"],"kind":"user","query":"al","items":[{"text":"@alice"}]}));
        assert_eq!(c.items.len(), 1);
        c.receive(json!({"id":c.id,"url":pr["url"],"kind":"user","query":"bo","items":[{"text":"@bonnie"}]}));
        assert_eq!(c.items.len(), 2);
        c.sync(&text, cursor, &pr);
        assert!(c.accept(&mut text, &mut cursor));
        assert_eq!(text, "Ask @alice  about this");
        assert_eq!(cursor, 11);
    }
}
