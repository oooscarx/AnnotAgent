from __future__ import annotations

import hashlib
import json
import math
import random
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, PngImagePlugin


ROOT = Path(__file__).resolve().parents[1]
IMAGES = ROOT / "images"
THUMBNAILS = ROOT / "thumbnails"
REFERENCES = ROOT / "references"
W, H = 1024, 768
REFERENCE_RECORDS: list[dict] = []

INK = "#20221f"
MUTED = "#666a64"
DESK = "#d8d1c5"
WALL = "#f2f0eb"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rounded(draw: ImageDraw.ImageDraw, box, radius, fill, outline=None, width=1):
    draw.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)


def base(seed: int, wall=WALL, desk=DESK) -> Image.Image:
    random.seed(seed)
    im = Image.new("RGB", (W, H), wall)
    d = ImageDraw.Draw(im)
    d.rectangle((0, 424, W, H), fill=desk)
    d.rectangle((0, 418, W, 426), fill="#bdb5a8")
    # Subtle procedural grain keeps the objects readable without looking like a UI fixture.
    px = im.load()
    for _ in range(28_000):
        x, y = random.randrange(W), random.randrange(H)
        r, g, b = px[x, y]
        delta = random.choice((-2, -1, 1, 2))
        px[x, y] = tuple(max(0, min(255, c + delta)) for c in (r, g, b))
    return im


