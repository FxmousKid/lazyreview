#!/usr/bin/env python3
"""Render the native-frame tour to a silent MP4, without a browser or network.

uv run --offline --with pillow python demo/render_video.py \
  --frames demo/frames.json --output demo/lazyreview-demo.mp4

Only the actual escaped Ratatui cell markup in frames.json becomes terminal
footage. Captions, camera movement and key indicators are an explanatory layer.
Frames stream directly to ffmpeg; no image sequence is written to disk.
"""
from __future__ import annotations

import argparse
from collections import OrderedDict
from html.parser import HTMLParser
import json
import math
from pathlib import Path
import shutil
import subprocess
import unicodedata

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
FONT_ROOT = Path.home() / "Library/Fonts"
MONO = FONT_ROOT / "JetBrainsMono-Regular.ttf"
MONO_BOLD = FONT_ROOT / "JetBrainsMono-Bold.ttf"
NERD = FONT_ROOT / "JetBrainsMonoNerdFontMono-Regular.ttf"
SANS = Path("/System/Library/Fonts/SFNS.ttf")
SYMBOL_FONT = Path("/System/Library/Fonts/Apple Symbols.ttf")
UNICODE_FONT = Path("/System/Library/Fonts/Supplemental/Arial Unicode.ttf")
BG = "#090c10"
SURFACE = "#0d1117"
TEXT = "#e6edf3"
MUTED = "#93a2b7"
ACCENT = "#a5c9ff"
WIDTH, HEIGHT = 1920, 1080
STAGE = (80, 234, 1760, 676)
FONT_SIZE, LINE_HEIGHT = 20, 26


class NativeMarkup(HTMLParser):
    """Parse the deliberately tiny exporter markup; never load external assets."""
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.style = {"color": TEXT, "background": SURFACE, "font-weight": "400"}
        self.stack = []
        self.runs = []

    def handle_starttag(self, tag, attrs):
        if tag == "pre" and not attrs:
            return
        if tag != "span" or len(attrs) != 1 or attrs[0][0] != "style":
            raise ValueError("Only native pre/span markup is supported")
        self.stack.append(self.style.copy())
        style = self.style.copy()
        for declaration in attrs[0][1].split(";"):
            if not declaration:
                continue
            key, value = declaration.split(":", 1)
            if key not in {"color", "background", "font-weight", "font-style", "text-decoration"}:
                raise ValueError(f"Unexpected terminal style: {key}")
            if key in {"color", "background"}:
                if len(value) != 7 or value[0] != "#":
                    raise ValueError("Terminal colors must be six-digit RGB")
                int(value[1:], 16)
            style[key] = value
        self.style = style

    def handle_endtag(self, tag):
        if tag == "span":
            if not self.stack:
                raise ValueError("Unbalanced terminal span")
            self.style = self.stack.pop()
        elif tag != "pre":
            raise ValueError("Unexpected terminal tag")

    def handle_data(self, data):
        self.runs.append((data, self.style.copy()))


def char_width(char):
    if unicodedata.combining(char) or char in "\ufe0e\ufe0f\u200d":
        return 0
    return 2 if unicodedata.east_asian_width(char) in {"W", "F"} else 1


def rgb(value):
    return tuple(int(value[i:i + 2], 16) for i in (1, 3, 5))


def font(path, size):
    if not path.exists():
        raise FileNotFoundError(f"Font unavailable: {path}")
    return ImageFont.truetype(str(path), size)


def wrap(text, face, width):
    lines = []
    for paragraph in text.splitlines() or [""]:
        line = ""
        for word in paragraph.split():
            candidate = f"{line} {word}" if line else word
            if line and face.getlength(candidate) > width:
                lines.append(line)
                line = word
            else:
                line = candidate
        lines.append(line)
    return lines


def text(draw, xy, content, face, fill=TEXT, anchor=None):
    draw.text(xy, content, font=face, fill=fill, anchor=anchor, features=["-kern", "-liga"])


def roundrect(draw, box, fill, outline=None, radius=7, width=1):
    draw.rounded_rectangle(box, radius, fill=fill, outline=outline, width=width)


def ease_in_out(value):
    """Exact CSS cubic-bezier(.77, 0, .175, 1), inverted in x."""
    if value <= 0:
        return 0.0
    if value >= 1:
        return 1.0
    lo, hi = 0.0, 1.0
    for _ in range(24):
        t = (lo + hi) / 2
        x = 3 * (1 - t) ** 2 * t * .77 + 3 * (1 - t) * t * t * .175 + t ** 3
        if x < value:
            lo = t
        else:
            hi = t
    return 3 * (1 - t) * t * t + t ** 3


