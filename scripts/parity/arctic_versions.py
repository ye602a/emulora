"""Factor Arctic LUT, neutral response and print-pack changes on the RS CPU photo path."""
import argparse
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory

import numpy as np
from PIL import Image, ImageDraw
from negative_hue_trims import CROPS

REV = "28bf883e1672e884307edc75852549376e13644e"
BASE = "src/spektrafilm/data/"
TABLE = "luts/spectral_upsampling/arctic2026alpha02/reflectance_xy_tc.npy"
PAIRS = [("kodak_portra_400", "kodak_portra_endura"), ("kodak_ektar_100", "kodak_endura_premier")]


def display(raw):
    x = raw.clip(0, 1)
    return 255 * np.where(x <= .0031308, 12.92*x, 1.055*x**(1/2.4)-.055)


def run(args, temp):
    def blob(name):
        return subprocess.check_output(["git", "-C", args.upstream, "show", REV + ":" + BASE + name])

    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    sheet = Image.new("RGB", (480, 144))
    for i, (_, name, x, y) in enumerate(CROPS):
        with Image.open(args.fixtures / (name + ".png")) as photo:
            sheet.paste(photo.crop((x, y, x+96, y+96)), (96*i, 0))
    for i, rgb in enumerate([(118,)*3, (220,)*3, (255,0,0), (0,0,255), (220,0,255)]):
        sheet.paste(rgb, (96*i,96,96*i+96,144))
    sheet.save(out/"input.png")
    recipe = {"settings":{"rgb_to_raw_method":"arctic2026alpha02"},
              "camera":{"auto_exposure":False, "film_format_mm":2.4},
              "film_render":{"grain":{"active":False}, "halation":{"active":False, "boost_ev":0}},
              "print_render":{"glare":{"active":False}}, "scanner":{"unsharp_mask":[0,0]},
              "io":{"output_color_space":"sRGB","output_cctf_encoding":False,
                    "output_gamut_compress":{"algorithm":"off"}}}
    params = out/"recipe.json"
    params.write_text(json.dumps(recipe))
    data = temp/"data"
    shutil.copytree("data", data)
    alpha = Path("data", TABLE).read_bytes()
    beta = blob("luts/spectral_upsampling/arctic2026beta04_reflectance_xy_tc.npy")
    # Inspect a physical wavelength directly, independent of the print path.
    peaks = {}
    for version, content in (("alpha02", alpha), ("beta04", beta)):
        lut = np.load(io.BytesIO(content))
        if lut.shape != (192, 192, 81) or not np.isfinite(lut).all():
            raise ValueError(f"unexpected Arctic table: {version}")
        peaks[version] = {}
        for name, x, y in (("sRGB_blue", .15, .06), ("sRGB_red", .64, .33)):
            cell = tuple(np.rint(np.array([(1-x)**2, y/(1-x)]) * 191).astype(int))
            peaks[version][name] = 380 + 5 * int(lut[cell].argmax())
    pack_path = Path("filters/neutral_print_filters.json")
    packs = [Path("data", pack_path).read_bytes(), blob(pack_path.as_posix())]
    report = {"nearest_cell_peak_nm": peaks}
    for film, paper in PAIRS:
        stock = Path("profiles", film + ".json")
        original = Path("data",stock).read_bytes()
        pack_rows = [json.loads(p)[paper]["TH-KG3"][film] for p in packs]
        results, controls = {}, {}
        for mode in ("alpha", "beta", "matched", "pack"):
            (data/TABLE).write_bytes(alpha if mode == "alpha" else beta)
            (data/pack_path).write_bytes(packs[int(mode == "pack")])
            if mode in ("alpha", "beta"):
                (data/stock).write_bytes(original)
            elif mode == "matched":
                profile = json.loads(original)
                # At fixed stock and illuminant, multiplying sensitivity scales the
                # complete integrated exposure plane by the same channel gain.
                for row in profile["data"]["log_sensitivity"]:
                    for i, value in enumerate(row):
                        if value is not None:
                            row[i] += float(controls["alpha"][i]-controls["beta"][i])
                (data/stock).write_text(json.dumps(profile))
            raw, log = out/"print.bin", out/"log_raw.bin"
            for path in (raw, log):
                path.unlink(missing_ok=True)
            subprocess.run([args.cli,"process",out/"input.png","--film",film,"--paper",paper,
                            "--data-dir",data,"--params",params,"--raw-out",raw,"--output",out/"print.png"],
                           env=dict(os.environ,SPEKTRAFILM_BACKEND="cpu",
                                    SPEKTRAFILM_DUMP_FILM_LOG_RAW=str(log)),check=True)
            rgb, exposure = [np.fromfile(p,"<f8").reshape(144,480,3) for p in (raw,log)]
            if not np.isfinite(rgb).all() or not np.isfinite(exposure).all():
                raise ValueError(f"{film} {mode}: nonfinite output")
            results[mode] = display(rgb)
            controls[mode] = exposure[120,48]
        np.testing.assert_allclose(controls["matched"], controls["alpha"], atol=2e-5)
        np.testing.assert_array_equal(controls["matched"], controls["pack"])
        preview = Image.new("RGB",(568,576),"white")
        for i,(mode,img) in enumerate(results.items()):
            preview.paste(Image.fromarray(np.rint(img).astype("uint8")), (88,i*144))
            ImageDraw.Draw(preview).text((5,i*144+45),mode,fill="black")
        preview.save(out/(film+".png"))
        stats = {}
        regions = [(name,np.s_[12:84,i*96+12:i*96+84]) for i,(name,*_) in enumerate(CROPS)]
        regions += [(name,np.s_[108:132,i*96+12:i*96+84]) for i,name in
                    enumerate(("gray","light-gray","red","blue","magenta"))]
        for a,b in (("alpha","beta"),("alpha","matched"),("matched","pack")):
            delta = abs(results[a]-results[b]).max(axis=2)
            stats[a+"-"+b] = {name:np.percentile(delta[roi],[50,95]).round(2).tolist()
                               for name,roi in regions}
        report[film] = dict(paper=paper,print_packs=pack_rows,
                            gray_delta_stops=((controls["beta"]-controls["alpha"])/np.log10(2)).tolist(),
                            display_code_delta=stats)
    (out/"measurements.json").write_text(json.dumps(report,indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("cli","upstream","fixtures","output"):
        parser.add_argument(key,type=Path)
    with TemporaryDirectory() as scratch:
        run(parser.parse_args(),Path(scratch))