def shadow(im: Image.Image, box, blur=18, opacity=46):
    layer = Image.new("RGBA", im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    d.ellipse(box, fill=(42, 40, 36, opacity))
    layer = layer.filter(ImageFilter.GaussianBlur(blur))
    im.paste(layer, (0, 0), layer)


def cup(im: Image.Image, x: int, y: int, w: int, h: int, color: str, handle="right") -> tuple[int, int, int, int]:
    d = ImageDraw.Draw(im)
    shadow(im, (x - 18, y + h - 16, x + w + 46, y + h + 34), 14)
    body = (x, y + 20, x + w, y + h)
    rounded(d, body, max(18, w // 9), color, "#323633", 4)
    d.ellipse((x, y + 5, x + w, y + 42), fill="#f5f5f0", outline="#323633", width=4)
    d.ellipse((x + 9, y + 13, x + w - 9, y + 35), fill="#4b3528")
    d.arc((x + 15, y + 16, x + w - 15, y + 36), 200, 342, fill="#d9c1a6", width=3)
    if handle == "right":
        hb = (x + w - 4, y + h // 3, x + w + w // 2, y + h * 3 // 4)
    else:
        hb = (x - w // 2, y + h // 3, x + 4, y + h * 3 // 4)
    d.ellipse(hb, fill=color, outline="#323633", width=4)
    inset = 13
    d.ellipse((hb[0] + inset, hb[1] + inset, hb[2] - inset, hb[3] - inset), fill=WALL, outline="#323633", width=3)
    return (min(x, hb[0]), y + 5, max(x + w, hb[2]), y + h)


def bottle(im: Image.Image, x: int, y: int, w: int, h: int, color: str) -> tuple[int, int, int, int]:
    d = ImageDraw.Draw(im)
    shadow(im, (x - 18, y + h - 12, x + w + 35, y + h + 30), 14)
    neck_w = int(w * 0.45)
    neck_x = x + (w - neck_w) // 2
    d.rectangle((neck_x, y + 30, neck_x + neck_w, y + 105), fill=color, outline="#303431", width=4)
    rounded(d, (x, y + 88, x + w, y + h), w // 4, color, "#303431", 4)
    rounded(d, (neck_x - 5, y + 8, neck_x + neck_w + 5, y + 42), 8, "#d8d5ca", "#303431", 4)
    d.rounded_rectangle((x + 12, y + h * 45 // 100, x + w - 12, y + h * 70 // 100), 10, fill="#f7f7f2", outline="#60645f", width=2)
    d.line((x + 22, y + h * 57 // 100, x + w - 22, y + h * 57 // 100), fill=color, width=8)
    d.line((x + w * 3 // 10, y + 115, x + w * 3 // 10, y + h - 24), fill="#ffffff", width=max(3, w // 16))
    return (x, y + 8, x + w, y + h)


def add_book(im: Image.Image, box, color="#6f7d72"):
    d = ImageDraw.Draw(im)
    x1, y1, x2, y2 = box
    rounded(d, box, 8, color, "#363a36", 3)
    d.line((x1 + 14, y1 + 18, x2 - 16, y1 + 18), fill="#e4e1d7", width=3)
    d.line((x1 + 14, y1 + 34, x2 - 55, y1 + 34), fill="#e4e1d7", width=3)


def add_plant(im: Image.Image, x=780, y=310):
    d = ImageDraw.Draw(im)
    rounded(d, (x, y + 120, x + 130, y + 280), 18, "#b45d3e", "#3c403a", 3)
    for dx, dy, angle in [(65, 126, -60), (58, 112, -25), (74, 105, 20), (48, 88, 60), (84, 78, 92)]:
        length = 120
        ex = x + dx + math.cos(math.radians(angle)) * length
        ey = y + dy + math.sin(math.radians(angle)) * length
        d.line((x + dx, y + dy, ex, ey), fill="#4e6d4c", width=10)
        d.ellipse((ex - 28, ey - 13, ex + 28, ey + 13), fill="#6f8a62", outline="#354b35", width=2)


def save(name: str, im: Image.Image, objects: list[dict], split: str, group: str, note: str):
    IMAGES.mkdir(parents=True, exist_ok=True)
    THUMBNAILS.mkdir(parents=True, exist_ok=True)
    REFERENCES.mkdir(parents=True, exist_ok=True)
    path = IMAGES / f"{name}.png"
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text("AssetType", "synthetic_procedural")
    metadata.add_text("DemoPack", "object-detection-review@1.0.0")
    metadata.add_text("License", "CC0-1.0")
    metadata.add_text("Generator", "tools/build_pack.py")
    im.save(path, optimize=True, pnginfo=metadata)
    thumb = im.copy()
    thumb.thumbnail((384, 288), Image.Resampling.LANCZOS)
    thumbnail_metadata = PngImagePlugin.PngInfo()
    thumbnail_metadata.add_text("AssetType", "synthetic_procedural_thumbnail")
    thumbnail_metadata.add_text("SourceImage", f"images/{name}.png")
    thumbnail_metadata.add_text("DemoPack", "object-detection-review@1.0.0")
    thumbnail_metadata.add_text("License", "CC0-1.0")
    thumb.save(THUMBNAILS / f"{name}.png", optimize=True, pnginfo=thumbnail_metadata)
    record = {
        "schema_version": 1,
        "image": f"images/{name}.png",
        "split": split,
        "source_group": group,
        "source_type": "synthetic_procedural",
        "coordinate_space": {"width": W, "height": H, "orientation": "displayed_pixels"},
        "candidate_source": "pack_author_reference",
        "candidate_use": "preset_experience_and_offline_evaluation_only",
        "forbidden_uses": ["real_model_prompt", "real_model_input", "real_model_cache"],
        "objects": objects,
        "review_note": note,
        "supplier_review": {"status": "checked", "reviewer": "AnnotAgent product asset review", "date": "2026-09-12"},
    }
    REFERENCE_RECORDS.append(record)


def obj(identifier, label, bbox, *, candidate=None, state="reference_checked", issue=None):
    item = {"id": identifier, "label": label, "bbox_xyxy": list(bbox), "reference_state": state}
    if candidate is not None:
        item["preset_candidate_bbox_xyxy"] = list(candidate)
    if issue:
        item["review_issue"] = issue
    return item


def build():
    REFERENCE_RECORDS.clear()
    im = base(101, wall="#eee9df", desk="#d5c9b8")
    add_book(im, (95, 500, 360, 620), "#6d7b85")
    b = cup(im, 410, 315, 230, 300, "#56768c")
    save("desk_01", im, [obj("cup-01", "cup", b)], "train", "scene-01", "Large single cup with a clearly visible handle.")

    im = base(202, wall="#e8ece7", desk="#cfc5b8")
    b = bottle(im, 405, 210, 190, 420, "#6d8b78")
    add_book(im, (675, 510, 910, 630), "#8c6c58")
    save("desk_02", im, [obj("bottle-01", "bottle", b)], "train", "scene-02", "Large single bottle with cap and full body visible.")

    im = base(303, wall="#f0ede6", desk="#c9c1b5")
    c = cup(im, 115, 350, 190, 245, "#b96f4f", "left")
    b1 = bottle(im, 465, 215, 150, 370, "#54798a")
    b2 = bottle(im, 720, 295, 125, 300, "#8a7654")
    save("desk_03", im, [obj("cup-02", "cup", c), obj("bottle-02", "bottle", b1), obj("bottle-03", "bottle", b2)], "train", "scene-03", "Multi-object scene with both classes and distinct scales.")

    im = base(404, wall="#ebeae6", desk="#d1c8bb")
    add_book(im, (110, 500, 435, 635), "#536775")
    add_book(im, (240, 440, 570, 530), "#9b7555")
    add_plant(im, 735, 285)
    save("desk_04", im, [], "train", "scene-04", "Clear negative image: books and a plant, with no cup or bottle.")

    im = base(505, wall="#ece8e1", desk="#c8beb0")
    c = cup(im, 175, 350, 180, 240, "#708d64")
    b = bottle(im, 650, 255, 145, 345, "#775c83")
    save("desk_05", im, [obj("cup-03", "cup", c), obj("bottle-04", "bottle", b)], "validation", "scene-05", "Validation scene covers both classes with clear separation.")

    im = base(606, wall="#eeeae2", desk="#d1c5b5")
    c = cup(im, 330, 325, 230, 285, "#657c91")
    # Deliberately strong cast shadow makes a plausible loose candidate easy to correct.
    overlay = Image.new("RGBA", im.size, (0, 0, 0, 0))
    od = ImageDraw.Draw(overlay)
    od.ellipse((520, 560, 820, 655), fill=(55, 50, 44, 50))
    overlay = overlay.filter(ImageFilter.GaussianBlur(18))
    im.paste(overlay, (0, 0), overlay)
    b = bottle(im, 700, 295, 120, 300, "#9a694b")
    loose = (c[0], c[1], 820, 655)
    save("desk_06", im, [obj("cup-04", "cup", c, candidate=loose, state="needs_user_review", issue="Preset candidate includes the cup's cast shadow and neighboring bottle area; tighten it to the physical cup and handle."), obj("bottle-05", "bottle", b)], "validation", "scene-06", "Boundary teaching case: the loose cup candidate includes its shadow and neighboring bottle area; the bottle reference is straightforward.")

    candidates = {
        "format": "annotagent.demo-candidates",
        "version": 1,
        "demo_id": "object-detection-review",
        "demo_version": "1.0.0",
        "images": [],
    }
    evaluation = {
        "format": "annotagent.demo-reference-evaluation",
        "version": 1,
        "demo_id": "object-detection-review",
        "demo_version": "1.0.0",
        "usage": "offline_evaluation_only",
        "forbidden_uses": ["live_model_prompt", "live_model_input", "live_model_cache"],
        "images": [],
    }
    for index, record in enumerate(REFERENCE_RECORDS, 1):
        image_id = f"image-{index:02d}"
        source_name = Path(record["image"]).name
        image_path = IMAGES / source_name
        items = []
        for item in record["objects"]:
            x1, y1, x2, y2 = item.get("preset_candidate_bbox_xyxy", item["bbox_xyxy"])
            items.append({
                "id": f"preset-{image_id}-{item['id']}",
                "label_id": item["label"],
                "annotation_kind": "bounding_box",
                "value": {
                    "x": round(x1 / W, 6),
                    "y": round(y1 / H, 6),
                    "width": round((x2 - x1) / W, 6),
                    "height": round((y2 - y1) / H, 6),
                },
                "score": None,
                "source_artifact_id": f"preset-artifact-{image_id}-{item['id']}",
            })
        candidates["images"].append({
            "image_asset_id": image_id,
            "image_sha256": sha256(image_path),
            "candidates": items,
        })
        expected = []
        for item in record["objects"]:
            x1, y1, x2, y2 = item["bbox_xyxy"]
            expected.append({
                "id": item["id"],
                "label_id": item["label"],
                "value": {
                    "x": round(x1 / W, 6),
                    "y": round(y1 / H, 6),
                    "width": round((x2 - x1) / W, 6),
                    "height": round((y2 - y1) / H, 6),
                },
                "reference_state": item["reference_state"],
                "review_issue": item.get("review_issue"),
            })
        evaluation["images"].append({
            "image_asset_id": image_id,
            "image_sha256": sha256(image_path),
            "split": record["split"],
            "source_group": record["source_group"],
            "expected_objects": expected,
            "review_note": record["review_note"],
        })
    candidate_path = REFERENCES / "candidates.json"
    candidate_path.write_text(json.dumps(candidates, ensure_ascii=False, indent=2) + "\n")
    evaluation_path = REFERENCES / "evaluation.json"
    evaluation_path.write_text(json.dumps(evaluation, ensure_ascii=False, indent=2) + "\n")

    cover = Image.new("RGB", (640, 360), "#f7f7f4")
    for index, thumb_path in enumerate(sorted(THUMBNAILS.glob("desk_*.png"))):
        with Image.open(thumb_path) as source:
            tile = source.convert("RGB")
            tile.thumbnail((196, 147), Image.Resampling.LANCZOS)
            x = 16 + (index % 3) * 208
            y = 24 + (index // 3) * 168
            cover.paste(tile, (x, y))
    cover_metadata = PngImagePlugin.PngInfo()
    cover_metadata.add_text("AssetType", "synthetic_procedural_contact_sheet")
    cover_metadata.add_text("DemoPack", "object-detection-review@1.0.0")
    cover_metadata.add_text("License", "CC0-1.0")
    cover.save(ROOT / "thumbnail.png", optimize=True, pnginfo=cover_metadata)

    def asset(asset_id: str, kind: str, relative: str, width=None, height=None, *, split=None, group=None):
        path = ROOT / relative
        mime_type = {
            ".json": "application/json",
            ".md": "text/markdown",
            ".png": "image/png",
        }[path.suffix]
        value = {
            "id": asset_id,
            "kind": kind,
            "path": relative,
            "sha256": sha256(path),
            "mime_type": mime_type,
            "bytes": path.stat().st_size,
            "width": width,
            "height": height,
        }
        if kind == "image":
            value["split"] = split
            value["source_group"] = group
        return value

    assets = [asset("thumbnail", "thumbnail", "thumbnail.png", 640, 360)]
    for index in range(1, 7):
        name = f"desk_{index:02d}.png"
        assets.append(asset(
            f"image-{index:02d}",
            "image",
            f"images/{name}",
            W,
            H,
            split="train" if index <= 4 else "validation",
            group=f"scene-{index:02d}",
        ))
        assets.append(asset(
            f"thumbnail-image-{index:02d}",
            "thumbnail",
            f"thumbnails/{name}",
            384,
            288,
        ))
    assets.append(asset("attribution", "attribution", "ATTRIBUTION.md"))
    assets.append(asset("preset-candidates", "preset_candidates", "references/candidates.json"))
    manifest = {
        "schema_version": 1,
        "demo_id": "object-detection-review",
        "version": "1.0.0",
        "display": {
            "title": "标注桌面物品",
            "summary": "6 张原创合成图片 · cup / bottle · YOLO 检测数据包",
            "learning_objectives": [
                "区分本次模型结果与预置候选",
                "检查边界并完成逐图审核后再打包",
            ],
        },
        "project": {
            "display_name": "桌面物品检测示例",
            "goal": "请标出这 6 张图片里的杯子和瓶子，准备 Ultralytics YOLO 检测数据。边界不确定时让我确认，审核齐全后打包。",
        },
        "assets": assets,
        "task": {
            "source_image_asset_ids": [f"image-{index:02d}" for index in range(1, 7)],
            "labels": [
                {"id": "cup", "name": "cup", "annotation_kind": "bounding_box", "color": "#355E8C"},
                {"id": "bottle", "name": "bottle", "annotation_kind": "bounding_box", "color": "#286448"},
            ],
            "training_target": {
                "framework": "ultralytics",
                "task": "ultralytics_yolo_detection",
                "format_revision": 1,
            },
            "split_policy": {"train": 0.666667, "validation": 0.333333, "test": 0.0, "seed": 42},
            "review_policy": "human_whole_image",
        },
        "modes": {
            "preset_candidates": {"enabled": True, "asset_id": "preset-candidates"},
            "live_model": {"enabled": True, "required_capabilities": ["vision_language"]},
        },
        "license": {
            "spdx_id": "CC0-1.0",
            "source_url": "https://creativecommons.org/publicdomain/zero/1.0/legalcode",
            "attribution_asset_id": "attribution",
        },
    }
    (ROOT / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")

    checks = []
    for image in sorted(IMAGES.glob("*.png")):
        with Image.open(image) as loaded:
            loaded.verify()
        checks.append({
            "file": f"images/{image.name}",
            "sha256": sha256(image),
            "width": W,
            "height": H,
            "thumbnail": f"thumbnails/{image.name}",
            "thumbnail_sha256": sha256(THUMBNAILS / image.name),
        })
    (ROOT / "asset-checks.json").write_text(json.dumps({
        "pack": "object-detection-review@1.0.0",
        "source_type": "synthetic_procedural",
        "license": "CC0-1.0",
        "thumbnail": {"file": "thumbnail.png", "sha256": sha256(ROOT / "thumbnail.png")},
        "preset_candidates": {"file": "references/candidates.json", "sha256": sha256(candidate_path)},
        "offline_evaluation": {"file": "references/evaluation.json", "sha256": sha256(evaluation_path)},
        "images": checks,
    }, indent=2) + "\n")


if __name__ == "__main__":
    build()
