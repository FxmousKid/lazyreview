# Terminal Mermaid rendering

LazyReview renders common Mermaid flowcharts, state diagrams and sequence diagrams as selectable Unicode text in the shared Markdown reader. The initial frame keeps source while a background job runs; completed diagrams replace it automatically. Narrow or unsupported diagrams retain their source. This is a terminal approximation, not full Mermaid compatibility.

## Choice and research

Checked 2026-10-18 against primary upstream sources and published packages:

| Library | Output / license | Decision |
| --- | --- | --- |
| [beautiful-mermaid 1.1.3](https://github.com/lukilabs/beautiful-mermaid) | Unicode/ASCII and SVG; MIT | Selected its synchronous text entry point. Bundles to 146,761 bytes with no external imports. Node already ships as a LazyReview requirement. |
| [merman / merman-ascii](https://github.com/Latias94/merman) | Native Rust terminal rendering; MIT OR Apache-2.0 | Promising width-aware API, but published stable 0.7.0 and current 0.9.1 require Rust 1.95. Our declared minimum is 1.88; installed compiler was 1.93.1. |
| [mermansi 0.1.6](https://github.com/kingsword09/mermansi) | Native Rust Unicode/ASCII wrapper; MIT OR Apache-2.0 | Also requires Rust 1.95. Avoided changing the application's toolchain for this feature. |
| [mermaid-rs-renderer 0.3.1](https://github.com/1jehuang/mermaid-rs-renderer) | Native Rust SVG/PNG; MIT | Useful for future graphical artifacts; does not directly solve terminal text. Would need an image protocol or another conversion layer. |
| [mermaid-ascii](https://github.com/AlexanderGrooff/mermaid-ascii) | Go terminal renderer; MIT | Foundation of beautiful-mermaid's text engine. A separate installed Go executable adds packaging work. |

No browser, CDN, Mermaid CLI, runtime package installation or handwritten diagram parser is involved. The pinned upstream package owns parsing and layout.

## Supported boundary

- Multiline `graph` / `flowchart` with TD, TB, LR or BT; `stateDiagram` / `stateDiagram-v2`; `sequenceDiagram`.
- Source fallback for other families, RL, initialization/frontmatter directives, click/callback actions, newer `@{...}` shapes, explicit activation commands and autonumbering. These are unsupported or lossy upstream paths.
- Class and ER paths are deliberately disabled: local conformance samples exposed simplified class signatures and clipped ER relationship labels. Wide/combining label characters also retain source because the upstream layout counts JavaScript cells.
- Terminal output does not reproduce arbitrary CSS. The supported subset can differ from GitHub's renderer; source remains available on GitHub and is retained automatically when a boundary is detected.
- A diagram must fit the reading column. Its box art is never blindly wrapped, truncated or relaid out with different edge semantics.

## Runtime contract

`tui/src/mermaid.rs` exposes `request(source, width) -> Pending | Ready(Arc<Vec<String>>) | Fallback(reason)`. Reader caches include `revision()`. Call `shutdown()` during teardown to kill/reap our child and join our worker.

One background worker, eight queued requests, 32 LRU entries. Requests are limited to 8 KiB/128 source lines, output to 64 KiB/256 rows/240 columns. Each uncached job has a two-second deadline and a 64 MiB V8 old-space limit; timeout kills and reaps that child. This is a heap setting, not a hard process RSS limit. Cached redraws spawn nothing. Worker input uses an anonymous private file; bundled code uses a temporary private directory. No source is put in argv, logs or a persistent cache. The child receives no inherited credentials or Node preload options. The bundled renderer contains no network or dynamic-code API calls; this is not an OS network sandbox.

## Verification

```sh
node --test tests/mermaid.test.mjs
cargo test --locked --manifest-path tui/Cargo.toml mermaid::tests
cargo test --locked --manifest-path tui/Cargo.toml mermaid::tests::benchmark_cache_latency -- --ignored --nocapture
```

The tests check supported labels/geometry, narrow-column fallback, unsupported forms and source/output limits. The ignored benchmark reports local cold and cached latency; it is not a portable timing gate or a claim of full Mermaid parity.

## Vendored dependency

Normal builds embed `vendor/beautiful-mermaid-1.1.3.mjs` and its adjacent MIT license; they do not install npm dependencies. The bundle was generated from the published package's `src/ascii/index.ts` using esbuild 0.25.12. The repository currently ships the bundle, without its generation tooling.

For an upgrade, reproduce the text-only bundle in a scratch directory from pinned package/bundler versions. Check the package hash before execution, reject external runtime imports and dynamic code/network APIs, retain the upstream license, and run both check suites above. Add the reproducible build command/tooling in the same change as any replacement bundle.

Current provenance:

- Package tarball SHA-256: `4d436f27bb0102985940c3efe317153f4d1f53f20f029f226b60e1ab4b6b9ff0`.
- Checked-in bundle SHA-256: `3d6fe46162020cb820d473d20f5d8bc3c3405b09ff2118cd7504117e5b2126ee`.

Keep bundle, license, tests and this document together when upgrading.
