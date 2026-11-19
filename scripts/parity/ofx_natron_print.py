"""NatronRenderer script for the published Windows OFX Portra/Endura study.

Set OFX_PLUGIN_PATH to the unpacked bundle directory. Set PRINT_STUDY_INPUT to
the Batch 13 PNG, PRINT_STUDY_DIR to a scratch directory and run once for each
OFX_CASE (transport, off, on, negative, diffusion, uv, ir, uv-ir). OFX_PRINT_MODEL selects a release
print-silver preset for the on case. NatronRenderer --no-settings SCRIPT.py 1 writes
one full-resolution PNG. Compare transport to the source before using output.
"""
import os
from pathlib import Path

case = os.environ["OFX_CASE"]
assert case in ("transport","off","on","negative","diffusion","uv","ir","uv-ir")
model = os.environ.get("OFX_PRINT_MODEL", "Stock Character")
reader = app1.createReader(os.environ["PRINT_STUDY_INPUT"])


def raw_io(node):
    for name in ("ocioInputSpace", "ocioOutputSpace"):
        node.getParam(name).set("raw")


raw_io(reader)
source = reader
if case != "transport":
    source = app1.createNode("org.spektrafilm")
    source.connectInput(0, reader)
    for name,label in {"film":"Kodak Portra 400","paper":"Kodak Professional Portra Endura",
                       "inputColorSpace":"sRGB","sdrOutputColorSpace":"sRGB",
                       "rgbToRawMethod":"Hanatos 2025","printBleachBypassModel":model}.items():
        param = source.getParam(name)
        options = [param.getOption(i) for i in range(param.getNumOptions())]
        param.set(options.index(label))
    if case in ("uv", "uv-ir"):
        source.getParam("cameraUvFilterEnabled").set(True)
        source.getParam("cameraUvCutNm").set(430.)
    if case in ("ir", "uv-ir"):
        source.getParam("cameraIrFilterEnabled").set(True)
        source.getParam("cameraIrCutNm").set(650.)
    if case == "diffusion":
        source.getParam("cameraDiffusionEnabled").set(True)
        source.getParam("cameraDiffusionStrength").set(0.5)
    for name,value in {"dirAmount":0.0,"grainEnabled":False,"halationEnabled":False,
                       "scannerEnabled":False,"autoExposure":False,"glarePercent":0.0,
                       "negativeBleachBypassAmount":float(case=="negative"),
                       "printBleachBypassAmount":float(case=="on")}.items():
        source.getParam(name).set(value)
suffix = "" if model == "Stock Character" else "-" + model.lower().replace(" ", "-")
writer = app1.createWriter(str(Path(os.environ["PRINT_STUDY_DIR"])/("ofx-"+case+suffix+".png")))
writer.getParam("formatType").set(0)  # Match input dimensions.
writer.connectInput(0,source)
raw_io(writer)
