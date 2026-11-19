"""Compare each packaged Hanatos poly4 on the same Batch 13 sRGB montage.

Run from the RS root: python scripts/parity/hanatos_surface.py CLI INPUT_PNG OUT
Outputs are local; no study dependencies are required by the renderer.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

import numpy as np
from PIL import Image

from negative_hue_trims import CROPS, angle, oklab, wrap


def lab(rgb):
    linear = np.where(rgb <= .04045, rgb / 12.92, ((rgb + .055) / 1.055) ** 2.4)
    return oklab(linear)


def main(cli, source, out, ev, films):
    source, out = Path(source), Path(out)
    out.mkdir(parents=True, exist_ok=True)
    src = np.asarray(Image.open(source).convert("RGB")) / 255.
    assert src.shape == (144, 480, 3)
    ref = lab(src)
    regions = {name: (slice(12, 84), slice(i*96+12, i*96+84))
               for i, (name, *_) in enumerate(CROPS)}
    regions.update({name: (120, x) for name, x in
                    [("gray", 48), ("light-gray", 144), ("red", 240), ("blue", 336), ("violet", 432)]})
    report = {"input_sha256": hashlib.sha256(source.read_bytes()).hexdigest(), "camera_ev": ev, "films": {}}
    for profile in sorted(Path("data/profiles").glob("*.json")):
        if films and profile.stem not in films:
            continue
        stock = json.loads(profile.read_text(encoding="utf8"))
        coefs = stock["data"].get("hanatos2025_adaptation_surface_params")
        if not coefs:
            continue
        assert np.shape(coefs) == (3, 15) and np.isfinite(coefs).all()
        slide = stock["info"]["type"] == "positive"
        results = []
        for enabled in [False, True]:
            tag = profile.stem + ("-on" if enabled else "-off")
            recipe = {"camera": {"auto_exposure": False, "exposure_compensation_ev": ev},
                      "settings": {"rgb_to_raw_method": "hanatos2025",
                                   "apply_hanatos2025_adaptation_surface": enabled},
                      "film_render": {"grain": {"active": False}, "halation": {"active": False, "boost_ev": 0},
                                      "dir_couplers": {"active": False}, "glare": {"active": False}},
                      "print_render": {"glare": {"active": False}},
                      "scanner": {"unsharp_mask": [0, 0]},
                      "io": {"scan_film": slide, "output_color_space": "sRGB", "output_cctf_encoding": True}}
            params = out / "params.json"
            params.write_text(json.dumps(recipe))
            route = ["--scan-film"] if slide else ["--paper", "kodak_portra_endura"]
            proc = subprocess.run([cli, "process", source, "--film", profile.stem,
                *route, "--params", params, "--raw-out", out/"raw.bin",
                "--output", out/(tag+".png")], env=dict(os.environ, SPEKTRAFILM_BACKEND="wgpu"),
                capture_output=True, check=True)
            assert b"Backend: wgpu" in proc.stderr
            rgb = np.fromfile(out/"raw.bin", "<f8").reshape(src.shape)
            assert np.isfinite(rgb).all()
            assert rgb[120, 144].mean() > rgb[120, 48].mean(), f"unexpected gray polarity: {tag}"
            results.append(rgb.clip(0, 1))
        before, after = results
        labs = [lab(before), lab(after)]
        row = {}
        for name, roi in regions.items():
            def mean_color(img):
                return img[roi].reshape(-1, 3).mean(axis=0)
            target = angle(mean_color(ref)[1:])
            hues = [float(abs(wrap(angle(mean_color(v)[1:]) - target))) for v in labs]
            row[name] = {"p95_rgb_codes": round(float(np.percentile(
                abs(after[roi]-before[roi]).max(axis=-1)*255, 95)), 2),
                "mean_hue_error_deg_off_on": np.round(hues, 2).tolist(),
                "mean_rgb_off_on": [np.round(mean_color(v)*255, 1).tolist() for v in results],
                "clipped_fraction_off_on": [round(float(((v[roi]<=0)|(v[roi]>=1)).any(axis=-1).mean()),4)
                                            for v in results]}
        report["films"][profile.stem] = row
        (out/"report.json").write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for arg in ("cli", "source", "out"):
        parser.add_argument(arg)
    parser.add_argument("--ev", type=float, default=0.)
    parser.add_argument("--films", nargs="+")
    main(**vars(parser.parse_args()))
