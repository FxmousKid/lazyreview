# LazyReview

Start with [README.md](README.md). Commands and required checks by change type live in [VALIDATION.md](VALIDATION.md); contribution rules live in [CONTRIBUTING.md](CONTRIBUTING.md).

| Change | Source / canonical guide |
| --- | --- |
| Native UI, keys, navigation | `tui/src/`; [keymap](tui/README.md), [review workflow](tui/REVIEWING.md) |
| GitHub operations | `lib/github.ts`, `lib/review.ts`, `tui/src/bridge.mjs`; [review workflow](tui/REVIEWING.md) |
| AI providers and sessions | `tui/src/ai-*.mjs`; [integration contract](tui/CLI-INTEGRATIONS.md), [Claude context/storage](tui/CLAUDE.md) |
| Hosted interactive TUI | `scripts/tui-sessions.mjs`, `tui/src/browser_demo.rs`, `site/terminal/`; [service contract](site/INTERACTIVE-TUI.md) |
| Recorded tour / pictures | `demo/`, `site/render-stills.py`; [rebuild](demo/README.md), [provenance](site/ASSETS.md) |

AI adapter changes update the capability table, setup/trust steps and tests together. Use public CLI protocols; never parse provider-private transcripts. Exercise launcher → chooser → PR → click → type as well as direct launches. Preserve unrelated CLI and terminal sessions.

Keep the browser TUI sample-only: no visitor tokens, live PRs, real providers or arbitrary commands. Regenerate recorded HTML, MP4 and native pictures together when their interface changes; use the shared key-event timeline. Do not replace native browser rendering with a second implementation of the UI.

Document durable behavior in the linked guide. Avoid duplicate runbooks, passing test counts and dated status reports. Live provider inference and GitHub writes require explicit authorization; default checks stay offline.
