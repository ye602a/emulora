"""Compare RS metering to the shipping LAB host on exactly the same small PNG.
Put LAB's host_smoke.py on PYTHONPATH. Run from the RS root.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

import numpy as np
from PIL import Image
from host_smoke import Host


def run(cli, lab, source, out):
    out.mkdir(parents=True, exist_ok=True)
    photo = Image.open(source).convert("RGB")
    photo.thumbnail((256, 256), Image.Resampling.BOX)
    photo.save(out / "photo.png")
    # Backlit target with dark surroundings plus a broad bright region.
    scene = np.full((90, 120, 3), 64, dtype=np.uint8)
    scene[:, :15] = 250
    scene[:, -15:] = 3
    Image.fromarray(scene).save(out / "backlit.png")
    host = Host([str(lab/"spektralab-host.exe"), "--resources", str(lab/"host/engine")])
    results = {}
    try:
        for name in ["photo", "backlit"]:
            image = out / (name + ".png")
            opened, _ = host.call("open", {"path": str(image.resolve()), "params": {
                "film_stock": "kodak_portra_400", "auto_exposure_method": "balanced"}})
            assert opened["ok"], opened
            session = opened["result"]["session"]
            solved, _ = host.call("solve", {"session": session, "target": "exposure"})
            assert solved["ok"], solved
            refs = solved["result"]["exposure_ev_by_method"]
            assert set(refs) == {"balanced", "center", "protect_highlights", "protect_shadows"}
            host.call("close", {"session": session})
            results[name] = {}
            for method, lab_ev in refs.items():
                row = {"lab_ev": lab_ev}
                for backend in ["cpu", "wgpu"]:
                    renders = []
                    for automatic in [True, False]:
                        camera = {"auto_exposure": automatic, "auto_exposure_method": method,
                                  "exposure_compensation_ev": 0 if automatic else row[backend+"_ev"]}
                        recipe = {"camera": camera, "film_render": {"grain": {"active": False},
                                  "halation": {"active": False, "boost_ev": 0}, "dir_couplers": {"active": False}},
                                  "print_render": {"glare": {"active": False}}, "enlarger": {"print_exposure_compensation": False}}
                        params = out/"params.json"
                        params.write_text(json.dumps(recipe))
                        png = out/(f"{name}-{method}-{backend}.png" if automatic else "manual.png")
                        raw = out/"raw.bin"
                        proc = subprocess.run([cli, "process", image, "--film", "kodak_portra_400",
                            "--paper", "kodak_portra_endura", "--params", params, "--raw-out", raw, "-o", png],
                            capture_output=True, text=True, env=dict(os.environ, SPEKTRAFILM_BACKEND=backend,
                            RUST_LOG="spektrafilm_core::stages::filming=info"))
                        assert proc.returncode == 0 and f"backend: {backend}" in proc.stderr.lower(), proc.stderr
                        if automatic:
                            log = re.sub(r"\x1b\[[0-9;]*m", "", proc.stdout + proc.stderr)
                            row[backend+"_ev"] = float(re.search(r"\bev=(-?[\d.eE+\-]+)", log)[1])
                        renders.append(np.fromfile(raw, "<f8"))
                    delta = float(np.max(abs(renders[0] - renders[1])))
                    assert delta < 2e-5, (name, method, backend, delta)
                    row[backend+"_manual_max_delta"] = delta
                assert row["cpu_ev"] == row["wgpu_ev"]
                tolerance = 0.02 if name == "photo" else 1e-5
                assert abs(row["cpu_ev"] - lab_ev) < tolerance, (name, method, row)
                results[name][method] = row
            (out/"metrics.json").write_text(json.dumps(results, indent=2))
    finally:
        host.call("shutdown")
        host.p.wait(timeout=10)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("cli", "lab", "source", "out"):
        parser.add_argument(arg, type=Path)
    run(**vars(parser.parse_args()))
