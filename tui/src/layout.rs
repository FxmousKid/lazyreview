use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};

#[derive(Clone, Debug)]
pub struct Panes {
    pub tree: bool,
    pub diff: bool,
    pub panel: bool,
    pub zoom: bool,
    pub left_width: u16,
    pub right_width: u16,
    pub areas: [Rect; 3],
    pub controls: [Rect; 3],
    area: Rect,
    drag: Option<usize>,
}
impl Default for Panes {
    fn default() -> Self {
        Self {
            tree: true,
            diff: true,
            panel: true,
            zoom: false,
            left_width: 34,
            right_width: 42,
            areas: [Rect::default(); 3],
            controls: [Rect::default(); 3],
            area: Rect::default(),
            drag: None,
        }
    }
}
impl Panes {
    pub fn chooser() -> Self {
        Self {
            panel: false,
            left_width: 46,
            ..Self::default()
        }
    }
    pub fn toggle(&mut self, index: usize) -> bool {
        let visible = self.visible();
        if visible.len() == 1 && visible[0] == index {
            return false;
        }
        match index {
            0 => self.tree = !self.tree,
            1 => self.diff = !self.diff,
            _ => self.panel = !self.panel,
        };
        self.zoom = false;
        true
    }
    /// Fit desired content widths without changing visibility, focus or zoom.
    /// `calculate` bounds these requests when the terminal cannot fit both.
    pub fn fit(&mut self, tree_width: u16, reading_width: u16) {
        self.left_width = tree_width.max(18);
        self.right_width = reading_width.max(18);
    }
    pub fn control_at(&self, event: MouseEvent) -> Option<usize> {
        (event.kind == MouseEventKind::Down(MouseButton::Left))
            .then(|| {
                self.controls
                    .iter()
                    .position(|r| r.width > 0 && r.contains((event.column, event.row).into()))
            })
            .flatten()
    }
    pub fn activate_control(&mut self, index: usize) -> bool {
        if self.areas[index].width == 0 {
            match index {
                0 => self.tree = true,
                1 => self.diff = true,
                _ => self.panel = true,
            }
            self.zoom = false;
            true
        } else {
            self.toggle(index)
        }
    }
    pub fn visible(&self) -> Vec<usize> {
        [self.tree, self.diff, self.panel]
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.then_some(i))
            .collect()
    }
    pub fn cycle(&self, current: usize, forward: bool) -> usize {
        let panes = self.visible();
        let at = panes.iter().position(|&p| p == current).unwrap_or(0);
        panes[(at + if forward { 1 } else { panes.len() - 1 }) % panes.len()]
    }
    pub fn calculate(&mut self, area: Rect, focus: usize) -> [Rect; 3] {
        self.area = area;
        self.areas = [Rect::default(); 3];
        let visible = self.visible();
        if self.zoom || area.width < 24 + 18 * (visible.len() as u16 - 1) {
            self.areas[focus] = area;
            return self.areas;
        }
        if !self.diff {
            let left = if self.tree && self.panel {
                self.left_width.clamp(18, area.width.saturating_sub(18))
            } else if self.tree {
                area.width
            } else {
                0
            };
            let constraints = [
                Constraint::Length(left),
                Constraint::Length(0),
                Constraint::Length(area.width.saturating_sub(left)),
            ];
            self.areas
                .copy_from_slice(&Layout::horizontal(constraints).split(area));
            return self.areas;
        }
        let mut left = if self.tree {
            self.left_width.max(18)
        } else {
            0
        };
        let mut right = if self.panel {
            self.right_width.max(18)
        } else {
            0
        };
        let available = area.width.saturating_sub(24);
        if u32::from(left) + u32::from(right) > u32::from(available) {
            // Keep the diff usable and share insufficient space between the
            // side panes in proportion to their requested width above minimum.
            let base_left = if self.tree { 18 } else { 0 };
            let base_right = if self.panel { 18 } else { 0 };
            let extra = u32::from(available.saturating_sub(base_left + base_right));
            let wants_left = u32::from(left.saturating_sub(base_left));
            let wants_right = u32::from(right.saturating_sub(base_right));
            let total = (wants_left + wants_right).max(1);
            let left_extra = extra * wants_left / total;
            left = base_left + left_extra as u16;
            right = base_right + (extra - left_extra) as u16;
        }
        let rects = Layout::horizontal([
            Constraint::Length(left),
            Constraint::Min(24),
            Constraint::Length(right),
        ])
        .split(area);
        self.areas.copy_from_slice(&rects);
        self.areas
    }
    pub fn resize(&mut self, focus: usize, delta: i16) {
        self.zoom = false;
        if !self.diff {
            if self.tree && self.panel {
                let delta = if focus == 2 {
                    -i32::from(delta)
                } else {
                    i32::from(delta)
                };
                self.left_width = (i32::from(self.areas[0].width) + delta)
                    .clamp(18, i32::from(self.area.width.saturating_sub(18).max(18)))
                    as u16;
            }
            return;
        }
        let max = self
            .area
            .width
            .saturating_sub(24 + if self.tree && self.panel { 18 } else { 0 })
            .max(18);
        let update =
            |n: u16, d: i16| (i32::from(n) + i32::from(d)).clamp(18, i32::from(max)) as u16;
        match focus {
            0 => self.left_width = update(self.areas[0].width.max(18), delta),
            2 => self.right_width = update(self.areas[2].width.max(18), delta),
            _ => {
                if self.tree {
                    self.left_width = update(self.areas[0].width.max(18), -delta);
                }
                if self.panel {
                    self.right_width = update(self.areas[2].width.max(18), -delta);
                }
            }
        }
    }
    // Returns the clicked pane; divider drags resize without changing keyboard focus.
    pub fn mouse(&mut self, event: MouseEvent) -> Option<usize> {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !self.zoom
                    && self.area.height > 0
                    && event.row >= self.area.y
                    && event.row < self.area.bottom()
                {
                    for i in [0, 2] {
                        let rect = self.areas[i];
                        let x = if i == 0 {
                            rect.right().saturating_sub(1)
                        } else {
                            rect.x
                        };
                        if rect.width > 0 && event.column.abs_diff(x) <= 1 {
                            self.drag = Some(i);
                            return None;
                        }
                    }
                }
                self.areas
                    .iter()
                    .position(|r| r.width > 0 && r.contains((event.column, event.row).into()))
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(i) = self.drag {
                    let desired = if i == 0 {
                        event.column.saturating_sub(self.area.x) + 1
                    } else {
                        self.area.right().saturating_sub(event.column)
                    };
                    self.resize(
                        i,
                        (i32::from(desired) - i32::from(self.areas[i].width)).clamp(-32768, 32767)
                            as i16,
                    );
                }
                None
            }
            MouseEventKind::Up(_) => {
                self.drag = None;
                None
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resizing_and_folding_preserve_a_usable_diff_at_all_widths() {
        let mut p = Panes::default();
        let before = p.calculate(Rect::new(0, 3, 180, 40), 1);
        p.resize(0, 8);
        let after = p.calculate(Rect::new(0, 3, 180, 40), 1);
        assert_eq!(after[0].width, before[0].width + 8);
        p.panel = false;
        assert_eq!(p.cycle(1, true), 0);
        assert_eq!(p.calculate(Rect::new(0, 3, 100, 40), 1)[2].width, 0);
        for width in 0..200 {
            let a = p.calculate(Rect::new(0, 0, width, 20), 1);
            assert!(a.iter().all(|r| r.right() <= width));
        }
    }
    #[test]
    fn fitted_reading_column_uses_available_space_and_constrains_small_terminals() {
        let mut p = Panes::default();
        p.fit(41, 89);
        let fitted = p.calculate(Rect::new(0, 0, 180, 40), 1);
        assert_eq!(fitted.map(|r| r.width), [41, 50, 89]);
        // A fixed half-budget cap used to truncate the reading column to 78,
        // despite enough total space for both requested sidebar widths.
        for width in 60..240 {
            let areas = p.calculate(Rect::new(0, 0, width, 20), 1);
            assert!(areas[0].width >= 18);
            assert!(areas[1].width >= 24);
            assert!(areas[2].width >= 18);
            assert_eq!(areas.iter().map(|r| r.width).sum::<u16>(), width);
            assert!(areas.iter().all(|r| r.right() <= width));
        }
        p.tree = false;
        p.zoom = true;
        p.fit(67, 89);
        assert!(!p.tree && p.zoom);
        assert_eq!(p.calculate(Rect::new(0, 0, 180, 40), 1)[1].width, 180);
    }
    #[test]
    fn divider_drag_changes_width_without_switching_focus() {
        use crossterm::event::KeyModifiers;
        let mut p = Panes::default();
        p.calculate(Rect::new(0, 0, 160, 40), 1);
        let event = |kind, column| MouseEvent {
            kind,
            column,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            p.mouse(event(MouseEventKind::Down(MouseButton::Left), 33)),
            None
        );
        p.mouse(event(MouseEventKind::Drag(MouseButton::Left), 40));
        assert_eq!(p.calculate(Rect::new(0, 0, 160, 40), 1)[0].width, 41);
        p.mouse(event(MouseEventKind::Up(MouseButton::Left), 40));
    }

    #[test]
    fn both_remaining_panes_resize_when_changes_are_folded() {
        let mut p = Panes {
            diff: false,
            ..Panes::default()
        };
        let area = Rect::new(0, 0, 120, 40);
        let before = p.calculate(area, 0);
        p.resize(0, 8);
        let after = p.calculate(area, 0);
        assert_eq!(after[0].width, before[0].width + 8);
        assert_eq!(after[2].width, before[2].width - 8);
        p.resize(2, 5);
        assert_eq!(p.calculate(area, 2)[2].width, after[2].width + 5);
        p.resize(2, i16::MAX);
        assert_eq!(p.calculate(area, 2)[0].width, 18);
        for width in 0..200 {
            let areas = p.calculate(Rect::new(0, 0, width, 20), 0);
            assert!(areas.iter().all(|r| r.right() <= width));
        }
    }
}
