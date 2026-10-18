//! Nonblocking, offline Mermaid-to-Unicode rendering. No redraw starts a process.
//! An exact-pinned upstream renderer runs on a bounded worker, then an LRU serves
//! frames. Invalid/unsupported/oversized diagrams keep their original source.
use ratatui::text::Line;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_SOURCE: usize = 8192;
const MAX_OUTPUT: usize = 65536;
const CACHE_SIZE: usize = 32;
const QUEUE_SIZE: usize = 8;
const BUDGET: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackReason {
    Unsupported,
    TooWide,
    Limit,
    Unavailable,
}
#[derive(Clone, Debug)]
pub enum RenderStatus {
    Pending,
    Ready(Arc<Vec<String>>),
    Fallback(FallbackReason),
}
#[derive(Clone, PartialEq, Eq)]
struct Key {
    source: String,
    width: u16,
}
type Cache = Arc<Mutex<VecDeque<(Key, RenderStatus)>>>;
type Active = Arc<Mutex<Option<Arc<Mutex<Child>>>>>;
struct Renderer {
    cache: Cache,
    tx: SyncSender<Key>,
    revision: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    active: Active,
    worker: Mutex<Option<JoinHandle<()>>>,
}
static RENDERER: OnceLock<Renderer> = OnceLock::new();

/// Never parses, lays out, or waits for Node on the drawing thread.
pub fn request(source: &str, width: u16) -> RenderStatus {
    if let Err(reason) = validate(source, width) {
        return RenderStatus::Fallback(reason);
    }
    RENDERER.get_or_init(Renderer::new).request(source, width)
}

/// Include in Markdown/reader cache keys so completed diagrams repaint.
pub fn revision() -> u64 {
    RENDERER
        .get()
        .map_or(0, |renderer| renderer.revision.load(Ordering::Relaxed))
}

/// Stop our worker and reap only our own renderer child when LazyReview exits.
pub fn shutdown() {
    if let Some(renderer) = RENDERER.get() {
        renderer.shutdown();
    }
}

fn validate(source: &str, width: u16) -> Result<(), FallbackReason> {
    if source.len() > MAX_SOURCE || source.lines().count() > 128 || source.is_empty() {
        return Err(FallbackReason::Limit);
    }
    if width < 12 {
        return Err(FallbackReason::TooWide);
    }
    if source.chars().any(|c| {
        !matches!(c, '\n' | '\t')
            && (c.is_control()
                || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                // Upstream lays out labels in JS character cells. Do not show
                // broken geometry for wide glyphs, combining marks or emoji.
                || (!c.is_ascii() && Line::from(c.to_string()).width() != 1))
    }) {
        return Err(FallbackReason::Unsupported);
    }
    Ok(())
}

