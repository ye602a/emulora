"""Compare OFX public camera-response formulas on the RS optical print path."""
import argparse
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

import numpy as np
from PIL import Image, ImageDraw
from negative_hue_trims import CROPS
from arctic_versions import display, PAIRS


def run(cli, ofx, fixtures, output):
    output.mkdir(exist_ok=True, parents=True)
    crops = CROPS + [("foliage", "kodim18", 414, 10)]
    chart = Image.new("RGB", (96*len(crops), 96))
    for i, (_, name, x, y) in enumerate(crops):
        with Image.open(fixtures/(name+".png")) as src:
            chart.paste(src.crop((x,y,x+96,y+96)), (96*i,0))
    chart.save(output/"input.png")
    source = Path("crates/spektrafilm-math/src/spectral.rs").read_text(encoding="utf-8")
    spd = re.search(r"const ILLUMINANT_D55_F64: [^=]+= \[([^\]]+)",source).group(1)
    light = np.array([float(x) for x in spd.split(",") if x.strip()])[:,None]
    report = {}
    with tempfile.TemporaryDirectory() as tmp:
        data = Path(tmp)/"data"
        shutil.copytree("data",data)
        for film,paper in PAIRS:
            def upstream(prefix):
                path = "Resources/data/profiles/"+prefix+film+".json"
                return json.loads(subprocess.check_output(["git","-C",ofx,"show","86476af:"+path]))["data"]
            path = Path("profiles",film+".json")
            original = json.loads(Path("data",path).read_bytes())
            assert original["info"]["reference_illuminant"] == "D55"
            p = upstream("")["hanatos2025_adaptation_window_params"]
            assert np.allclose(p, original["data"]["hanatos2025_adaptation_window_params"])
            uv,su,ir,si = p
            wl = np.arange(380,781,5)
            erf = np.vectorize(math.erf)
            w = (.5+.5*erf((wl-uv)/(su*np.sqrt(2))))*(.5+.5*erf((ir-wl)/(si*np.sqrt(2))))
            logs = np.array(original["data"]["log_sensitivity"],dtype=float)
            sens = np.nan_to_num(10**logs)
            norm = (sens*light*w[:,None]).sum(axis=0)/(sens*light).sum(axis=0)
            archived = np.array(upstream("archive/")["bandpass_hanatos2025"],dtype=float)
            modes = {"RS default":None, "OFX 2025":archived,"OFX 2026":w[:,None]/norm}
            frames = {}
            for name, bandpass in modes.items():
                stock = json.loads(Path("data",path).read_bytes())
                if bandpass is not None:
                    with np.errstate(divide="ignore",invalid="ignore"):
                        adjusted = logs + np.log10(bandpass)
                    stock["data"]["log_sensitivity"] = [[float(v) if np.isfinite(v) else None for v in row] for row in adjusted]
                (data/path).write_text(json.dumps(stock))
                recipe = {"camera":{"auto_exposure":False,"film_format_mm":chart.width*.005},
                          "film_render":{"grain":{"active":False},"halation":{"active":False,"boost_ev":0}},
                          "print_render":{"glare":{"active":False}},"scanner":{"unsharp_mask":[0,0]},
                          "io":{"output_color_space":"sRGB","output_cctf_encoding":False,"output_gamut_compress":{"algorithm":"off"}}}
                if bandpass is not None:
                    recipe["settings"] = {"apply_hanatos2025_adaptation_window":False}
                params=output/"recipe.json"
                params.write_text(json.dumps(recipe))
                raw=output/"frame.f64"
                subprocess.run([cli,"process",output/"input.png","--params",params,"--data-dir",data,"--film",film,"--paper",paper,"--output",output/"frame.png","--raw-out",raw],check=True,env=dict(os.environ,SPEKTRAFILM_BACKEND="cpu"))
                frames[name]=display(np.fromfile(raw,"<f8").reshape(96,chart.width,3))
                assert np.isfinite(frames[name]).all()
            np.testing.assert_array_equal(np.rint(frames["RS default"]), np.rint(frames["OFX 2026"]))
            preview=Image.new("RGB",(chart.width+90,288),"white")
            for i,(name,img) in enumerate(frames.items()):
                preview.paste(Image.fromarray(np.rint(img).astype("uint8")),(90,96*i))
                ImageDraw.Draw(preview).text((4,96*i+39),name,fill="black")
            preview.save(output/(film+".png"))
            report[film]={}
            for a in ("RS default","OFX 2025"):
                delta=abs(frames[a]-frames["OFX 2026"]).max(axis=2)
                report[film][a+" vs OFX 2026"]={label:np.percentile(delta[12:84,96*i+12:96*i+84],[50,95,100]).round(5).tolist() for i,(label,*_) in enumerate(crops)}
    (output/"metrics.json").write_text(json.dumps(report,indent=2))


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    for key in ("cli","ofx","fixtures","output"):
        parser.add_argument(key,type=Path)
    run(**vars(parser.parse_args()))
