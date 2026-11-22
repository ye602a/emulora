#!/usr/bin/env python3
"""Inventory the bundled engine data. Run with --write after intentional data changes.

Sizes and SHA-256 use canonical LF for text and exact bytes for binary files
and the frozen negative calibration JSONs. See the checkout rules in .gitattributes.
"""

import argparse
import hashlib
import json
from pathlib import Path
import sys


DATA = Path(__file__).resolve().parents[1] / "data"
MANIFEST = DATA / "MANIFEST.json"
TEXT_SUFFIXES = {".json", ".md", ".toml", ".txt"}


def classify(path):
    if path.startswith("profiles/") and path.endswith(".json"):
        return "film-or-paper-profile", "CC-BY-SA-4.0"
    if path == "filters/neutral_print_filters.json":
        return "print-filter-calibration", "CC-BY-SA-4.0"
    if path.startswith("luts/spectral_upsampling/") and Path(path).suffix in {".npy", ".toml"}:
        return "spectral-upsampling", "CC-BY-SA-4.0"
    if path == "negative_scan_ap0_v1/heldout.json":
        return "validation-fixture", "CC-BY-SA-4.0"
    if path.startswith("negative_scan_ap0_v1/") and path.endswith(".json"):
        return "negative-scan-calibration", "CC-BY-SA-4.0"
    if path in {
        "CHANGELOG.txt",
        "SPEKTRALAB_BW_NOTICE.md",
        "negative_scan_ap0_v1/CHANGELOG.txt",
        "negative_scan_ap0_v1/PROVENANCE.md",
    }:
        return "data-documentation", "GPL-3.0-only"
    raise ValueError("No data category and license assigned to " + path)


def describe(path, file):
    category, license_id = classify(path)
    content = file.read_bytes()
    frozen_json = path.startswith("negative_scan_ap0_v1/") and file.suffix == ".json"
    if file.suffix in TEXT_SUFFIXES and not frozen_json:
        content = content.replace(b"\r\n", b"\n")
    return {
        "path": path,
        "bytes": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
        "category": category,
        "license": license_id,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="regenerate the manifest")
    args = parser.parse_args()

    try:
        files = {
            file.relative_to(DATA).as_posix(): file
            for file in DATA.rglob("*")
            if file.is_file() and file != MANIFEST
        }
        if args.write:
            entries = [describe(path, files[path]) for path in sorted(files)]
            document = {"version": 1, "files": entries}
            MANIFEST.write_bytes((json.dumps(document, indent=2) + "\n").encode("utf-8"))
            print("Wrote data/MANIFEST.json ({} files)".format(len(entries)))
            return 0

        document = json.loads(MANIFEST.read_text(encoding="utf-8"))
        if document["version"] != 1:
            raise ValueError("Unsupported data manifest version")
        entries = document["files"]
        paths = [entry["path"] for entry in entries]
        if paths != sorted(set(paths)):
            raise ValueError("Manifest paths must be unique and sorted")
        expected = set(paths)
        actual = files.keys()
        problems = ["Missing data file: " + path for path in sorted(expected - actual)]
        problems += ["Unlisted data file: " + path for path in sorted(actual - expected)]
        for entry in entries:
            path = entry["path"]
            if path in actual and describe(path, files[path]) != entry:
                problems.append("Data content or metadata changed: " + path)
        if problems:
            print("\n".join(problems), file=sys.stderr)
            return 1
        print("Verified data/MANIFEST.json ({} files)".format(len(entries)))
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("Data manifest error: {}".format(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
