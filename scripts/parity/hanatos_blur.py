"""Probe wavelength smoothing with private RS data copies; run from the repository root."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import numpy as np
from PIL import Image

from hanatos_surface import lab
from negative_hue_trims import CROPS, angle, wrap


def main(cli, source, out):
    out.mkdir(parents=True, exist_ok=True)
    src = np.asarray(Image.open(source).convert("RGB")) / 255
    assert src.shape == (144, 480, 3)
    regions = {name: (slice(12, 84), slice(i * 96 + 12, i * 96 + 84))
               for i, (name, *_) in enumerate(CROPS)}
    regions.update({name: (120, x) for name, x in
                    [("gray", 48), ("red", 240), ("blue", 336), ("violet", 432)]})
    stocks = ["kodak_portra_400", "kodak_ektar_100", "kodak_ultramax_400",
              "kodak_vision3_200t", "fujifilm_provia_100f"]
    baseline = {}
    report = {}
    with tempfile.TemporaryDirectory(dir=out) as temp:
        data = Path(temp) / "data"
        shutil.copytree("data", data)
        lut = data / "luts/spectral_upsampling/irradiance_xy_tc.npy"
        original = np.load(lut)
        for sigma in [0, 1, 2, 4]:
            if sigma:
                radius = 4 * sigma
                weights = np.exp(-0.5 * (np.arange(-radius, radius + 1) / sigma) ** 2)
                padded = np.pad(original, [(0, 0), (0, 0), (radius, radius)], mode="symmetric")
                smooth = np.zeros(original.shape, dtype=np.float64)
                for i, weight in enumerate(weights / weights.sum()):
                    smooth += padded[..., i:i + original.shape[-1]].astype(np.float64) * weight
                np.save(lut, smooth.astype(original.dtype))
            for stock in stocks:
                slide = stock == "fujifilm_provia_100f"
                params = {"camera": {"auto_exposure": False},
                          "settings": {"rgb_to_raw_method": "hanatos2025"},
                          "film_render": {"grain": {"active": False}, "halation": {"active": False, "boost_ev": 0},
                                          "dir_couplers": {"active": False}, "glare": {"active": False}},
                          "print_render": {"glare": {"active": False}}, "scanner": {"unsharp_mask": [0, 0]},
                          "io": {"scan_film": slide, "output_color_space": "sRGB", "output_cctf_encoding": True}}
                recipe = Path(temp) / "params.json"
                recipe.write_text(json.dumps(params))
                raw = Path(temp) / "raw.bin"
                tag = f"{stock}-{sigma}"
                route = ["--scan-film"] if slide else ["--paper", "kodak_portra_endura"]
                proc = subprocess.run([cli, "process", source, "--film", stock, *route, "--params", recipe,
                                       "--data-dir", data, "--raw-out", raw, "--output", out / (tag + ".png")],
                                      capture_output=True, env=dict(os.environ, SPEKTRAFILM_BACKEND="wgpu"))
                assert proc.returncode == 0 and b"Backend: wgpu" in proc.stderr, proc.stderr
                rgb = np.fromfile(raw, "<f8").reshape(src.shape)
                assert np.isfinite(rgb).all() and rgb[120, 144].mean() > rgb[120, 48].mean()
                rgb = rgb.clip(0, 1)
                if not sigma:
                    baseline[stock] = rgb
                row = {}
                for name, roi in regions.items():
                    hues = [angle(lab(v[roi]).reshape(-1, 3).mean(axis=0)[1:]) for v in [src, baseline[stock], rgb]]
                    row[name] = {"hue_error_deg_off_on": np.abs(wrap(np.array(hues[1:]) - hues[0])).round(2).tolist(),
                                 "p95_max_channel_codes": round(float(np.percentile(
                                     abs(rgb[roi] - baseline[stock][roi]).max(axis=-1) * 255, 95)), 2)}
                report[tag] = row
        # Isolate the neighboring white-normalization question at zero blur.
        neutral = {}
        gray = Path(temp) / "gray.png"
        Image.new("RGB", (1, 1), (118, 118, 118)).save(gray)
        for stock in stocks:
            readings = []
            for window in [False, True]:
                params["settings"]["apply_hanatos2025_adaptation_window"] = window
                recipe.write_text(json.dumps(params))
                log = Path(temp) / "log.bin"
                log.unlink(missing_ok=True)
                subprocess.run([cli, "process", gray, "--film", stock, "--scan-film",
                                "--data-dir", "data", "--params", recipe, "--output", Path(temp) / "gray-out.png"],
                               capture_output=True, check=True, env=dict(os.environ, SPEKTRAFILM_BACKEND="cpu",
                               SPEKTRAFILM_DUMP_FILM_LOG_RAW=str(log.resolve())))
                readings.append(np.fromfile(log, "<f8").reshape(3))
            change = 100 * (10 ** (readings[1] - readings[0]) - 1)
            assert np.isfinite(change).all()
            neutral[stock] = change.round(4).tolist()
        (out / "window-gray.json").write_text(json.dumps(neutral, indent=2))
    (out / "report.json").write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ["cli", "source", "out"]:
        parser.add_argument(name, type=Path)
    main(**vars(parser.parse_args()))
