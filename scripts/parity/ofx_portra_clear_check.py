"""Compare normal CPU and Vulkan Portra prints to the matching OFX float sRGB capture."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile

import numpy as np
import tifffile


def check(args, work):
    target = tifffile.imread(args.reference)
    if target.dtype != np.float32 or target.ndim != 3 or target.shape[2] not in (3, 4):
        raise ValueError("expected a float RGB or RGBA reference")
    target = target[:, :, :3].astype(float)
    if not np.isfinite(target).all():
        raise ValueError("nonfinite OFX reference pixels")
    target = target.clip(0, 1)
    width, height = args.region
    if not (0 < width <= target.shape[1] and 0 < height <= target.shape[0]):
        raise ValueError("invalid photo region")
    for backend, label in [("cpu", b"CPU (rayon)"), ("wgpu", b"wgpu (GPU)")]:
        raw = work / f"{backend}.bin"
        p = subprocess.run([
            args.cli, "process", args.photo, "--film", "kodak_portra_400", "--paper", "kodak_portra_endura",
            "--data-dir", args.data, "--params", args.recipe,
            "-o", work / "frame.png", "--raw-out", raw,
        ], env=dict(os.environ, SPEKTRAFILM_BACKEND=backend), capture_output=True, check=True)
        if label not in p.stderr:
            raise RuntimeError(p.stderr.decode(errors="replace"))
        rgb = np.fromfile(raw, "<f8").reshape(target.shape)
        if not np.isfinite(rgb).all():
            raise ValueError(f"nonfinite {backend} print pixels")
        rgb = rgb.clip(0, 1)
        error = abs(rgb - target).max(axis=2) * 255
        p95, p99 = np.percentile(error[:height, :width], [95, 99])
        print(f"{backend} P95 {p95:.3f}, P99 {p99:.3f}")
        if not (p95 <= 2 and p99 <= 4):
            raise SystemExit("Portra print exceeds display tolerance")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "data", "photo", "reference"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--recipe", type=Path, default=Path("recipes/ofx/portra_400_endura_clear.json"))
    parser.add_argument("--region", nargs=2, type=int, required=True, metavar=("WIDTH", "HEIGHT"))
    with tempfile.TemporaryDirectory() as work:
        check(parser.parse_args(), Path(work))
