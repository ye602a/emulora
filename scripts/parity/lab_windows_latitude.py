"""Compare RS placement and real renders with LAB Windows using the same linear ProPhoto files.
Set PYTHONPATH to the shipping host_smoke.py directory. Also requires numpy, Pillow and tifffile.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess

from host_smoke import Host
import numpy as np
from PIL import Image
import tifffile


def linear(v):
    return np.where(v <= .04045, v/12.92, ((v+.055)/1.055)**2.4)


def run(cli, lab, source, out, compensate):
    out.mkdir(exist_ok=True, parents=True)
    im = Image.open(source).convert("RGB")
    im.thumbnail((256, 256), Image.Resampling.LANCZOS)
    image = linear(np.asarray(im)/255.) @ np.array([[.5293459,.3300728,.1405813],
        [.0983743,.8734610,.0281647],[.0168832,.1176725,.8654443]]).T
    # A second, independent wedge on a different stop grid from the medium probe.
    stops = np.linspace(-9, 8, 205)
    wedge = np.broadcast_to((.184*2**stops)[None,:,None], (8,len(stops),3))
    for tag, pixels in [("photo",image),("gray",wedge)]:
        tifffile.imwrite(out/(tag+".tif"), pixels.astype("<f4"), photometric="rgb", metadata=None)
    host = Host([str(lab/"spektralab-host.exe"), "--resources", str(lab/"host/engine")])
    def call(method, **params):
        reply, payload = host.call(method, params)
        assert reply["ok"], reply
        return reply["result"], payload
    report = {}
    try:
        for stock in ["kodak_portra_400","kodak_ektar_100","fujifilm_provia_100f"]:
            slide = stock == "fujifilm_provia_100f"
            for ev in [-2,0,2]:
                for tag in ["gray","photo"]:
                    path = (out/(tag+".tif")).resolve()
                    name = f"{stock}-{ev}-{tag}"
                    params = {"film_stock":stock,"print_stock":"kodak_portra_endura","scan_film":slide,
                              "output_color_space":"sRGB","auto_exposure":False,"exposure_compensation_ev":ev,
                              "grain_active":False,"dir_couplers_active":False,"halation_active":False,
                              "halation_boost_ev":0,"glare_active":False}
                    opened,_ = call("open",path=str(path),params=params)
                    sid = opened["session"]
                    lat,_ = call("scene_latitude",session=sid)
                    info,blob = call("render",session=sid,tier="full",format="rgba16")
                    call("close",session=sid)
                    shape = wedge.shape if tag=="gray" else image.shape
                    lab_rgb = np.frombuffer(blob,"<u2").reshape(*shape[:2],4)[:,:,:3]/65535
                    recipe={"camera":{"auto_exposure":False,"exposure_compensation_ev":ev},
                        "enlarger":{"print_exposure_compensation":compensate},
                        "scanner":{"unsharp_mask":[0,0],"white_correction":False,"black_correction":False},
                        "film_render":{"grain":{"active":False},"halation":{"active":False,"boost_ev":0},
                                       "dir_couplers":{"active":False},"glare":{"active":False}},
                        "print_render":{"glare":{"active":False}},
                        "io":{"input_color_space":"ProPhoto RGB","output_color_space":"sRGB","output_cctf_encoding":True}}
                    config=out/"recipe.json"
                    config.write_text(json.dumps(recipe))
                    route=["--scan-film"] if slide else ["--paper","kodak_portra_endura"]
                    args=[cli,"process",path,"--film",stock,*route,"--params",config,"--latitude",
                          "--output",out/(name+".png"),"--raw-out",out/"raw.bin"]
                    result=subprocess.run(args,env=dict(os.environ,SPEKTRAFILM_BACKEND="wgpu"),
                                          capture_output=True,text=True)
                    assert result.returncode==0 and "Backend: wgpu" in result.stderr,result.stderr
                    rs=json.loads(next(x[10:] for x in result.stderr.splitlines() if x.startswith("Latitude: ")))
                    rs_rgb=np.fromfile(out/"raw.bin","<f8").reshape(shape)
                    assert np.isfinite(rs_rgb).all()
                    row={"rs":rs,"lab_bounds":[lat["medium"][k] for k in ["shadow_ev","highlight_ev"]],
                         "lab_scene_extremes":[lat["scene"][k] for k in ["p0_1","p99_9"]]}
                    for engine,rgb,black,white in [("lab",lab_rgb,lat["medium"]["y_black"],lat["medium"]["y_white"]),
                                                   ("rs",rs_rgb,rs["black_y"],rs["white_y"])]:
                        y=linear(rgb) @ np.array([.2126,.7152,.0722])
                        lo,hi=black*10**.04,black+.9*(white-black)
                        row[engine+"_render_below_above_pct"]=[float(100*(y<lo).mean()),float(100*(y>hi).mean())]
                        if tag=="gray":
                            line=y.mean(axis=0)
                            def crossing(value):
                                inds=np.where((line[:-1]<=value)&(line[1:]>value))[0]
                                if not len(inds): return None
                                i=inds[0]
                                return float(np.interp(value,line[i:i+2],stops[i:i+2]))
                            row[engine+"_wedge_crossings"]=[crossing(lo),crossing(hi)]
                        elif ev==0:
                            Image.fromarray(np.rint(rgb.clip(0,1)*255).astype("uint8")).save(out/(name+"-"+engine+".png"))
                    measured=rs["output"]
                    pct=[100*measured[k]/measured["sampled"] for k in ("below_shadow","above_highlight")]
                    np.testing.assert_allclose(pct,row["rs_render_below_above_pct"],atol=0.02)
                    bounds=[rs["shadow_ev"],rs["highlight_ev"]]
                    if tag=="gray":
                        np.testing.assert_allclose(bounds,row["rs_wedge_crossings"],atol=.01)
                        np.testing.assert_allclose(row["lab_bounds"],row["lab_wedge_crossings"],atol=.01)
                    if compensate:
                        np.testing.assert_allclose(bounds,row["lab_bounds"],atol=.02)
                    report[name]=row
                    (out/"report.json").write_text(json.dumps(report,indent=2))
    finally:
        host.call("shutdown")
        host.p.wait(timeout=10)


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("cli","lab","source","out"):
        parser.add_argument(name,type=Path)
    parser.add_argument("--compensate", action="store_true", help="Match the Windows LAB release enlarger")
    run(**vars(parser.parse_args()))
