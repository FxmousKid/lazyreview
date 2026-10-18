"""Crop genuine Ratatui exports for the showcase; requires the demo renderer's fonts/Pillow."""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'demo'))
from render_video import Renderer, LINE_HEIGHT

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--review', type=Path, required=True, help='export_review_flow_preview JSON')
args = parser.parse_args()
out = ROOT / 'site/assets'
data = json.loads((ROOT / 'demo/frames.json').read_text())
renderer = Renderer(data)

def save(image, name, width):
    if image.width > width:
        image = image.resize((width, round(image.height * width / image.width)))
    image.save(out / name, quality=92, method=6)

save(renderer.native(5), 'workspace.webp', 1800)
save(renderer.native(1).crop((0, 2*LINE_HEIGHT, 56*renderer.cell, 27*LINE_HEIGHT)), 'files.webp', 800)
save(renderer.native(11).crop((47*renderer.cell, 13*LINE_HEIGHT, 133*renderer.cell, 31*LINE_HEIGHT)), 'context.webp', 1200)
review = json.loads(args.review.read_text())[1]
review_renderer = Renderer({'cols': review['width'], 'rows': review['height'], 'frames': [review]})
save(review_renderer.native(0).crop((0, 0, review_renderer.native_width, 18*LINE_HEIGHT)), 'review.webp', 1200)
