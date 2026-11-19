"""Measure negative density granularity with a virtual 48 um aperture."""
import argparse
import json
import os
from pathlib import Path
import subprocess

import numpy as np
from PIL import Image


def aperture_readings(density, pitch):
    # Non-overlapping squares enclose one circular aperture each. Integrate
    # transmitted light, including fractional coverage of the boundary pixels.
    side = int(np.ceil(48 / pitch))
    sub = 32
    coord = ((np.arange(side * sub) + 0.5) / sub - side / 2) * pitch
    circle = coord[:, None]**2 + coord[None, :]**2 <= 24**2
    weights = circle.reshape(side, sub, side, sub).mean(axis=(1, 3))
    weights /= weights.sum()
    rows, cols = np.array(density.shape) // side
    blocks = density[:rows*side, :cols*side].reshape(rows, side, cols, side)
    return -np.log10(np.einsum("ijkl,jl->ik", 10.0**(-blocks), weights))


def run(cli, scratch, gpu, film, particle_area):
    scratch.mkdir(parents=True, exist_ok=True)
    silver = film == "kodak_doublex"
    density_path = scratch / "negative.f64"
    params = scratch / "recipe.json"

    def render(size, pitch, ev, active):
        src = scratch / "flat.png"
        Image.new("RGB", (size, size), (118,) * 3).save(src)
        grain = {"active": active, "gpu_multilayer": gpu}
        if particle_area is not None:
            grain["agx_particle_area_um2"] = particle_area
        params.write_text(json.dumps({
            "camera": {"auto_exposure": False, "film_format_mm": size*pitch/1000,
                       "exposure_compensation_ev": ev},
            "film_render": {"development_time": 6.5 if silver else None, "grain": grain,
                            "halation": {"active": False, "boost_ev": 0}},
            "print_render": {"glare": {"active": False}},
        }))
        density_path.unlink(missing_ok=True)
        cmd = [str(cli), "process", str(src), "--params", str(params),
                        "--film", film, "--paper", "kodak_2302" if silver else "kodak_portra_endura",
                        "--output", str(scratch / "output.png")]
        env = dict(os.environ)
        if gpu:
            cmd.extend(["--raw-out", str(density_path)])
            env.update(SPEKTRAFILM_BACKEND="wgpu")
        else:
            env.update(SPEKTRAFILM_BACKEND="cpu", SPEKTRAFILM_DUMP_FILM_DENSITY=str(density_path))
        proc = subprocess.run(cmd, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        mode = ("simple" if silver else "layered") if active else "off"
        if proc.returncode or (gpu and f"aperture probe captured resident density: {mode}" not in proc.stdout):
            raise RuntimeError(proc.stdout)
        density = np.fromfile(density_path, "<f8").reshape(size, size, 3)
        if not np.isfinite(density).all() or (silver and not np.all(density == density[..., :1])):
            raise ValueError("expected finite single-silver density replicated across RGB")
        return density[16:-16, 16:-16, :]

    # Locate D=1 on the resolved curve before introducing noise.
    low, high = -8.0, 8.0
    for _ in range(15):
        ev = (low + high) / 2
        mean = render(64, 6, ev, False)[..., 1].mean()
        if abs(mean - 1) <= 0.0005:
            break
        if mean < 1:
            low = ev
        else:
            high = ev
    if abs(mean - 1) > 0.0005:
        raise ValueError("could not match the reference density")
    results = []
    for pitch in (3, 6):
        for active in (False, True):
            # Match the sampled physical area at both pitches.
            image = render(32 + 3072 // pitch, pitch, ev, active)
            for channel in ((1,) if silver else (0, 1, 2)):
                flat = aperture_readings(image[..., channel], pitch).ravel()
                rms = flat.std(ddof=1)
                if channel == 1 and abs(flat.mean() - 1) > .015:
                    raise ValueError(f"{pitch=} {active=} reference mean={flat.mean():.6f}")
                if (active and rms <= 1e-4) or (not active and rms > 1e-4):
                    raise ValueError(f"{pitch=} {active=} rms={rms:.6f}")
                results.append(dict(film=film, backend="resident" if gpu else "cpu",
                                    area_override_um2=particle_area, channel="silver" if silver else "CMY"[channel],
                                    pitch_um=pitch, grain=active, ev=ev, apertures=flat.size,
                                    mean_D=flat.mean(), rms_1000=1000*rms,
                                    skew=float(np.mean(((flat-flat.mean())/rms)**3)) if active else None))
    (scratch / "measurements.json").write_text(json.dumps(results, indent=2))
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("scratch", type=Path)
    parser.add_argument("--gpu-probe", action="store_true", help="CLI built with the resident capture patch")
    parser.add_argument("--film", choices=("kodak_doublex", "kodak_portra_400"), default="kodak_doublex")
    parser.add_argument("--particle-area", type=float, help="override particle area in square micrometers")
    args = parser.parse_args()
    run(args.cli.resolve(), args.scratch.resolve(), args.gpu_probe, args.film, args.particle_area)
