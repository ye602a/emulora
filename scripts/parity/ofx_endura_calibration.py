"""Fit RS enlarger settings to OFX float prints on a common Portra 400 negative.

Training uses the 256-step neutral strip at the bottom of the density replay
fixture. Validation uses an independent capture, cropped to its original image.
Run from the RS root with the ofx_density_print example built.
"""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

import numpy as np
import tifffile


def capture(cineon, optical):
    neg = tifffile.imread(cineon)
    target = tifffile.imread(optical)
    if neg.shape != target.shape or neg.ndim != 3 or neg.shape[2] not in (3, 4):
        raise ValueError("expected same-size RGB or RGBA captures")
    if neg.dtype != np.float32 or target.dtype != np.float32:
        raise ValueError("expected float32 TIFFs with raw transport")
    codes = neg[:, :, :3].astype(np.float64)
    rgb = target[:, :, :3].astype(np.float64)
    if not np.isfinite(rgb).all() or not np.all((codes > 0) & (codes < 1)):
        raise ValueError("invalid print or clipped Cineon capture")
    return np.maximum((codes * 1023 - 95) / 500, 0), rgb * 255


def run(args, work):
    recipe = json.loads(args.recipe.read_text())
    head = recipe.setdefault("enlarger", {})
    settings = np.array([head.get("y_filter_shift", 0), head.get("m_filter_shift", 0),
                         np.log2(head.get("print_exposure", 1))], dtype=np.float64)
    params = work / "params.json"
    raw = work / "output.bin"
    density = work / "density.bin"

    def render(x, width, height):
        head.update(y_filter_shift=float(x[0]), m_filter_shift=float(x[1]), print_exposure=float(2**x[2]))
        params.write_text(json.dumps(recipe))
        subprocess.run([args.replay, params, density, str(width), str(height), raw], check=True)
        return np.fromfile(raw, "<f8").reshape(height, width, 3) * 255

    negative, target = capture(args.train_cineon, args.train_print)
    if negative.shape[0] < 16 or negative.shape[1] < 256:
        raise ValueError("training fixture requires the appended 256-step strip")
    steps = np.arange(32, 225, 8)
    negative[-8, steps].astype("<f8").tofile(density)
    gray = target[-8, steps]
    columns = [2] if args.exposure_only else [0, 1, 2]
    for _ in range(6):
        prediction = render(settings, len(steps), 1)[0]
        jacobian = []
        for column in columns:
            shifted = settings.copy()
            step = [0.1, 0.1, 0.003][column]
            shifted[column] += step
            slope = (render(shifted, len(steps), 1)[0] - prediction) / step
            jacobian.append(slope.ravel())
        update, _, rank, _ = np.linalg.lstsq(np.column_stack(jacobian), (gray - prediction).ravel(), rcond=None)
        if rank != len(columns):
            raise ValueError("neutral steps do not identify the requested settings")
        settings[columns] += update
        if np.max(np.abs(update)) < 0.001:
            break
    else:
        raise ValueError("enlarger fit did not converge")

    negative, target = capture(args.test_cineon, args.test_print)
    h, w = target.shape[:2]
    width, height = args.region
    if not (0 < width <= w and 0 < height <= h):
        raise ValueError("invalid held-out image region")
    negative.astype("<f8").tofile(density)
    rgb = render(settings, w, h)
    rgb, target = rgb[:height, :width], target[:height, :width]
    raw_p95 = np.percentile(np.abs(rgb - target).max(axis=2), 95)
    error = np.abs(rgb.clip(0, 255) - target.clip(0, 255)).max(axis=2)
    p95, p99 = np.percentile(error, [95, 99])
    print(f"Y {settings[0]:+.6f}, M {settings[1]:+.6f}, print EV {settings[2]:+.6f}; "
          f"held-out display P95 {p95:.3f}, P99 {p99:.3f}; unclipped P95 {raw_p95:.3f}")
    if not (p95 <= 2 and p99 <= 4):
        raise SystemExit("shared-negative print exceeds reference tolerance")
    args.output.write_text(json.dumps(recipe, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("replay", "recipe", "train_cineon", "train_print", "test_cineon", "test_print", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--region", required=True, nargs=2, type=int, metavar=("WIDTH", "HEIGHT"))
    parser.add_argument("--exposure-only", action="store_true", help="hold recipe filtration fixed")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as folder:
        run(args, Path(folder))
