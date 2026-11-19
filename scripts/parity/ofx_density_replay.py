"""Compare both developed negatives through the RS Portra 400 / Endura print path."""
import argparse
from pathlib import Path
import subprocess
import tempfile

import numpy as np
from PIL import Image
import tifffile


def run(args):
    cineon = tifffile.imread(args.cineon)
    if cineon.dtype != np.float32 or cineon.ndim != 3 or cineon.shape[2] not in (3, 4):
        raise ValueError("expected a raw RGB or RGBA float32 density TIFF")
    rgb = cineon[:, :, :3].astype(np.float64)
    if not np.isfinite(rgb).all() or np.any(rgb <= 0) or np.any(rgb >= 1):
        raise ValueError("nonfinite or clipped Cineon samples cannot recover the negative")
    h, w, _ = rgb.shape
    reference = np.asarray(Image.open(args.ofx_print).convert("RGB"), dtype=np.float64)
    if reference.shape != rgb.shape:
        raise ValueError("capture the OFX print and density from the same frame")
    width, height = args.region or (w, h)
    if not (0 < width <= w and 0 < height <= h):
        raise ValueError("comparison region exceeds frame bounds")
    direct = np.fromfile(args.rs_print, "<f8").reshape(rgb.shape)
    with tempfile.TemporaryDirectory() as scratch:
        decoded = Path(scratch) / "ofx-negative.bin"
        np.maximum((rgb * 1023 - 95) / 500, 0).astype("<f8").tofile(decoded)
        for label, negative in [("RS control", args.rs_density), ("OFX negative", decoded)]:
            output = Path(scratch) / "print.bin"
            subprocess.run([args.replay, args.recipe, negative, str(w), str(h), output], check=True)
            replay = np.fromfile(output, "<f8").reshape(rgb.shape)
            if not np.isfinite(replay).all():
                raise ValueError("nonfinite print")
            if label == "RS control" and not np.array_equal(replay, direct):
                raise ValueError("density replay did not reproduce the production RS CPU print")
            error = np.abs(replay[:height, :width].clip(0, 1) * 255 - reference[:height, :width])
            print(label, "P95 / P99", np.percentile(error.max(axis=2), [95, 99]).round(3))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("replay", "recipe", "cineon", "ofx_print", "rs_density", "rs_print"):
        parser.add_argument(arg, type=Path)
    parser.add_argument("--region", nargs=2, type=int, metavar=("WIDTH", "HEIGHT"),
                        help="top-left region measured after replaying the full frame")
    run(parser.parse_args())
