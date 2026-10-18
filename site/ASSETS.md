# Asset provenance

All product pictures come from the actual Ratatui renderer with synthetic PR data. No customer code, private PR or live Claude transcript is pictured.

- `workspace.webp`, `files.webp`, `context.webp`: frames 5, 1 and 11 of `demo/frames.json`, cropped in `render-stills.py`.
- `review.webp`: the expanded synthetic review from `ui::tests::export_review_flow_preview`, cropped to the comment list.
- `tour.mp4` in the built site: `demo/lazyreview-demo.mp4`, copied unchanged. See [the demo guide](../demo/README.md) for its timeline, renderer and scripted-reply disclosure.
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

The shared video renderer currently expects local macOS JetBrains Mono and system fonts; see its font constants before rendering on another platform. Building or hosting the website uses the committed stills and does not require Python or those fonts.
