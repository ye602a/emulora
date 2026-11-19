"""Write a Portra 400 / Endura reference recipe for an OFX 0.3.12 lens setting."""
import argparse
import json
from pathlib import Path


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cut", choices=("uv", "ir", "uv-ir"))
    parser.add_argument("output", type=argparse.FileType("x", encoding="utf-8"))
    parser.add_argument("--uncompensated", action="store_true", help="hold the clear print exposure fixed")
    args = parser.parse_args()
    base = Path(__file__).resolve().parents[2] / "recipes/ofx/portra_400_endura_clear.json"
    recipe = json.loads(base.read_text(encoding="utf-8"))
    camera = recipe["camera"]
    camera["normalize_camera_cuts"] = False
    for band in args.cut.split("-"):
        camera["filter_" + band][0] = 1
    if args.uncompensated:
        # Absolute exposure of the clear reference with RS print normalization disabled.
        recipe["enlarger"].update(normalize_print_exposure=False, print_exposure_compensation=False,
                                  print_exposure=0.812967445)
    with args.output as output:
        output.write(json.dumps(recipe, indent=2) + "\n")
