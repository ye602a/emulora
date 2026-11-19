"""Compare 500T reflected-light kernels with fixed optics, material and pitch."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

import numpy as np
from PIL import Image, ImageDraw
from arctic_versions import display


def run(cli, photo, out, full_context):
    out.mkdir(exist_ok=True, parents=True)
    point = Image.new("RGB", (513,513))
    point.putpixel((256,256),(255,255,255))
    point.save(out/"point.png")
    with Image.open(photo) as src:
        edge = src.copy() if full_context else src.crop((185,125,698,638))
        edge.save(out/"edge.png")
    recipe = {"camera":{"auto_exposure":False,"film_format_mm":2.565},
              "film_render":{"grain":{"active":False},"dir_couplers":{"active":False}},
              "print_render":{"glare":{"active":False}},"scanner":{"unsharp_mask":[0,0]},
              "io":{"output_color_space":"sRGB","output_cctf_encoding":False,
                    "output_gamut_compress":{"algorithm":"off"}}}
    report={}
    for scene in ("point","edge"):
        frames={}
        size = point.size if scene=="point" else edge.size
        recipe["camera"]["film_format_mm"] = max(size)*.005
        for mode,strength in (("coated",[.015,.005,0]),("strength only",[.30,.03,.003]),
                              ("back reflection",[.30,.03,.003])):
            recipe["film_render"]["halation"]={"active":True,"boost_ev":0,"scatter_amount":0,
                "halation_first_sigma_um":[50]*3,"halation_strength":strength,"halation_renormalize":True}
            params=out/"recipe.json"
            params.write_text(json.dumps(recipe))
            env=dict(os.environ,SPEKTRAFILM_BACKEND="cpu")
            env.pop("RS_STUDY_NO_BACKING",None)
            if mode=="back reflection":
                env["RS_STUDY_NO_BACKING"]="1"
            if scene=="point":
                env["SPEKTRAFILM_DUMP_FILM_LOG_RAW"]=str((out/"lograw").resolve())
            proc=subprocess.run([cli,"process",out/(scene+".png"),"--film","kodak_vision3_500t",
                "--paper","kodak_2383","--params",params,"--iters","4" if scene=="edge" else "1",
                "--output",out/"result.png","--raw-out",out/"rgb"],env=env,check=True,capture_output=True,text=True)
            item={}
            rgb=np.fromfile(out/"rgb","<f8").reshape(size[1],size[0],3)
            if full_context and scene=="edge":
                rgb=rgb[125:638,185:698]
            assert np.isfinite(rgb).all()
            frames[mode]=display(rgb)
            if scene=="point":
                exposure=10**np.fromfile(out/"lograw","<f8").reshape(513,513,3)
                y,x=np.indices(exposure.shape[:2])
                radii=np.hypot(y-256,x-256).ravel()
                order=np.argsort(radii)
                for c in (0,1):
                    halo=np.maximum(exposure[...,c]-exposure[0,0,c],0)
                    halo[256,256]=0
                    mass=np.cumsum(halo.ravel()[order])
                    if mode == "back reflection":
                        # Same sampled domain and removed central pixel as the CPU impulse.
                        sigmas = ([.184586,1.121207,1.651048,4.677567],
                                  [.178469,1.000078,1.581389,9.448921])[c]
                        weights = ([.041510,-.5,1.0345,.423990],
                                   [.106789,-.5,1.384389,.008822])[c]
                        radius_um = 5*radii
                        target = sum(w*np.exp(-.5*(radius_um/(120*s))**2)/(120*s)**2
                                     for s,w in zip(sigmas,weights))
                        target[radius_um==0]=0
                        item["RG"[c]+"_inside_1mm_cpu_target"]=[
                            halo.ravel()[radius_um<1000].sum()/mass[-1],
                            target[radius_um<1000].sum()/target.sum()]
                    item["RG"[c]+"_r90_um"]=5*radii[order[np.searchsorted(mass,.9*mass[-1])]]
            else:
                times=list(map(int,re.findall(r"halo_us=(\d+)",proc.stderr)))
                item["warm_halo_ms"]=[v/1000 for v in times[-3:]]
                item["warm_pipeline_ms"]=list(map(int,re.findall(r"Pipeline \(iter \d+\): (\d+) ms \(warm\)",proc.stderr)))
                delta=np.abs(frames[mode]-frames["coated"]).max(axis=2)
                item["interior_p50_p95_p99_max_codes"]=np.percentile(delta[50:-50,50:-50],[50,95,99,100]).tolist()
            report[scene+" "+mode]=item
        if scene=="edge":
            sheet=Image.new("RGB",(3*513,540),"white")
            for i,(name,img) in enumerate(frames.items()):
                sheet.paste(Image.fromarray(img.round().astype("uint8")),(513*i,27))
                ImageDraw.Draw(sheet).text((513*i+8,7),name,fill="black")
            sheet.save(out/"comparison.png")
            delta=np.abs(frames["back reflection"]-frames["strength only"]).max(axis=2)
            report["geometry_only_p50_p95_p99_max_codes"]=np.percentile(delta[50:-50,50:-50],[50,95,99,100]).tolist()
    (out/"metrics.json").write_text(json.dumps(report,indent=2))


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    for key in ("cli","photo","out"):
        parser.add_argument(key,type=Path)
    parser.add_argument("--full-context", action="store_true", help="render whole photo before measuring its edge crop")
    run(**vars(parser.parse_args()))
