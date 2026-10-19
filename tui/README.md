# Native TUI

LazyReview uses Rust/Ratatui for the terminal UI and a bundled Node adapter for GitHub. [Reviewing](REVIEWING.md) covers the complete workflow; [CLI integrations](CLI-INTEGRATIONS.md) covers provider setup and permissions.

## Run

From the repository root (Rust 1.88+, a C compiler and Node.js 22.18+):

```sh
cargo build --release --locked --manifest-path tui/Cargo.toml
./lazyreview
./lazyreview --demo
./lazyreview --pr https://github.com/OWNER/REPO/pull/123
```

The launcher uses `bin/lazyreview` when installed, otherwise `tui/target/release/lazyreview`. Authenticate with `gh auth login` or an existing `GH_TOKEN` / `GITHUB_TOKEN`. Listing and opening PRs are read-only. `--demo` uses synthetic data with no network; `--check PR_URL` is a read-only integration smoke; `--snapshot` prints the sample without taking over the terminal.

Without arguments, choose a repository and then a PR. Repositories containing your authored open PRs are sorted by the latest authored PR/default-branch commit; their PR pane includes all authors. `h/l` changes panes, `j/k` moves within one, and Enter opens the selection.

Press `/` or click Search for the floating history finder. Fuzzy-search your repositories, select one, then search your authored PRs by title/number (all states, newest-created first). Tab switches to all authors. An exact `owner/repo` opens repositories outside your history, including public repositories without login. Ctrl-r refreshes; Escape closes the popup. Arrows / Ctrl-j/k select, Alt-Left returns to repositories, and bracketed paste fills the query.

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

## Appearance

The default `lazygit` interface theme reads `gui.theme` from your Lazygit config, without changing it. Its background follows the terminal. `T` cycles Lazygit → original blue → custom; `--theme lazygit|blue|custom` selects one for a launch.

Settings live at `$XDG_CONFIG_HOME/lazyreview/config.yml` (default `~/.config/lazyreview/config.yml`). If absent, LazyReview reads the old `review-workbench/config.yml` in place; it never moves that file. New settings use the canonical path. See [config.example.yml](config.example.yml) and the [blue preset](themes/blue.yml).

```yaml
gui:
  themePreset: lazygit
  companionStyle: paper # ledger | paper | studio
  syntaxTheme: github-dark # github-dark | lunar
  theme:
    activeBorderColor: ['#a184df', bold]
    selectedLineBgColor: ['#2d3f76']
```

Theme overrides use Lazygit's color lists: named colors, `#rrggbb`, `default`, and text modifiers. Supported fields are `activeBorderColor`, `inactiveBorderColor`, `searchingActiveBorderColor`, `optionsTextColor`, `selectedLineBgColor`, `selectedLineFgColor`, `unstagedChangesColor`, and `defaultFgColor`. Overrides apply to `lazygit` / `custom`; `blue` retains its fixed original palette. Unrelated Lazygit settings are ignored. Lookup starts with `$XDG_CONFIG_HOME/lazygit/config.yml`, then normal macOS Application Support paths.

`g v` selects Ledger, Paper or Studio for the companion and PR reader. Paper is the default and caps a maximized reading column at 84 cells. `--companion-style paper` tries one without saving it. These reader styles are independent of `ai.interface`, which selects the formatted reader or embedded CLI.

Offline previews and the standalone style gallery come from actual native cells:

```sh
./lazyreview --preview-ui ai --companion-style paper
./lazyreview --preview-ui description --companion-style studio
./lazyreview --theme blue --ui-gallery /tmp/lazyreview-layouts.html
```

## Code and annotations

Syntect/Oniguruma and bundled Two Face grammars color both diff sides, including TSX. `H` switches GitHub Dark / Lunar; `--syntax-theme github-dark|lunar` overrides the saved setting. Syntax does not set the terminal background. Lunar uses transparent UI colors and green/red diff fills (`#193824` / `#2a1717`) inspired by the author's LunarVim setup.

A background worker reads original/modified sources directly at the captured merge-base/head, retaining renamed paths and fork repositories. Modified files need two parallel Contents requests; added/deleted files request only their existing side. Parsing retains multiline context from the start of each side and stops after its last diff line. Stale results cannot repaint a different file. Four source pairs and eight palette entries are cached in memory; no source is written to disk.

Unavailable/binary content, unknown grammars, files over 200 KB, lines over 5,000 bytes or over 40,000 tokens retain plain text. Partial syntax is labelled beside file statistics; `f` retries source reads. Search and selection overlays preserve GitHub line coordinates. [Performance checks](../VALIDATION.md#native-terminal-regression).

File names show **● GitHub comment** and **◇ queued AI note** counts. GitHub counts include drafts, posted comments and replies; code attachments and pinned selections do not count. Separate colored `╭ │ ╰` gutters mark annotated ranges in inline and split diffs. Consumed/removed AI notes lose their markers; outdated GitHub comments retain counts without pointing at newer code. Whole-file notes count without inventing a line range.

## Reading and AI

`D` opens the PR timeline: description, dated comments/reviews, and one-line commits. `C` jumps to discussion, `g m` to commits, and `b` opens inline threads. Markdown/HTML render as selectable text, with tables, fenced syntax and an [offline Mermaid subset](MERMAID.md). Scripts do not run and images are not fetched. See [reading controls](REVIEWING.md#read-the-pr).

`g a` opens the companion. Claude's Paper interface is the default; `--ai-interface terminal` embeds its native CLI. `--ai-provider codex|grok` selects another terminal provider. Each PR/profile keeps its own native session. Ask about the whole PR or attach selected code, files and private notes; successful answers consume delivered notes. **Ctrl-]** releases CLI typing and focuses the previous pane. [Context, artifacts and recovery](CLAUDE.md) · [Provider controls and permissions](CLI-INTEGRATIONS.md).

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

## Implementation and limits

Rust owns input, Ratatui rendering, pane state, selection and confirmation. Crossterm restores the terminal; API calls, suggestions, syntax parsing and diagram rendering use separate workers. The native app embeds the checked-in JavaScript/TypeScript adapters, writes source modules to a private temporary directory and starts owned Node stdio children. Interactive providers run in owned PTYs with resize and VT emulation. Normal native use opens no TCP listener and depends on no hosted app; the optional [browser TUI](../site/INTERACTIVE-TUI.md) has its own sample-only HTTP adapter.

GitHub stores saved reviews, comments and viewed state. Filters, hidden files, pane widths/folding and unsent text are session-local. Pane preferences survive switching PRs within that run. Chat IDs and artifacts persist per PR/profile through the provider adapter. [Storage and permissions](CLI-INTEGRATIONS.md#context-permissions-and-storage).

This is not a full Vim emulator. Composers support cursor movement, Unicode editing, multiline paste and suggestions; external-editor integration is not implemented. `=` fits the longest tree entry and an 84-cell reader while preserving folding/zoom. Folded panes retain restore buttons and are skipped by Tab. Narrow layouts show the focused pane; 150 columns is comfortable. Fonts come from your terminal.

See [checks and known limits](../VALIDATION.md) before claiming GitHub/provider compatibility. Live GitHub writes remain untested; deterministic replays use sample data and mock responses. [Interaction principles](UX.md) explains the UI decisions.

## Demo assets

[Recorded native tour](../demo/lazyreview-demo.html) · [Silent MP4](../demo/lazyreview-demo.mp4) · [Rebuild](../demo/README.md)

The [hands-on browser TUI](../site/INTERACTIVE-TUI.md) uses the same Rust app and synthetic data. It is separate from both the recording and the earlier HTML web preview.
