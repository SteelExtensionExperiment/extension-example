#!/usr/bin/env python3
"""Prepare image assets for Steel UI from authored PNGs."""

import argparse
import json
from pathlib import Path
import re

from PIL import Image

SLOT_PIXELS = 16


def positive(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("expected a positive integer")
    return number


def slot_size(value):
    if not re.fullmatch(r"[1-9][0-9]*x[1-9][0-9]*", value):
        raise argparse.ArgumentTypeError("use WIDTHxHEIGHT in slots, for example 2x1")
    width, height = value.split("x")
    return int(width), int(height)


def read_png(path):
    with Image.open(path, formats=["PNG"]) as image:
        return image.convert("RGBA")


def tiles(args):
    if not re.fullmatch(r"[a-z0-9_-]+", args.name):
        raise ValueError("tile name must use lowercase resource path characters")
    fallback_pattern = r"minecraft:[a-z0-9_-]+(?:/[a-z0-9_-]+)*"
    if args.fallback == "minecraft:air" or not re.fullmatch(fallback_pattern, args.fallback):
        raise ValueError("choose a visible vanilla fallback item")

    source = read_png(args.source)
    columns, rows = args.size
    expected = (columns * SLOT_PIXELS, rows * SLOT_PIXELS)
    if source.size != expected:
        raise ValueError(
            f"expected {expected[0]}x{expected[1]} pixels for {columns}x{rows} slots, "
            f"got {source.width}x{source.height}"
        )

    args.output.mkdir(parents=True, exist_ok=True)
    names = {}
    entries = []
    for row in range(rows):
        for column in range(columns):
            x, y = column * SLOT_PIXELS, row * SLOT_PIXELS
            tile = source.crop((x, y, x + SLOT_PIXELS, y + SLOT_PIXELS))
            pixels = tile.tobytes()
            if pixels not in names:
                name = f"{args.name}-{len(names)}"
                tile.save(args.output / f"{name}.png", format="PNG")
                names[pixels] = name
            name = names[pixels]
            entries.append(
                "    ::steel_ui_api::WidgetTexture {\n"
                f"        name: {json.dumps(name)},\n"
                f"        fallback: {json.dumps(args.fallback)},\n"
                f"        png: include_bytes!({json.dumps(name + '.png')}),\n"
                "    },"
            )
    declaration = args.output / f"{args.name}.rs"
    declaration.write_text("[\n" + "\n".join(entries) + "\n]\n", encoding="utf-8")
    print(f"{declaration} ({len(names)} PNGs for {len(entries)} slots)")


def gauge(args):
    if args.source.resolve() == args.output.resolve():
        raise ValueError("choose a different path for the frame atlas")
    image = read_png(args.source)
    width, height = image.size
    axis = height if args.direction in ("up", "down") else width
    if args.steps > axis:
        raise ValueError(
            f"{args.steps} steps require at least that many pixels along the fill direction"
        )

    atlas = Image.new("RGBA", (width, height * (args.steps + 1)))
    for level in range(1, args.steps + 1):
        visible = axis * level // args.steps
        left, top, right, bottom = {
            "up": (0, height - visible, width, height),
            "down": (0, 0, width, visible),
            "right": (0, 0, visible, height),
            "left": (width - visible, 0, width, height),
        }[args.direction]
        atlas.paste(image.crop((left, top, right, bottom)), (left, level * height + top))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    atlas.save(args.output, format="PNG")
    print(f"{args.output} ({args.steps + 1} frames, {width}x{height} pixels each)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(required=True)
    tile_command = commands.add_parser("tiles", help="split an image into reusable slot tiles")
    tile_command.add_argument("source", type=Path)
    tile_command.add_argument("output", type=Path, help="directory for the PNGs and Rust array")
    tile_command.add_argument("--name", required=True)
    tile_command.add_argument("--size", type=slot_size, required=True)
    tile_command.add_argument("--fallback", required=True)
    tile_command.set_defaults(run=tiles)

    gauge_command = commands.add_parser("gauge", help="generate a vertical atlas of gauge frames")
    gauge_command.add_argument("source", type=Path, help="PNG of the full gauge")
    gauge_command.add_argument("output", type=Path, help="PNG atlas with an empty first frame")
    gauge_command.add_argument("--steps", type=positive, required=True)
    gauge_command.add_argument("--direction", choices=("up", "down", "right", "left"), default="up")
    gauge_command.set_defaults(run=gauge)

    args = parser.parse_args()
    try:
        args.run(args)
    except (OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
