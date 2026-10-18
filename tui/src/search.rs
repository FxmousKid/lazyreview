use crate::app::{array, s};
use regex::{Regex, RegexBuilder};
use serde_json::Value;

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub enum Scope {
    #[default]
    Both,
    Original,
    Modified,
}
impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Both => "Both",
            Self::Original => "Original",
            Self::Modified => "Modified",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Both => Self::Original,
            Self::Original => Self::Modified,
            Self::Modified => Self::Both,
        }
    }
    pub fn allows(self, line: &Value) -> bool {
        match self {
            Self::Both => true,
            Self::Original => line["old"].is_number(),
            Self::Modified => line["new"].is_number(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Default)]
pub struct Search {
    pub query: String,
    pub scope: Scope,
    pub hits: Vec<Hit>,
    pub active: Option<usize>,
    pub regex: Option<Regex>,
}
pub fn text(line: &Value) -> String {
    crate::ui::clean(s(line, "text")).replace('\t', "    ")
}
impl Search {
    pub fn rebuild(&mut self, file: &Value, split: bool) {
        self.regex = if self.query.is_empty() {
            None
        } else {
            RegexBuilder::new(&regex::escape(&self.query))
                .case_insensitive(!self.query.chars().any(char::is_uppercase))
                .build()
                .ok()
        };
        self.hits.clear();
        self.active = None;
        let Some(re) = &self.regex else {
            return;
        };
        let lines = array(file, "lines");
        let mut order: Vec<usize> = (0..lines.len()).collect();
        if split {
            order.clear();
            for row in array(file, "splitRows") {
                for side in ["left", "right"] {
                    if !row[side].is_null() {
                        if let Some(i) = lines.iter().position(|line| line == &row[side]) {
                            // Unchanged context is displayed twice but is one search result.
                            if order.last() != Some(&i) {
                                order.push(i);
                            }
                        }
                    }
                }
            }
        }
        for i in order {
            let line = &lines[i];
            if s(line, "kind") == "hunk" || !self.scope.allows(line) {
                continue;
            }
            for m in re.find_iter(&text(line)) {
                self.hits.push(Hit {
                    line: i,
                    start: m.start(),
                    end: m.end(),
                });
            }
        }
    }
    pub fn first_from(&mut self, line: usize) -> Option<Hit> {
        self.active = if self.hits.is_empty() {
            None
        } else {
            Some(self.hits.iter().position(|h| h.line >= line).unwrap_or(0))
        };
        self.current().cloned()
    }
    pub fn current(&self) -> Option<&Hit> {
        self.active.and_then(|i| self.hits.get(i))
    }
    pub fn step(&mut self, forward: bool, line: usize) -> Option<Hit> {
        if self.hits.is_empty() {
            return None;
        }
        let n = self.hits.len();
        self.active = Some(match self.active.filter(|&i| self.hits[i].line == line) {
            Some(i) => (i + if forward { 1 } else { n - 1 }) % n,
            None if forward => self.hits.iter().position(|h| h.line > line).unwrap_or(0),
            None => self
                .hits
                .iter()
                .rposition(|h| h.line < line)
                .unwrap_or(n - 1),
        });
        self.current().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn literal_smartcase_unicode_and_split_side_search_keep_anchors() {
        let old = json!({"kind":"delete","text":"écho ÉCHO a.b a.b","old":4});
        let new = json!({"kind":"add","text":"écho ÉCHO a.b","new":5});
        let context = json!({"kind":"context","text":"écho","new":6,"old":5});
        let file = json!({"lines":[{"kind":"hunk","text":"écho"},old,new,context],"splitRows":[{"left":old,"right":new},{"left":context,"right":context}]});
        let mut search = Search {
            query: "écho".into(),
            ..Search::default()
        };
        search.rebuild(&file, true);
        assert_eq!(search.hits.len(), 5);
        assert_eq!(search.first_from(1).unwrap().line, 1);
        assert_eq!(search.step(false, 1).unwrap().line, 3);
        search.scope = Scope::Modified;
        search.rebuild(&file, true);
        assert_eq!(search.hits.len(), 3);
        search.query = "ÉCHO".into();
        search.rebuild(&file, false);
        assert_eq!(search.hits.len(), 1);
        search.query = "a.b".into();
        search.scope = Scope::Original;
        search.rebuild(&file, true);
        assert_eq!(search.hits.len(), 2);
        search.query = "missing".into();
        search.rebuild(&file, true);
        assert!(search.step(true, 0).is_none());
    }
}
