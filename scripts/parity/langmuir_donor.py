"""Compare matched CPU CLI builds on Batch 8 photo crops and a gray exposure wedge."""
import argparse
import csv
import json
import os
from pathlib import Path
import subprocess

import numpy as np
from PIL import Image

from negative_hue_trims import CROPS, PAIRS, angle, oklab, wrap


def run(args):
    out = args.scratch.resolve()
    out.mkdir(exist_ok=True, parents=True)
    source = Image.new("RGB", (480, 160))
    for i, (_, file, x, y) in enumerate(CROPS):
        with Image.open(args.fixtures / (file + ".png")) as photo:
            source.paste(photo.crop((x, y, x + 96, y + 96)), (96 * i, 0))
    for i, level in enumerate(np.linspace(4, 250, 12).round().astype(int)):
        source.paste((int(level),) * 3, (40*i, 96, 40*i+40, 160))
    source.save(out / "source.png")
    recipe = {
        "camera": {"auto_exposure": False, "film_format_mm": 2.4},
        "film_render": {"grain": {"active": False}, "halation": {"active": False, "boost_ev": 0},
                        "dir_couplers": {"active": True}},
        "print_render": {"glare": {"active": False}},
        "scanner": {"unsharp_mask": [0, 0]},
        "io": {"output_color_space": "sRGB", "output_cctf_encoding": False,
               "output_gamut_compress": {"algorithm": "off"}},
    }
    with (out / "measurements.csv").open("w", newline="") as f:
        rows = csv.writer(f)
        rows.writerow(["film", "ev", "region", "pixels", "median_delta_L", "median_delta_C",
                       "median_abs_hue_deg", "p95_oklab_distance", "p95_abs_negative_density"])
        for film, paper in PAIRS[:3]:
            for ev in (-2, 0, 2):
                recipe["camera"]["exposure_compensation_ev"] = ev
                params = out / "recipe.json"
                params.write_text(json.dumps(recipe))
                renders, negatives = [], []
                for label, exe in (("linear", args.linear), ("donor", args.donor)):
                    tag = f"{film}_{ev}_{label}"
                    neg = out / "density.bin"
                    raw = out / "print.bin"
                    neg.unlink(missing_ok=True)
                    raw.unlink(missing_ok=True)
                    with (out / (tag + ".log")).open("w") as log:
                        subprocess.run([str(exe.resolve()), "process", str(out / "source.png"),
                                        "--params", str(params), "--film", film, "--paper", paper,
                                        "--raw-out", str(raw), "--output", str(out / (tag + ".png"))],
                                       env=dict(os.environ, SPEKTRAFILM_BACKEND="cpu",
                                                SPEKTRAFILM_DUMP_FILM_DENSITY=str(neg)),
                                       check=True, stdout=log, stderr=log)
                    rgb, den = [np.fromfile(p, "<f8").reshape(160, 480, 3) for p in (raw, neg)]
                    if not np.isfinite(rgb).all() or not np.isfinite(den).all():
                        raise ValueError(tag + ": non-finite output")
                    lab = oklab(rgb)
                    wedge = [lab[124:132, i*40+16:i*40+24, 0].mean() for i in range(12)]
                    if np.min(np.diff(wedge)) < -1e-5:
                        raise ValueError(tag + ": gray wedge reverses")
                    renders.append(lab)
                    negatives.append(den)
                if np.array_equal(negatives[0], negatives[1]):
                    raise ValueError(f"{film} EV {ev}: probe produced the baseline negative")
                regions = [(name, np.s_[12:84, i*96+12:i*96+84]) for i, (name, *_) in enumerate(CROPS)]
                regions += [(f"gray{i:02}", np.s_[124:132, i*40+16:i*40+24]) for i in range(12)]
                for name, roi in regions:
                    a, b = [lab[roi] for lab in renders]
                    ca, cb = [np.linalg.norm(v[..., 1:], axis=-1) for v in (a, b)]
                    mask = (ca > .035) & (cb > .035)
                    hue = abs(wrap(angle(a[..., 1:]) - angle(b[..., 1:])))
                    rows.writerow([film, ev, name, int(mask.sum()), np.median(b[..., 0]-a[..., 0]),
                                   np.median(cb-ca), np.median(hue[mask]) if mask.any() else "",
                                   np.percentile(np.linalg.norm(b-a, axis=-1), 95),
                                   np.percentile(abs(negatives[1][roi]-negatives[0][roi]), 95)])
                f.flush()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("fixtures", "scratch", "linear", "donor"):
        parser.add_argument(key, type=Path)
    run(parser.parse_args())
