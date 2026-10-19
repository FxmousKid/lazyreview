# Reviewing a real PR

## Start

From the repository root after [building](README.md#run):

```sh
gh auth login
./lazyreview
# Or open a particular PR:
./lazyreview --pr https://github.com/OWNER/REPO/pull/123
```

Node.js 22.18+ must be on PATH. The executable reads your existing `gh` login, `GH_TOKEN`, or `GITHUB_TOKEN`; it never saves credentials. For a fine-grained token, select the repository and allow **Contents: read**, **Pull requests: read/write**, and **Issues: read/write** for general comments. Your account still needs access, and organization SSO policies may apply.

The chooser shows repositories with your open PRs, most recent contribution first. `h/l` focuses repositories/PRs; `j/k` moves within that pane; Enter opens its selection. **`/` or Search opens a floating finder**: fuzzy-search your repository history, choose one, then fuzzy-search all your PRs by number or title (open, merged and closed, newest first). Arrows / Ctrl-j/k select; Tab includes all authors, Alt-Left returns to repositories, and Escape closes. Enter an exact `owner/repo` to browse outside your history. `f` filters the normal chooser list; `Ctrl-p` returns here from a review. [Search scope and pagination](README.md#chooser-keys).

## Inspect code

- Tab / Shift-Tab changes review panes. In Files, `h/l` recursively folds/unfolds the selected subtree. `j/k` selects rows; Enter opens a file. **`n/N` jumps directly to the next/previous file**, skipping headings and folders and revealing a folded destination. It respects active file filters.
- File badges: `+` added, `~` modified, `−` deleted, `→` renamed. Green/red counts show added/deleted lines. `✓` means viewed.
- `1–5` filters Code / Tests / Docs / Config / Assets; `0` shows all. Type headings remain open. `/` filters paths, `o` sorts, `x` hides a file, `z` reveals hidden files.
- `/` in Changes searches code incrementally. Enter keeps highlights, `n/N` moves forward/backward and wraps; Escape clears them. Search is literal and uses smart case (lowercase ignores case; uppercase matches case). Tab while typing selects Both / Original / Modified; split views search the two versions of the same file, counting shared context once. Escape while typing restores the previous search. Files keeps its own `/` path filter.
- In Changes, `n/N` jumps files when no diff search is active; with search highlights, it moves between matches. `s` switches inline/side-by-side. `]c/[c` jumps hunks; `J/K` always jumps files. `h/l` scrolls code horizontally.
- `\` opens **Panes**: choose any pane with `j/k`, then Enter to fold/unfold it. The companion starts closed. `+/-` or dragging dividers resizes; `Z` maximizes; `=` fits file names and the reading column while keeping folded panes closed. Open panes have `[−]` controls; collapsed panes keep labelled restore buttons.

## Ask Claude about unfamiliar code

Press `g a`, then Enter for first-time setup using your existing Claude Code login. The companion prepares a brief PR orientation in the background and resumes the same conversation on future visits. `i` opens a general PR question. `F` explicitly includes the focused file; `a` / `A` queue selected code or whole files. Select a range with `v`, press `t`, and add a temporary question; repeat across files. `g c` inspects these private notes and code. They clear after a successful answer, and remain queued if it fails or stops. They never post to GitHub. In the AI pane, `N` starts a fresh chat, `X` stops generation and `R` retries preparation. Review navigation remains available while answers stream. [Settings, context, persistence and recovery](CLAUDE.md).

Drag text in the companion, PR timeline or review thread to copy it on release, even while the Paper question composer is open. Keyboard selection uses `v`, `h/j/k/l`, then `y`; Escape cancels. Copy stays inside the reading pane and preserves rendered line wraps.

To use the actual Claude Code interface, launch `./lazyreview --ai-interface terminal --pr URL` or set `ai.interface: terminal`. Open `g a`; **Ctrl-]** leaves input and focuses the previous visible pane; **Ctrl-\** keeps companion-local controls, and **i** or **Enter** returns to Claude. Native typing forwards all normal keys to the CLI. Existing PR sessions resume by their exact ID. Queued notes are injected on prompt submission and consumed only after the successful completion hook. Paper remains the default; its per-turn budget/time limits do not apply to native mode. [Native controls and boundaries](CLAUDE.md#interactive-cli-mode).

## Comment on one or several ranges

1. Focus Changes. Press `v`, then `j/k` to extend a contiguous range.
2. Press Space to pin it. Navigate to another hunk or file and repeat `v`, `j/k`, Space.
3. Press `c`, write the comment, then **Enter** to save it to your pending GitHub review.

Without pinned ranges, `c` uses the current range or line. Pinning the same range again removes it. Escape clears a visual selection; another Escape clears pinned ranges. Up to 20 ranges can be pinned.

GitHub review comments have one native contiguous anchor. The first pinned range becomes that anchor; other ranges become immutable code links in the same comment. Links respect the original/modified side, renamed paths, the fork repository, and the loaded commit. This produces **one comment**, not duplicate threads. See [GitHub's review comment coordinates](https://docs.github.com/en/rest/pulls/comments#create-a-review-comment-for-a-pull-request).

In the compact composer:

- Enter saves the draft. Replies/general comments are published immediately when Enter is pressed; their titles identify the action.
- Alt-Enter or Ctrl-n adds a newline. Bracketed paste preserves newlines without submitting. Arrows and Home/End move the cursor.
- Type `@` at a word boundary for people, or `#` for an issue/PR. Up/Down selects a suggestion; Tab/Enter inserts it. The following Enter submits. Escape dismisses suggestions first, then closes the composer. Fully typed mentions/references also work without suggestions.
- Escape keeps the unsent text for `e` to reopen. Failed requests preserve the composer. If a write's outcome is uncertain, close the composer and press `f` to refresh before another write.

## Review your review, then publish

Press **`r`** to open **Your review** on the **Drafts** tab. Files start folded with a comment count. `j/k` selects an entry; Enter or a click opens it, `l` unfolds and `h` folds. Opening a file shows one-line comment previews; only one full Markdown body stays open at a time. **`P`** or the tabs switches Drafts / Posted. Ctrl-d/u scrolls long bodies; `gg/G` selects the first/last entry. Private AI notes are excluded from this list and submission counts.

Click **Review changes** in the header, **Submit** in the pane, or press **`S`**. Tab / Shift-Tab or a click selects Comment / Approve / Request changes. Write a summary, then Enter or **Continue** opens a compact confirmation with the outcome, summary and draft/file counts. **Enter or clicking Publish review publishes once**; Escape / Back returns to editing. Closing and reopening the sheet preserves its summary and outcome. Your own PR allows Comment only. A blank summary is allowed for an approval or when publishing saved draft comments.

GitHub stores saved pending comments. They are not sent to the PR author until the review is submitted. The adapter checks the head/base before writes and rejects stale anchors and older pending reviews. GitHub does not allow approving or requesting changes on your own PR.

`b` opens threads; `i` replies, and `R` opens resolve/reopen confirmation. Markdown and common GitHub HTML render headings, emphasis, code, lists, task lists, tables, quotes and links. HTML coverage reports use real table cells; narrow panes show labelled records instead of clipping columns. `<details>` summaries and their contents are shown expanded. Scripts/styles are omitted; images show alt text without fetching assets. `p` cycles companion views. `g ?` (or `g !`) opens all shortcuts; `f` filters them; one Escape closes the overlay.

## Verify a review workflow

Use the [check map](../VALIDATION.md) for offline replays and explicit live reads. `tui-review-flow-pty.py` exercises the actual bridge from draft to inspection and exactly one publication, using sample state. GitHub write tests use mocked responses; no real comment/review has been published by this validation. Live writes need a disposable PR and verification on GitHub. Provider checks have separate setup and usage boundaries in [CLI integrations](CLI-INTEGRATIONS.md).

## Read the PR

`D` opens one **PR timeline**: the description first, followed by dated discussion comments, submitted reviews and compact commit rows in chronological order. Comments have author/timestamp borders; commits keep their short SHA and subject on one line. `C` jumps to the first comment/review, and `g m` jumps to the first commit in that same document. `j/k` and Ctrl-d/u scroll; `h/l` or `[[`/`]]` jump between entries; `o` opens the timeline index; `gg/G` go to the ends. `i` opens a general PR comment composer.

`b` opens inline review threads with the same bordered Markdown cards. Here `j/k` selects threads, Ctrl-d/u scrolls the selected conversation, `i` replies, and `R` opens resolve/reopen confirmation. `h/l` returns to the timeline. Both reading surfaces support text selection and copying; the usual `\` controls fold the pane. GitHub timestamps are displayed in UTC; entries without timestamps appear last rather than receiving an invented date.

Code fences use labelled boxes and the selected syntax palette. Suggestions remain literal, read-only replacement text. Explicit `markdown` / `md` blocks preview Markdown (up to two nested levels), so bot prompts containing nested suggestions remain readable; other languages preserve their literal fences. Colors load in the background, and oversized blocks stay plain.

`H` changes syntax palette between GitHub Dark (default) and Lunar. The interface theme remains independent (`T`). For a persistent preference, set `gui.syntaxTheme: lunar`; to try it once, launch `./lazyreview --demo --syntax-theme lunar`.
