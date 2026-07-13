#!/usr/bin/env python3
"""Create deterministic, labeled dense QR raster scenes for WP-012.

The scenes deliberately use ordinary Model 2 version-1, EC-M QR symbols on a
flat white raster.  They are an end-to-end density baseline, not a substitute
for photographs or the BoofCV corpus.  Each same-stem ``.txt`` file uses the
strict BoofCV ``SETS`` quadrilateral layout consumed by ``qrtool reading-rate``
and therefore exercises its max-cardinality bipartite geometry matcher.

Requires python-qrcode and Pillow.  The generator records the installed
qrcode version in its manifest so a regenerated corpus is attributable.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import math
from pathlib import Path

import qrcode
from PIL import Image


COUNTS = (1, 2, 5, 10, 25, 50, 100)
BOX_SIZE = 5
BORDER = 4
GAP = 12
MARGIN = 16
VERSION = 1
ERROR_CORRECTION = qrcode.constants.ERROR_CORRECT_M


def scene_layout(count: int) -> tuple[int, int]:
    """Return the deterministic near-square grid for a scene."""
    columns = math.ceil(math.sqrt(count))
    return columns, math.ceil(count / columns)


def qr_image(payload: str) -> Image.Image:
    qr = qrcode.QRCode(
        version=VERSION,
        error_correction=ERROR_CORRECTION,
        box_size=BOX_SIZE,
        border=BORDER,
    )
    qr.add_data(payload)
    qr.make(fit=False)
    return qr.make_image(fill_color="black", back_color="white").convert("L")


def write_scene(root: Path, count: int) -> dict[str, object]:
    columns, rows = scene_layout(count)
    exemplar = qr_image("WP012-000")
    symbol_width, symbol_height = exemplar.size
    width = 2 * MARGIN + columns * symbol_width + (columns - 1) * GAP
    height = 2 * MARGIN + rows * symbol_height + (rows - 1) * GAP
    image = Image.new("L", (width, height), color=255)
    labels: list[tuple[int, int, int, int]] = []
    payloads: list[str] = []
    code_width = (17 + 4 * VERSION) * BOX_SIZE

    for index in range(count):
        column = index % columns
        row = index // columns
        x = MARGIN + column * (symbol_width + GAP)
        y = MARGIN + row * (symbol_height + GAP)
        payload = f"WP012-{index:03d}"
        image.paste(qr_image(payload), (x, y))
        # Exclude the quiet zone: QR detector positions describe the module
        # square rather than the surrounding whitespace.
        code_x = x + BORDER * BOX_SIZE
        code_y = y + BORDER * BOX_SIZE
        labels.append((code_x, code_y, code_x + code_width, code_y + code_width))
        payloads.append(payload)

    stem = f"density_{count:03d}"
    image_path = root / f"{stem}.png"
    label_path = root / f"{stem}.txt"
    image.save(image_path, optimize=False)
    label_path.write_text(
        "# WP-012 controlled raster scene; quadrilaterals exclude quiet zones\n"
        "SETS\n"
        + "".join(f"{x0} {y0} {x1} {y0} {x1} {y1} {x0} {y1}\n" for x0, y0, x1, y1 in labels),
        encoding="utf-8",
    )
    return {
        "id": stem,
        "symbols": count,
        "grid": [columns, rows],
        "dimensions": [width, height],
        "payloads": payloads,
        "image_sha256": hashlib.sha256(image_path.read_bytes()).hexdigest(),
        "label_sha256": hashlib.sha256(label_path.read_bytes()).hexdigest(),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("tests/fixtures/wp012_raster_scenes/controlled_dense"),
        help="directory that will contain same-stem PNG and BoofCV label files",
    )
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    scenes = [write_scene(args.output, count) for count in COUNTS]
    manifest = {
        "schema_version": 1,
        "purpose": "WP-012 controlled raster multi-QR density baseline",
        "generator": "scripts/generate_wp012_raster_scenes.py",
        "qrcode_backend": f"python-qrcode=={importlib.metadata.version('qrcode')}",
        "qr": {"version": VERSION, "error_correction": "M", "box_size": BOX_SIZE, "border": BORDER},
        "layout": {"gap": GAP, "margin": MARGIN, "labels": "module square excluding quiet zone"},
        "scenes": scenes,
    }
    (args.output.parent / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
