from __future__ import annotations

import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parents[3]
PACK = ROOT / "examples/demo-packs/object-detection-review/1.0.0"
OUT = Path(__file__).with_name("reference-candidates.png")
FONT = "/System/Library/Fonts/Helvetica.ttc"


def font(size: int, bold: bool = False):
    return ImageFont.truetype(FONT, size, index=1 if bold else 0)


data = json.loads((PACK / "references/candidates.json").read_text())
canvas = Image.new("RGB", (1600, 1000), "#F7F7F4")
draw = ImageDraw.Draw(canvas)
draw.text((60, 44), "SYNTHETIC PRESET CANDIDATES", fill="#20221F", font=font(40, True))
draw.text((60, 102), "No model inference · supplier candidates still require user review", fill="#666A64", font=font(24))

colors = {"cup": "#355E8C", "bottle": "#286448"}
for index, record in enumerate(data["images"]):
    col, row = index % 3, index // 3
    x0, y0 = 60 + col * 510, 170 + row * 390
    source = Image.open(PACK / f"images/desk_{index + 1:02d}.png").convert("RGB")
    source.thumbnail((460, 345), Image.Resampling.LANCZOS)
    canvas.paste(source, (x0, y0))
    draw.rectangle((x0 - 1, y0 - 1, x0 + 461, y0 + 346), outline="#DADDD5", width=2)
    sx, sy = source.width / 1024, source.height / 768
    for candidate in record["candidates"]:
        value = candidate["value"]
        left = x0 + value["x"] * 1024 * sx
        top = y0 + value["y"] * 768 * sy
        right = left + value["width"] * 1024 * sx
        bottom = top + value["height"] * 768 * sy
        color = colors[candidate["label_id"]]
        draw.rectangle((left, top, right, bottom), outline=color, width=4)
        draw.rectangle((left, max(y0, top - 28), left + 94, top), fill=color)
        draw.text((left + 7, max(y0 + 2, top - 25)), candidate["label_id"], fill="white", font=font(18, True))
    footer = f"image-{index + 1:02d} · " + ("no candidates" if not record["candidates"] else f"{len(record['candidates'])} candidate(s)")
    if index == 5:
        footer += " · cup boundary intentionally loose"
    draw.text((x0, y0 + 352), footer, fill="#666A64", font=font(19))

canvas.save(OUT, optimize=True)
