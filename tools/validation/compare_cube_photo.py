"""Measure trilinear CUBE error against CLI --raw-out for the same 8-bit sRGB PNG.

Offline diagnostic for reduced photos and charts. Requires NumPy and Pillow.
The reference photo recipe must select the pointwise settings used by the bake.
"""
import argparse
from pathlib import Path

import numpy as np
from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input_png", type=Path)
    parser.add_argument("reference_f64", type=Path)
    parser.add_argument("cubes", type=Path, nargs="+")
    args = parser.parse_args()
    with Image.open(args.input_png) as image:
        if image.format != "PNG" or image.mode != "RGB":
            parser.error("expected an 8-bit RGB PNG")
        rgb = np.asarray(image, dtype=np.float64) / 255
    linear = np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)
    reference = np.fromfile(args.reference_f64, dtype="<f8").reshape(rgb.shape)

    for path in args.cubes:
        lines = path.read_text(encoding="utf-8").splitlines()
        if "# Input: linear sRGB [0,1]" not in lines or "# Output: encoded sRGB [0,1]" not in lines:
            parser.error("expected a linear sRGB to encoded sRGB spektrafilm-rs cube")
        size = int(next(line.split()[1] for line in lines if line.startswith("LUT_3D_SIZE ")))
        table = np.loadtxt(lines[lines.index("DOMAIN_MAX 1 1 1") + 1:]).reshape(size, size, size, 3)
        coords = linear * (size - 1)
        lower = np.minimum(coords.astype(int), size - 2)
        fraction = coords - lower
        result = np.zeros_like(rgb)
        for corner in np.ndindex(2, 2, 2):
            red, green, blue = np.moveaxis(lower + corner, -1, 0)
            weight = np.prod(np.where(corner, fraction, 1 - fraction), axis=-1, keepdims=True)
            result += weight * table[blue, green, red]
        error = np.abs(result - reference) * 255
        rms = np.sqrt(np.mean(error ** 2))
        p95, p99, peak = np.percentile(error, [95, 99, 100])
        print(f"{path.name}  RMS {rms:.4f}  p95 {p95:.4f}  p99 {p99:.4f}  max {peak:.4f}  (RGB8 codes)")


if __name__ == "__main__":
    main()
