# LazyReview — terminal proof of concept

A Rust/Ratatui PR reviewer with nested file-type trees, Vim navigation, unified/split diffs, multiple AI attachments, and real GitHub review operations.

See [the real-PR review walkthrough](REVIEWING.md) for comments, multi-range selection, authentication and validation. [UI research](UX.md) records the interaction decisions.

## Run

From the repository root, when the supplied binary is present:

```sh
./lazyreview
./lazyreview --demo
./lazyreview --pr https://github.com/OWNER/REPO/pull/123
```

Without arguments, the executable opens a **repository → PR chooser** using your existing GitHub login. The left pane contains repositories where you have an authored open PR, ordered by your newest authored PR or default-branch commit. Select a repository with `j/k` and Enter to see its open PRs from **all authors**, then select a PR to reach the same review screen as `--pr URL`.

**Press `/` or click Search** to open the floating history finder. It loads all your authored PRs, including merged and closed PRs, and adds GitHub's recent contribution repositories. Type a fuzzy repository query, choose with arrows / Ctrl-j/k, then Enter. The next step shows your PRs in that repository, newest-created first; fuzzy-match the title or number (for example `142 cache`). Tab switches between your PRs and all authors. Alt-Left returns to repositories; Escape closes the popup without changing the underlying chooser. Bracketed paste works in the query.

An exact `owner/repo` absent from the personal list appears as **Browse this repository**. Enter loads all of its PRs, including other people's work; this also works for public repositories without signing in. Ctrl-r refreshes the current search data. Results stay in memory for this run, so reopening Search is immediate.

`--demo` explicitly opens the synthetic PR without network access. `--pr URL` still opens a PR directly. Listing and opening PRs are read-only.

### Chooser keys

| Keys | Action |
| --- | --- |
| `h/l`, Tab / Shift-Tab | Switch repository / PR pane |
| `j/k`, arrows, `gg/G` | Select repository or PR |
| Enter | Load repository PRs / open selected PR |
| `/` or Search button | Floating fuzzy repository → PR history search |
| `f` | Filter the current PR page / search GitHub repository names |
| `1` | Repositories containing your authored open PRs |
| `2` | Open PRs requesting your review |
| `3` | Your authored open PRs |
| `4` | Focus repository search |
| `n/p` | Next / previous result page |
| `r` | Refresh list and reread GitHub login |
| `o` | Paste a PR URL directly |
| `d` | Open the demo |
| `q` | Quit |

In a review, `Ctrl-p` returns to the chooser. Escape in the chooser returns to that review and preserves its unsent text; selecting a different PR starts a new review workspace. If authentication is missing, the chooser explains how to run `gh auth login` and stays open; public repositories and direct public URLs can still be used.

Repository discovery paginates the signed-in user's open PRs via GraphQL and reads author-filtered [commit history](https://docs.github.com/en/graphql/reference/commits#commit) in batches. Recency means the later of an authored PR's creation date and the viewer's newest default-branch commit; unknown contribution dates sort last. Commit counts remain secondary metadata.