def ease_out(value):
    """Exact CSS cubic-bezier(.23, 1, .32, 1), inverted in x."""
    if value <= 0:
        return 0.0
    if value >= 1:
        return 1.0
    lo, hi = 0.0, 1.0
    for _ in range(24):
        t = (lo + hi) / 2
        x = 3 * (1 - t) ** 2 * t * .23 + 3 * (1 - t) * t * t * .32 + t ** 3
        if x < value:
            lo = t
        else:
            hi = t
    return 3 * (1 - t) ** 2 * t + 3 * (1 - t) * t * t + t ** 3


def camera(frame, cols, rows, native_width, native_height):
    width, height = STAGE[2:]
    scale = min(width / native_width, height / native_height)
    x, y = (width - native_width * scale) / 2, (height - native_height * scale) / 2
    focus = frame.get("focus")
    if focus:
        fx, fy, fw, fh = focus
        cell, line = native_width / cols, native_height / rows
        zoom = max(scale, min(width / ((fw + 6) * cell), height / ((fh + 3) * line), scale * 2.6))
        x = width / 2 - (fx + fw / 2) * cell * zoom
        y = height / 2 - (fy + fh / 2) * line * zoom
        x = max(width - native_width * zoom, min(0, x))
        y = (height - native_height * zoom) / 2 if native_height * zoom <= height else max(height - native_height * zoom, min(0, y))
        scale = zoom
        if fy <= 2:
            y = 0  # Keep the native title/pane edge whole, instead of clipping half a row.
    return x, y, scale


