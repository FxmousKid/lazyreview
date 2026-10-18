use crate::theme::current as theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use std::time::Instant;
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub fn frame(started: Instant) -> usize {
    (started.elapsed().as_millis() / 100) as usize
}
pub fn render(f: &mut Frame, label: &str, started: Instant) {
    let area = f.area();
    if area.width < 20 || area.height < 6 {
        return;
    }
    let width = area.width.saturating_sub(2).min(36);
    let height = 5;
    let rect = Rect::new(
        area.right() - width - 1,
        area.bottom() - height - 1,
        width,
        height,
    );
    let index = frame(started);
    let cells = width.saturating_sub(4) as usize;
    let at = index % cells.max(1);
    let track: Vec<Span> = (0..cells)
        .map(|i| {
            Span::styled(
                if i.abs_diff(at) <= 1 { "━" } else { "─" },
                Style::default().fg(if i.abs_diff(at) <= 1 {
                    theme().accent
                } else {
                    theme().muted
                }),
            )
        })
        .collect();
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme().active)
        .style(Style::default().bg(theme().bg).fg(theme().fg));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(format!(" {} {label}", FRAMES[index % 10])),
            Line::from(track),
            Line::styled(
                format!(" {:.1}s", started.elapsed().as_secs_f32()),
                Style::default().fg(theme().muted),
            ),
        ]),
        inner,
    );
}
