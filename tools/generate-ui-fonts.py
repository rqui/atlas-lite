#!/usr/bin/env python3
"""Generate the firmware's printable-ASCII 1-bpp UI font strikes.

Source font files are intentionally not checked in. Pass licensed Inter and
Atkinson Hyperlegible Next variable TTFs explicitly; the generated Rust file
records their SHA-256 digests. Pillow is a build-time tool only.
"""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


@dataclass(frozen=True)
class Strike:
    family: str
    profile: str
    role: str
    source_px: int
    line_height: int
    weight: int


STRIKES = (
    Strike("INTER", "COMPACT", "DETAIL", 11, 16, 500),
    Strike("INTER", "COMPACT", "BODY", 13, 18, 500),
    Strike("INTER", "COMPACT", "HEADING", 18, 24, 600),
    Strike("INTER", "COMPACT", "LARGE", 22, 29, 600),
    Strike("INTER", "STANDARD", "DETAIL", 15, 20, 500),
    Strike("INTER", "STANDARD", "BODY", 18, 24, 500),
    Strike("INTER", "STANDARD", "HEADING", 24, 32, 600),
    Strike("INTER", "STANDARD", "LARGE", 34, 44, 600),
    Strike("INTER", "LARGE", "DETAIL", 17, 22, 500),
    Strike("INTER", "LARGE", "BODY", 21, 28, 500),
    Strike("INTER", "LARGE", "HEADING", 28, 36, 600),
    Strike("INTER", "LARGE", "LARGE", 38, 48, 600),
    Strike("ATKINSON", "COMPACT", "DETAIL", 11, 13, 500),
    Strike("ATKINSON", "COMPACT", "BODY", 13, 15, 500),
    Strike("ATKINSON", "COMPACT", "HEADING", 18, 20, 600),
    Strike("ATKINSON", "COMPACT", "LARGE", 22, 24, 600),
    Strike("ATKINSON", "STANDARD", "DETAIL", 18, 20, 500),
    Strike("ATKINSON", "STANDARD", "BODY", 22, 24, 500),
    Strike("ATKINSON", "STANDARD", "HEADING", 29, 32, 600),
    Strike("ATKINSON", "STANDARD", "LARGE", 40, 44, 600),
    Strike("ATKINSON", "LARGE", "DETAIL", 20, 22, 500),
    Strike("ATKINSON", "LARGE", "BODY", 26, 28, 500),
    Strike("ATKINSON", "LARGE", "HEADING", 33, 36, 600),
    Strike("ATKINSON", "LARGE", "LARGE", 44, 48, 600),
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def configured_font(path: Path, strike: Strike) -> ImageFont.FreeTypeFont:
    font = ImageFont.truetype(str(path), strike.source_px)
    axes = font.get_variation_axes()
    if len(axes) == 2:  # Inter: optical size, weight.
        font.set_variation_by_axes([min(max(strike.source_px, 14), 32), strike.weight])
    elif len(axes) == 1:  # Atkinson Hyperlegible Next: weight.
        font.set_variation_by_axes([strike.weight])
    else:
        raise RuntimeError(f"unexpected variation axes for {path}: {axes!r}")
    return font


def rasterize(font: ImageFont.FreeTypeFont, character: str):
    advance = max(1, round(font.getlength(character)))
    if character == " ":
        return (0, 0, advance, 0, 0, b"")

    left, top, right, bottom = font.getbbox(character, anchor="ls")
    width = max(1, right - left)
    height = max(1, bottom - top)
    image = Image.new("L", (width, height), 0)
    ImageDraw.Draw(image).text((-left, -top), character, font=font, fill=255, anchor="ls")
    binary = image.point(lambda value: 255 if value >= 96 else 0, mode="1")
    ink = binary.getbbox()
    if ink is None:
        return (0, 0, advance, 0, 0, b"")
    crop_left, crop_top, crop_right, crop_bottom = ink
    binary = binary.crop(ink)
    width, height = binary.size
    left += crop_left
    top += crop_top
    stride = (width + 7) // 8
    packed = bytearray(stride * height)
    pixels = binary.load()
    for y in range(height):
        for x in range(width):
            if pixels[x, y]:
                packed[y * stride + x // 8] |= 0x80 >> (x % 8)
    return (width, height, advance, left, top, bytes(packed))


def rust_strike(path: Path, strike: Strike) -> str:
    name = f"{strike.family}_{strike.profile}_{strike.role}"
    font = configured_font(path, strike)
    glyphs = []
    bitmap = bytearray()
    for codepoint in range(32, 127):
        width, height, advance, left, top, packed = rasterize(font, chr(codepoint))
        glyphs.append((len(bitmap), width, height, advance, left, top))
        bitmap.extend(packed)

    lines = [
        f"// {name}: source-px={strike.source_px} line-height={strike.line_height} weight={strike.weight}",
        f"const {name}_GLYPHS: [Glyph; 95] = [",
    ]
    for offset, width, height, advance, left, top in glyphs:
        lines.append(
            "    Glyph { "
            f"offset: {offset}, width: {width}, height: {height}, advance: {advance}, "
            f"left: {left}, top: {top} "
            "},"
        )
    lines.append("];")
    lines.append(f"const {name}_BITMAP: [u8; {len(bitmap)}] = [")
    for offset in range(0, len(bitmap), 16):
        chunk = ", ".join(f"0x{value:02X}" for value in bitmap[offset : offset + 16])
        lines.append(f"    {chunk},")
    lines.extend(
        [
            "];",
            f"pub static {name}: BitmapFont = BitmapFont {{",
            f"    glyphs: &{name}_GLYPHS,",
            f"    bitmap: &{name}_BITMAP,",
            f"    line_height: {strike.line_height},",
            "};",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inter", required=True, type=Path)
    parser.add_argument("--atkinson", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    header = f"""//! Generated 1-bpp ASCII glyph atlases for Atlas Lite UI.
//!
//! Generated by `tools/generate-ui-fonts.py` from licensed variable fonts.
//! Raw font files are intentionally not distributed. Source SHA-256:
//! Inter `{digest(args.inter)}`;
//! Atkinson Hyperlegible Next `{digest(args.atkinson)}`.

use super::{{BitmapFont, Glyph}};

"""
    sections = []
    for strike in STRIKES:
        path = args.inter if strike.family == "INTER" else args.atkinson
        sections.append(rust_strike(path, strike))
    args.output.write_text(header + "\n".join(sections), encoding="utf-8")


if __name__ == "__main__":
    main()
