# Interactive TUI demo

`/lazyreview/demo/` runs the **real Rust App, key handlers, mouse handlers and Ratatui renderer**. The browser paints its terminal cells on a fixed grid with the bundled local font files. It does not rebuild the interface with HTML widgets.

```sh
cargo build --release --locked --manifest-path tui/Cargo.toml
npm run site:dev
# http://127.0.0.1:4179/lazyreview/demo/
```

The browser batches keyboard, paste, mouse and resize events to `POST /lazyreview/demo/api`. Each visitor gets a random session ID and a private `lazyreview --browser-demo` process. Replies contain changed terminal cells and public sample UI state; idle polling picks up syntax/Markdown updates. Reset and leaving the page close the session. Keyboard capture is confined to the terminal; `Ctrl-]` releases it to the page controls.

The native adapter calls `App::key`, `App::paste`, `App::mouse`, `App::receive` and `ui::render`. Its bridge is **sample-only**: GitHub discovery/loading, provider calls and live syntax reads are rejected before token discovery. Drafts, review submission, viewed state and conversations mutate only that visitor's in-memory sample. AI replies are labelled fixtures. No arbitrary command, filename, bridge operation or PR URL is accepted from the browser.

The server starts the binary directly, with an explicit Node-only PATH and a fresh temporary HOME/config/cache. User tokens, CLI logins, SSH settings and local repository files are not inherited. The process group is private and reaped on close; temporary files are removed on exit. Runtime defaults: 8 sessions total, 30 creations/minute, 100 input requests/second/session, 32 KiB requests, 64 events/batch, a 40–220 × 16–70 cell grid, 5-minute idle expiry and 30-minute lifetime. No prompt text is logged or persisted.

Hosting requires the Node/native service, not a static upload. `LAZYREVIEW_DEMO_BINARY` can select an explicitly built executable during development; `HOST`, `PORT` and `LAZYREVIEW_SITE_ROOT` control the HTTP service. No account or API key is required.

Validation:

```sh
cargo test --locked --manifest-path tui/Cargo.toml
npm test
npm run test:demo-server
npm run test:site
```

The browser check uses real dispatched keys and mouse clicks for file navigation/filtering, split diffs, notes from two files, questions, draft comments, review/help screens, keyboard release and resizing. The service check verifies session isolation, request/origin limits, expiry and cleanup. The recorded tour remains at `/demo/tour/`; the earlier web UI preview remains at `/demo/web/`.
