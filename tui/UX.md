# Interaction principles

Code is the main workspace. Keep instructions in searchable help (`g ?`, then `f`); the screen shows identity, state and the next useful action. The [keymap](README.md#controls) and [review walkthrough](REVIEWING.md) define current behavior.

- **Focus determines navigation.** Tab changes review panes; `j/k` stays within the focused pane. `/` targets files or the diff. Split search covers both revisions of one file, with explicit Original/Modified scope and `n/N` match navigation.
- **Show work without inventing progress.** A spinner and elapsed time indicate PR loading. Keep review navigation responsive while API, syntax, diagrams and AI work run separately.
- **Reveal detail when asked.** The companion starts folded; one pane control restores any pane. Your review starts with folded file groups, then comment previews, then one full body. Saved drafts and posted comments remain separate.
- **Keep context beside actions.** Compact composers leave code visible. Code can be selected across files for private AI questions or a GitHub comment; those destinations stay distinct.
- **Make writes predictable.** Enter saves a line-comment draft; replies/general comments send immediately. Active completion suggestions consume the first Enter. Multiline paste stays intact, and Alt-Enter/Ctrl-n inserts a newline. Final review publication and thread-state changes have confirmation screens.
- **Preserve code coordinates.** The first pinned range becomes GitHub's native comment anchor; extra ranges become immutable code links. Search, syntax and annotation gutters never change line identity. Stale comparison checks remain in the adapter.
- **Use color with structure.** File status has a symbol as well as color. Comment counts stay with their file; annotated lines have gutters. Interface and syntax palettes are independent, and the default background follows the terminal.
- **Render content safely.** Markdown keeps tables, code, lists, tasks and link labels. Unknown diagrams retain source. HTML, terminal controls and remote assets never execute. Text stays selectable.

These choices follow GitHub's [progressive disclosure](https://primer.style/product/ui-patterns/progressive-disclosure/), [loading feedback](https://primer.style/product/ui-patterns/loading/) and [review workflow](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/reviewing-proposed-changes-in-a-pull-request). GitHub's [review comment API](https://docs.github.com/en/rest/pulls/comments#create-a-review-comment-for-a-pull-request) supplies one contiguous anchor, which explains the multi-range linking behavior.

Syntax uses Syntect, following [Yazi's background highlighting](https://github.com/sxyazi/yazi/blob/main/yazi-core/src/highlighter.rs). GitHub Dark draws scope colors from the [official GitHub theme](https://github.com/primer/github-vscode-theme/blob/main/src/theme.js); Lunar's transparent interface and soft diff fills come from the author's inspected LunarVim configuration. The [Mermaid guide](MERMAID.md) defines diagram fidelity and source fallback.

Editing/deleting posted GitHub comments, remote image rendering and external-editor integration are not implemented. [Checks and limits](../VALIDATION.md) distinguishes automated sample coverage from live validation.
