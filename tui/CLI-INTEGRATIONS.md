# Native AI CLI integrations

Use the CLI you already sign in to. LazyReview owns the pane and the PR link; the CLI owns authentication, conversation history and compaction. There are no direct model API calls or private transcript readers.

```sh
./lazyreview --ai-interface terminal                 # Claude; opens the PR chooser
./lazyreview --ai-provider codex                     # Codex in the companion pane
./lazyreview --ai-provider grok --pr https://github.com/OWNER/REPO/pull/123
```

These flags enable AI for this launch. For a persistent choice, merge into `~/.config/lazyreview/config.yml`:

```yaml
ai:
  enabled: true
  provider: grok       # claude | codex | grok
  interface: terminal
  profile: default    # use a different profile for another account
  autoPrepare: false  # wait for your first question
```

Omit `command` and `model` to use provider defaults. Remove old `command: claude` / `model: sonnet` settings when switching provider in YAML. An executable wrapper can set account-specific environment variables; `command` is a path, never shell code. Changing provider/profile selects a separate PR conversation; changing model keeps it. Claude's previous associations remain compatible.

| Provider | CLI | Interfaces | Native resume | Context | Version checked |
| --- | --- | --- | --- | --- | --- |
| claude | Claude Code | Paper, terminal | --resume UUID | prompt hook | 2.1.302 |
| codex | Codex | terminal | resume UUID | review_context + hooks | 0.163.0 |
| grok | Grok | terminal | --resume UUID | review_context + hooks | 1.0.62 |

Open with **g a**. Click the CLI prompt to type. **Ctrl-]** releases keyboard control and focuses the previous visible pane; **Ctrl-\** keeps companion-local navigation; **i**, **Enter** or clicking the prompt returns it to the CLI. Drag selects and copies text without changing keyboard control. In LazyReview navigation: **N** creates a new chat, **R** relaunches/resumes, **X** interrupts. While typing, keys belong to the embedded CLI.

Mouse-aware CLIs receive a click on release, using their requested terminal mouse protocol and pane-local coordinates. Drags always select/copy text and send no click to the CLI. Claude's keyboard-only workspace trust menu has a narrow adapter: **click Yes or No to choose it**, including wrapped labels. When the clicked option is not selected, LazyReview waits for the CLI to redraw that selection before confirming it. Other keyboard-only menus still use arrows and Enter. Mouse motion, wheel scrolling and dragging remain LazyReview controls.

After mouse copying or scrolling, the next key or paste resumes the live view and reaches the CLI immediately; no extra **i** or Enter is needed. This also cancels an unfinished mouse gesture if the terminal loses its button-up event, without confirming the abandoned click. Explicit **Ctrl-\** navigation and keyboard selection retain their own controls.

## claude

Claude uses public `--session-id`, `--resume`, `--name` and lifecycle hooks. The prompt hook supplies bounded patches plus queued code questions; successful completion clears only the delivered items. Both Paper and terminal use the same association. See [Claude-specific setup and controls](CLAUDE.md).

Claude removes built-in tools, user/plugin setting sources and unrelated MCPs for this review session. Paper's `maxBudgetUsd`, time and turn caps do **not** apply to native terminal mode.

## codex

On first opening, LazyReview uses Codex's public stdio app-server to create and name an empty persistent thread. This does not run a model turn. The terminal then opens `codex resume UUID`; later openings resume that exact ID. LazyReview never locates a thread by reading Codex's private session files.

The review MCP and developer instructions supply PR context. Codex may require reviewing/trusting the generated hooks through **/hooks** before completion receipts are available. Stable adapter paths keep that trust valid across launches; actual hook changes require a new review. LazyReview does not bypass trust. Until hooks run, queued notes remain queued instead of being silently consumed.

## grok

Grok uses public `--session-id` / `--resume` and generated project MCP/hook configuration inside the private PR directory. Use the native folder-trust prompt or **/hooks-trust** for this generated directory, then **Ctrl-\**, **R** to relaunch if prompted. The public `SessionStart` receipt establishes the association. The displayed LazyReview title identifies the PR; Grok has no equivalent public `--name` flag in the checked version.

Grok currently discards `UserPromptSubmit.additionalContext`. Its adapter therefore instructs the model to call **review_context** before answering. That tool supplies current patches and private questions. Notes clear only when the tool delivered them and a matching successful `Stop` arrives. Failed, interrupted, stale and undelivered turns retain notes. This distinction is tested; do not copy Claude's hook assumptions into a Grok adapter.

## Context, permissions and storage

All providers receive the same immutable base/head GitHub read tools and local artifact tools. `review_context` returns bounded implementation evidence, not only the PR description. Missing/excerpted patches are disclosed and can be read through `changes` / `read_file`. Attaching a file or code range does not create a GitHub comment.

Codex/Grok run with read-only filesystem sandboxes and their normal approval controls. They retain their own account configuration, global policies, hooks and configured tools; unlike the restricted Claude adapter, they are **not** an exclusive review-tool sandbox. LazyReview never passes an approval-bypass flag or changes the global CLI configuration. Normal provider trust/permission prompts remain visible in the pane.

Associations stay in the historical `$XDG_STATE_HOME/lazyreview/claude/` directory (default `~/.local/state/...`); the hash includes provider + GitHub PR + profile. It stores the native ID, snapshot, bounded display cache, artifacts and public hook receipts. `adapter/` contains LazyReview's stable helper scripts; it contains no credentials. The CLI keeps its own history. Never delete active session state to fix a resume failure: use **N** for an explicit new chat.

