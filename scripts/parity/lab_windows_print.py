"""Compare shipping LAB Vulkan and RS Vulkan on one linear ProPhoto TIFF.

Supply host_smoke.py from LAB's spektralab-windows-v1.3.4-beta.3 tag via PYTHONPATH,
along with tifffile. Run from the RS root. All inputs/outputs are local.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

import numpy as np
from PIL import Image
import tifffile
from host_smoke import Host

from negative_hue_trims import CROPS


def rs_print(cli, image, film, paper, recipe, out, name, shape):
    (out/"params.json").write_text(json.dumps(recipe))
    proc = subprocess.run([cli,"process",image,"--film",film,"--paper",paper,"--params",out/"params.json",
                           "--output",out/"out.png","--raw-out",out/"raw.bin"],
                          env=dict(os.environ,SPEKTRAFILM_BACKEND="wgpu"), check=True,
                          capture_output=True, text=True)
    (out/(name+".log")).write_text(proc.stderr)
    assert "Backend: wgpu" in proc.stderr
    rs = np.fromfile(out/"raw.bin","<f8").reshape(*shape,3).clip(0,1)*255
    assert np.isfinite(rs).all()
    return rs


def run(cli, lab, source, out):
    out.mkdir(exist_ok=True, parents=True)
    tif = out / "shared.tif"
    # Same quantized linear input in both engines. Bradford sRGB (D65) to ProPhoto (D50).
    srgb = np.asarray(Image.open(source).convert("RGB"), dtype=float) / 255
    linear = np.where(srgb <= 0.04045, srgb/12.92, ((srgb+0.055)/1.055)**2.4)
    matrix = np.array([[.5293459,.3300728,.1405813],[.0983743,.8734610,.0281647],
                       [.0168832,.1176725,.8654443]])
    tifffile.imwrite(tif, np.rint((linear @ matrix.T).clip(0,1)*65535).astype("<u2"),
                     photometric="rgb", metadata=None)
    h, w = linear.shape[:2]
    assert (w,h) == (480,144)
    host = Host([str(lab/"spektralab-host.exe"), "--resources", str(lab/"host/engine")])

    def call(method, **params):
        reply, payload = host.call(method, params)
        if not reply["ok"]:
            raise ValueError((method, reply))
        return reply["result"], payload

    try:
        hello, _ = call("hello")
        report = {"lab_backend": hello["backend"], "build": hello["build_info"],
                  "input_sha256": hashlib.sha256(tif.read_bytes()).hexdigest(), "cases": {}}
        for film, paper in [("kodak_portra_400","kodak_portra_endura"),
                            ("kodak_vision3_500t","kodak_2383"),
                            ("kodak_vision3_200t","kodak_2393"), ("kodak_portra_400","kodak_2383"), ("kodak_vision3_500t","kodak_portra_endura")]:
            for mode, y, m, flash in [("optical",0,0,0), ("filters",5,-5,0), ("preflash",0,0,0.1)]:
                if (film,paper) not in [("kodak_portra_400","kodak_portra_endura"), ("kodak_vision3_500t","kodak_2383")] and mode != "optical":
                    continue
                name = film+"-"+paper+"-"+mode
                opened, _ = call("open", path=str(tif), params={"film_stock":film, "print_stock":paper,
                                 "output_color_space":"sRGB", "auto_exposure":False,"grain_active":False,
                                 "glare_active":False,"halation_active":False,"halation_boost_ev":0,
                                 "dir_couplers_active":False, "y_filter_shift":y/5,"m_filter_shift":m/5,"filter_shift_scale":5,
                                 "preflash_exposure":flash,"film_format_mm":35})
                sid = opened["session"]
                resolved = opened["params"]
                for key,expected in {"film_stock":film,"print_stock":paper,"grain_active":False,
                        "dir_couplers_active":False,"glare_active":False,"halation_active":False,
                        "auto_exposure":False,"y_filter_shift":y/5,"m_filter_shift":m/5,"filter_shift_scale":5,
                        "preflash_exposure":flash,"output_color_space":"sRGB"}.items():
                    assert resolved[key] == expected,(key,resolved[key],expected)
                info, blob = call("render",session=sid,tier="full",format="rgba16")
                assert info["image"]["color_space"] == "sRGB"
                lab_rgb = np.frombuffer(blob,"<u2").reshape(h,w,4)[:,:,:3] / 257
                call("close",session=sid)
                recipe={"camera":{"auto_exposure":False,"film_format_mm":35},
                        "film_render":{"grain":{"active":False},"dir_couplers":{"active":False},
                                       "halation":{"active":False,"boost_ev":0}},
                        "print_render":{"glare":{"active":False}},
                        "io":{"input_color_space":"ProPhoto RGB","output_color_space":"sRGB"},
                        "enlarger":{"y_filter_shift":y,"m_filter_shift":m,"preflash_exposure":flash}}
                rs = rs_print(cli, tif, film, paper, recipe, out, name, (h,w))
                diff=np.max(abs(rs-lab_rgb),axis=2)
                if mode=="optical":
                    base_rs,base_lab=rs,lab_rgb
                delta=np.max(abs((rs-base_rs)-(lab_rgb-base_lab)),axis=2)
                report["cases"][name]={"lab_neutral_cmy":[resolved[k+"_filter_neutral"] for k in "cmy"],
                    "gray_rs_lab":[rs[120,48].round(2).tolist(),lab_rgb[120,48].round(2).tolist()],
                    "p50_p95":np.percentile(diff,[50,95]).round(2).tolist(),
                    "change_disagreement_p95":round(float(np.percentile(delta,95)),2),
                    "crop_p95":{key:round(float(np.percentile(diff[12:84,96*i+12:96*i+84],95)),2)
                                for i,(key,*_) in enumerate(CROPS)}}
                sheet=np.vstack([rs,lab_rgb])
                Image.fromarray(np.rint(sheet).astype("uint8")).save(out/(name+".png"))
                (out/"metrics.json").write_text(json.dumps(report,indent=2))
        # A point on a gray pedestal keeps the small response above the print toe.
        h,w = 129,801
        samples = np.full((h,w,3),12000,dtype="<u2")
        samples[h//2,w//2] = 65535
        point = out/"point.tif"
        tifffile.imwrite(point,samples,photometric="rgb",metadata=None)
        film,paper = "kodak_portra_400","kodak_portra_endura"
        radius = np.hypot(*np.indices((h,w))-np.array([h//2,w//2])[:,None,None])
        order = np.argsort(radius,axis=None)
        report["lens"] = {}
        for sigma in (0,10,25):
            opened,_ = call("open",path=str(point),params={
                "film_stock":film,"print_stock":paper,"output_color_space":"sRGB",
                "auto_exposure":False,"grain_active":False,"glare_active":False,"halation_active":False,
                "dir_couplers_active":False,"halation_boost_ev":0,"film_format_mm":w*.005,"lens_blur_um":sigma})
            sid = opened["session"]
            info,blob = call("render",session=sid,tier="full",format="rgba16")
            assert info["image"]["color_space"] == "sRGB"
            lab_rgb = np.frombuffer(blob,"<u2").reshape(h,w,4)[:,:,:3]/257
            call("close",session=sid)
            recipe["camera"].update(lens_blur_um=sigma,film_format_mm=w*.005)
            recipe["enlarger"] = {}
            rs = rs_print(cli,point,film,paper,recipe,out,"point-"+str(sigma),(h,w))
            row = {"max_display_error":float(abs(rs-lab_rgb).max())}
            for engine,pixels in [("rs",rs),("lab",lab_rgb)]:
                response = abs(pixels[...,1]-pixels[0,0,1])
                mass = np.cumsum(response.ravel()[order])
                row[engine+"_r90_um"] = 5*float(radius.ravel()[order[np.searchsorted(mass,.9*mass[-1])]])
            report["lens"][sigma] = row
        (out/"metrics.json").write_text(json.dumps(report,indent=2))
    finally:
        host.call("shutdown")
        host.p.wait(timeout=10)


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    for arg in ("cli","lab","source","out"):
        parser.add_argument(arg,type=Path)
    a=parser.parse_args()
    run(*(p.resolve() for p in (a.cli,a.lab,a.source,a.out)))