class Renderer:
    def __init__(self, data):
        self.data = data
        self.mono = font(MONO, FONT_SIZE)
        self.bold = font(MONO_BOLD, FONT_SIZE)
        self.nerd = font(NERD, FONT_SIZE)
        self.symbols = font(SYMBOL_FONT, FONT_SIZE)
        self.unicode = font(UNICODE_FONT, FONT_SIZE)
        missing_symbol = self.symbols.getmask("\uffff")
        self.missing_symbol = (missing_symbol.size, bytes(missing_symbol))
        missing_mask = self.nerd.getmask("\uffff")
        self.missing_glyph = (missing_mask.size, bytes(missing_mask))
        self.glyph_fallback = {}
        self.ui = {size: font(SANS, size) for size in (12, 14, 16, 18, 20, 21, 23, 26, 28, 38, 48, 62)}
        self.keyfont = font(MONO, 48)
        self.cell = round(self.mono.getlength("M"))
        self.native_width, self.native_height = self.cell * data["cols"], LINE_HEIGHT * data["rows"]
        self.cache = OrderedDict()
        self.chrome = self.base_chrome()

    def base_chrome(self):
        image = Image.new("RGB", (WIDTH, HEIGHT), BG)
        draw = ImageDraw.Draw(image)
        text(draw, (80, 59), "lazyreview", self.ui[18], "#a9b4c3")
        text(draw, (1840, 59), "Two workflows. One PR.", self.ui[14], "#718094", "ra")
        roundrect(draw, (79, 201, 1841, 941), "#12171e", "#323c4b", 11)
        draw.rectangle((80, 233, 1840, 911), fill=SURFACE)
        for x, color in [(100, "#97636c"), (115, "#9b8a63"), (130, "#628e7b")]:
            draw.ellipse((x, 214, x + 7, 221), fill=color)
        text(draw, (960, 218), "review", self.ui[12], "#7f8b9d", "mm")
        text(draw, (1819, 218), f'{self.data["cols"]} × {self.data["rows"]}', self.ui[12], "#7f8b9d", "rm")
        draw.line((80, 233, 1840, 233), "#242d3b")
        draw.line((80, 911, 1840, 911), "#242d3b")
        text(draw, (100, 926), "Native terminal · sample PR · scripted reply", self.ui[12], "#71849e", "lm")
        text(draw, (1820, 926), "Paper companion", self.ui[12], "#71849e", "rm")
        return image

    def native(self, index):
        if index in self.cache:
            self.cache.move_to_end(index)
            return self.cache[index]
        parser = NativeMarkup()
        parser.feed(self.data["frames"][index]["html"])
        if parser.stack:
            raise ValueError("Unbalanced terminal markup")
        image = Image.new("RGB", (self.native_width, self.native_height), SURFACE)
        draw = ImageDraw.Draw(image)
        col = row = 0
        for content, style in parser.runs:
            for char in content:
                if char == "\n":
                    col = 0
                    row += 1
                    continue
                width = char_width(char)
                if row >= self.data["rows"]:
                    break
                if col < self.data["cols"]:
                    x, y = col * self.cell, row * LINE_HEIGHT
                    if width:
                        draw.rectangle((x, y, x + self.cell * width - 1, y + LINE_HEIGHT - 1), fill=style["background"])
                    face = self.nerd if ord(char) > 127 else self.bold if style.get("font-weight") == "700" else self.mono
                    # A fixed cell origin preserves alignment and terminal gutters.
                    if ord(char) > 127:
                        if char not in self.glyph_fallback:
                            glyph_mask = self.nerd.getmask(char)
                            self.glyph_fallback[char] = (glyph_mask.size, bytes(glyph_mask)) == self.missing_glyph
                        if self.glyph_fallback[char]:
                            symbol_mask = self.symbols.getmask(char)
                            fallback = self.unicode if (symbol_mask.size, bytes(symbol_mask)) == self.missing_symbol else self.symbols
                            bounds = fallback.getbbox(char)
                            glyph_width, glyph_height = bounds[2] - bounds[0], bounds[3] - bounds[1]
                            text(draw, (x + (self.cell * max(1, width) - glyph_width) / 2 - bounds[0], y + (LINE_HEIGHT - glyph_height) / 2 - bounds[1]), char, fallback, style["color"])
                        else:
                            text(draw, (x, y), char, face, style["color"])
                    else:
                        text(draw, (x, y), char, face, style["color"])
                    if style.get("text-decoration") == "underline":
                        draw.line((x, y + 24, x + width * self.cell, y + 24), style["color"])
                col += width
        self.cache[index] = image
        while len(self.cache) > 4:
            self.cache.popitem(last=False)
        return image

    def card(self, frame):
        image = Image.new("RGB", (WIDTH, HEIGHT), BG)
        draw = ImageDraw.Draw(image)
        text(draw, (80, 59), "lazyreview", self.ui[18], "#a9b4c3")
        text(draw, (WIDTH / 2, 392), frame["chapter"], self.ui[20], ACCENT, "mm")
        lines = wrap(frame["title"], self.ui[62], 1450)
        top = 480 - (len(lines) - 1) * 38
        for n, line in enumerate(lines):
            text(draw, (WIDTH / 2, top + n * 76), line, self.ui[62], TEXT, "mm")
        text(draw, (WIDTH / 2, top + (len(lines) - 1) * 76 + 95), frame["caption"], self.ui[28], MUTED, "mm")
        text(draw, (WIDTH / 2, 1018), "Sample PR · scripted reply · no narration", self.ui[14], "#566477", "mm")
        return image

    def render(self, index, within, position, total):
        frame = self.data["frames"][index]
        prior = self.data["frames"][max(0, index - 1)]
        if frame.get("kind") == "card":
            image = self.card(frame)
            # Chapter cards fade in from quiet darkness; no moving terminal text.
            if within < 500:
                fraction = ease_out(within / 500)
                image = Image.blend(Image.new("RGB", image.size, BG), image, fraction)
            return image
        image = self.chrome.copy()
        draw = ImageDraw.Draw(image)
        text(draw, (WIDTH / 2, 128), frame["title"], self.ui[38], TEXT, "mm")
        text(draw, (WIDTH / 2, 174), frame["caption"], self.ui[23], MUTED, "mm")
        native = self.native(index)
        target = camera(frame, self.data["cols"], self.data["rows"], self.native_width, self.native_height)
        previous = target if prior.get("kind") == "card" else camera(prior, self.data["cols"], self.data["rows"], self.native_width, self.native_height)
        fraction = ease_in_out(min(1, within / 720))
        x, y, scale = tuple(a + (b - a) * fraction for a, b in zip(previous, target))
        scene = native.transform(STAGE[2:], Image.Transform.AFFINE, (1 / scale, 0, -x / scale, 0, 1 / scale, -y / scale), resample=Image.Resampling.BICUBIC, fillcolor=rgb(SURFACE))
        image.paste(scene, STAGE[:2])
        draw.line((80, 982, 1840, 982), fill="#263140", width=2)
        draw.line((80, 982, 80 + (position / total) * 1760, 982), fill=ACCENT, width=2)
        text(draw, (80, 1015), frame["chapter"], self.ui[14], "#7c8da3", "lm")
        text(draw, (1840, 1015), f"{int(position / 60000)}:{int(position / 1000) % 60:02d} / {int(total / 60000)}:{int(total / 1000) % 60:02d}", self.ui[14], "#738aa8", "rm")
        cast = frame["keycast"]
        recent = [event for event in cast["events"] if event["at"] <= within]
        if recent and within < cast["end"]:
            alpha = min(ease_out((within - recent[0]["at"]) / 200), ease_out((cast["end"] - within) / cast["fade"]))
            overlay = Image.new("RGBA", image.size)
            pen = ImageDraw.Draw(overlay)
            widths = [self.keyfont.getlength(event["label"]) + 24 for event in recent]
            width = sum(widths) + 36
            x = (WIDTH - width) / 2
            y = STAGE[1] + STAGE[3] * .73 + 5 * (1 - alpha)
            roundrect(pen, (x, y, x + width, y + 88), (0, 0, 0, 220), radius=14)
            left = x + 18
            for event, key_width in zip(recent, widths):
                age = within - event["at"]
                color = "#ffffff" if age < 200 else "#e6edf3"
                text(pen, (left + key_width / 2, y + 44 + 3 * (1 - ease_out(age / 160))), event["label"], self.keyfont, color, "mm")
                left += key_width
            overlay.putalpha(overlay.getchannel("A").point(lambda value: int(value * alpha)))
            image.paste(overlay, (0, 0), overlay)
        if prior.get("kind") == "card" and within < 500:
            image = Image.blend(self.card(prior), image, ease_out(within / 500))
        return image


