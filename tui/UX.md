# TUI interaction research — October 14, 2026

The review screen should prioritize code and comments. Instructions belong in searchable help; the main view keeps only object names, state, and the next relevant action.

| Before | After | Reason and evidence |
| --- | --- | --- |
| Numbered scope instructions always visible | Repository/PR panes with focused borders; scope keys in help | Keep navigation state visible while moving optional commands into disclosure. [Primer: progressive disclosure](https://primer.style/product/ui-patterns/progressive-disclosure/) |
| Static text while loading a PR | A small animated spinner, moving track, and elapsed time; preserve the current screen | Show continuing activity without claiming a percentage or replacing the entire view. [Primer: loading](https://primer.style/product/ui-patterns/loading/) |
| AI pane consumes space by default | Companion starts folded; one pane-control surface handles every pane | Favor the core review task and reveal supporting tools on demand. [Primer: view patterns](https://primer.style/product/scenario-patterns/view/) |
| Nearly full-screen comment editor | Compact context-labelled composer, predictable Enter, explicit newline keys | Retain the code around the comment. Enter accepts an active suggestion first, preventing accidental submission. |
| Raw Markdown comments | Styled headings, emphasis, code, lists, tasks, tables and links | GitHub comments are Markdown; preserve their structure. [GitHub: formatting](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax) |
| No mention/reference discovery | Debounced, repository-scoped `@` / `#` suggestions | GitHub suggests participants/collaborators and issue/PR references. Suggestions run in a separate worker and cannot delay review writes. [GitHub: mentionable users](https://docs.github.com/en/graphql/reference/repos) |
| One temporary selection | Pin multiple ranges, retain one native anchor, link the others | GitHub exposes one `path`, `side`, `startLine`, and `line` per thread, not arbitrary disjoint anchors. [GitHub: review comment API](https://docs.github.com/en/rest/pulls/comments#create-a-review-comment-for-a-pull-request) |
| Submit flow is hard to discover | Your review collects drafts and posted comments; a final preview publishes the pending review | Distinguish saving individual observations from publishing the review. [GitHub: reviewing changes](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/reviewing-proposed-changes-in-a-pull-request) |

Navigation and folding are immediate. Animation is limited to asynchronous work. File status uses both a symbol and color; diff statistics remain visible even when the filename is truncated. Existing textual keys stay available in `g ?`, filtered with `f`.

Enter submission differs from GitHub's text-area shortcut. Multi-line pastes are bracketed, and Alt-Enter/Ctrl-n inserts newlines. Markdown never executes HTML, terminal control sequences, or remote images.

## What was checked

See [the validation record](../VALIDATION.md) for exact runs and limitations. Coverage includes an 85-file fixture, renamed/deleted-side links, separate hunks, pending/public comments, publication/replies/resolution, stale heads/bases, uncertain writes, Unicode editing, autocomplete races, small terminals, mouse resizing, and terminal restoration.

The current "Your review" view is an inspection and publication surface. Editing/deleting existing GitHub comments and rendering remote images are not implemented. Those would require separate interactions and tests, rather than hidden shortcuts or unsupported mock controls.

## Disclosure, search and review actions

| Before | After | Why |
| --- | --- | --- |
| Every comment body is visible in Your review | Collapsed file groups and individually expandable comments | Reveal details deliberately; distinguish file headers with backgrounds and dividers. |
| `/` always filters paths | Pane-local file filtering or incremental diff search | Keep the target tied to keyboard focus; preserve the code around matches. |
| Split diff search has ambiguous scope | Both versions by default; Tab selects Original/Modified; n/N wraps through matches | The two columns are revisions of one file. Shared context counts once. Matching only changes would hide useful context. |
| A flat line exposes every counter | Rounded viewed badge and Review changes action; comments badge only when relevant | Follow GitHub's review progress and explicit Review changes entry point; keep attachments inside the companion and pinned selection counts inside Changes. |

The installed LunarVim configuration was inspected: `lunar`, `incsearch`, 24-bit color, Tree-sitter highlighting, and custom soft diff backgrounds. Search uses literal smart-case matching and keeps byte/line coordinates intact. A failed search leaves the code visible. A final review still needs its preview confirmation before publication.

Syntax comparisons live in the separate `lazyreview-syntax-lab` project. The selected Syntect treatment is now integrated in the main executable; Tree-sitter comparisons remain isolated. [Yazi's implementation](https://github.com/sxyazi/yazi/blob/main/yazi-core/src/highlighter.rs) uses Syntect; its [themes use tmTheme files](https://yazi-rs.github.io/docs/configuration/theme/#syntect_theme). [Tree-sitter's highlighting library](https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html) provides captures mapped to colors, not an automatic Neovim theme loader. The lab compares both existing engines and two diff treatments using Lunar-inspired colors.

## Compact controls and PR reading

| Before | After | Why |
| --- | --- | --- |
| Three-row borders with a shared blue-looking fill | One-row, content-sized fills with separate rounded caps and untouched gaps | Save three screen rows, prevent backgrounds merging and retain independent mouse targets. |
| Product name, repository, live label and input mode repeated above/below code | PR title above code; repository/number bottom left; shortcuts bottom right | Keep identity visible without repeating product or transport details. Demo still has a small sample label. |
| Description and discussion hidden in a long companion cycle | D description, C discussion, b review threads; h/l moves among those three | Reuse existing Markdown/API data and retain j/k entry navigation and Ctrl-d/u scrolling. |
| Lunar lab paints the whole terminal blue | Token-only Syntect palette, transparent interface and the user's actual diff fills | `lua/options.lua` sets `lvim.transparent_window = true`; `config.lua` overrides DiffAdd/Delete separately. |

[Yazi's highlighter](https://github.com/sxyazi/yazi/blob/main/yazi-core/src/highlighter.rs) parses from the beginning, runs outside rendering, uses Syntect scopes, and deliberately leaves syntax backgrounds unset. The main TUI follows those principles. GitHub Dark is the chosen default; Lunar is available via H. Both diff sides use independent complete-source parser state, and matching source text is required before applying cached tokens to a patch line.

GitHub Dark uses the keyword/storage, string and punctuation distinctions from the [official GitHub theme](https://github.com/primer/github-vscode-theme/blob/main/src/theme.js), translated into Syntect scope rules.