impl Renderer {
    fn new() -> Self {
        let cache: Cache = Arc::new(Mutex::new(VecDeque::new()));
        let revision = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let active: Active = Arc::new(Mutex::new(None));
        let (tx, rx) = mpsc::sync_channel::<Key>(QUEUE_SIZE);
        let (worker_cache, worker_revision, worker_stop, worker_active) = (
            cache.clone(),
            revision.clone(),
            stop.clone(),
            active.clone(),
        );
        let worker = thread::spawn(move || {
            let files = prepare();
            while !worker_stop.load(Ordering::Relaxed) {
                let key = match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(key) => key,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                };
                // Resizes/navigation can evict a queued request before it starts.
                if !worker_cache.lock().unwrap().iter().any(|(k, _)| k == &key) {
                    continue;
                }
                let result = match &files {
                    Some(dir) => run(dir.path(), &key, BUDGET, &worker_active),
                    None => RenderStatus::Fallback(FallbackReason::Unavailable),
                };
                let mut cache = worker_cache.lock().unwrap();
                if let Some((_, entry)) = cache.iter_mut().find(|(k, _)| k == &key) {
                    *entry = result;
                    worker_revision.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        Self {
            cache,
            tx,
            revision,
            stop,
            active,
            worker: Mutex::new(Some(worker)),
        }
    }
    fn request(&self, source: &str, width: u16) -> RenderStatus {
        if self.stop.load(Ordering::Relaxed) {
            return RenderStatus::Fallback(FallbackReason::Unavailable);
        }
        let width = width.min(240);
        let mut cache = self.cache.lock().unwrap();
        if let Some(index) = cache
            .iter()
            .position(|(k, _)| k.width == width && k.source == source)
        {
            let entry = cache.remove(index).unwrap();
            let result = entry.1.clone();
            cache.push_back(entry);
            return result;
        }
        let key = Key {
            source: source.into(),
            width,
        };
        // Queue pressure must not block input or leave an unqueued Pending entry.
        if self.tx.try_send(key.clone()).is_err() {
            return RenderStatus::Fallback(FallbackReason::Unavailable);
        }
        if cache.len() >= CACHE_SIZE {
            cache.pop_front();
        }
        cache.push_back((key, RenderStatus::Pending));
        RenderStatus::Pending
    }
    fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(child) = self.active.lock().unwrap().as_ref() {
            let mut child = child.lock().unwrap();
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}

fn prepare() -> Option<tempfile::TempDir> {
    let dir = tempfile::tempdir().ok()?;
    for (name, content) in [
        ("mermaid.mjs", include_str!("mermaid.mjs")),
        (
            "beautiful-mermaid-1.1.3.mjs",
            include_str!("../../vendor/beautiful-mermaid-1.1.3.mjs"),
        ),
    ] {
        std::fs::write(dir.path().join(name), content).ok()?;
    }
    Some(dir)
}

fn run(dir: &Path, key: &Key, budget: Duration, active: &Active) -> RenderStatus {
    run_inner(dir, key, budget, active)
        .unwrap_or(RenderStatus::Fallback(FallbackReason::Unavailable))
}
fn run_inner(dir: &Path, key: &Key, budget: Duration, active: &Active) -> Option<RenderStatus> {
    // A private stdin file avoids any potentially blocking pipe write. No source
    // appears in argv, environment, diagnostics, or a persistent cache.
    let mut input = tempfile::tempfile().ok()?;
    serde_json::to_writer(&mut input, &json!({"source":key.source,"width":key.width})).ok()?;
    input.flush().ok()?;
    input.seek(SeekFrom::Start(0)).ok()?;
    let mut child = Command::new("node")
        .arg("--max-old-space-size=64")
        .arg(dir.join("mermaid.mjs"))
        .current_dir(dir)
        // No inherited credentials, preload scripts, CLI hooks or Node options.
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("NO_COLOR", "1")
        .stdin(Stdio::from(input))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let child = Arc::new(Mutex::new(child));
    *active.lock().unwrap() = Some(child.clone());
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take((MAX_OUTPUT + 1) as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes)
    });
    let start = Instant::now();
    let success = loop {
        let done = child.lock().unwrap().try_wait();
        match done {
            Ok(Some(status)) => break status.success(),
            Err(_) => break false,
            _ if start.elapsed() >= budget => break false,
            _ => thread::sleep(Duration::from_millis(5)),
        }
    };
    {
        let mut child = child.lock().unwrap();
        if !success {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
    *active.lock().unwrap() = None;
    let output = reader.join().ok()??;
    if !success || output.len() > MAX_OUTPUT {
        return None;
    }
    let value: Value = serde_json::from_slice(&output).ok()?;
    if let Some(reason) = value.get("error").and_then(Value::as_str) {
        return Some(RenderStatus::Fallback(match reason {
            "wide" => FallbackReason::TooWide,
            "limit" => FallbackReason::Limit,
            _ => FallbackReason::Unsupported,
        }));
    }
    let lines = value.get("lines")?.as_array()?;
    if lines.is_empty() || lines.len() > 256 {
        return None;
    }
    let mut text = Vec::with_capacity(lines.len());
    for line in lines {
        let line = line.as_str()?;
        if line.chars().any(char::is_control) {
            return None;
        }
        if Line::from(line).width() > key.width as usize {
            return Some(RenderStatus::Fallback(FallbackReason::TooWide));
        }
        text.push(line.to_owned());
    }
    Some(RenderStatus::Ready(Arc::new(text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ready(renderer: &Renderer, source: &str, width: u16) -> RenderStatus {
        let start = Instant::now();
        loop {
            match renderer.request(source, width) {
                RenderStatus::Pending if start.elapsed() < Duration::from_secs(5) => {
                    thread::sleep(Duration::from_millis(5))
                }
                result => return result,
            }
        }
    }
    #[test]
    fn async_diagrams_cache_and_preserve_geometry() {
        let renderer = Renderer::new();
        let source = "flowchart TD\nA[Review] --> B[Understand]\nB --> C[Comment]";
        let start = Instant::now();
        assert!(matches!(
            renderer.request(source, 60),
            RenderStatus::Pending
        ));
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "enqueue must not render"
        );
        let RenderStatus::Ready(lines) = ready(&renderer, source, 60) else {
            panic!("Node renderer unavailable")
        };
        assert!(lines.join("\n").contains("Understand"));
        assert!(lines.join("\n").contains('▼'));
        assert!(
            lines
                .iter()
                .all(|line| Line::from(line.as_str()).width() <= 60)
        );
        let revision = renderer.revision.load(Ordering::Relaxed);
        let RenderStatus::Ready(cached) = renderer.request(source, 60) else {
            panic!("cache miss")
        };
        assert!(Arc::ptr_eq(&lines, &cached));
        assert_eq!(revision, renderer.revision.load(Ordering::Relaxed));
        assert!(matches!(
            ready(&renderer, source, 12),
            RenderStatus::Ready(_)
        ));
        assert!(matches!(
            ready(
                &renderer,
                "flowchart LR\nA[Before migration] --> B[After migration]",
                20
            ),
            RenderStatus::Fallback(FallbackReason::TooWide)
        ));
        renderer.shutdown();
        assert!(renderer.active.lock().unwrap().is_none());
    }
    #[test]
    fn input_boundaries_do_not_start_a_renderer() {
        assert_eq!(
            validate(&"A".repeat(MAX_SOURCE + 1), 80),
            Err(FallbackReason::Limit)
        );
        assert_eq!(
            validate("graph TD\n A[\u{1b}[31m]", 80),
            Err(FallbackReason::Unsupported)
        );
        assert_eq!(
            validate("graph TD\n A[東京]", 80),
            Err(FallbackReason::Unsupported)
        );
        assert_eq!(validate("graph TD\n A[éclair]", 80), Ok(()));
        assert_eq!(
            validate("graph TD\n A-->B", 8),
            Err(FallbackReason::TooWide)
        );
    }
    #[test]
    fn helper_timeout_is_reaped_and_output_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let active: Active = Arc::new(Mutex::new(None));
        let key = Key {
            source: "graph TD\n A-->B".into(),
            width: 80,
        };
        std::fs::write(
            dir.path().join("mermaid.mjs"),
            "setInterval(() => {}, 1000)",
        )
        .unwrap();
        let start = Instant::now();
        assert!(matches!(
            run(dir.path(), &key, Duration::from_millis(150), &active),
            RenderStatus::Fallback(FallbackReason::Unavailable)
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(active.lock().unwrap().is_none());
        std::fs::write(
            dir.path().join("mermaid.mjs"),
            "process.stdout.write('x'.repeat(100000));",
        )
        .unwrap();
        assert!(matches!(
            run(dir.path(), &key, Duration::from_millis(150), &active),
            RenderStatus::Fallback(FallbackReason::Unavailable)
        ));
    }

    #[test]
    fn request_pressure_stays_bounded() {
        let renderer = Renderer::new();
        let start = Instant::now();
        for i in 0..200 {
            renderer.request(&format!("flowchart TD\nA{i} --> B{i}"), 60);
        }
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(renderer.cache.lock().unwrap().len() <= CACHE_SIZE);
        renderer.shutdown();
    }

    #[test]
    #[ignore = "manual latency measurement; no model or network"]
    fn benchmark_cache_latency() {
        let source = "flowchart TD\n PR[Pull request] --> Patch[Read patches]\n Patch --> Context[Ask companion]";
        let start = Instant::now();
        let renderer = Renderer::new();
        assert!(matches!(
            renderer.request(source, 84),
            RenderStatus::Pending
        ));
        let enqueue = start.elapsed();
        assert!(matches!(
            ready(&renderer, source, 84),
            RenderStatus::Ready(_)
        ));
        let cold = start.elapsed();
        let start = Instant::now();
        for _ in 0..10000 {
            validate(source, 84).unwrap();
            assert!(matches!(
                renderer.request(source, 84),
                RenderStatus::Ready(_)
            ));
        }
        println!(
            "Mermaid: first enqueue {:?}; cold ready {:?}; warm mean {:?}",
            enqueue,
            cold,
            start.elapsed() / 10000
        );
        renderer.shutdown();
    }
}
