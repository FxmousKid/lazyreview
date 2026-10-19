# LazyReview

Review GitHub pull requests in your terminal, with Vim keys and an AI companion.

[Interactive TUI demo](site/INTERACTIVE-TUI.md) · [Recorded tour](demo/README.md) · [Full keymap](tui/README.md) · [AI setup](tui/CLI-INTEGRATIONS.md)

Filter code, tests and docs. Read inline or split diffs, queue private questions across files, and publish native GitHub reviews. Claude Code keeps a separate persistent chat for each PR; Codex and Grok also work in the terminal pane.

## Run

Requires **Rust 1.88+**, **Node.js 22.18+**, and [GitHub CLI](https://cli.github.com/).

```sh
git clone https://github.com/FxmousKid/lazyreview.git
cd lazyreview
cargo build --release --locked --manifest-path tui/Cargo.toml
gh auth login
./lazyreview
```

`./lazyreview --demo` runs offline. Open a PR directly with `--pr https://github.com/OWNER/REPO/pull/123`. Add `--ai-interface terminal` to embed Claude Code; sign in with `claude` first. The default Paper companion also uses your Claude Code login.

## Keys

| Key | Action |
| --- | --- |
| `Tab` / `Shift-Tab` | Change pane |
| `j/k`, `h/l` | Move; fold/open folders |
| `n/N` | Next/previous file, or active search match |
| `/` | Search files or the focused diff |
| `s`, `m` | Split/inline diff; mark viewed |
| `v` → `t` | Select lines → private AI note; repeat across files |
| `g c`, `g a` | Inspect AI context; open companion |
| `Ctrl-]` | Leave the CLI and step to the previous pane |
| `c`, `r`, `S` | Draft GitHub comment; inspect review; submit flow |
| `D`, `b` | PR timeline; inline threads |
| `\`, `=`, `g ?` | Fold panes; fit widths; searchable help |

AI notes are supplied with your **next prompt**, through Claude’s context hook. They are not visible typed messages or GitHub comments. Successful answers consume delivered notes; errors and interruptions keep them. [How context works](tui/CLAUDE.md).

Settings: `~/.config/lazyreview/config.yml` ([example](tui/config.example.yml)). Drag pane borders to resize; themes import Lazygit colors. GitHub stores review state; local filters and drafts stay in your session. This is an early project; [limits and validation](VALIDATION.md).

## Develop

`npm test` · `cargo test --locked --manifest-path tui/Cargo.toml` · `npm run site:dev`

The showcase lives at `/lazyreview/`. Try the real TUI at `/lazyreview/demo/`: Vim keys, selections, AI notes and sample reviews. The web UI preview remains at `/lazyreview/demo/web/`; the recording is at `/lazyreview/demo/tour/`. Hosted sessions use isolated native processes and synthetic data.

MIT © Iyan Nazarian