The normal open-PR list uses GitHub's [search API](https://docs.github.com/en/rest/search/search#search-issues-and-pull-requests), 50 results per page. It explicitly reports partial search results and the 1,000-result ceiling; narrow by repository when needed. Scope qualifiers follow [GitHub's search documentation](https://docs.github.com/en/search-github/searching-on-github/searching-issues-and-pull-requests).

The floating history finder has **no 1,000-result search ceiling**: it follows every [user pull-request and contributed-repository connection](https://docs.github.com/en/graphql/reference/users#user) cursor, deduplicates PRs, and filters locally. All-author repository browsing follows every [List pull requests](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests) page. GitHub describes `repositoriesContributedTo` as *recent* contributions; full authored PR history supplements older repositories, but old commit-only repositories outside that connection may require entering `owner/repo`. Private repository visibility follows the token's access. Failed contribution/date lookups leave a visible warning; authored PR history failures are errors rather than silently incomplete results.

To build from source (Rust 1.88+; validated with 1.93.1, plus a C compiler for the bundled Oniguruma parser; Xcode Command Line Tools on macOS):

```sh
cargo build --release --locked --manifest-path tui/Cargo.toml
./tui/target/release/lazyreview
```

**Runtime:** Node.js 22.18+ must be on PATH. For private repositories and review actions run `gh auth login`, or provide `GH_TOKEN` / `GITHUB_TOKEN` in your shell. The token must have access to the repository and relevant Contents, Pull requests, and Issues permissions. No token is stored by this program. The optional companion uses your installed Claude Code, Codex or Grok and its existing login. Press `g a` in a PR to connect, or run `./lazyreview --setup-ai` once. See [Claude setup, context and session behavior](CLAUDE.md).

```sh
./lazyreview --check https://github.com/primer/react/pull/8520
./lazyreview --snapshot
```

`--check` is read-only. `--snapshot` renders the sample screen as plain text without changing terminal state.

## Companion and PR reading layouts

`g v` opens the layout picker: **Ledger** is dense, **Paper** is the default reading column,
and **Studio** uses filled message headers and section bands. Selection applies to both
Claude and the PR description, keeps the chat intact, and lasts for this run.
Set `gui.companionStyle: paper` in your config for a persistent preference. This is the reader style; `ai.interface` independently chooses the Paper reader or the embedded Claude Code terminal.

```sh
./lazyreview --preview-ui ai --companion-style paper
./lazyreview --preview-ui description --companion-style studio
```

These previews are synthetic and offline. In a live review, `g a` opens Claude;
`i` composes inside that pane, Enter sends, Ctrl-n inserts a newline, and Esc retains
the draft (`e` reopens it). `j/k` scrolls without being pulled back by incoming tokens;
`G` returns to the latest answer and resumes following. Status and current code context
stay visible; attachment selection/removal still uses `[a`, `]a`, and `d`.

`D` opens the **PR timeline**, a single scrollable document with the description at the top, then historical discussion comments, submitted reviews and commits ordered by timestamp. Each comment has an author/date border and rendered Markdown; commits use compact short-SHA/subject rows. `C` jumps to the first comment or review; `g m` jumps to the first commit. `o` indexes description headings and timeline entries; `h/l` or `[[`/`]]` jumps between them. `j/k` and Ctrl-d/u scroll, `gg/G` go to the ends, and `Z` maximizes the pane. `i` adds a general PR comment.

`b` opens inline review threads using the same comment cards. `j/k` selects threads, Ctrl-d/u scrolls their replies, `i` replies, and `R` opens resolve/reopen confirmation. `h/l` returns to the main timeline. Reader text is selectable in both surfaces. Paper limits the reading column to 84 cells when maximized; scrollbar and percentage reflect wrapped rows. Timestamps are UTC; missing dates remain visible at the end. Markdown/HTML tables adapt to narrow cards, and `<details>` is expanded. Images are not fetched and scripts do not run.

Fenced code has a labelled box and uses the selected GitHub Dark/Lunar syntax colors. Text appears immediately; a bounded background worker applies colors without blocking input. Tabs use four-cell stops and long code lines wrap without dropping indentation or spaces. `suggestion` fences show literal replacement text in a **Suggested change** box; they do not apply changes. Explicit `markdown` / `md` fences render a labelled **Markdown preview**, including nested suggestions, up to two preview levels; deeper or over-32-KB previews remain source. Other code languages keep literal nested fences. Unknown, over-16-KB, over-160-line, or over-2,048-byte-line blocks stay plain. Up to 32 blocks are colored in one rendered view; additional blocks remain readable without repeated background work.

[Compare all layouts](../companion-options.html). Regenerate this standalone gallery
from actual Ratatui cells with `./lazyreview --theme blue --ui-gallery companion-options.html`.
It is a visual fixture, not a live web client.

## Themes

The default `lazygit` theme reads your existing Lazygit `gui.theme` from `$XDG_CONFIG_HOME/lazygit/config.yml` (or `~/.config/lazygit/config.yml`), then the standard macOS Application Support paths. Its background follows your terminal. Your Lazygit file is never changed.

Press `T` to cycle Lazygit → original blue → custom for this session, or select one at launch:

```sh
./lazyreview --theme blue
./lazyreview --theme lazygit
```

Existing settings and native sessions are retained through the rename; see [rename compatibility](../README.md#rename-compatibility).

Persistent preferences live in `$XDG_CONFIG_HOME/lazyreview/config.yml` (default `~/.config/lazyreview/config.yml`). See [config.example.yml](config.example.yml) and the saved [blue preset](themes/blue.yml). Configuration uses Lazygit's [color lists and modifiers](https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md#color-attributes):

```yaml
gui:
  themePreset: lazygit # lazygit, blue, custom
  theme:              # optional overrides, same Lazygit keys
    activeBorderColor: ['#a184df', bold]
    selectedLineBgColor: ['#2d3f76']
```

Supported theme fields: `activeBorderColor`, `inactiveBorderColor`, `searchingActiveBorderColor`, `optionsTextColor`, `selectedLineBgColor`, `selectedLineFgColor`, `unstagedChangesColor`, `defaultFgColor`. Named colors, `#rrggbb`, `default`, and text modifiers are supported. Unrelated Lazygit settings are ignored. The `blue` preset preserves the original fixed background and palette; custom overrides apply to the `lazygit` and `custom` presets.

## Comments beside the code

File names show compact **● GitHub comment** and **◇ queued AI note** counts. GitHub counts include your own drafts, submitted comments and replies; bare code attachments and pinned selections do not count. The same colors mark annotated ranges beside the diff with `╭ │ ╰` (a short `╶` for one line), in both inline and side-by-side layouts. Overlapping GitHub/AI ranges retain separate gutters. `=` includes the badges when fitting the tree.

AI markers disappear when their note is consumed or removed. Submitted GitHub comments keep their markers. Outdated comments remain in the file count but never point at newer code; only ranges present in the current snapshot are marked. Renamed/deleted files retain their correct original or modified side. Whole-file AI notes count without inventing a line range.

## Syntax highlighting

The diff uses Syntect with the Oniguruma backend, [as Yazi does](https://github.com/sxyazi/yazi/blob/main/Cargo.toml), and bundled Two Face grammars (including TSX). GitHub Dark is the default syntax palette; `H` switches to Lunar and back. `--syntax-theme github-dark|lunar` overrides `gui.syntaxTheme` in the same configuration file.

Syntax colors do not set the terminal background. Lunar follows the installed LunarVim configuration: transparent window, green strings, orange numbers, purple keywords, blue functions and cyan types; its added/deleted diff backgrounds are `#193824` and `#2a1717`. Existing Lazygit interface colors still apply separately through `T` / `--theme`.

A dedicated worker reads the original/modified sources directly at the already captured merge-base/head, including renamed paths and fork repositories. A modified file needs two parallel Contents requests; syntax reads do not reload the PR, changed-file pages, or comparison. Added/deleted files need only their existing side. The guarded review/write adapter is unchanged.

Parsing starts at the beginning of each side to retain multiline context and stops after its last line represented in the diff. Trailing code cannot delay the displayed colors. Only the currently requested file is queued; results from previous files cannot repaint the current file. Up to four source pairs and eight rendered palette entries are cached in memory, retaining recently used entries. A cached palette/file return requires no network or parsing. No source is written to disk. Search and pinned-range backgrounds overlay syntax without changing GitHub line coordinates.

Each side falls back to plain text for unavailable content, unknown grammars, binary files, files over 200 KB, lines over 5,000 bytes, or more than 40,000 syntax tokens. The diff remains usable; partial syntax is labelled beside the file statistics. Refresh (`f`) retries unavailable syntax sources. Full-file reads require GitHub Contents access. Demo mode uses complete synthetic sources and makes no network requests.

## AI companion

Native Claude, Codex and Grok CLIs are supported. Use `--ai-provider codex` or `--ai-provider grok` for terminal mode; see [provider setup and integration contract](CLI-INTEGRATIONS.md).

One persistent Claude session per PR/profile, with read-only repository tools and local artifacts. Paper is the default: styled replies, a docked composer and background preparation. Reopening a PR restores its conversation; new commits update the context.

Set `ai.interface: terminal`, or run `./lazyreview --ai-interface terminal --pr URL`, to embed the actual Claude Code CLI in the right pane. It uses your existing login and exact stored session ID, with a readable PR/profile title. **Ctrl-]** leaves input and focuses the previous visible pane; **Ctrl-\** keeps companion-local navigation; **i** or **Enter** returns to Claude typing. While typing, shortcuts go to Claude. In LazyReview navigation, `N` starts a new chat, `X` interrupts and `R` resumes/restarts the CLI. Queued code/notes are attached through public prompt/completion hooks, without parsing private transcripts. Paper's budget and timeout controls do not apply to the interactive CLI. [Configuration, controls and boundaries](CLAUDE.md).

## Controls

Press `?` or `g` then `?` for the in-app reference. Press `f`, type words from a key or action, then Enter to filter the list. One Escape closes help, including while filtering. Type headings are always expanded; directories still fold.

| Keys | Action |
| --- | --- |
| Tab / Shift-Tab | Next / previous visible pane |
| `+` / `-`, drag vertical borders | Grow / shrink the focused pane |
| `\`, then `j/k` and Enter | Fold / unfold any pane; companion starts closed |
| `Z`, `=` | Maximize focused pane / fit file names and reading width (keeps folded state) |
| `T` / `H` | Cycle interface themes / GitHub Dark and Lunar syntax palettes |
| `h/l` | Recursively collapse / expand subtrees; scroll code horizontally |
| `j/k`, `gg/G` | Move within the focused pane |
| `Ctrl-d/u` | Half-page through diff or right panel |
| `n/N` in Files | Next / previous file; skip folders, reveal folded destinations, respect filters |
| `J/K`, `]f/[f` | Next / previous file |
| `]c/[c` | Next / previous hunk |
| Enter in tree | Expand a directory or open a file |
| `1`–`5`, `0` | Code / Tests / Docs / Config / Assets; all types |
| `/` in Files / Changes | Filter paths / search code; Tab chooses Both, Original or Modified |
| `n/N` in Changes | Next/previous search match; without a search, next/previous file |
| `o`, `u` | Cycle file sort / unviewed only |
| `x`, `z` | Hide file / show hidden files |
| `s`, Left/Right | Inline or side-by-side diff / horizontal code scroll |
| `v`, then `j/k`, Space | Select and pin a range; repeat across hunks/files |
| `a`, `A` | Queue selected code/diff / whole file for the next Claude answer |
| `t` in Changes / AI | Add a temporary note to the selected code / queued item |
| `g c` | Inspect queued code/notes; Enter edits, d removes, i asks |
| Drag text in AI, timeline or threads | Select and copy on release, including while composing a Paper question |
| `v`, `h/j/k/l`, `y` in a reader | Select text, extend selection, copy; Escape cancels |
| `]a/[a`, `d` in AI panel | Choose / remove an attachment |
| `m` | Toggle GitHub viewed state |
| `c` | Compose on pinned ranges (first anchor, other ranges linked) |
| `r`, then Enter or `h/l` | Inspect folded Drafts; one full comment body opens at a time |
| `P` in Your review, or click its tabs | Switch Drafts / Posted |
| `S` or click Review changes / Submit | Open the review sheet; Tab / Shift-Tab or click selects outcome |
| `D` / `C` / `g m` / `b` | Timeline top / first comment / first commit / inline threads |
| `p` | Cycle AI / threads / PR timeline / your review |
| `i` | Compose AI question, thread reply, or general comment |
| `g a` | Open / set up Claude Code companion |
| `F` in AI | Whole PR (default) / focused-file context |
| `o`, `[[ / ]]` in AI | Conversation index / previous-next question |
| `g o`, then Enter | Saved artifacts / open selected diagram, page or notes |
| `N / X / R` in AI | New chat / stop / retry; in terminal mode, first use Ctrl-\ for companion-local navigation |
| Ctrl-] in native Claude | Leave input and focus previous pane; Ctrl-\ keeps companion controls |
| `R` in threads | Resolve / reopen with confirmation |
| Enter in a composer | Save line-comment draft / send reply, general comment or AI question; preview final review |
| Alt-Enter / Ctrl-n | Insert a newline; pasted newlines are preserved |
| `@` / `#`, then Tab or Enter | Insert a person / issue or PR suggestion |
| Enter / Escape in review preview | Publish review / return to composer |
| `Ctrl-p`, `O`, `f` | PR chooser / paste PR URL / refresh current PR |
| `e`, Escape | Reopen cancelled composer / leave mode |
| `q` | Quit; in a composer it remains ordinary text |

Enter saves line comments to a pending GitHub review, and immediately sends replies/general comments. Final review publication and thread resolution have confirmation screens. Marking a file viewed is immediate. The compact confirmation shows the captured PR, outcome, summary, and draft/file counts; full comment bodies remain in Your review. Summary and outcome survive Back or closing/reopening the sheet. Your own PR offers Comment only. The adapter rechecks the snapshot before writing; uncertain write outcomes require a refresh before retrying.

## Implementation

Rust owns rendering, input, pane state, selection and confirmation. Ratatui supplies layout/widgets; Crossterm supplies terminal events/restoration. API work runs in a worker thread; suggestions use a separate, lazily started worker; syntax fetching/parsing has its own worker. Neither blocks rendering or keyboard input.

The executable embeds `lib/github.ts`, `lib/review.ts`, `lib/sample.ts`, `src/discovery.mjs`, `src/repositories.mjs`, `src/completion.mjs`, `src/comment.mjs`, `src/syntax.mjs`, and `src/bridge.mjs`. The AI integration separately embeds `src/ai-session.mjs`, `src/ai-context.mjs`, `src/ai-tools.mjs`, `src/artifacts.mjs`, `src/ai-terminal.mjs`, `src/ai-native-hook.mjs`, `src/ai-providers.mjs` and `src/ai-codex.mjs`. On launch it writes these **source files only** to a temporary directory and starts a private Node stdio child process. Requests and credentials are never saved there. There is no TCP listener and no hosted web app dependency. The process and directory are cleaned up on normal exit.

Interactive CLI mode additionally runs in an owned PTY with VT terminal emulation and resize forwarding. CommonMark content and HTML reports are parsed with pulldown-cmark and html5ever through Scraper; the renderer never executes HTML or fetches assets.

This deliberately shares the web adapter's pagination, immutable content refs, diff-range validation and pending-review checks. It is **not a completely native Rust networking stack**. That would remove Node but duplicate code in this proof of concept.

Rust was selected for Ratatui's layouts and widgets; C/ncurses is viable and was not ruled out for a later version. References: [Ratatui](https://docs.rs/ratatui/0.30.2/ratatui/), [Crossterm](https://docs.rs/crossterm/0.29.0/crossterm/), [ncurses](https://invisible-island.net/ncurses/ncurses.html).

## Boundaries

This is a usable proof of concept, not a full Vim emulator. Composers support cursor movement, Backspace/Delete, clear, multiline input/paste, and mention/reference suggestions; external-editor integration is not implemented. Filters, hidden files and unsent text are session-local. Claude chats persist per PR/profile through native CLI session IDs; saved diagrams, pages and notes stay with that association across new chats and revisions. GitHub stores saved pending reviews, submitted comments, and viewed state. Missing patches and unsupported files are disclosed.

`=` fits the file tree to its longest visible or collapsed descendant entry (indentation, status and line counts included), and targets an 84-cell reading column on the right. Narrow terminals reserve room for Changes. It preserves folding and zoom. Each open pane has a clickable `[−]`; collapsed panes have labelled `[+ Files]`, `[+ Claude]`, or equivalent restore buttons. `g ?` / `g !`, then `f`, searches shortcuts; one Escape closes the overlay.

Pane widths and folded state are session-local and survive switching PRs. Folded panes are skipped by Tab. On very narrow terminals only the focused pane is shown; Tab / Shift-Tab still navigates the visible pane set. A 150-column terminal is comfortable. Terminal fonts come from your terminal profile.

Sample AI is explicitly prewritten. Live TUI AI uses Claude Code, Codex or Grok native sessions with PR read tools and local artifact creation; see provider-specific permission boundaries in [CLI integrations](CLI-INTEGRATIONS.md). Paper streaming and exact CLI session resumption were verified on a public PR; interactive-terminal validation is tracked separately. GitHub writes remain untested live. See [validation](../VALIDATION.md) and [local AI storage/context boundaries](CLAUDE.md).

## Checks

```sh
cargo test --locked --manifest-path tui/Cargo.toml
python3 tests/tui-pty.py
python3 tests/tui-controls-pty.py
python3 tests/tui-search-pty.py
python3 tests/tui-reading-pty.py
node --test tests/*.test.mjs tests/*.test.ts
```

The PTY smoke requires a built release binary and runs against sample data in an isolated pseudo-terminal. It never attaches to or restarts existing terminals or Zellij sessions.

Offline floating-search regression: `python3 tests/tui-picker-search-pty.py` (set `LAZYREVIEW_BINARY` for a nondefault build).

Optional live chooser smoke: `python3 tests/tui-picker-pty.py --live`. This reads your list and opens the selected PR; it never submits reviews, marks files viewed, or logs PR contents.

Repeatable syntax measurements (the two live probes only read GitHub):

```sh
cargo test --release --locked --manifest-path tui/Cargo.toml benchmark_syntax_latency -- --ignored --nocapture
node tests/syntax-performance.mjs --live --pr https://github.com/primer/react/pull/8520
python3 tests/tui-syntax-performance.py --live --pr https://github.com/primer/react/pull/8520
```

The parser benchmark compares full 80 KB TSX/TypeScript/Rust files and a diff ending at line 40. The API probe reports three source-read samples and verifies immutable line contents. The PTY probe measures first diff frame to applied syntax, then a cached palette return; `--binary PATH` selects another build. Network timings vary. Regression tests enforce two parallel reads and bounded parsing without relying on machine-specific timing thresholds. See [the validation record](../VALIDATION.md) for before/after observations.

## Silent demo

[Watch the offline interactive tour](../demo/lazyreview-demo.html) or
[play the silent MP4](../demo/lazyreview-demo.mp4). Captions, key presses and
pane zoom demonstrate actual native renders with synthetic data and scripted replies.
See [rebuild instructions](../demo/README.md).

Mermaid fences in descriptions, comments, threads and Paper replies use the same offline Unicode renderer. [Supported subset and rebuild guide](MERMAID.md).
