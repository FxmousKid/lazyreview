# LazyReview

The native app is `tui/`; `tui/README.md` and `tui/REVIEWING.md` document use and GitHub operations.

AI CLI integration changes must follow `tui/CLI-INTEGRATIONS.md`: keep its capability table, config examples, trust/setup steps and validation evidence current in the same change. The registry/table contract is tested. Use public CLI protocols; do not parse provider-private transcripts. Validate the actual launcher → chooser → PR → click → type path as well as direct PR launches. Preserve other running CLI and terminal sessions.

Demo changes regenerate `demo/lazyreview-demo.html` and `demo/lazyreview-demo.mp4` using `demo/README.md`; keep both driven by the same key-event timeline.
