"""Compare shipping and reconstructed-white CLIs on the Batch 13 montage."""
import argparse
import json
import os
from pathlib import Path
import subprocess

import numpy as np
from PIL import Image

from hanatos_surface import lab
from negative_hue_trims import CROPS, angle, wrap


def main(current, candidate, source, out):
    out.mkdir(parents=True, exist_ok=True)
    src = np.asarray(Image.open(source).convert("RGB")) / 255
    assert src.shape == (144, 480, 3)
    gray = out / "gray.png"
    Image.fromarray(np.tile(np.array([64, 118, 188], np.uint8)[None, :, None], (1, 1, 3))).save(gray)
    regions = {n: (slice(12, 84), slice(i*96+12, i*96+84)) for i, (n, *_) in enumerate(CROPS)}
    regions.update({n: (120, x) for n, x in [("red", 240), ("blue", 336), ("violet", 432)]})
    report = {}
    for stock in ("kodak_portra_400", "kodak_ektar_100", "kodak_ultramax_400",
                  "kodak_vision3_200t", "fujifilm_provia_100f"):
        slide = stock == "fujifilm_provia_100f"
        route = ["--scan-film"] if slide else ["--paper", "kodak_portra_endura"]
        for cuts in [False, True]:
            tag = stock + ("-cuts" if cuts else "-clear")
            row = {}
            readings = []
            prints = []
            for model, cli, window in [("off", current, False), ("legacy", current, True),
                                       ("candidate", candidate, True)]:
                recipe = {"settings": {"rgb_to_raw_method": "hanatos2025", "apply_hanatos2025_adaptation_window": window},
                          "camera": {"auto_exposure": False}, "scanner": {"unsharp_mask": [0, 0]},
                          "film_render": {"grain": {"active": False}, "dir_couplers": {"active": False},
                                          "halation": {"active": False, "boost_ev": 0}, "glare": {"active": False}},
                          "print_render": {"glare": {"active": False}},
                          "io": {"output_color_space": "sRGB", "output_cctf_encoding": True}}
                if cuts:
                    recipe["camera"].update(filter_uv=[1, 430, 10], filter_ir=[0.5, 650, 10])
                params = out / "params.json"
                params.write_text(json.dumps(recipe))
                for backend, image in [("cpu", gray), ("wgpu", source)]:
                    if backend == "wgpu" and not window:
                        continue
                    raw, log = out / "raw.bin", out / "log.bin"
                    log.unlink(missing_ok=True)
                    png = out / f"{tag}-{model}-{backend}.png"
                    proc = subprocess.run([cli, "process", image, "--film", stock, *route, "--params", params,
                                           "--output", png, "--raw-out", raw], capture_output=True,
                                          env=dict(os.environ, SPEKTRAFILM_BACKEND=backend,
                                                   SPEKTRAFILM_DUMP_FILM_LOG_RAW=str(log.resolve())))
                    assert proc.returncode == 0 and f"backend: {backend}".encode() in proc.stderr.lower(), proc.stderr
                    if backend == "cpu":
                        readings.append(np.fromfile(log, "<f8").reshape(3, 3))
                    else:
                        rgb = np.fromfile(raw, "<f8").reshape(src.shape)
                        assert np.isfinite(rgb).all() and rgb[120, 144].mean() > rgb[120, 48].mean()
                        prints.append(rgb.clip(0, 1))
            for model, values in zip(("legacy", "candidate"), readings[1:]):
                drift = 100 * (10 ** (values - readings[0]) - 1)
                assert np.isfinite(drift).all()
                row[model + "_gray_layer_percent"] = drift.round(5).tolist()
            delta = np.abs(prints[1] - prints[0]).max(axis=-1) * 255
            row["p95_screen_codes"] = round(np.percentile(delta, 95), 2)
            row["gray_screen_codes"] = [(v[120, 48] * 255).round(2).tolist() for v in prints]
            row["hue_error_deg"] = {}
            for name, roi in regions.items():
                hues = [angle(lab(v[roi]).reshape(-1, 3).mean(axis=0)[1:]) for v in [src, *prints]]
                row["hue_error_deg"][name] = np.abs(wrap(np.array(hues[1:]) - hues[0])).round(2).tolist()
            if stock == "kodak_portra_400" and not cuts:
                # Fit against a uniform gray, then test the fit on the held-out photo.
                def print_with_trims(trims, image, label):
                    yellow, magenta, ev = trims
                    recipe["enlarger"] = {"y_filter_shift": yellow, "m_filter_shift": magenta,
                                          "print_exposure": 2 ** ev}
                    params.write_text(json.dumps(recipe))
                    proc = subprocess.run([candidate, "process", image, "--film", stock, *route, "--params", params,
                                           "--output", out / (label + ".png"), "--raw-out", raw], capture_output=True,
                                          env=dict(os.environ, SPEKTRAFILM_BACKEND="wgpu"))
                    assert proc.returncode == 0 and b"Backend: wgpu" in proc.stderr, proc.stderr
                    with Image.open(image) as im:
                        return np.fromfile(raw, "<f8").reshape(im.height, im.width, 3)

                # Use the exact original gray code in the montage.
                gray_code = np.rint(src[120, 48] * 255).astype(int)
                assert gray_code.min() == gray_code.max()
                tile = out / "fit-gray.png"
                Image.new("RGB", (1, 1), tuple(gray_code)).save(tile)
                trims = np.zeros(3)
                steps = np.array([0.5, 0.5, 0.01])
                target = prints[0][120, 48]
                for _ in range(6):
                    center = print_with_trims(trims, tile, "fit")[0, 0]
                    if np.max(np.abs(center - target)) * 255 < 0.05:
                        break
                    jac = np.column_stack([(print_with_trims(trims + d, tile, "fit")[0, 0] - center) / step
                                           for d, step in zip(np.diag(steps), steps)])
                    trims += np.linalg.solve(jac, target - center)
                matched = print_with_trims(trims, source, "portra-matched").clip(0, 1)
                residual = abs(matched - prints[0]).max(axis=-1) * 255
                assert residual[120, 48] < 0.1
                row["matched"] = {"y_m_ev": trims.tolist(), "gray_codes": float(residual[120, 48]),
                                  "p95_codes": float(np.percentile(residual, 95)),
                                  "crop_p95_codes": {n: float(np.percentile(residual[r], 95))
                                                     for n, r in regions.items()}}
            report[tag] = row
    (out / "report.json").write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("current", "candidate", "source", "out"):
        parser.add_argument(name, type=Path)
    main(**vars(parser.parse_args()))
