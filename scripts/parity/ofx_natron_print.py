"""NatronRenderer script for the published Windows OFX Portra/Endura study.

Set OFX_PLUGIN_PATH to the unpacked bundle directory. Set PRINT_STUDY_INPUT to
the Batch 13 PNG, PRINT_STUDY_DIR to a scratch directory and run once for each
OFX_CASE (transport, off, on, negative, diffusion, uv, ir, uv-ir). OFX_PRINT_MODEL selects a release
print-silver preset for the on case. In the negative case OFX_NEGATIVE_MODEL,
OFX_NEGATIVE_AMOUNT (0..1) and OFX_CYAN_COUPLING (0..2) select an isolated trial. OFX_NEGATIVE_SCAN=1 skips the paper. NatronRenderer --no-settings SCRIPT.py 1 writes
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
    if case == "negative":
        preset = os.environ.get("OFX_NEGATIVE_MODEL", "Modern Neutral")
        amount = float(os.environ.get("OFX_NEGATIVE_AMOUNT", "1"))
        coupling = float(os.environ.get("OFX_CYAN_COUPLING", "1"))
        assert 0 <= amount <= 1 and 0 <= coupling <= 2
        param = source.getParam("negativeBleachBypassModel")
        param.set([param.getOption(i) for i in range(param.getNumOptions())].index(preset))
        source.getParam("negativeLeucoCyanCoupling").set(coupling)
        if os.environ.get("OFX_NEGATIVE_SCAN") == "1":
            process = source.getParam("process")
            process.set([process.getOption(i) for i in range(process.getNumOptions())].index("Scan negative"))
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
                       "negativeBleachBypassAmount":amount if case=="negative" else 0.0,
                       "printBleachBypassAmount":float(case=="on")}.items():
        source.getParam(name).set(value)
suffix = "" if model == "Stock Character" else "-" + model.lower().replace(" ", "-")
if case == "negative" and any(k in os.environ for k in ("OFX_NEGATIVE_MODEL", "OFX_NEGATIVE_AMOUNT", "OFX_CYAN_COUPLING")):
    suffix = "-%s-%g-c%g" % (preset.lower().replace(" ", "-"), amount, coupling)
if case == "negative" and os.environ.get("OFX_NEGATIVE_SCAN") == "1":
    suffix += "-scan"
writer = app1.createWriter(str(Path(os.environ["PRINT_STUDY_DIR"])/("ofx-"+case+suffix+".png")))
writer.getParam("formatType").set(0)  # Match input dimensions.
writer.connectInput(0,source)
raw_io(writer)
