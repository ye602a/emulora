"""Convert the pinned CC BY-SA silver profiles. Supply a local LAB checkout
and the JSON array printed by the RS fit_multigrade example. No network I/O.
"""
import argparse
import json
from pathlib import Path
import subprocess

REV = "1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94"
PAPER = "ilford_multigrade_iv_rc"
STOCKS = ["kodak_tri_x_400", "kodak_tmax_100", "fujifilm_neopan_acros_100_ii",
          "ilford_hp5_plus_400", PAPER]


def single(rows):
    if any(not (a == b == c) for a, b, c in rows):
        raise ValueError("negative contains distinct emulsion responses")
    return [[r[0]] for r in rows]


def convert(stock, upstream, grades):
    source = f"{REV}:engine/resources_product/profiles/{stock}.json"
    profile = json.loads(subprocess.check_output(["git", "-C", str(upstream), "show", source]))
    data = profile["data"]
    if stock == PAPER:
        profile["info"]["channel_model"] = "multigrade_silver"
        data["paper_grades"] = grades
        changes = "Retained three emulsions and additive silver; RS grade calibration."
    else:
        for key in ("log_sensitivity", "density_curves"):
            data[key] = single(data[key])
        data["channel_density"] = [[sum(row)] for row in data["channel_density"]]
        for row in data["density_curves_layers"]:
            for j, layer in enumerate(row):
                if not (layer[0] == layer[1] == layer[2] or
                        all(v == 0 for i, v in enumerate(layer) if i != j)):
                    raise ValueError("unexpected sublayer channel layout")
                row[j] = [layer[j]]
        model = data["density_curves_model"]
        for key in ("centers", "amplitudes", "sigmas"):
            a, b, c = model[key]
            if not a == b == c:
                raise ValueError("negative contains distinct curve fits")
            model[key] = [a]
        changes = "Converted broadcast emulsion and fitted layers to single-channel RS storage; summed the silver-density weights."
    meta = profile["metadata"]
    meta["license"] = (f"SpektraLab / Hanze Qiu, https://github.com/JamesQiu2005/SpektraLab/tree/{REV}. "
                       "CC BY-SA 4.0, https://creativecommons.org/licenses/by-sa/4.0/. "
                       "RS adaptation: see data/SPEKTRALAB_BW_NOTICE.md.")
    meta["rs_changes"] = changes
    return profile


if __name__ == "__main__":
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("lab", type=Path)
    args.add_argument("grades", type=Path)
    args.add_argument("--out", type=Path, default=Path(__file__).resolve().parents[1] / "data/profiles")
    opts = args.parse_args()
    packs = json.loads(opts.grades.read_text(encoding="utf-8-sig"))
    if len(packs) != 7:
        raise ValueError("expected grades 00, 0, 1, 2, 3, 4 and 5")
    opts.out.mkdir(parents=True, exist_ok=True)
    for stock in STOCKS:
        result = convert(stock, opts.lab, packs)
        with (opts.out / (stock + ".json")).open("w", encoding="utf-8", newline="\n") as f:
            json.dump(result, f, ensure_ascii=False, indent=1, allow_nan=False)
            f.write("\n")
