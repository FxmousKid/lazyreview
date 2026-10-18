# A silent tour of LazyReview

Open `lazyreview-demo.html`, or play `lazyreview-demo.mp4`.

The tour follows two workflows: narrow a PR to the relevant code, then ask Claude about selections from two files. It lasts **88 seconds** across 17 scenes, with reading holds and separate title cards. A KeyCastr-style overlay shows each recent keystroke in the lower third as it occurs, holds the sequence for two seconds, then fades so the code stays readable. Future keys are never shown in advance. There is no keyboard diagram or narration.

All terminal footage comes from the real Ratatui renderer after real input handlers. The PR and reply are explicitly labelled synthetic fixtures. This tour demonstrates the **Paper companion**, not a live Claude request or the native terminal mode.

The HTML is one offline file with no dependencies or network requests. Space plays/pauses; arrows step through scenes. The timeline, chapter buttons, speed control, and Full workspace toggle remain available. Playback starts only when requested. Reduced-motion mode disables camera movement.

## Rebuild

```sh
./lazyreview --export-demo demo/frames.json
node demo/build.mjs
node --test tests/demo.test.mjs
uv run --offline --with pillow python demo/render_video.py \
  --frames demo/frames.json --output demo/lazyreview-demo.mp4
```

The exporter asserts that both code selections come from different files, notes clear after a successful answer, and the copied sentence comes only from the companion. The HTML builder rejects active markup, invalid crop bounds, ambiguous frame IDs, and invalid timing.

The MP4 is 1920 × 1080, 15 fps H.264 with no audio. Pillow uses local JetBrains Mono/system fonts and streams frames directly to ffmpeg. A contact sheet is generated beside the video. `--stills-only` skips encoding. Both players use the same exported frames and timing.

## Motion and keystrokes

This is occasional explanatory motion. The camera moves over 720 ms with `cubic-bezier(0.77, 0, 0.175, 1)`; title cards enter over 500 ms and keystrokes over 200 ms with `cubic-bezier(0.23, 1, 0.32, 1)`. New keycaps arrive over 160 ms using the same ease-out curve; bezel size updates immediately, and only transform/opacity animate in the HTML. Direct scrubbing and scene navigation update immediately; hiding the browser tab pauses playback. The actual TUI remains instant.

The effect used in many tutorials is a **keystroke visualizer**. [KeyCastr](https://github.com/keycastr/keycastr) supplies it on macOS; [Screenkey](https://www.thregr.org/wavexx/software/screenkey/) does it on Linux. This demo reproduces the KeyCastr presentation from its scripted shortcut events. **Nothing needs installing**, and no Accessibility/Input Monitoring permission is needed: it never listens to the system keyboard. A small translucent black bezel grows as `g`, then `a` arrive; Enter appears as `↵`. There are no arrows, dim future keys, or keyboard diagram. The timing and key labels come from `build.mjs` for both HTML and MP4.

The KeyCastr [default visualizer source](https://github.com/keycastr/keycastr/blob/main/keycastr/KCDefaultVisualizer.m) informed the grouped recent keys, dark bezel, two-second hold, and 200 ms fade. This is a separate implementation for the recorded tour, not an embedded or installed copy of KeyCastr.

## Frame format

Each frame has a unique `id`, `chapter`, `title`, `caption`, `keys`, `duration` (milliseconds), optional `focus` rectangle in terminal cells, and escaped native `html`. `kind: "card"` creates a title card. Terminal markup allows only `pre` and styled `span` elements; source code is escaped. `build.mjs --timeline demo/frames.json` adds a deterministic `keycast` timeline (`events`, `end`, `fade`) to each frame without changing the native frame data. Both renderers consume that timeline; tests cover reveal order, chord labels, clearing, and short scenes.

Visual checks should focus on whether the reading holds are long enough, the key pills appear where expected, and the camera follows the workflow without obscuring code. Browser playback needs a manual feel-check when browser access is unavailable.
