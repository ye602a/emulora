"""Verify the black-and-white / grade 2 yellow-filter recipes against the Windows LAB host.

Run from the RS root. Put the release tag's engine/tests/host_smoke.py on
PYTHONPATH. Inputs are sRGB PNGs; both engines receive the same linear TIFF.
"""
import argparse
from itertools import product
import json
import os
from pathlib import Path
import subprocess

import numpy as np
from PIL import Image
import tifffile
from host_smoke import Host


def shared_input(source, path):
    image = Image.open(source).convert("RGB")
    image.thumbnail((512, 512), Image.Resampling.LANCZOS)
    rgb = np.asarray(image, dtype=np.float64) / 255
    linear = np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)
    # Same linear ProPhoto transport as lab_windows_print.py.
    matrix = np.array([[.5293459, .3300728, .1405813], [.0983743, .8734610, .0281647],
                       [.0168832, .1176725, .8654443]])
    tifffile.imwrite(path, (linear @ matrix.T).astype("<f4"), photometric="rgb", metadata=None)
    return rgb.shape


def run(args):
    args.out.mkdir(parents=True, exist_ok=True)
    resources = args.lab / "host/engine"
    grade = json.loads((resources / "paper_grades.json").read_text())["grades"]["2"]
    host = Host([str(args.lab / "spektralab-host.exe"), "--resources", str(resources)])

    def call(method, **params):
        reply, data = host.call(method, params)
        if not reply["ok"]:
            raise RuntimeError((method, reply))
        return reply["result"], data

    results = []
    try:
        reference, _ = call("hello")
        assert reference["host_version"] == "0.1.0+9ceca33", reference["host_version"]
        assert reference["backend"]["render_core"] == "native-vulkan", reference["backend"]
        for index, source in enumerate(args.photo):
            tif = (args.out / f"{index}-{source.stem}.tif").resolve()
            shape = shared_input(source, tif)
            for film, glass in product(args.films, args.filters):
                name = f"{tif.stem}-{film}-{glass}"
                params = dict(film_stock=film, print_stock="ilford_multigrade_iv_rc",
                              output_color_space="sRGB", camera_filter="" if glass == "clear" else glass,
                              film_format_mm=35, auto_exposure=False, grain_active=False,
                              glare_active=False, dir_couplers_active=False, halation_active=False,
                              halation_boost_ev=0, c_filter_neutral=0, **grade)
                opened, _ = call("open", path=str(tif), params=params)
                assert opened["detected_input"]["input_color_space"] == "ProPhoto RGB"
                assert not opened["detected_input"]["input_cctf_decoding"]
                for key, value in params.items():
                    assert opened["params"][key] == value, (key, value, opened["params"][key])
                session = opened["session"]
                _, blob = call("render", session=session, tier="full", format="rgba16")
                call("close", session=session)
                lab = np.frombuffer(blob, "<u2").reshape(*shape[:2], 4)[:, :, :3] / 257
                Image.fromarray(lab.round().astype("uint8")).save(args.out / f"{name}-lab.png")

                recipe = "mg2-clear" if glass == "clear" else f"trix400-mg2-{glass}"
                for backend in ("cpu", "wgpu"):
                    raw = args.out / f"{name}-{backend}.bin"
                    p = subprocess.run([
                        args.cli, "process", tif, "--film", params["film_stock"], "--paper", params["print_stock"],
                        "--params", f"recipes/spektralab/{recipe}.json",
                        "-o", raw.with_suffix(".png"), "--raw-out", raw,
                    ], env={**os.environ, "SPEKTRAFILM_BACKEND": backend}, capture_output=True, check=True)
                    expected = b"CPU (rayon)" if backend == "cpu" else b"wgpu (GPU)"
                    assert expected in p.stderr, p.stderr
                    rs = np.fromfile(raw, "<f8").reshape(shape) * 255
                    error = np.max(np.abs(rs - lab), axis=2)
                    p50, p95, p99 = np.percentile(error, [50, 95, 99])
                    results.append(dict(image=source.name, film=film, filter=glass, backend=backend,
                                        p50=p50, p95=p95, p99=p99, peak=float(error.max())))
    finally:
        host.call("shutdown")
    (args.out / "metrics.json").write_text(json.dumps({"reference": reference["build_info"], "backend": reference["backend"],
                                                   "cases": results}, indent=2, allow_nan=False))
    for row in results:
        print(row)
    # Full-frame 8-bit-equivalent display codes, maximum channel per pixel.
    if any(row["p95"] > 2 or row["p99"] > 4 for row in results):
        raise SystemExit("LAB reference tolerance exceeded; see metrics.json")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--lab", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--films", nargs="+", default=["kodak_tri_x_400"], choices=[
        "kodak_tri_x_400", "kodak_tmax_100", "fujifilm_neopan_acros_100_ii", "ilford_hp5_plus_400"])
    parser.add_argument("--filters", nargs="+", default=["clear", "w8", "w12"], choices=["clear", "w8", "w12"])
    parser.add_argument("photo", nargs="+", type=Path)
    run(parser.parse_args())
