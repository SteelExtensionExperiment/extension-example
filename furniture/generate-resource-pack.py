#!/usr/bin/env python3
"""Generate optional vertical-slab models/icons from this checkout's vanilla assets."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import zipfile


def generate(steel, output):
    source = steel / "minecraft-src/minecraft"
    assets = source / "resources/assets/minecraft"
    constants = (source / "src/net/minecraft/SharedConstants.java").read_text()
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--no-deps", "--offline", "--format-version", "1",
        "--manifest-path", str(steel / "Cargo.toml"),
    ], text=True))
    version = next(package["version"] for package in metadata["packages"] if package["name"] == "steel")
    target = version.split("+mc", 1)[1]
    source_version = subprocess.check_output(["git", "-C", str(source.parent), "log", "-1", "--format=%s"], text=True).strip()
    if source_version != target:
        raise ValueError("minecraft-src does not match the workspace Minecraft target")
    formats = []
    for part in ("MAJOR", "MINOR"):
        match = re.search(rf"RESOURCE_PACK_FORMAT_{part}\s*=\s*(\d+)", constants)
        if not match:
            raise ValueError(f"resource pack {part.lower()} format is unavailable")
        formats.append(int(match[1]))

    blocks = json.loads((steel / "steel-registry/build_assets/blocks.json").read_text())["blocks"]
    entries = {}
    count = 0
    for block in blocks:
        if "SLAB_TYPE" not in block["properties"]:
            continue
        name = block["name"]
        if not name.endswith("_slab"):
            raise ValueError(f"unexpected slab name: {name}")
        item = json.loads((assets / "items" / f"{name}.json").read_text())["model"]
        if item["type"] != "minecraft:model":
            raise ValueError(f"unsupported item model for {name}")
        key = name.removesuffix("_slab") + "_vertical_slab"
        keys = [key, "vertical_slab"] if name == "spruce_slab" else [key]
        # Inherit the slab's actual textures and GUI/hand transforms. This is
        # authored vertical geometry, with full-size front/back slab textures.
        faces = {
            side: {"texture": texture, "uv": uv}
            for side, texture, uv in (
                ("north", "#top", [0, 0, 16, 16]),
                ("south", "#bottom", [0, 0, 16, 16]),
                ("west", "#side", [0, 0, 8, 16]),
                ("east", "#side", [0, 0, 8, 16]),
                ("up", "#side", [0, 0, 16, 8]),
                ("down", "#side", [0, 0, 16, 8]),
            )
        }
        for variant in keys:
            model = {"parent": item["model"], "elements": [{"from": [0, 0, 8], "to": [16, 16, 16], "faces": faces}]}
            entries[f"assets/furniture/models/block/{variant}.json"] = model
            entries[f"assets/furniture/items/{variant}.json"] = {
                "model": {"type": "minecraft:model", "model": f"furniture:block/{variant}"}
            }
        count += 1
    if not count:
        raise ValueError("no extracted slab materials found")
    entries["pack.mcmeta"] = {"pack": {
        "description": f"Steel furniture: vertical slab models and icons ({target})",
        "min_format": formats, "max_format": formats,
    }}
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path, value in sorted(entries.items()):
            info = zipfile.ZipInfo(path)
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, json.dumps(value, separators=(",", ":")))
    print(f"{output}: {count} slab materials plus the original spruce alias")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--steel", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parent / "dist/furniture-resource-pack.zip")
    args = parser.parse_args()
    try:
        generate(args.steel.resolve(), args.output)
    except (OSError, KeyError, ValueError, StopIteration, subprocess.CalledProcessError) as error:
        parser.exit(1, f"cannot generate furniture resource pack: {error}\n")
