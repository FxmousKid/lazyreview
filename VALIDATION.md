# Validation

The automated checks cover diff parsing, immutable GitHub reads, stale comparison guards, review publication contracts, provider sessions/hooks, note consumption, Markdown, syntax, pane layout and input ownership.

```sh
npm test
cargo test --locked --manifest-path tui/Cargo.toml
cargo build --release --locked --manifest-path tui/Cargo.toml
LAZYREVIEW_BINARY="$PWD/tui/target/release/lazyreview" python3 tests/tui-native-entry-pty.py
LAZYREVIEW_BINARY="$PWD/tui/target/release/lazyreview" python3 tests/tui-review-flow-pty.py
```

Terminal workflows use synthetic data and isolated configuration/state. Real Claude tests verified copy-to-type, trust-menu No/exit and native resume; read-only GitHub probes verified public PR loading. Live GitHub review writes require a disposable PR and have not been exercised. The browser simulator is a separate, explicitly scripted sample.

## October 19, 2026

- 130 Rust tests passed; 9 manual rendering/benchmark tests were explicitly ignored. Locked release build and formatting passed.
- 124 JavaScript/API contract tests passed.
- Offline PTY replays passed: launcher → chooser → CLI override/setup, click-to-type, copy-to-type, fold/unfold, direct Tab return, trust-menu activation, and draft → inspection → review outcome → exactly one submission.
- Real Chrome passed at 1440, 768 and 390 px: file filters, multi-file private notes, context consumption, escaping, pane return, folded drafts, mobile controls, reduced motion and JavaScript-disabled fallback. No external requests.

## Boundaries

- GitHub public APIs do not provide an atomic expected-head guard for every write. LazyReview rechecks immediately before mutation, but a final race remains.
- Only returned patch lines can anchor comments. Binary/omitted patches and GitHub's 3,000-file limit remain API limits.
- Claude's review adapter uses a private state directory and review tools. It currently does not inherit the launch checkout's tools/settings. Codex/Grok retain their documented provider configuration and permission controls.
- Physical mouse delivery depends on the terminal. Native input regressions include isolated Zellij, completed copies, missing button-up, normal clicks and selection. Optional local diagnostics are documented in [CLI integrations](tui/CLI-INTEGRATIONS.md#local-input-diagnostics).
- AI-supplied/read code counts indicate available evidence, not proof that a model inspected every file. Existing paid-inference probes do not establish latency guarantees for arbitrary PRs.

Provider setup and live checks: [CLI integrations](tui/CLI-INTEGRATIONS.md). Review workflow: [Reviewing](tui/REVIEWING.md).
