//! Selection uses the cells actually rendered by Ratatui, excluding pane chrome.
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{buffer::Buffer, layout::Rect, style::Style, text::Span};
use std::collections::BTreeMap;
#[derive(Default)]
pub struct Selection {
    pub area: Rect,
    pub offset: usize,
    token: String,
    rows: BTreeMap<usize, Vec<(u16, String)>>,
    anchor: Option<(usize, u16)>,
    cursor: (usize, u16),
    dragging: bool,
    moved: bool,
    pub keyboard: bool,
    pub visible: bool,
}
impl Selection {
    pub fn active(&self) -> bool {
        self.anchor.is_some()
    }
    /// An unfinished mouse gesture can outlive a lost terminal release event.
    /// Keyboard input may cancel it; a completed copy or keyboard selection stays put.
    pub fn mouse_pending(&self) -> bool {
        self.dragging && !self.moved && !self.keyboard
    }
    pub fn clear(&mut self) {
        self.anchor = None;
        self.dragging = false;
        self.keyboard = false;
    }
    pub fn capture(&mut self, buffer: &Buffer, area: Rect, offset: usize, token: String) {
        if self.token != token || self.area.width != area.width {
            self.clear();
            self.rows.clear();
        }
        self.token = token;
        self.area = area;
        self.offset = offset;
        self.visible = true;
        if !self.active() {
            self.rows.clear();
        }
        for y in 0..area.height {
            let mut cells = vec![];
            let mut x = 0;
            while x < area.width {
                let symbol = buffer[(area.x + x, area.y + y)].symbol().to_string();
                let width = Span::raw(symbol.as_str()).width().max(1) as u16;
                cells.push((x, symbol));
                x = x.saturating_add(width);
            }
            self.rows.insert(offset + y as usize, cells);
        }
    }
    fn at(&self, x: u16, y: u16) -> (usize, u16) {
        (
            self.offset
                + y.clamp(self.area.y, self.area.bottom().saturating_sub(1))
                    .saturating_sub(self.area.y) as usize,
            x.clamp(self.area.x, self.area.right().saturating_sub(1))
                .saturating_sub(self.area.x),
        )
    }
    /// Handled flag plus a copy request on completed drag. Never writes clipboard itself.
    pub fn mouse(&mut self, e: MouseEvent) -> (bool, Option<String>) {
        if !self.visible || self.area.width == 0 || self.area.height == 0 {
            return (false, None);
        }
        match e.kind {
            MouseEventKind::Down(MouseButton::Left)
                if self.area.contains((e.column, e.row).into()) =>
            {
                self.cursor = self.at(e.column, e.row);
                self.anchor = Some(self.cursor);
                self.dragging = true;
                self.moved = false;
                self.keyboard = false;
                (true, None)
            }
            MouseEventKind::Drag(MouseButton::Left) if self.dragging => {
                self.cursor = self.at(e.column, e.row);
                self.moved |= Some(self.cursor) != self.anchor;
                (true, None)
            }
            MouseEventKind::Up(MouseButton::Left) if self.dragging => {
                self.cursor = self.at(e.column, e.row);
                self.dragging = false;
                // Some terminals/multiplexers coalesce motion events. Release
                // coordinates are sufficient to complete the selection.
                self.moved |= Some(self.cursor) != self.anchor;
                if self.moved {
                    (true, Some(self.text()))
                } else {
                    self.clear();
                    (true, None)
                }
            }
            _ => (false, None),
        }
    }
    pub fn begin_keyboard(&mut self) {
        self.cursor = (self.offset, 0);
        self.anchor = Some(self.cursor);
        self.keyboard = true;
    }
    pub fn step(&mut self, c: char, max: usize) -> usize {
        match c {
            'h' => self.cursor.1 = self.cursor.1.saturating_sub(1),
            'l' => self.cursor.1 = (self.cursor.1 + 1).min(self.area.width.saturating_sub(1)),
            'j' => self.cursor.0 = (self.cursor.0 + 1).min(max),
            'k' => self.cursor.0 = self.cursor.0.saturating_sub(1),
            '$' => self.cursor.1 = self.area.width.saturating_sub(1),
            '0' => self.cursor.1 = 0,
            _ => {}
        };
        self.cursor.0
    }
    pub fn text(&self) -> String {
        let Some(anchor) = self.anchor else {
            return String::new();
        };
        let (start, end) = if anchor <= self.cursor {
            (anchor, self.cursor)
        } else {
            (self.cursor, anchor)
        };
        let mut lines = vec![];
        for row in start.0..=end.0 {
            let Some(cells) = self.rows.get(&row) else {
                continue;
            };
            let text: String = cells
                .iter()
                .filter(|(col, s)| {
                    let width = Span::raw(s.as_str()).width().max(1) as u16;
                    (row, *col + width.saturating_sub(1)) >= start && (row, *col) <= end
                })
                .map(|(_, s)| s.as_str())
                .collect();
            lines.push(text.trim_end().to_string());
        }
        lines.join("\n")
    }
    pub fn highlight(&self, buffer: &mut Buffer, style: Style) {
        let Some(anchor) = self.anchor else { return };
        let (start, end) = if anchor <= self.cursor {
            (anchor, self.cursor)
        } else {
            (self.cursor, anchor)
        };
        for y in 0..self.area.height {
            for x in 0..self.area.width {
                let point = (self.offset + y as usize, x);
                if point >= start && point <= end {
                    buffer[(self.area.x + x, self.area.y + y)].set_style(style);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::widgets::{Paragraph, Widget};
    #[test]
    fn release_coordinates_complete_coalesced_motion_but_clicks_do_not_copy() {
        let area = Rect::new(5, 2, 12, 2);
        let mut b = Buffer::empty(Rect::new(0, 0, 30, 10));
        Paragraph::new("Hello 世界").render(area, &mut b);
        let mut s = Selection::default();
        s.capture(&b, area, 0, "pr".into());
        let event = |kind, x| MouseEvent {
            kind,
            column: x,
            row: 2,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        s.mouse(event(MouseEventKind::Down(MouseButton::Left), 5));
        assert_eq!(
            s.mouse(event(MouseEventKind::Up(MouseButton::Left), 5)),
            (true, None)
        );
        assert!(!s.active());
        s.mouse(event(MouseEventKind::Down(MouseButton::Left), 9));
        assert_eq!(
            s.mouse(event(MouseEventKind::Up(MouseButton::Left), 5)),
            (true, Some("Hello".into()))
        );
        s.visible = false;
        assert_eq!(
            s.mouse(event(MouseEventKind::Down(MouseButton::Left), 5)),
            (false, None)
        );
    }
    #[test]
    fn copy_is_pane_local_unicode_safe_and_survives_scrolling() {
        let area = Rect::new(5, 2, 12, 2);
        let mut b = Buffer::empty(Rect::new(0, 0, 30, 10));
        Paragraph::new("Hello 世界\nSecond line").render(area, &mut b);
        let mut s = Selection::default();
        s.capture(&b, area, 0, "pr".into());
        s.begin_keyboard();
        s.step('$', 2);
        assert_eq!(s.text(), "Hello 世界");
        s.step('j', 2);
        assert_eq!(s.text(), "Hello 世界\nSecond line");
        b.reset();
        Paragraph::new("Second line\nThird").render(area, &mut b);
        s.capture(&b, area, 1, "pr".into());
        s.step('j', 2);
        assert_eq!(s.text(), "Hello 世界\nSecond line\nThird");
        s.capture(&b, area, 1, "new-pr".into());
        assert!(!s.active());
    }
}
