# Checks and limits

Run commands from the repository root. Node.js 22.18+ and Rust 1.88+ are required; native builds also need a C compiler. No npm installation is needed for the checked-in source.

## Choose a check

| Changed surface | Checks |
| --- | --- |
| GitHub adapters, discovery, provider/session contracts | `npm test` |
| Native UI, navigation, Markdown, syntax, layout | `cargo test --locked --manifest-path tui/Cargo.toml` and `cargo fmt --manifest-path tui/Cargo.toml -- --check` |
| Hosted TUI protocol or session lifecycle | Release build, then `npm run test:demo-server` |
| Showcase, browser input or native/browser rendering | `npm run site:build`, release build, then `npm run test:site` |
| Recorded tour or native pictures | [Rebuild the shared assets](demo/README.md#rebuild), then `node --test tests/demo.test.mjs` and `npm run site:build` |
| Documentation | Verify linked paths and commands; run any contract test affected by a documented table |

`npm test` includes the Node/TypeScript contracts. Browser-native tests skip when no native executable is available, so a JavaScript-only pass does **not** validate the hosted TUI. CI builds the release binary before its dedicated service/browser checks; [the workflow](.github/workflows/check.yml) defines the full gate.

`npm run test:site` needs installed Chrome/Chromium (`CHROME_PATH` overrides discovery). It checks the interactive native TUI and the separate web preview with dispatched keys/mouse, multiple viewport sizes, escaped content, sample review flows and reduced motion. The showcase's TUI requires the Node/native HTTP service; a static upload only supports the recording and web preview.

## Native terminal regression

Build once, then choose the replay for the changed workflow:

```sh
cargo build --release --locked --manifest-path tui/Cargo.toml
export LAZYREVIEW_BINARY="$PWD/tui/target/release/lazyreview"
python3 tests/tui-pty.py                 # ranges, comments, review, threads
python3 tests/tui-controls-pty.py        # panes, help, themes, resize
python3 tests/tui-search-pty.py          # diff search
python3 tests/tui-reading-pty.py         # rendered PR timeline and threads
python3 tests/tui-native-entry-pty.py    # launcher → chooser → embedded CLI input
python3 tests/tui-review-flow-pty.py     # exactly one publication through the bridge
```

These use synthetic PRs or fake providers and do not publish to GitHub. Clipboard/input replays may need macOS tools; platform requirements are visible in each script. Keep unrelated terminals and Zellij sessions running. [CLI diagnostics](tui/CLI-INTEGRATIONS.md#local-input-diagnostics) covers isolated menu, copy and Zellij replays.

Performance probes are opt-in, with machine/network-dependent results:

```sh
cargo test --release --locked --manifest-path tui/Cargo.toml benchmark_syntax_latency -- --ignored --nocapture
cargo test --release --locked --manifest-path tui/Cargo.toml benchmark_split_search_latency -- --ignored --nocapture
cargo test --release --locked --manifest-path tui/Cargo.toml mermaid::tests::benchmark_cache_latency -- --ignored --nocapture
npm run benchmark:browser # Chrome + built native binary; optional showcase URL argument
```

The browser probe reports idle/navigation work counts (frames, glyphs, rectangles and transcript updates). Prefer these deterministic work bounds, revision checks and a before/after workload over a timing threshold tied to one machine. Ignored rendering exporters and benchmarks are manual tools, not a substitute for the default regression suite.

## Live checks

`./lazyreview --check PR_URL` and `node tests/tui-live-read.mjs --live --pr PR_URL` read GitHub without posting. Provider probes can consume the signed-in account's usage; choose them explicitly from [CLI integrations](tui/CLI-INTEGRATIONS.md) or [Claude validation](tui/CLAUDE.md#research-and-validation). A fake CLI cannot establish compatibility with a real installed version.

Live GitHub review writes have not been validated. Use an owned disposable PR to test publication, replies and resolution, then inspect the result on GitHub. The sample backend proves state transitions and payload contracts; it does not establish real permissions or network behavior.

## Boundaries

- GitHub does not expose an atomic expected-head guard for every write. LazyReview rechecks before mutation, but a final race remains. Unknown write outcomes require refresh before retrying.
- Only returned patch lines can anchor comments. Binary/omitted patches and GitHub's 3,000-file limit remain API limits.
- Claude uses private review settings and read/artifact tools; it does not inherit the launch checkout's tools/settings. Codex/Grok retain their documented provider configuration and permission controls.
- Supplied/read evidence counts show available context and successful reads, not proof that a model inspected every file. Provider probes establish no general latency guarantee.
- Physical mouse delivery depends on the host terminal. Isolated PTY/Zellij regressions do not prove every Alacritty input path. The optional local input trace excludes typed text and source.
- The browser TUI runs the real Rust UI with synthetic data and fixture AI replies. It cannot run real provider CLIs or publish reviews. Sessions are temporary and have no persistent volume; see [service isolation and limits](site/INTERACTIVE-TUI.md).
