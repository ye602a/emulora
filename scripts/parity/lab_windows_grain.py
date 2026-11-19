"""Decode LAB's normalized DI transport and measure 48 µm grain apertures.

Use host_smoke.py from the Windows beta.3 release tag on PYTHONPATH.
Compare to the matching Portra case in grain_aperture.py (6 µm, +1.984375 EV).
"""
import argparse
import json
from pathlib import Path
import struct

from host_smoke import Host
import numpy as np
from PIL import Image
import tifffile

from grain_aperture import aperture_readings


def lut_endpoints(resources):
    blob = (resources / "spektrafilm_constants.bin").read_bytes()
    fmt = "<64sIIIIIIQQ"
    assert blob[:4] == b"SPKR" and int.from_bytes(blob[4:8], "little") == 1
    for i in range(int.from_bytes(blob[8:12], "little")):
        name, dtype, rank, channels, size, _, _, offset, length = struct.unpack_from(
            fmt, blob, 16+i*struct.calcsize(fmt))
        if name.rstrip(b"\0") == b"print_lut_axes/kodak_portra_endura":
            assert rank == 2 and channels == 3
            axes = np.frombuffer(blob[offset:offset+length], ["<f8","<f4","<f2"][dtype]).reshape(3,size)
            return axes[:,0], axes[:,-1]
    raise ValueError("missing Portra Endura DI axes")


def run(lab, out):
    resources = lab/"host/engine"
    lo, hi = lut_endpoints(resources)
    out.mkdir(parents=True, exist_ok=True)
    size = 1056
    source = out/"grain-flat.png"
    Image.new("RGB", (size,size), (118,)*3).save(source)
    host = Host([str(lab/"spektralab-host.exe"), "--resources", str(resources)])

    def call(method, **params):
        reply, _ = host.call(method, params)
        if not reply["ok"]:
            raise ValueError(reply)
        return reply["result"]

    results = {}
    try:
        for active in (False,True):
            params = {"film_stock":"kodak_portra_400","print_stock":"kodak_portra_endura",
                      "auto_exposure":False,"halation_active":False,"halation_boost_ev":0,
                      "glare_active":False,"film_format_mm":size*.006,"grain_active":active,
                      "exposure_compensation_ev":1.984375}
            sid = call("open", path=str(source), params=params)["session"]
            path = out/("grain-on.tif" if active else "grain-off.tif")
            call("export_di", session=sid, path=str(path), overwrite=True)
            call("close", session=sid)
            raw = tifffile.imread(path)
            assert raw.min() > 0 and raw.max() < 65535
            density = lo + raw.astype(float)/65535*(hi-lo)
            stats = []
            for y,x in [(16,16),(16,528),(528,16),(528,528)]:
                block = density[y:y+512,x:x+512]
                readings = np.stack([aperture_readings(block[...,c],6).ravel() for c in range(3)],axis=1)
                mean = readings.mean(axis=0)
                centered = readings - mean
                sd = readings.std(axis=0,ddof=1)
                row = {"mean":mean.tolist(),"rms_1000":(1000*sd).tolist()}
                if active:
                    row["channel_correlation"] = np.corrcoef(readings.T).tolist()
                    row["skew"] = np.mean((centered/sd)**3, axis=0).tolist()
                else:
                    assert abs(mean[1]-1) < .001
                stats.append(row)
            results["on" if active else "off"] = stats
    finally:
        host.call("shutdown")
        host.p.wait(timeout=10)
    (out/"grain-metrics.json").write_text(json.dumps(results,indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lab", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    run(args.lab.resolve(), args.out.resolve())
