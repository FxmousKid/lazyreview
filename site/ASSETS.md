# Asset provenance

All product pictures come from the actual Ratatui renderer with synthetic PR data. No customer code, private PR or live Claude transcript is pictured.

- `workspace.webp`, `files.webp`, `context.webp`: frames 5, 1 and 11 of `demo/frames.json`, cropped in `render-stills.py`.
- `review.webp`: the expanded synthetic review from `ui::tests::export_review_flow_preview`, cropped to the comment list.
- `/demo/tour/`: lossless native frame images and the shared tour player. Image/source/profile hashes are checked during every site build.
- `tour.mp4` in the built site: `demo/lazyreview-demo.mp4`, copied unchanged. See [the demo guide](../demo/README.md) for its timeline, renderer and scripted-reply disclosure.
- `/demo/`: live Ratatui cell updates painted with the same pinned JetBrainsMono Nerd Font files and display profile. This is interactive sample state, not video.
- Bricolage Grotesque: [Google Fonts source](https://github.com/google/fonts/tree/main/ofl/bricolagegrotesque), under the included `assets/Bricolage-OFL.txt` (SIL Open Font License).
- The small LazyReview mark is project-authored SVG.

Regenerate stills after changing the native interface:

```sh
mkdir -p outputs
LAZYREVIEW_REVIEW_PREVIEW="$PWD/outputs/review.json" \
  cargo test --locked --manifest-path tui/Cargo.toml export_review_flow_preview -- --ignored
uv run --with pillow python site/render-stills.py --review outputs/review.json
npm run site:build
```

The shared renderer uses the exact bundled JetBrainsMono Nerd Font files from the local Alacritty setup, plus macOS symbol fallbacks. The sanitized colors, font hashes and metrics live in `demo/terminal-profile.json`. See `demo/README.md` for the portable backdrop and rasterization limits. Building or hosting the website uses the committed stills and does not require Python or those fonts.
