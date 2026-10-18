# Contributing

Read [AGENTS.md](AGENTS.md) and the relevant [TUI guide](tui/README.md). Keep changes focused, document new keys, and include the smallest regression that proves the behavior.

```sh
npm test
cargo test --locked --manifest-path tui/Cargo.toml
cargo fmt --manifest-path tui/Cargo.toml -- --check
npm run site:build
npm run test:site   # installed Chrome/Chromium; CHROME_PATH overrides discovery
```

For AI adapters, follow [CLI integrations](tui/CLI-INTEGRATIONS.md) and update its capability table in the same change. Never commit tokens, local state or provider transcripts. Use synthetic fixtures or your own disposable PR for tests that write to GitHub.

The showcase is plain HTML/CSS/JS. Its sample is deliberately local and scripted; keep that visible. Update actual screenshots/video when the demonstrated interface changes. Fonts use the included OFL license; vendored Mermaid code retains its upstream license.
