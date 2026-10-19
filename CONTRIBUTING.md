# Contributing

Read [AGENTS.md](AGENTS.md) for the source map and [VALIDATION.md](VALIDATION.md) for checks by change type. Keep changes focused, update affected keys/docs, and test observable behavior. Prefer a regression that would fail before the fix; merge duplicate coverage rather than adding assertions about source spelling or private structure.

```sh
npm test
cargo test --locked --manifest-path tui/Cargo.toml
cargo fmt --manifest-path tui/Cargo.toml -- --check
```

For AI adapters, follow [CLI integrations](tui/CLI-INTEGRATIONS.md) and update its capability table in the same change. Never commit tokens, local state or provider transcripts. Use synthetic fixtures or your own disposable PR for tests that write to GitHub.

The showcase uses plain HTML/CSS/JS; its interactive demo runs the real Rust app in isolated sample-only processes. Keep sample AI replies labelled. Regenerate native screenshots/video after changing the demonstrated interface. Fonts retain their included OFL license; vendored Mermaid retains its upstream license.
