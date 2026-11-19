"""Measure Arctic's reconstructed-white normalization on optical prints.

Run from the RS root with the CLI, Batch 13 input sheet and an output directory.
The solver matches the current print's middle gray using Y/M and print exposure.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory

import numpy as np
from PIL import Image, ImageDraw

from arctic_versions import display
from negative_hue_trims import CROPS


def measure(cli, source, output, scratch):
    output.mkdir(exist_ok=True, parents=True)
    data = scratch / "data"
    shutil.copytree("data", data)
    gray = scratch / "gray.png"
    levels = [32, 64, 118, 188, 235, 255]
    Image.fromarray(np.tile(np.uint8(levels)[None, :, None], (1, 1, 3))).save(gray)
    recipe = {
        "settings": {"rgb_to_raw_method": "arctic2026beta04"},
        "camera": {"auto_exposure": False},
        "film_render": {"grain": {"active": False}, "dir_couplers": {"active": False},
                        "halation": {"active": False, "boost_ev": 0}},
        "print_render": {"glare": {"active": False}}, "scanner": {"unsharp_mask": [0, 0]},
        "io": {"output_color_space": "sRGB", "output_cctf_encoding": False,
               "output_gamut_compress": {"algorithm": "off"}},
    }
    report = {}
    for film, paper in [("kodak_portra_400", "kodak_portra_endura"),
                        ("kodak_vision3_500t", "kodak_2383")]:
        def render(image, trims):
            y, m, ev = map(float, trims)
            recipe["enlarger"] = {"y_filter_shift": y, "m_filter_shift": m, "print_exposure": 2**ev}
            params = scratch / "params.json"
            params.write_text(json.dumps(recipe))
            log, raw = scratch / "log.bin", scratch / "raw.bin"
            for path in (log, raw):
                path.unlink(missing_ok=True)
            subprocess.run([cli, "process", image, "--film", film, "--paper", paper, "--data-dir",
                            data, "--params", params, "--output", scratch/"out.png", "--raw-out", raw],
                           env=dict(os.environ, SPEKTRAFILM_BACKEND="cpu",
                                    SPEKTRAFILM_DUMP_FILM_LOG_RAW=str(log)), check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            with Image.open(image) as im:
                shape = im.height, im.width, 3
            result = [np.fromfile(p, "<f8").reshape(shape) for p in (raw, log)]
            if not all(np.isfinite(v).all() for v in result):
                raise ValueError("non-finite print or film exposure")
            return result

        def fit(target):
            trims = zero.copy()
            steps = np.array([0.5, 0.5, 0.01])
            for _ in range(10):
                center = render(gray, trims)[0][0, 2]
                if np.max(np.abs(display(center) - display(target))) < 0.04:
                    break
                jac = np.column_stack([(render(gray, trims + offset)[0][0, 2] - center) / step
                                       for offset, step in zip(np.diag(steps), steps)])
                trims += np.linalg.solve(jac, target - center)
            return trims

        zero = np.zeros(3)
        base, base_log = render(gray, zero)
        target = base[0, 2]
        # Match luminance to the current gray while making its sRGB channels equal.
        neutral_target = np.repeat(target @ np.array([0.2126, 0.7152, 0.0722]), 3)
        current_trims = fit(neutral_target)
        current_neutral, _ = render(gray, current_trims)
        current_neutral_photo, _ = render(source, current_trims)
        photo, _ = render(source, zero)
        # The white cell of the integrated LUT is scaled at runtime by X+Y+Z.
        # A constant gain on each sensitivity channel scales every LUT cell identically.
        white_brightness = 0.95042966940215057 + 1 + 1.0888005470297513
        gains = white_brightness / (10**base_log[0, -1] - 1e-10)
        stock = data/"profiles"/(film+".json")
        profile = json.loads(stock.read_text())
        for row in profile["data"]["log_sensitivity"]:
            for c, value in enumerate(row):
                if value is not None:
                    row[c] += float(np.log10(gains[c]))
        stock.write_text(json.dumps(profile))
        fixed, norm_log = render(gray, zero)
        np.testing.assert_allclose(10**norm_log, (10**base_log - 1e-10)*gains + 1e-10, rtol=2e-5)
        fixed_photo, _ = render(source, zero)

        trims = fit(target)
        matched, matched_log = render(gray, trims)
        np.testing.assert_array_equal(matched_log, norm_log)
        assert np.max(abs(display(matched[0, 2])-display(target))) < 0.1
        matched_photo, _ = render(source, trims)

        photos = list(map(display, [photo, fixed_photo, matched_photo]))
        h, w, _ = photo.shape
        assert (w,h) == (480,144)
        sheet = Image.new("RGB", (w, (h+20)*3), "white")
        for i, (name, pic) in enumerate(zip(["Current", "Normalized, fixed pack",
                                              "Normalized, matched middle gray"], photos)):
            y = (h+20)*i
            ImageDraw.Draw(sheet).text((4, y+2), name, fill="black")
            sheet.paste(Image.fromarray(np.rint(pic).astype("uint8")), (0, y+20))
        sheet.save(output/(film+".png"))
        metrics = {}
        for tag, chart, pic in zip(["fixed", "matched"], [fixed, matched], photos[1:]):
            delta = abs(pic-photos[0]).max(axis=2)
            metrics[tag] = {
                "gray_max_channel_delta_codes": abs(display(chart)-display(base)).max(axis=2)[0].round(3).tolist(),
                "crop_p50_p95_codes": {name: np.percentile(delta[12:84, i*96+12:i*96+84],
                                      [50,95]).round(2).tolist() for i,(name,*_) in enumerate(CROPS)}}
        normalized_trims = fit(neutral_target)
        normalized_neutral, _ = render(gray, normalized_trims)
        normalized_neutral_photo, _ = render(source, normalized_trims)
        for chart in (current_neutral, normalized_neutral):
            assert np.max(abs(display(chart[0, 2])-display(neutral_target))) < 0.1
        neutral_delta = abs(display(normalized_neutral_photo)-display(current_neutral_photo)).max(axis=2)
        metrics["both_neutral"] = {
            "middle_gray_current_fixed_normalized_fixed": [display(v[0, 2]).round(2).tolist() for v in (base, fixed)],
            "current_y_m_ev": current_trims.tolist(), "normalized_y_m_ev": normalized_trims.tolist(),
            "gray_max_channel_delta_codes": abs(display(current_neutral)-display(normalized_neutral)).max(axis=2)[0].round(3).tolist(),
            "crop_p50_p95_codes": {name: np.percentile(neutral_delta[12:84,i*96+12:i*96+84],[50,95]).round(2).tolist()
                                   for i,(name,*_) in enumerate(CROPS)}}
        report[film] = dict(paper=paper, levels=levels, sensitivity_gain=gains.tolist(),
                            y_m_print_ev=trims.tolist(), **metrics)
        (output/"metrics.json").write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("cli", "source", "output"):
        parser.add_argument(arg, type=Path)
    args = parser.parse_args()
    with TemporaryDirectory() as temp:
        measure(args.cli.resolve(), args.source.resolve(), args.output.resolve(), Path(temp))
