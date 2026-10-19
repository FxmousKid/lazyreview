# A silent tour of LazyReview

Open **`/lazyreview/demo/tui/`** on the showcase, open `lazyreview-demo.html` offline, or play `lazyreview-demo.mp4`. The separate browser UI preview remains at `/lazyreview/demo/`.

The tour follows two workflows: narrow a PR to the relevant code, then ask Claude about selections from two files. It lasts **88 seconds** across 17 scenes, with reading holds and separate title cards. A KeyCastr-style overlay shows each recent keystroke in the lower third as it occurs, holds the sequence for two seconds, then fades so the code stays readable. Future keys are never shown in advance. There is no keyboard diagram or narration.

All terminal footage comes from the real Ratatui renderer after real input handlers. The PR and reply are explicitly labelled synthetic fixtures. This tour demonstrates the TUI’s **Paper companion**. AI replies are scripted; it does not launch an embedded Claude CLI.

The HTML is one offline file with no dependencies or network requests. Space plays/pauses; arrows step through scenes. The timeline, chapter buttons, speed control, Full workspace toggle and Actual size mode remain available. Actual size uses the original logical cell dimensions and lets you scroll instead of shrinking text. Playback starts only when requested. Reduced-motion mode disables camera movement.

## Rebuild

```sh
./lazyreview --export-demo demo/frames.json
uv run --offline --with pillow python demo/render_video.py \
  --frames demo/frames.json --native-images demo/native --output demo/lazyreview-demo.mp4
node demo/build.mjs
node --test tests/demo.test.mjs
npm run site:build
```

The exporter asserts that both code selections come from different files, notes clear after a successful answer, and the copied sentence comes only from the companion. The HTML builder rejects active markup, invalid crop bounds, ambiguous frame IDs, and invalid timing.

The MP4 is 1920 × 1080, 15 fps H.264 with no audio. Pillow uses the bundled, hash-checked JetBrainsMono Nerd Font files and macOS symbol fallbacks and streams frames directly to ffmpeg. A contact sheet is generated beside the video. `--stills-only` skips encoding. Both players use the same exported frames and timing.

## Motion and keystrokes

This is occasional explanatory motion. The camera moves over 720 ms with `cubic-bezier(0.77, 0, 0.175, 1)`; title cards enter over 500 ms and keystrokes over 200 ms with `cubic-bezier(0.23, 1, 0.32, 1)`. New keycaps arrive over 160 ms using the same ease-out curve; bezel size updates immediately, and only transform/opacity animate in the HTML. Direct scrubbing and scene navigation update immediately; hiding the browser tab pauses playback. The actual TUI remains instant.

The effect used in many tutorials is a **keystroke visualizer**. [KeyCastr](https://github.com/keycastr/keycastr) supplies it on macOS; [Screenkey](https://www.thregr.org/wavexx/software/screenkey/) does it on Linux. This demo reproduces the KeyCastr presentation from its scripted shortcut events. **Nothing needs installing**, and no Accessibility/Input Monitoring permission is needed: it never listens to the system keyboard. A small translucent black bezel grows as `g`, then `a` arrive; Enter appears as `↵`. There are no arrows, dim future keys, or keyboard diagram. The timing and key labels come from `build.mjs` for both HTML and MP4.

The KeyCastr [default visualizer source](https://github.com/keycastr/keycastr/blob/main/keycastr/KCDefaultVisualizer.m) informed the grouped recent keys, dark bezel, two-second hold, and 200 ms fade. This is a separate implementation for the recorded tour, not an embedded or installed copy of KeyCastr.

## Frame format

Each frame has a unique `id`, `chapter`, `title`, `caption`, `keys`, `duration` (milliseconds), optional `focus` rectangle in terminal cells, and escaped native `html`. `kind: "card"` creates a title card. Terminal markup allows only `pre` and styled `span` elements; source code is escaped. `build.mjs --timeline demo/frames.json` adds a deterministic `keycast` timeline (`events`, `end`, `fade`) to each frame without changing the native frame data. Both renderers consume that timeline; tests cover reveal order, chord labels, clearing, and short scenes.

Visual checks should focus on whether the reading holds are long enough, the key pills appear where expected, and the camera follows the workflow without obscuring code. Browser playback needs a manual feel-check when browser access is unavailable.

## Local display profile

`terminal-profile.json` is a sanitized snapshot of the author's actual setup: **JetBrainsMono Nerd Font 16pt**, Regular/Bold/Medium Italic, offset `(0, 1)`, Alacritty **3024**, 70% background opacity, purple Lazygit theme, GitHub Dark syntax and the Paper companion. The configured Heavy Italic face is absent locally; Alacritty falls back to Regular, which the export preserves. Font bytes and their SIL license live in `fonts/`. No AI settings, filesystem paths or session content are included.

The Rust exporter pins this display profile and resolves terminal-default/ANSI colors separately from explicit RGB colors. It runs real app handlers and `ui::render`; it does not recreate the UI in HTML. Pillow positions each native glyph on its original terminal cell, with background opacity separate from text opacity. The browser displays lossless cell images, so installed browser fonts cannot change the result. Images, profile and source frames are bound by SHA-256; a stale or edited image fails the build. Both the hosted and offline player use the same cells and key timeline as the MP4.

The 2× grid uses CoreText metrics measured directly from the installed regular font: advance 19.2, ascent 32.6401, descent 9.6001 at size 32. Alacritty rounds ascent/descent and floors the resulting advance/line height plus offset, yielding 19×44 device pixels (9.5×22 logical pixels). [Alacritty font loading](https://github.com/alacritty/alacritty/blob/master/alacritty/src/renderer/text/glyph_cache.rs) and [crossfont metrics](https://github.com/alacritty/crossfont/blob/master/src/darwin/mod.rs) explain face fallback and metric calculation.

This is a native-cell export, not a screen recording of the desktop. The matte is the terminal's `#090300`; macOS backdrop blur, wallpaper, GPU font/box rasterization and Zellij's surrounding chrome are not captured. Window decorations are absent, matching the local configuration. Fit/camera modes scale the picture; Actual size preserves its logical text size. Rendering needs Pillow, ffmpeg and the named macOS symbol fallback fonts; serving the committed assets needs only Node or a static host.
