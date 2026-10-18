//! Opt-in local input diagnostics. Never serialize text, clipboard data or PR context.
use crate::app::{App, Pane, Panel};
use crossterm::event::{Event, KeyCode, MouseEventKind};
use serde_json::json;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    time::Instant,
};

const MAX_BYTES: u64 = 262_144;
pub struct InputTrace {
    file: Option<File>,
    bytes: u64,
    start: Instant,
}
impl InputTrace {
    pub fn from_env() -> Self {
        let file = std::env::var_os("LAZYREVIEW_INPUT_TRACE")
            .and_then(|path| Self::open(Path::new(&path)).ok());
        let bytes = file
            .as_ref()
            .and_then(|f| f.metadata().ok())
            .map_or(0, |m| m.len());
        Self {
            file,
            bytes,
            start: Instant::now(),
        }
    }
    fn open(path: &Path) -> std::io::Result<File> {
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::other("Trace path must be a regular file"));
        }
        Ok(file)
    }
    pub fn record(&mut self, event: &Event, app: Option<&App>, route: &str) {
        if self.file.is_none() || self.bytes >= MAX_BYTES {
            return;
        }
        let Some(app) = app else {
            return;
        };
        if !app.ai.native_mode || app.panel != Panel::Ai || !app.panes.panel {
            return;
        }
        let mut value = json!({
            "ms":self.start.elapsed().as_millis().min(u64::MAX as u128) as u64,
            "route":route,
            "focus":match app.pane { Pane::Tree=>"files", Pane::Diff=>"changes", Pane::Panel=>"companion" },
            "input":app.ai.native_input,
            "selection":app.reader.selection.active(),
            "pending_press":app.reader.selection.mouse_pending(),
            "modal":app.modal.is_some(),
            "live":app.panel_scroll==app.reader.max_scroll,
            "area":[app.ai.native_area.x,app.ai.native_area.y,app.ai.native_area.width,app.ai.native_area.height]
        });
        match event {
            Event::Key(key) => {
                value["event"] = json!(match key.code {
                    KeyCode::Up => "up",
                    KeyCode::Down => "down",
                    KeyCode::Enter => "enter",
                    KeyCode::Esc => "escape",
                    KeyCode::Tab => "tab",
                    KeyCode::BackTab => "backtab",
                    _ => "key",
                });
            }
            Event::Mouse(mouse) => {
                value["event"] = json!(match mouse.kind {
                    MouseEventKind::Down(_) => "mouse_down",
                    MouseEventKind::Up(_) => "mouse_up",
                    MouseEventKind::Drag(_) => "mouse_drag",
                    MouseEventKind::Moved => "mouse_move",
                    _ => "mouse_scroll",
                });
                value["cell"] = json!([mouse.column, mouse.row]);
            }
            Event::Paste(_) => value["event"] = json!("paste"),
            _ => return,
        }
        let Ok(mut line) = serde_json::to_vec(&value) else {
            return;
        };
        line.push(b'\n');
        if self.bytes + line.len() as u64 > MAX_BYTES {
            self.file = None;
            return;
        }
        if self
            .file
            .as_mut()
            .is_some_and(|file| file.write_all(&line).is_ok())
        {
            self.bytes += line.len() as u64;
        } else {
            self.file = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    #[test]
    fn trace_excludes_text_and_context_and_stops_at_its_size_limit() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("events.jsonl");
        let mut trace = InputTrace {
            file: Some(InputTrace::open(&path).unwrap()),
            bytes: 0,
            start: Instant::now(),
        };
        let mut app = App::new(json!({"title":"PRIVATE_PR_TITLE","files":[]}));
        app.ai.native_mode = true;
        app.panel = Panel::Ai;
        app.panes.panel = true;
        trace.record(
            &Event::Paste("PRIVATE_PASTE_TOKEN".into()),
            Some(&app),
            "received",
        );
        for c in "PRIVATE_TYPED_TOKEN".chars() {
            trace.record(
                &Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)),
                Some(&app),
                "cli",
            );
        }
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.contains("PRIVATE"));
        assert!(content.contains("paste") && content.contains("cli"));
        for line in content.lines() {
            assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
        }
        trace.bytes = MAX_BYTES;
        trace.record(&Event::Paste("omitted".into()), Some(&app), "received");
        assert_eq!(std::fs::read_to_string(path).unwrap(), content);
    }
    #[cfg(unix)]
    #[test]
    fn trace_does_not_follow_a_symlink() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        std::fs::write(&target, "unchanged").unwrap();
        let link = temp.path().join("trace");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(InputTrace::open(&link).is_err());
        assert_eq!(std::fs::read_to_string(target).unwrap(), "unchanged");
    }
}
