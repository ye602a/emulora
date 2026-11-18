"""Compare optical and negative-derived print hue using an isolated, trim-enabled CLI.

Run from the repository root with a fixture directory (Kodak 03/04/15 and the
500x610 sari preview named textile.png), a scratch directory and the probe exe.
Only the two Y/M eligibility checks are removed in the probe build.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess

import numpy as np
from PIL import Image, ImageDraw

PAIRS = [
    ("kodak_portra_400", "kodak_portra_endura"),
    ("kodak_gold_200", "fujifilm_crystal_archive_typeii"),
    ("kodak_ektar_100", "kodak_endura_premier"),
    ("fujifilm_pro_400h", "fujifilm_crystal_archive_typeii"),
]
# 96 px photo crops at original scale. Exclude a 12 px margin around each
# tile to keep measurements away from neighbouring subjects during local DIR.
CROPS = [
    ("blue-cap", "kodim03", 602, 309),
    ("red-cap", "kodim03", 293, 214),
    ("silk", "textile", 190, 412),
    ("cheek04", "kodim04", 133, 347),
    ("cheek15", "kodim15", 388, 217),
]
TRIMS = [(0, 0), (5, 0), (-5, 0), (0, 5), (0, -5), (10, -10)]


def oklab(rgb):
    lms = np.cbrt(rgb @ np.array([[.4122214708, .5363325363, .0514459929],
                                  [.2119034982, .6806995451, .1073969566],
                                  [.0883024619, .2817188376, .6299787005]]).T)
    return lms @ np.array([[.2104542553, .7936177850, -.0040720468],
                           [1.9779984951, -2.4285922050, .4505937099],
                           [.0259040371, .7827717662, -.8086757660]]).T


def angle(ab):
    return np.degrees(np.arctan2(ab[..., 1], ab[..., 0]))


def wrap(angle_deg):
    return (angle_deg + 180) % 360 - 180


def run(args):
    out = args.scratch.resolve()
    out.mkdir(parents=True, exist_ok=True)
    sheet = Image.new("RGB", (96 * len(CROPS), 96))
    for idx, (_, file, x, y) in enumerate(CROPS):
        with Image.open(args.fixtures / (file + ".png")) as src:
            sheet.paste(src.crop((x, y, x + 96, y + 96)), (idx * 96, 0))
    sheet.save(out / "source.png")
    encoded = np.asarray(sheet) / 255
    source = oklab(np.where(encoded <= .04045, encoded / 12.92, ((encoded + .055) / 1.055)**2.4))
    recipe = {
        "camera": {"auto_exposure": False, "film_format_mm": sheet.width * .005},
        "film_render": {"grain": {"active": False}, "halation": {"active": False, "boost_ev": 0},
                        "dir_couplers": {"active": True}},
        "print_render": {"glare": {"active": False}},
        "scanner": {"unsharp_mask": [0, 0]},
        "io": {"output_color_space": "sRGB", "output_cctf_encoding": False,
               "output_gamut_compress": {"algorithm": "off"}},
    }
    preview = Image.new("RGB", (650, 5 * 96), "white")
    labels = ImageDraw.Draw(preview)
    preview.paste(sheet, (170, 0))
    labels.text((8, 39), "Input sRGB", fill="black")
    negative = out / "negative.bin"
    env = dict(os.environ, SPEKTRAFILM_BACKEND="cpu", SPEKTRAFILM_DUMP_FILM_DENSITY=str(negative))
    with (out / "measurements.tsv").open("w", newline="") as f:
        rows = csv.writer(f, delimiter="\t")
        rows.writerow(("film", "paper", "Y", "M", "region", "pixels", "negative_sha256",
                       "opt_source_deg", "hue_source_deg", "opt_trim_deg", "hue_trim_deg",
                       "trim_disagreement_deg"))
        for film, paper in PAIRS:
            digest = None
            for y, m in TRIMS:
                renders = []
                for active in [False, True]:
                    tag = f"{film}-{paper}-y{y}m{m}-{int(active)}"
                    recipe["enlarger"] = {"y_filter_shift": y, "m_filter_shift": m}
                    recipe["print_render"]["negative_hue"] = active
                    params = out / "recipe.json"
                    params.write_text(json.dumps(recipe))
                    negative.unlink(missing_ok=True)
                    with (out / (tag + ".log")).open("w") as log:
                        subprocess.run([str(args.cli.resolve()), "process", str(out / "source.png"),
                                        "--film", film, "--paper", paper, "--params", str(params),
                                        "--output", str(out / (tag + ".png")),
                                        "--raw-out", str(out / "print.rgb")],
                                       stdout=log, stderr=log, env=env, check=True)
                    raw = np.fromfile(out / "print.rgb", "<f8").reshape(source.shape)
                    if not np.isfinite(raw).all() or negative.stat().st_size != raw.nbytes:
                        raise ValueError(tag + ": invalid render or density dump")
                    current = hashlib.sha256(negative.read_bytes()).hexdigest()
                    if digest is not None and current != digest:
                        raise ValueError(tag + ": developed negative changed")
                    digest = current
                    renders.append(angle(oklab(raw)[..., 1:]))
                    if film == "kodak_ektar_100" and (y, m) in [(0, 0), (0, 5)]:
                        line = 1 + int(active) + 2 * int(m != 0)
                        rgb = np.clip(raw, 0, 1)
                        encoded = np.where(rgb <= .0031308, 12.92 * rgb, 1.055 * rgb**(1/2.4) - .055)
                        preview.paste(Image.fromarray(np.rint(255 * encoded).astype("uint8")), (170, 96 * line))
                        mode = "Negative hue" if active else "Optical"
                        labels.text((8, 39 + 96 * line), f"{mode}  Y{y:+} M{m:+}", fill="black")
                if (y, m) == (0, 0):
                    neutral = renders
                for i, (name, *_) in enumerate(CROPS):
                    roi = np.s_[12:84, i * 96 + 12:i * 96 + 84]
                    src = source[roi]
                    hue = angle(src[..., 1:])
                    mask = (src[..., 0] > .19) & (np.linalg.norm(src[..., 1:], axis=-1) > .035)
                    if name in ("blue-cap", "silk"):
                        lo, hi = (240, 315) if name == "blue-cap" else (265, 355)
                        mask &= (hue % 360 > lo) & (hue % 360 < hi)
                    if name == "red-cap":
                        mask &= np.linalg.norm(src[..., 1:], axis=-1) > .12
                    if name.startswith("cheek"):
                        # Cheek rectangles in the original upright Kodak photographs.
                        left, top, right, bottom = ((139, 361, 225, 430) if name == "cheek04"
                                                    else (408, 256, 461, 297))
                        yy, xx = np.indices(mask.shape)
                        _, _, crop_x, crop_y = CROPS[i]
                        xx += crop_x + 12
                        yy += crop_y + 12
                        mask &= (xx >= left) & (xx < right) & (yy >= top) & (yy < bottom)
                    if not mask.any():
                        raise ValueError(name + ": empty region")
                    errors = [wrap(h[roi] - hue)[mask] for h in renders]
                    moves = [wrap(h[roi] - n[roi])[mask] for h, n in zip(renders, neutral)]
                    values = errors + moves + [wrap(moves[1] - moves[0])]
                    rows.writerow([film, paper, y, m, name, mask.sum(), digest] +
                                  [round(float(np.median(abs(v))), 3) for v in values])
                f.flush()
    preview.save(out / "ektar-premier.png")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("fixtures", "scratch", "cli"):
        parser.add_argument(key, type=Path)
    run(parser.parse_args())
