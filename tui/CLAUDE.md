# Claude Code companion

For Codex/Grok native terminals and the reusable adapter contract, see [CLI integrations](CLI-INTEGRATIONS.md).

LazyReview uses your installed Claude Code CLI and its native sessions. It does not call the model API directly, parse Claude's private JSONL files, or replay a home-grown conversation history.

## Connect and use

Install Claude Code and run `claude auth login` if needed. Both interfaces were verified with Claude Code **2.1.302**; use a recent CLI supporting `--permission-prompts none` (2.1.259+).

Paper is the default interface. It formats Claude replies in the LazyReview reader. The optional **terminal** interface embeds the actual interactive Claude Code CLI in the companion pane; both use the same PR/profile session association.

Open a real PR, press **g a**, then **Enter** to enable Claude. Alternatively, run `./lazyreview --setup-ai` once. Setup keeps existing theme settings. It saves preferences, not credentials. The companion stays folded until you open it; its footer shows preparation/readiness.

These are the Paper controls. Native typing and navigation are described under [Interactive CLI mode](#interactive-cli-mode).

- **g a:** open the companion (or initial setup).
- **i, question, Enter:** ask. The whole PR is the default scope; no attachment or selected file is required.
- **F** in the AI pane: toggle PR / focused-file scope for the next question. File scope includes the visible line, selection and pinned ranges. Explicit attachments accompany either scope.
- **o**, then **j/k**, **Enter**: revisit a question. **[[ / ]]** jumps between questions; **G** returns to the latest reply.
- **g o**, then **j/k**, **Enter**: browse saved diagrams, pages and notes; open a standalone browser preview.
- **a / A:** queue selected code/diff or a whole file for the next answer.
- **t** in Changes: add a temporary question to the selected range. Repeat across files. **g c** inspects the queue; Enter edits a note, `d` removes it, `i` asks Claude. These notes never become GitHub comments. Successfully consumed items clear after the answer is saved; failed/stopped answers preserve them. Items added or edited during an answer remain queued.
- **Drag text** in the companion/description to select and copy on release, including while the Paper question composer is open. Or **v**, **h/j/k/l**, **y** to select and copy with the keyboard; Escape cancels. Clipboard commands run off the UI thread. Copy includes the rendered line wraps, without pane borders.
- **X:** stop the current answer or preparation. Ctrl-c in the focused, busy AI pane also stops; otherwise Ctrl-c exits normally.
- **N:** start a new native chat for this PR/profile. The prior session and its display cache remain available; neither is deleted.
- **R:** retry preparation after an error. After a failed question, `i` reopens the retained draft so you can send it again.
- **f:** refresh the PR. A new comparison updates the same conversation; a same-revision refresh updates available discussions without another preparation turn.
- **Ctrl-d/u, j/k:** read previous replies. Asking another question resumes following streamed text.

In Paper, a previously unseen PR gets a short preparation turn in the background once enabled. Your question interrupts that preparation. Review navigation and GitHub operations use independent workers. Opening the same unchanged PR restores the visible chat without an inference call. The first question resumes the native session by its exact ID.

## Preferences

Merge this section into `~/.config/lazyreview/config.yml` (or `$XDG_CONFIG_HOME/lazyreview/config.yml`):

```yaml
ai:
  enabled: true
  interface: paper # paper | terminal
  profile: default
  command: claude
  model: sonnet
  autoPrepare: true
  maxBudgetUsd: 1
```

`command` is an executable name/path, not a shell expression. A wrapper executable can supply a different Claude account environment; give it a distinct `profile`. The profile identifies the account/provider configuration without storing an API key. Changing the model or rotating a credential keeps the conversation. Changing profiles selects a separate conversation. Configuration changes take effect on the next launch.

Set `autoPrepare: false` to defer inference until your first question. Set `enabled: false` to disable the integration. Automatic preparation sends PR context through the CLI's configured account/provider and can consume Claude usage. In Paper, `maxBudgetUsd` is Claude's estimated **per-turn** cap, not a guaranteed billing ceiling; preparation has three agent turns / 60 seconds, and questions have twelve / 180 seconds. **These budget, turn and time limits do not apply to interactive terminal mode.** Both interfaces use existing CLI login or environment credentials; subscription access depends on Claude Code's own entitlement.

## Context and read tools

The initial briefing contains the PR description, file manifest (up to 500 entries, with an explicit total/truncation marker), exact base/head commits and a bounded 32,000-character patch budget. Explicit selections, named files, question terms, migrations and implementation changes take priority over documentation. Oversized patches receive labelled excerpts; omitted changes remain available through tools. Each question adds relevant implementation evidence that has not already been supplied for this comparison. Native resume reuses previously supplied code without replaying the display cache.

Explanations must use implementation evidence and distinguish description claims from code behavior. They do not trigger a full review or a blanket “not reviewed” disclaimer. Paper's compact **supplied / read** counts distinguish patches delivered in context from successful code-tool reads. A supplied patch is not proof of model attention. Failed calls, metadata and artifact tools do not count as code reads; excerpts and omissions are disclosed in the model context. New comparisons reset the evidence accounting.

Claude has seven context/repository read tools and three artifact tools:

| Tool | Reads |
| --- | --- |
| `review_context` | Current comparison, bounded patches and queued private questions |
| `overview` | PR metadata, comparison commits, explicit question scope |
| `changes` | Paginated file inventory or a returned patch |
| `read_file` | Any repository text file at the captured base/head, with numbered ranges |
| `tree` | Repository paths at that commit; supports path filtering and pagination |
| `search_changes` | Literal text matches within loaded patches |
| `discussion` | Loaded comments, inline threads and commits |
| `artifact_list` | Saved PR outputs and revision metadata |
| `artifact_read` | One saved output's source |
| `artifact_create` | **Writes** a new PR-local HTML page, SVG diagram or Markdown note |

File reads respect forks and original/modified sides. `read_file` takes the original path for the LEFT side of a rename, accepts at most 300 lines per call and inherits the existing 200 KB UTF-8 source limit. Tree truncation is explicit. These tools expose no arbitrary URL, shell, file editing, or GitHub mutation operation. LazyReview starts the CLI in its private state directory, disables user/project setting sources and plugins, supplies only this MCP server, and removes built-in tools. Paper disables hooks; terminal mode installs only LazyReview's own lifecycle hooks. Human review/comment actions continue using the existing guarded GitHub adapter.

The model gets the focus captured when a question is submitted. In terminal mode, LazyReview keeps that snapshot current and the next `UserPromptSubmit` hook supplies it; cursor movement does not trigger inference. New commits/base changes preserve the conversation but inject the updated comparison and instruction to recheck older code assumptions. Current patches and immutable file tools provide the new evidence. This is not a guarantee that the model has read every file, or that every past message remains in its active context window.

## Diagrams, pages and notes

Ask naturally, for example: **“Explain the purpose of this PR and save an HTML review guide”**, **“Draw the data flow as a diagram”**, or **“Save our decisions as notes.”** Claude calls the artifact tool; successful outputs appear on the Paper shelf above the composer and under **g o** in LazyReview navigation. This is a real local output, not a screenshot or a canned answer.

Each output records its PR, exact base/head, native session, title, summary and creation time. Older revisions are labelled in the picker. New chat and new commits preserve existing outputs. Claude can list/read them and create a revised copy; it cannot silently overwrite them. The shelf belongs to the PR/profile, independently of the current native chat.

Outputs are static, self-contained HTML, SVG or Markdown (250 KB per item, up to 200 per PR/profile). HTML/SVG previews open in a sandboxed frame with scripts, forms, external assets and network requests disabled. Markdown opens as source text. Use SVG directly for diagrams; no Mermaid renderer, external JavaScript or extra CLI is required. The preview includes a Source disclosure and can be copied as a standalone file. `source.json` retains the original content.

Paper background preparation cannot create outputs; its explicit chat turns can. Native mode exposes artifact tools during interactive turns, including its optional initial orientation. The model supplies content and metadata, never a filesystem path or shell command. Browser opening is a human key action. The tool is deliberately narrow: it does not edit repository files or grant GitHub write access.

## Persistence and recovery

The local association key is a SHA-256 hash of GitHub host, PR URL, Claude Code and profile. It selects the stored native session ID; the session ID itself is a UUID, not a replacement hash. Paper and terminal share the association. One process may own a PR/profile at a time; a second LazyReview reports the conflict. Locks from a dead process recover on the next open.

LazyReview's state lives under `$XDG_STATE_HOME/lazyreview/claude/` (default `~/.local/state/lazyreview/claude/`), with private directories and files. Each association stores:

- Native Claude session ID and last successfully prepared comparison.
- Current PR snapshot for read tools, including code and discussions.
- A bounded **display cache** of LazyReview messages and the preparation summary. It is never sent back as model history. The cache keeps at most 200 messages / approximately 240 KB; the UI discloses omitted earlier messages.
- MCP launch configuration, a process lock, and metadata/display caches archived by New chat.
- In terminal mode, LazyReview-owned prompt/completion receipts and revision fingerprints from public hooks. These are not Claude transcript files.
- `artifacts/<id>/meta.json`, `source.json`, and `preview.html`. Files are private (0600), directories 0700. Preview opening checks source integrity and regenerates the wrapper. Unfinished outputs are omitted.

Claude Code owns its conversation files and compaction. LazyReview neither reads nor edits those private transcripts. Messages sent outside LazyReview are available to native session resumption but are not imported into the Paper display cache. For terminal turns inside LazyReview, documented lifecycle hooks provide the submitted prompt and final assistant message for that display cache; they do not reproduce Claude's complete tool transcript. To inspect a previous session in Claude, use `claude --resume <session>` with the ID from the corresponding metadata file. Stop/close LazyReview's chat first.

If Claude's session was deleted, the next native resume reports an error; **N** explicitly starts a new chat. Authentication/model/budget failures preserve the question and leave preparation incomplete. Missing CLI: install it or fix `ai.command`, then retry. Quit stops only the companion's own process group and releases its lock; unrelated terminal sessions are untouched. Demo mode never starts Claude or accesses the network.

## Research and validation

- [Claude programmatic interface](https://code.claude.com/docs/en/headless): native resume IDs, streamed partial responses, cancellation, budget/turn limits. `--bare` does not use subscription OAuth, so this integration uses explicit settings/tool isolation while retaining login support.
- [Claude CLI reference](https://code.claude.com/docs/en/cli-reference): session IDs, tool allowlists, MCP configuration and settings scopes.
- [MCP stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) and [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools): the local repository-read and artifact tool bridge uses documented JSON-RPC messages.

```sh
node --test tests/ai-session.test.mjs tests/ai-evidence.test.mjs tests/artifacts.test.mjs
cargo test --locked --manifest-path tui/Cargo.toml
# Public GitHub reads + deterministic fake Claude, no inference:
python3 tests/tui-ai-pty.py --live
python3 tests/tui-native-pty.py --live
# Two short real interactive turns, native answer copying and session resume:
python3 tests/tui-native-live.py --live
# Two real bounded Claude turns, public PR, existing authentication:
node tests/ai-live.mjs --live
# Contradictory description versus SQL evidence, plus native resume (no GitHub):
node tests/ai-grounding-live.mjs --live
# Real artifact creation, native restart and artifact read-back:
node tests/artifacts-live.mjs --live
```

The original Paper live probe verified repository tool reads, streaming and exact session memory after restart. The artifact probe additionally created a real SVG and HTML page, restarted the native session, then listed and read back the saved diagram. See [validation](../VALIDATION.md) for exact evidence and limits. No GitHub writes were made. Voice, automatic import of externally written chat messages, full-repository text search, GitHub Enterprise and cross-machine session synchronization are not implemented.

## Interactive CLI mode

Try it for one launch:

```sh
./lazyreview --ai-interface terminal --pr https://github.com/OWNER/REPO/pull/123
```

Or set `ai.interface: terminal` in the config. `--ai-interface paper` returns to the default reader for one launch without changing the saved preference. `gui.companionStyle` controls Paper/description presentation and does not select the CLI interface.

Open **g a** to launch Claude Code in a private PTY inside the right pane. Its native UI receives terminal resizing and owns its prompts, slash commands and session rendering. With `autoPrepare: true`, the first launch of a new session asks for a short PR orientation; returning sessions resume without a new orientation prompt. The first child launch waits until you open the companion; folding it later does not stop the process.

- **Ctrl-]** leaves Claude input and focuses the previous visible pane, equivalent to leaving input then Shift-Tab. **Ctrl-\** keeps focus in the companion for its local navigation controls. While typing, keys such as `n`, `N`, Tab, Escape and Ctrl-c go to Claude instead of triggering LazyReview actions.
- On Claude's workspace trust menu, **click Yes or No to choose it**. Keyboard users can use **↑/↓**, then **Enter**. Wrapped option labels remain clickable. Dragging across text only copies it. Other keyboard-only Claude menus use their native keys.
- In LazyReview navigation, **i** or **Enter** returns to Claude typing. Tab changes panes; **N** starts a new PR chat, **R** restarts/resumes an exited CLI, and **X** sends an interrupt to the child.
- Dragging native chat text copies on release and freezes the displayed cells while Claude continues running. Copying or scrolling does not leave Claude typing mode: the next key or paste clears the mouse selection, returns to the live view and reaches Claude immediately. Keyboard selection uses **Ctrl-\**, **v**, **h/j/k/l**, then **y**; in that explicit navigation mode, **i** returns to typing and **Escape** cancels selection.
- Queue code and temporary notes with the usual **a / A / t** actions, then return to Claude. The public `UserPromptSubmit` hook adds current PR evidence, focus and the queued notes to the next submitted prompt. Notes clear only after a matching successful **Stop** event is persisted. Failure, interruption or a missing completion event retains them; edits made while the answer runs remain queued.

A first launch passes `--session-id <UUID>`; later launches pass `--resume <the exact UUID>`. Sessions receive a title such as `LazyReview owner/repo #123 · 7a9d2e10`, where the suffix identifies the PR/profile association. Switching interfaces or models keeps that session; **N** explicitly creates another UUID. Native `/clear` and `/resume` session changes update this association through `SessionStart`, archive the previous display cache and refresh the supplied code context. No private Claude JSONL file is read or rewritten.

The native UI uses the existing Claude login, LazyReview's repository/artifact MCP tools and its own lifecycle hooks. Built-in shell/file tools and other MCP servers remain disabled. It does not inherit Paper's budget, turn-count or timeout controls. Quit stops only the owned child process group. See [validation](../VALIDATION.md) for the current native-mode test evidence; earlier streaming probes apply to Paper.
