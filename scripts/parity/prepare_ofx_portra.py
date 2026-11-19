"""Prepare a private RS data directory for the OFX 0.3.12 clear Portra reference."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import struct
import tempfile


def fingerprint(rows):
    return hashlib.sha256(b"".join(struct.pack("<3d", *row) for row in rows)).hexdigest()


def prepare(data, archive, destination):
    data, destination = data.resolve(), destination.resolve()
    if destination.exists() or destination.is_relative_to(data):
        raise ValueError("choose a new directory outside the source data tree")
    relative = Path("profiles/kodak_portra_400.json")
    profile = json.loads((data / relative).read_text(encoding="utf-8"))
    source = json.loads(archive.read_text(encoding="utf-8"))
    sensitivity = profile["data"]["log_sensitivity"]
    window = source["data"]["bandpass_hanatos2025"]
    if (source["info"]["stock"] != "kodak_portra_400"
            or source["data"]["wavelengths"] != profile["data"]["wavelengths"]
            or fingerprint(window) != "c6d4402905726bc224bdb9c899e124a307991be6b03218e15fe5635e25a0d0ab"
            or fingerprint(sensitivity) != "de0c9b1572b9ebd33c82ad3c5668d13ae2b8abdc45068530a09b481abead5798"):
        raise ValueError("these profiles do not match the measured Portra response")
    profile["data"]["log_sensitivity"] = [
        [s + math.log10(w) for s, w in zip(row, weights)]
        for row, weights in zip(sensitivity, window)
    ]
    profile["metadata"]["reference_variant"] = "Portra 400 with archived Hanatos 2025 bandpass (OFX 86476af)"
    with tempfile.TemporaryDirectory(dir=destination.parent) as staging:
        copy = Path(staging) / "data"
        shutil.copytree(data, copy)
        (copy / relative).write_text(json.dumps(profile, allow_nan=False), encoding="utf-8")
        copy.rename(destination)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="archived Portra 400 profile from OFX or original Python")
    parser.add_argument("destination", type=Path)
    parser.add_argument("--data", type=Path, default=Path("data"))
    args = parser.parse_args()
    prepare(args.data, args.archive, args.destination)
