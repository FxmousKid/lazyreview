use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
};
pub struct Bridge {
    pub(crate) child: Arc<Mutex<Child>>,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    _dir: tempfile::TempDir,
}
impl Bridge {
    pub fn new() -> Result<Self, String> {
        Self::with_mode(false)
    }
    pub fn sample_only() -> Result<Self, String> {
        Self::with_mode(true)
    }
    fn with_mode(sample_only: bool) -> Result<Self, String> {
        let dir = tempfile::tempdir().map_err(|_| "Cannot create private bridge directory")?;
        for (name, content) in [
            ("bridge.mjs", include_str!("bridge.mjs")),
            ("discovery.mjs", include_str!("discovery.mjs")),
            ("repositories.mjs", include_str!("repositories.mjs")),
            ("completion.mjs", include_str!("completion.mjs")),
            ("comment.mjs", include_str!("comment.mjs")),
            ("syntax.mjs", include_str!("syntax.mjs")),
            ("github.ts", include_str!("../../lib/github.ts")),
            ("review.ts", include_str!("../../lib/review.ts")),
            ("sample.ts", include_str!("../../lib/sample.ts")),
        ] {
            std::fs::write(dir.path().join(name), content).map_err(|_| "Cannot prepare bridge")?;
        }
        let mut child = Command::new("node")
            .arg("--disable-warning=ExperimentalWarning")
            .arg(dir.path().join("bridge.mjs"))
            .env(
                "LAZYREVIEW_SAMPLE_ONLY",
                if sample_only { "1" } else { "0" },
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Node.js 22.18+ is required. Install Node and retry.")?;
        let input = child.stdin.take().ok_or("Cannot open bridge input")?;
        let output = BufReader::new(child.stdout.take().ok_or("Cannot open bridge output")?);
        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            input,
            output,
            _dir: dir,
        })
    }
    pub fn request(&mut self, value: &Value) -> Result<Value, String> {
        writeln!(self.input, "{value}").map_err(|_| "Bridge stopped. Check Node.js version.")?;
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .map_err(|_| "Cannot read bridge response")?;
        let response: Value = serde_json::from_str(&line)
            .map_err(|_| "Bridge stopped. Node.js 22.18+ is required.")?;
        if response["ok"] == true {
            Ok(response["data"].clone())
        } else {
            Err(response["error"]
                .as_str()
                .unwrap_or("Request failed")
                .to_string())
        }
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
pub struct Worker {
    child: Arc<Mutex<Child>>,
    join: Option<thread::JoinHandle<()>>,
    pub tx: Sender<Value>,
    pub rx: Receiver<(String, Result<Value, String>)>,
}
impl Worker {
    pub fn start(mut bridge: Bridge) -> Self {
        let child = bridge.child.clone();
        let (tx, requests) = mpsc::channel::<Value>();
        let (responses, rx) = mpsc::channel();
        let join = thread::spawn(move || {
            for request in requests {
                let op = request["operation"].as_str().unwrap_or("").to_string();
                if op == "__shutdown" {
                    break;
                }
                let result = bridge.request(&request);
                if responses.send((op, result)).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            tx,
            rx,
            join: Some(join),
        }
    }
}
pub fn initial(bridge: &mut Bridge, url: Option<&str>) -> Result<Value, String> {
    bridge.request(&match url {
        Some(url) => json!({"operation":"load","url":url}),
        None => json!({"operation":"sample"}),
    })
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.tx.send(json!({"operation":"__shutdown"}));
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_adapter_round_trip_without_network() {
        let mut b = Bridge::new().unwrap();
        let pr = initial(&mut b, None).unwrap();
        assert_eq!(pr["demo"], true);
        let path = pr["files"][0]["filename"].clone();
        let context = b
            .request(&json!({"operation":"attach","kind":"file","path":path}))
            .unwrap();
        assert!(context["content"].as_str().unwrap().contains("CommandMenu"));
        let answer=b.request(&json!({"operation":"chat","context":[context],"messages":[{"role":"user","content":"Explain"}]})).unwrap();
        assert!(
            answer["text"]
                .as_str()
                .unwrap()
                .contains("not an AI-generated answer")
        );
        let base = json!({"expectedHead":pr["head"],"expectedBase":pr["base"]});
        let mut mutate = |action: Value| {
            let mut input = base.clone();
            for (k, v) in action.as_object().unwrap() {
                input[k] = v.clone();
            }
            b.request(&input).unwrap()
        };
        let viewed = mutate(json!({"operation":"viewed","path":path,"viewed":true}));
        assert_eq!(viewed["files"][0]["viewed"], true);
        let draft = mutate(
            json!({"operation":"draft","path":path,"side":"RIGHT","startLine":10,"line":11,"body":"Test the empty list"}),
        );
        let last = draft["threads"].as_array().unwrap().last().unwrap();
        assert_eq!(last["pending"], true);
        let id = last["id"].clone();
        let comment_id = last["comments"][0]["id"].clone();
        let submitted =
            mutate(json!({"operation":"submit","event":"COMMENT","body":"Review complete"}));
        assert!(
            submitted["threads"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["pending"] != true)
        );
        let reply = mutate(json!({"operation":"reply","commentId":comment_id,"body":"Reply"}));
        assert_eq!(
            reply["threads"].as_array().unwrap().last().unwrap()["comments"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let resolved = mutate(json!({"operation":"resolve","threadId":id,"resolved":true}));
        assert_eq!(
            resolved["threads"].as_array().unwrap().last().unwrap()["resolved"],
            true
        );
        let conversation = mutate(json!({"operation":"conversation","body":"General comment"}));
        assert_eq!(
            conversation["conversation"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["body"],
            "General comment"
        );
    }
    #[test]
    fn hosted_bridge_refuses_live_reads_before_authentication() {
        let mut bridge = Bridge::sample_only().unwrap();
        assert!(initial(&mut bridge, None).unwrap()["demo"] == true);
        for input in [
            json!({"operation":"repositories"}),
            json!({"operation":"discover"}),
            json!({"operation":"load","url":"https://github.com/example/private/pull/1"}),
            json!({"operation":"syntax","demo":false}),
        ] {
            assert!(bridge.request(&input).unwrap_err().contains("synthetic PR"));
        }
    }
    #[test]
    fn worker_shutdown_restores_and_removes_child() {
        let b = Bridge::new().unwrap();
        let child = b.child.clone();
        let worker = Worker::start(b);
        drop(worker);
        assert!(child.lock().unwrap().try_wait().unwrap().is_some());
    }
}