def validate(data):
    if data.get("version") != 1 or not 40 <= data.get("cols", 0) <= 300 or not 15 <= data.get("rows", 0) <= 100:
        raise ValueError("Invalid native terminal frame dimensions/version")
    frames = data.get("frames", [])
    if not frames or len(frames) > 100:
        raise ValueError("Expected 1–100 native terminal frames")
    ids = set()
    for frame in frames:
        if frame["id"] in ids:
            raise ValueError("Duplicate frame id")
        ids.add(frame["id"])
        if not 500 <= frame["duration"] <= 30000:
            raise ValueError("Invalid frame duration")
        focus = frame.get("focus")
        if focus and (len(focus) != 4 or any(not math.isfinite(n) or n < 0 for n in focus) or focus[2] < 1 or focus[3] < 1 or focus[0] + focus[2] > data["cols"] or focus[1] + focus[3] > data["rows"]):
            raise ValueError("Focus rectangle exceeds native terminal bounds")
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frames", type=Path, default=HERE / "frames.json")
    parser.add_argument("--output", type=Path, default=HERE / "lazyreview-demo.mp4")
    parser.add_argument("--fps", type=int, default=15)
    parser.add_argument("--contact-sheet", type=Path)
    parser.add_argument("--stills-only", action="store_true", help="Inspect recorded frames without encoding video")
    args = parser.parse_args()
    if not 1 <= args.fps <= 60:
        parser.error("FPS must be between 1 and 60")
    # Build-time compiler is shared with HTML, including key labels and timing.
    # No browser, key logger, provider process, or network is involved.
    compiled = subprocess.run(["node", str(HERE / "build.mjs"), "--timeline", str(args.frames.resolve())], check=True, capture_output=True, text=True)
    data = validate(json.loads(compiled.stdout))
    renderer = Renderer(data)
    starts, total = [], 0
    for frame in data["frames"]:
        starts.append(total)
        total += frame["duration"]
    sheet_path = args.contact_sheet or args.output.with_suffix(".contact-sheet.jpg")
    count = len(data["frames"])
    sheet = Image.new("RGB", (960 * 2, 540 * math.ceil(count / 2)), BG)
    for i, frame in enumerate(data["frames"]):
        within = min(frame["duration"] - 1, 1200 if frame["keycast"]["events"] else max(1200, frame["duration"] / 2))
        still = renderer.render(i, within, starts[i] + within, total)
        sheet.paste(still.resize((960, 540), Image.Resampling.LANCZOS), ((i % 2) * 960, (i // 2) * 540))
    sheet.save(sheet_path, quality=94)
    print(json.dumps({"contact_sheet": str(sheet_path), "scenes": count, "duration_seconds": total / 1000}), flush=True)
    if args.stills_only:
        return
    ffmpeg = shutil.which("ffmpeg") or "/opt/homebrew/bin/ffmpeg"
    command = [ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{WIDTH}x{HEIGHT}", "-r", str(args.fps), "-i", "pipe:0", "-an", "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-metadata", "title=Review your PRs — LazyReview", "-metadata", "comment=Silent native terminal tour. Sample PR and scripted conversation.", str(args.output)]
    process = subprocess.Popen(command, stdin=subprocess.PIPE)
    frame_count = math.ceil(total * args.fps / 1000)
    scene = 0
    try:
        for n in range(frame_count):
            position = n * 1000 / args.fps
            while scene + 1 < count and position >= starts[scene + 1]:
                scene += 1
                print(json.dumps({"scene": scene + 1, "id": data["frames"][scene]["id"], "seconds": round(position / 1000, 2)}), flush=True)
            video_frame = renderer.render(scene, position - starts[scene], position, total)
            process.stdin.write(video_frame.tobytes())
        process.stdin.close()
        result = process.wait()
        if result:
            raise RuntimeError(f"ffmpeg exited {result}")
    except BaseException:
        process.terminate()
        process.wait()
        raise
    print(json.dumps({"output": str(args.output), "video_frames": frame_count, "fps": args.fps, "duration_seconds": frame_count / args.fps, "bytes": args.output.stat().st_size, "audio": False}), flush=True)


if __name__ == "__main__":
    main()
