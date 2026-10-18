//! Native clipboard commands, never a shell. Invoked only after a user copy action.
use std::{
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub fn copy(text: String) -> Result<(), String> {
    if text.is_empty() {
        return Err("Select text first.".into());
    }
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(target_os = "windows") {
        &[("clip", &[])]
    } else {
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])]
    };
    for (command, args) in candidates {
        let Ok(mut child) = Command::new(command)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        let Some(mut input) = child.stdin.take() else {
            continue;
        };
        let value = text.clone();
        let writer = std::thread::spawn(move || input.write_all(value.as_bytes()));
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let wrote = writer.join().is_ok_and(|v| v.is_ok());
                    if status.success() && wrote {
                        return Ok(());
                    }
                    break;
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = writer.join();
                    break;
                }
            }
        }
    }
    Err("Clipboard unavailable. Use your terminal's native selection modifier to copy.".into())
}