## Adding another CLI

1. Verify the installed version and its **public** launch, resume, context and lifecycle interfaces against primary docs. Declare missing capabilities; do not parse private JSONL or invent API flags.
2. Add metadata to `src/ai-providers.mjs`, config/CLI validation in `ai.rs` / `main.rs`, and a direct-argv launch adapter in `ai-terminal.mjs`. Add an embedded source entry in `ai.rs` for any new module. No shell interpolation of user text.
3. Return `{command, args, cwd, env, session, title}`. The Rust PTY owns input, resize, VT rendering, selection and child cleanup. To receive clicks, the CLI must request a supported terminal mouse protocol (SGR, legacy or UTF-8). Do not assume every keyboard-only menu supports mouse. An adapter must match a known visible dialog and invoke only the user's completed click; never approve automatically, activate during a drag, or guess an unseen choice. Never restart an unrelated CLI daemon/session.
4. Persist the real native session ID. Use an official protocol for discovery/bootstrap if the CLI cannot accept a caller-assigned UUID. New chat must be explicit. Keep credential values out of association keys and diagnostics.
5. Normalize **documented** event fields in `ai-native-hook.mjs`. Bind receipts to launch, session and turn; ignore stale events. Consume only exact delivered attachment versions after success. A provider with no usable context hook needs a supported tool/context channel and truthful capability documentation.
6. Keep hook commands in stable private paths. Respect native trust; never add `--yolo`, `--trust`, `--dangerously-*` or auto-approve flags to make a test pass.
7. Update this capability table, setup/trust steps and validation evidence in the same change. `tests/ai-providers.test.mjs` checks the registry against this table. Run the JS/Rust suites and actual chooser → PR → click → type, copy, resize, interrupt and resume PTY checks. A fake CLI test cannot establish compatibility with the real one.

Current reproducible checks and live-test limits are recorded in [VALIDATION.md](../VALIDATION.md).

Native trust-menu regression (synthetic PR and isolated config/state):

```sh
python3 tests/tui-native-menu-pty.py          # fixture Yes/No, repeated clicks, drag copy
python3 tests/tui-native-menu-pty.py --mouse # child-requested SGR mouse protocol
python3 tests/tui-native-menu-pty.py --live --width 70 --height 30
# Optional: add --zellij to run inside a private temporary Zellij session.
# Read your actual Zellij config without modifying it or existing sessions:
python3 tests/tui-native-menu-pty.py --live --auto-prepare \
  --zellij-config ~/.config/zellij/config.kdl --first-click --open mouse
# Simulate a lost button-up, then explicitly press Enter on No:
python3 tests/tui-native-menu-pty.py --live --auto-prepare \
  --zellij-config ~/.config/zellij/config.kdl --first-click --drop-release
# Copy trust-menu text, then immediately press Enter on No:
python3 tests/tui-native-menu-pty.py --live --auto-prepare \
  --zellij-config ~/.config/zellij/config.kdl --copy-first
```

The live probe clicks **No** and verifies Claude exits; it never accepts real workspace trust or runs a model turn. Fixture tests cover Yes activation. Narrow/short layouts cover wrapped labels and the title above the viewport. `--baseline` checks the former highlight-only behavior against an older executable selected by `LAZYREVIEW_BINARY`.

For an already-trusted checkout, `--live --trusted-repl /absolute/checkout --zellij-config /path/config.kdl --copy-first` tests complete unsent typing immediately after copying, Backspace, pane switching and clearing. Replace `--copy-first` with `--drop-release` to test a missing release. It aborts if the folder requires trust, sends no prompt and leaves production launch directories unchanged. Full-config tests use private sockets, data and state; they override only test-session metadata persistence.

## Local input diagnostics

If input still fails in a specific terminal, launch once with:

```sh
LAZYREVIEW_INPUT_TRACE=/private/tmp/lazyreview-input.jsonl ./lazyreview --ai-interface terminal
```

The optional trace records incoming mouse down/up/drag, arrow/Enter/Tab events, pane coordinates, focus/selection state and whether LazyReview forwarded an event to the CLI. Text keys are recorded only as `key`; typed text, pasted text, clipboard contents, PR identifiers and source are excluded. New files use mode 0600, symlinks are refused, and recording stops at 256 KiB. Recording is disabled unless the variable is set. The trace stays on your machine and is useful for distinguishing a missing host event from a LazyReview focus problem.

The missing-release replay is verified; the physical Alacritty mouse path remains unverified. Mouse selection and scrolling keep the CLI's keyboard focus: the next key or paste returns to live output and reaches the CLI. Ctrl-\ explicitly switches to companion-local navigation, where keyboard text selection remains available. A trust choice is activated only by a completed click or an explicit keyboard confirmation.

Primary references: [Claude CLI](https://code.claude.com/docs/en/cli-reference), [Claude hooks](https://code.claude.com/docs/en/hooks), [Codex app-server](https://developers.openai.com/codex/app-server), [Codex hooks](https://developers.openai.com/codex/hooks), [Grok CLI](https://docs.x.ai/build/cli/reference), [Grok configuration](https://docs.x.ai/build/settings). Grok's installed `~/.grok/docs/user-guide/10-hooks.md` documents the current hook payloads and prompt-context limitation.
