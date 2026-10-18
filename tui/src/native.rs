//! A private PTY and VT screen: child escape sequences never reach the host terminal.
use crate::{
    app::{App, Pane, Panel},
    theme::current as theme,
};
use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use serde_json::Value;
use std::{
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

struct Responses(SyncSender<Vec<u8>>);
impl vt100::Callbacks for Responses {
    fn unhandled_csi(
        &mut self,
        s: &mut vt100::Screen,
        i1: Option<u8>,
        _: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        let p = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        let response = match (c, p, i1) {
            ('n', 6, _) => {
                let (r, c) = s.cursor_position();
                format!("\x1b[{};{}R", r + 1, c + 1)
            }
            ('n', 5, _) => "\x1b[0n".into(),
            ('c', _, Some(b'>')) => "\x1b[>0;0;0c".into(),
            ('c', _, _) => "\x1b[?1;2c".into(),
            _ => return,
        };
        let _ = self.0.try_send(response.into_bytes());
    }
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, p: &[&[u8]]) {
        if p.len() > 1 && p[1] == b"?" {
            let response = match p[0] {
                b"10" => "\x1b]10;rgb:d0d0/d7d7/dede\x1b\\",
                b"11" => "\x1b]11;rgb:0d0d/1111/1717\x1b\\",
                _ => return,
            };
            let _ = self.0.try_send(response.as_bytes().to_vec());
        }
    }
}
pub struct View {
    parser: vt100::Parser<Responses>,
}
pub type Shared = Arc<Mutex<View>>;
pub struct Terminal {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    pub view: Shared,
    input: SyncSender<Vec<u8>>,
    size: (u16, u16),
    pub ended: bool,
    pending_trust: Option<PendingTrustClick>,
}
impl Terminal {
    pub fn start(spec: &Value, rows: u16, cols: u16) -> Result<Self, String> {
        let rows = rows.max(2);
        let cols = cols.max(12);
        let pair = portable_pty::native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| "Cannot create CLI terminal")?;
        let mut cmd =
            CommandBuilder::new(spec["command"].as_str().ok_or("Missing CLI executable")?);
        for arg in spec["args"].as_array().ok_or("Invalid CLI launch")? {
            cmd.arg(arg.as_str().ok_or("Invalid CLI argument")?);
        }
        cmd.cwd(spec["cwd"].as_str().ok_or("Missing session directory")?);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env_remove("CLAUDECODE");
        cmd.env_remove("NO_COLOR");
        cmd.env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
        if let Some(env) = spec.get("env") {
            for (name, value) in env.as_object().ok_or("Invalid CLI environment")? {
                if name.is_empty() || name.contains(['=', '\0']) {
                    return Err("Invalid CLI environment name".into());
                }
                let value = value.as_str().ok_or("Invalid CLI environment value")?;
                if value.contains('\0') {
                    return Err("Invalid CLI environment value".into());
                }
                cmd.env(name, value);
            }
        }
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|_| "Cannot read CLI terminal")?;
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|_| "Cannot write CLI terminal")?;
        // Acquire every fallible PTY handle before starting a child. A failed
        // handle setup must never leave an unowned CLI process running.
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|_| "Cannot launch companion CLI. Check ai.command.")?;
        drop(pair.slave);
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(128);
        thread::spawn(move || {
            for bytes in rx {
                if writer.write_all(&bytes).is_err() {
                    break;
                }
                let _ = writer.flush();
            }
        });
        let view = Arc::new(Mutex::new(View {
            parser: vt100::Parser::new_with_callbacks(rows, cols, 5000, Responses(tx.clone())),
        }));
        let screen = view.clone();
        thread::spawn(move || {
            let mut data = [0; 16384];
            while let Ok(n) = reader.read(&mut data) {
                if n == 0 {
                    break;
                }
                let Ok(mut view) = screen.lock() else {
                    break;
                };
                view.parser.process(&data[..n]);
            }
        });
        Ok(Self {
            master: pair.master,
            child,
            view,
            input: tx,
            size: (rows, cols),
            ended: false,
            pending_trust: None,
        })
    }
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let size = (rows.max(2), cols.max(12));
        if size == self.size {
            return;
        }
        self.cancel_click();
        if self
            .master
            .resize(PtySize {
                rows: size.0,
                cols: size.1,
                pixel_width: 0,
                pixel_height: 0,
            })
            .is_ok()
        {
            self.size = size;
            if let Ok(mut v) = self.view.lock() {
                v.parser.screen_mut().set_size(size.0, size.1);
            }
        }
    }
    pub fn poll(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            self.ended = true;
            self.cancel_click();
        }
        if let Some(pending) = self.pending_trust.take() {
            let current = self.view.lock().ok().and_then(|view| {
                let mut screen = view.parser.screen().clone();
                screen.set_scrollback(0);
                claude_trust_menu(&screen)
            });
            match pending.state(current.as_ref()) {
                TrustClickState::Confirm => {
                    let _ = self.write(vec![13]);
                }
                TrustClickState::Waiting => self.pending_trust = Some(pending),
                TrustClickState::Cancel => {}
            }
        }
    }
    pub fn cancel_click(&mut self) {
        self.pending_trust = None;
    }
    pub fn write(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.input
            .try_send(bytes)
            .map_err(|_| "CLI input is busy; try again.".into())
    }
    pub fn paste(&mut self, text: &str) -> Result<(), String> {
        self.cancel_click();
        let bracketed = self
            .view
            .lock()
            .is_ok_and(|v| v.parser.screen().bracketed_paste());
        // Never allow pasted bytes to escape bracketed-paste mode or trigger terminal controls.
        let clean = crate::ui::clean(text).replace('\r', "\n");
        self.write(if bracketed {
            format!("\x1b[200~{clean}\x1b[201~").into_bytes()
        } else {
            clean.into_bytes()
        })
    }
    pub fn key(&mut self, key: KeyEvent) -> Result<(), String> {
        self.cancel_click();
        let application = self
            .view
            .lock()
            .is_ok_and(|v| v.parser.screen().application_cursor());
        if let Some(bytes) = encode(key, application) {
            self.write(bytes)
        } else {
            Ok(())
        }
    }
    /// Called only after the parent knows this gesture was a plain click.
    /// Dragging must never send a child button press or approve a dialog.
    pub fn click(
        &mut self,
        row: u16,
        column: u16,
        modifiers: M,
        claude: bool,
        captured: Option<&vt100::Screen>,
    ) -> Result<(), String> {
        self.cancel_click();
        if self.ended {
            return Err("CLI has exited. Press R to resume.".into());
        }
        let (bytes, pending) = {
            let view = self.view.lock().map_err(|_| "CLI screen is unavailable")?;
            let mut screen = view.parser.screen().clone();
            screen.set_scrollback(0);
            if let Some(captured) = captured {
                let width = screen.size().1;
                if captured.rows(0, width).nth(row as usize)
                    != screen.rows(0, width).nth(row as usize)
                {
                    return Err("CLI changed; click again.".into());
                }
            }
            let pending = if claude && modifiers.is_empty() {
                claude_trust_menu(&screen).and_then(|menu| {
                    let target = menu.at(row)?;
                    (target != menu.selected).then(|| PendingTrustClick {
                        menu,
                        target,
                        expires: Instant::now() + Duration::from_secs(1),
                    })
                })
            } else {
                None
            };
            (
                click_bytes(&screen, row, column, modifiers, claude),
                pending,
            )
        };
        if let Some(bytes) = bytes {
            self.write(bytes)?;
            self.pending_trust = pending;
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
struct TrustMenu {
    rows: [(u16, u16); 2],
    selected: usize,
}
impl TrustMenu {
    fn at(&self, row: u16) -> Option<usize> {
        self.rows
            .iter()
            .position(|&(start, end)| (start..=end).contains(&row))
    }
}
struct PendingTrustClick {
    menu: TrustMenu,
    target: usize,
    expires: Instant,
}
#[derive(Debug, PartialEq)]
enum TrustClickState {
    Waiting,
    Confirm,
    Cancel,
}
impl PendingTrustClick {
    fn state(&self, current: Option<&TrustMenu>) -> TrustClickState {
        if Instant::now() >= self.expires {
            return TrustClickState::Cancel;
        }
        let Some(current) = current else {
            return TrustClickState::Cancel;
        };
        if current.rows != self.menu.rows {
            return TrustClickState::Cancel;
        }
        if current.selected == self.target {
            TrustClickState::Confirm
        } else {
            TrustClickState::Waiting
        }
    }
}
fn claude_trust_menu(screen: &vt100::Screen) -> Option<TrustMenu> {
    let text = screen
        .contents()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let headers = [
        "Accessing workspace:",
        "Do you trust the files in this folder?",
        "Do you trust this folder?",
    ];
    if !text.contains("Enter to confirm") || !text.contains("Esc to cancel") {
        return None;
    }
    let has_header = headers.iter().any(|header| text.contains(header)) || {
        // A short pane can leave the title above the viewport while both
        // choices and the confirmation hint are visible. Inspect only recent
        // terminal scrollback; never infer an option from offscreen text.
        let mut recent = screen.clone();
        let step = usize::from(screen.size().0).max(1);
        (step..=128 + step).step_by(step).any(|offset| {
            recent.set_scrollback(offset);
            let text = recent
                .contents()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            headers.iter().any(|header| text.contains(header))
        })
    };
    if !has_header {
        return None;
    }
    let lines = screen.rows(0, screen.size().1).collect::<Vec<_>>();
    let mut rows = [(0, 0); 2];
    let mut selected = None;
    for (i, label) in ["No, exit", "Yes, I trust this folder"].iter().enumerate() {
        let (row, end, text) = lines.iter().enumerate().find_map(|(row, text)| {
            let mut value = text.trim_start_matches([' ', '❯', '›', '>']).to_string();
            // Older Claude versions number these two options and can reverse
            // their order. Match the meaning, then navigate by screen position.
            if let Some(rest) = value
                .strip_prefix("1. ")
                .or_else(|| value.strip_prefix("2. "))
            {
                value = rest.to_string();
            }
            if value.is_empty() {
                return None;
            }
            for end in row..(row + 4).min(lines.len()) {
                if end != row {
                    if screen.row_wrapped((end - 1) as u16) {
                        let used = ratatui::text::Line::raw(lines[end - 1].as_str()).width();
                        value.push_str(
                            &" ".repeat(usize::from(screen.size().1).saturating_sub(used)),
                        );
                        value.push_str(&lines[end]);
                    } else {
                        value.push(' ');
                        value.push_str(lines[end].trim());
                    }
                }
                let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
                if normalized == *label {
                    return Some((row, end, text));
                }
                if !label.starts_with(&normalized) {
                    break;
                }
            }
            None
        })?;
        rows[i] = (row as u16, end as u16);
        if text.trim_start().starts_with(['❯', '›', '>']) {
            if selected.is_some() {
                return None;
            }
            selected = Some(i);
        }
    }
    Some(TrustMenu {
        rows,
        selected: selected?,
    })
}
fn click_bytes(
    screen: &vt100::Screen,
    row: u16,
    column: u16,
    modifiers: M,
    claude: bool,
) -> Option<Vec<u8>> {
    let (height, width) = screen.size();
    if row >= height || column >= width {
        return None;
    }
    // A completed click invokes the visible choice. If an arrow is needed,
    // Terminal::poll waits for the child to redraw that selection before Enter.
    if claude && modifiers.is_empty() {
        if let Some(menu) = claude_trust_menu(screen) {
            if let Some(target) = menu.at(row) {
                return if target == menu.selected {
                    Some(vec![13])
                } else {
                    encode(
                        KeyEvent::new(
                            if menu.rows[target].0 > menu.rows[menu.selected].0 {
                                K::Down
                            } else {
                                K::Up
                            },
                            M::NONE,
                        ),
                        screen.application_cursor(),
                    )
                };
            }
        }
    }
    use vt100::{MouseProtocolEncoding as E, MouseProtocolMode as P};
    let mode = screen.mouse_protocol_mode();
    if mode == P::None {
        return None;
    }
    let bits = 4 * u16::from(modifiers.contains(M::SHIFT))
        + 8 * u16::from(modifiers.contains(M::ALT))
        + 16 * u16::from(modifiers.contains(M::CONTROL));
    let event = |release: bool| -> Option<Vec<u8>> {
        let button = bits
            + if release && screen.mouse_protocol_encoding() != E::Sgr {
                3
            } else {
                0
            };
        match screen.mouse_protocol_encoding() {
            E::Sgr => Some(
                format!(
                    "\x1b[<{button};{};{}{}",
                    column + 1,
                    row + 1,
                    if release { 'm' } else { 'M' }
                )
                .into_bytes(),
            ),
            E::Default if column <= 222 && row <= 222 => Some(vec![
                27,
                b'[',
                b'M',
                (button + 32) as u8,
                (column + 33) as u8,
                (row + 33) as u8,
            ]),
            E::Utf8 if column <= 2014 && row <= 2014 => {
                let mut text = "\x1b[M".to_string();
                for value in [button + 32, column + 33, row + 33] {
                    text.push(char::from_u32(u32::from(value))?);
                }
                Some(text.into_bytes())
            }
            _ => None,
        }
    };
    let mut bytes = event(false)?;
    if mode != P::Press {
        bytes.extend(event(true)?);
    }
    Some(bytes)
}
impl Drop for Terminal {
    fn drop(&mut self) {
        // portable-pty creates a new session/process group; signal only this owned child tree.
        #[cfg(unix)]
        if let Some(pid) = self.child.process_id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
            }
        }
        #[cfg(not(unix))]
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_millis(700);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        #[cfg(unix)]
        if let Some(pid) = self.child.process_id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub fn encode(key: KeyEvent, application: bool) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(M::CONTROL);
    let alt = key.modifiers.contains(M::ALT);
    let seq = match key.code {
        K::Char(c) if ctrl => {
            let c = c.to_ascii_lowercase();
            let n = match c {
                'a'..='z' => (c as u8 - b'a') + 1,
                '[' | '3' => 27,
                '\\' | '4' => 28,
                ']' | '5' => 29,
                '^' | '6' => 30,
                '_' | '7' => 31,
                ' ' | '@' | '2' => 0,
                _ => return None,
            };
            vec![n]
        }
        K::Char(c) => c.to_string().into_bytes(),
        K::Enter => vec![13],
        K::Backspace => vec![127],
        K::Tab => vec![9],
        K::BackTab => b"\x1b[Z".to_vec(),
        K::Esc => vec![27],
        K::Up | K::Down | K::Right | K::Left | K::Home | K::End => {
            let c = match key.code {
                K::Up => 'A',
                K::Down => 'B',
                K::Right => 'C',
                K::Left => 'D',
                K::Home => 'H',
                _ => 'F',
            };
            let modifier = 1
                + u8::from(key.modifiers.contains(M::SHIFT))
                + 2 * u8::from(alt)
                + 4 * u8::from(ctrl);
            if modifier > 1 {
                format!("\x1b[1;{modifier}{c}").into_bytes()
            } else {
                format!("\x1b{}{c}", if application { "O" } else { "[" }).into_bytes()
            }
        }
        K::Delete => b"\x1b[3~".to_vec(),
        K::Insert => b"\x1b[2~".to_vec(),
        K::PageUp => b"\x1b[5~".to_vec(),
        K::PageDown => b"\x1b[6~".to_vec(),
        _ => return None,
    };
    if alt && matches!(key.code, K::Char(_) | K::Enter | K::Backspace) {
        Some([vec![27], seq].concat())
    } else {
        Some(seq)
    }
}
fn color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
pub fn render(f: &mut Frame, a: &mut App, area: Rect) {
    let label = if a.ai.provider_label.is_empty() {
        "Claude Code"
    } else {
        &a.ai.provider_label
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if a.pane == Pane::Panel {
            theme().accent
        } else {
            theme().border
        }))
        .title(format!(
            " {label}{} ",
            if a.ai.phase.starts_with("Exited") {
                " · Exited"
            } else {
                ""
            }
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 || inner.width == 0 {
        return;
    }
    let body = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    a.ai.native_area = body;
    let Some(view) = a.ai.native_view.clone() else {
        f.render_widget(
            Paragraph::new(if a.ai.error.is_empty() {
                format!("Starting {label}…")
            } else {
                a.ai.error.clone()
            }),
            body,
        );
        return;
    };
    let Ok(mut v) = view.lock() else {
        return;
    };
    let live = v.parser.screen_mut();
    if a.reader.selection.active() {
        if a.ai.native_frozen.is_none() {
            a.ai.native_frozen = Some(live.clone());
        }
    } else {
        a.ai.native_frozen = None;
    }
    // The scroll range belongs to the displayed source. Incoming child output
    // can grow the live buffer while the frozen copy selection stays unchanged.
    let screen = a.ai.native_frozen.as_mut().unwrap_or(live);
    screen.set_scrollback(usize::MAX);
    let max = screen.scrollback();
    a.reader.max_scroll = max;
    if a.ai.follow {
        a.panel_scroll = max;
    }
    a.panel_scroll = a.panel_scroll.min(max);
    screen.set_scrollback(max - a.panel_scroll);
    for y in 0..body.height {
        for x in 0..body.width {
            let Some(cell) = screen.cell(y, x) else {
                continue;
            };
            if cell.is_wide_continuation() {
                continue;
            }
            let mut style = Style::default()
                .fg(color(cell.fgcolor()))
                .bg(color(cell.bgcolor()));
            for (enabled, modifier) in [
                (cell.bold(), Modifier::BOLD),
                (cell.dim(), Modifier::DIM),
                (cell.italic(), Modifier::ITALIC),
                (cell.underline(), Modifier::UNDERLINED),
                (cell.inverse(), Modifier::REVERSED),
            ] {
                if enabled {
                    style = style.add_modifier(modifier);
                }
            }
            f.buffer_mut()[(body.x + x, body.y + y)]
                .set_symbol(if cell.has_contents() {
                    cell.contents()
                } else {
                    " "
                })
                .set_style(style);
        }
    }
    if a.ai.native_input
        && a.pane == Pane::Panel
        && a.panel == Panel::Ai
        && !screen.hide_cursor()
        && screen.scrollback() == 0
    {
        let (y, x) = screen.cursor_position();
        if y < body.height && x < body.width {
            f.set_cursor_position((body.x + x, body.y + y));
        }
    }
    a.reader.selection.capture(
        f.buffer_mut(),
        body,
        a.panel_scroll,
        format!("native:{}", a.ai.session),
    );
    a.reader
        .selection
        .highlight(f.buffer_mut(), theme().selected);
    let hint = if a.ai.phase.starts_with("Exited") {
        "CLI exited · R Resume"
    } else if a.ai.native_input && (a.reader.selection.active() || screen.scrollback() > 0) {
        "Type to resume · Ctrl-] LazyReview"
    } else if a.ai.native_input {
        "Ctrl-] LazyReview · drag to copy"
    } else {
        "Click/i Type · v Select · y Copy"
    };
    f.render_widget(
        Paragraph::new(hint).style(Style::default().fg(theme().muted)),
        Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trust_click_selects_target_then_confirms_only_after_child_redraw() {
        let mut p = vt100::Parser::new(20, 45, 0);
        p.process(b"Accessing workspace:\r\n\r\n");
        p.process(
            "❯ No, exit\r\n  Yes, I trust this folder\r\n\r\nEnter to confirm · Esc to cancel"
                .as_bytes(),
        );
        assert_eq!(
            click_bytes(p.screen(), 3, 6, M::NONE, true),
            Some(b"\x1b[B".to_vec())
        );
        assert_eq!(click_bytes(p.screen(), 2, 6, M::NONE, true), Some(vec![13]));
        assert_eq!(click_bytes(p.screen(), 0, 6, M::NONE, true), None);
        assert_eq!(click_bytes(p.screen(), 3, 6, M::NONE, false), None);
        let menu = claude_trust_menu(p.screen()).unwrap();
        assert_eq!(menu.rows, [(2, 2), (3, 3)]);
        let mut pending = PendingTrustClick {
            menu: menu.clone(),
            target: 1,
            expires: Instant::now() + Duration::from_secs(1),
        };
        assert_eq!(pending.state(Some(&menu)), TrustClickState::Waiting);
        let mut changed = menu.clone();
        changed.selected = 1;
        assert_eq!(pending.state(Some(&changed)), TrustClickState::Confirm);
        changed.rows[1].0 += 1;
        assert_eq!(pending.state(Some(&changed)), TrustClickState::Cancel);
        assert_eq!(pending.state(None), TrustClickState::Cancel);
        pending.expires = Instant::now();
        assert_eq!(pending.state(Some(&menu)), TrustClickState::Cancel);
        // Cursor-key mode is respected. The arrow and confirmation are separate.
        p.process(b"\x1b[?1h");
        assert_eq!(
            click_bytes(p.screen(), 3, 6, M::NONE, true),
            Some(b"\x1bOB".to_vec())
        );
        p.process(b"\x1b[2J");
        assert!(claude_trust_menu(p.screen()).is_none());
    }
    #[test]
    fn wrapped_numbered_trust_choices_use_visual_order_and_all_visible_label_rows() {
        let mut p = vt100::Parser::new(30, 22, 0);
        p.process("Do you trust the files in this folder?\r\n\r\n  1. Yes, I trust this folder\r\n❯ 2. No, exit\r\n\r\nEnter to confirm · Esc to cancel".as_bytes());
        let menu = claude_trust_menu(p.screen()).unwrap();
        assert_eq!(menu.selected, 0);
        assert!(menu.rows[1].1 > menu.rows[1].0);
        assert!(menu.rows[1].0 < menu.rows[0].0);
        for row in menu.rows[1].0..=menu.rows[1].1 {
            assert_eq!(
                click_bytes(p.screen(), row, 3, M::NONE, true),
                Some(b"\x1b[A".to_vec())
            );
        }
        assert_eq!(
            click_bytes(p.screen(), menu.rows[0].0, 3, M::NONE, true),
            Some(vec![13])
        );
        assert_eq!(
            click_bytes(p.screen(), menu.rows[0].0, 3, M::SHIFT, true),
            None
        );
        // Ink also wraps at words using explicit rows and indentation.
        let mut p = vt100::Parser::new(30, 22, 0);
        p.process("Accessing workspace:\r\n\r\n❯ No, exit\r\n  Yes, I trust this\r\n  folder\r\n\r\nEnter to confirm ·\r\nEsc to cancel".as_bytes());
        let menu = claude_trust_menu(p.screen()).unwrap();
        assert_eq!(menu.rows, [(2, 2), (3, 4)]);
        assert_eq!(
            click_bytes(p.screen(), 4, 3, M::NONE, true),
            Some(b"\x1b[B".to_vec())
        );
    }
    #[test]
    fn trust_choices_still_work_when_title_scrolled_above_short_viewport() {
        let mut p = vt100::Parser::new(8, 40, 128);
        p.process(b"Accessing workspace:\r\n\r\nSome context\r\n\r\nMore context\r\n\r\n\r\n\r\n");
        p.process(
            "❯ No, exit\r\n  Yes, I trust this folder\r\n\r\nEnter to confirm · Esc to cancel"
                .as_bytes(),
        );
        assert!(!p.screen().contents().contains("Accessing workspace:"));
        let menu = claude_trust_menu(p.screen()).unwrap();
        assert_eq!(
            click_bytes(p.screen(), menu.rows[1].0, 3, M::NONE, true),
            Some(b"\x1b[B".to_vec())
        );
    }
    #[test]
    fn mouse_clicks_are_forwarded_only_when_requested_with_local_coordinates() {
        let mut p = vt100::Parser::new(30, 80, 0);
        assert_eq!(click_bytes(p.screen(), 4, 8, M::NONE, false), None);
        p.process(b"\x1b[?1000h\x1b[?1006h");
        assert_eq!(
            click_bytes(p.screen(), 4, 8, M::NONE, false),
            Some(b"\x1b[<0;9;5M\x1b[<0;9;5m".to_vec())
        );
        assert_eq!(
            click_bytes(p.screen(), 4, 8, M::CONTROL, false),
            Some(b"\x1b[<16;9;5M\x1b[<16;9;5m".to_vec())
        );
        p.process(b"\x1b[?1000l");
        assert_eq!(click_bytes(p.screen(), 4, 8, M::NONE, false), None);
        p.process(b"\x1b[?1006l\x1b[?9h");
        assert_eq!(
            click_bytes(p.screen(), 4, 8, M::NONE, false),
            Some(vec![27, b'[', b'M', 32, 41, 37])
        );
    }
    #[test]
    fn selection_stays_on_frozen_rows_while_child_keeps_streaming() {
        let (tx, _) = mpsc::sync_channel(8);
        let view = Arc::new(Mutex::new(View {
            parser: vt100::Parser::new_with_callbacks(5, 78, 100, Responses(tx)),
        }));
        for i in 0..16 {
            view.lock()
                .unwrap()
                .parser
                .process(format!("Line {i}\r\n").as_bytes());
        }
        let mut a = App::new(serde_json::json!({"files":[],"threads":[]}));
        a.ai.native_view = Some(view.clone());
        a.ai.follow = true;
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 8)).unwrap();
        terminal.draw(|f| render(f, &mut a, f.area())).unwrap();
        a.ai.follow = false;
        a.reader.selection.begin_keyboard();
        a.reader.selection.step('$', a.reader.max_scroll + 4);
        terminal.draw(|f| render(f, &mut a, f.area())).unwrap();
        let selected = a.reader.selection.text();
        let offset = a.panel_scroll;
        let max = a.reader.max_scroll;
        assert!(selected.contains("Line"));
        for i in 16..28 {
            view.lock()
                .unwrap()
                .parser
                .process(format!("Line {i}\r\n").as_bytes());
        }
        terminal.draw(|f| render(f, &mut a, f.area())).unwrap();
        assert_eq!(
            a.reader.selection.text(),
            selected,
            "new output must not move frozen copy rows"
        );
        assert_eq!(a.panel_scroll, offset);
        assert_eq!(a.reader.max_scroll, max);
        a.reader.selection.clear();
        a.ai.follow = true;
        terminal.draw(|f| render(f, &mut a, f.area())).unwrap();
        assert!(a.reader.max_scroll > max);
        assert!(a.ai.native_frozen.is_none());
    }
    #[test]
    fn input_preserves_native_shortcuts() {
        assert_eq!(
            encode(KeyEvent::new(K::Char('c'), M::CONTROL), false),
            Some(vec![3])
        );
        assert_eq!(
            encode(KeyEvent::new(K::Up, M::NONE), true),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            encode(KeyEvent::new(K::Enter, M::ALT), false),
            Some(vec![27, 13])
        );
        assert_eq!(
            encode(KeyEvent::new(K::BackTab, M::SHIFT), false),
            Some(b"\x1b[Z".to_vec())
        );
    }
    #[test]
    fn emulator_renders_cells_not_host_control_sequences() {
        let (tx, _) = mpsc::sync_channel(8);
        let mut p = vt100::Parser::new_with_callbacks(6, 40, 100, Responses(tx));
        p.process(b"\x1b[31mClaude\x1b[0m\r\nHello \xe4\xb8\x96\xe7\x95\x8c\x1b]52;c;ZmFrZQ==\x07");
        assert!(p.screen().contents().contains("Hello 世界"));
        assert_eq!(
            p.screen().cell(0, 0).unwrap().fgcolor(),
            vt100::Color::Idx(1)
        );
    }
}
