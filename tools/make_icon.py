"""Draws the placeholder app icon: a clock face with a bell dot.

Usage: python tools/make_icon.py <out.png> [size]
"""

import sys

from PIL import Image, ImageDraw


def draw(size: int) -> Image.Image:
    s = size * 4  # supersample, then downscale for smooth edges
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    pad = s // 16
    d.rounded_rectangle((pad, pad, s - pad, s - pad), radius=s // 5, fill=(79, 70, 229, 255))
    c = s // 2
    r = s * 5 // 16
    d.ellipse((c - r, c - r, c + r, c + r), fill=(255, 255, 255, 255))
    w = max(4, s // 28)
    d.line((c, c, c, c - r * 3 // 4), fill=(31, 41, 55, 255), width=w)
    d.line((c, c, c + r * 3 // 5, c + r // 6), fill=(31, 41, 55, 255), width=w)
    dot = s // 9
    d.ellipse((s - pad - dot * 2, pad, s - pad, pad + dot * 2), fill=(239, 68, 68, 255))
    return img.resize((size, size), Image.LANCZOS)


if __name__ == "__main__":
    out = sys.argv[1]
    size = int(sys.argv[2]) if len(sys.argv) > 2 else 1024
    draw(size).save(out)
