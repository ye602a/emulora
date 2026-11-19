"""NatronRenderer script for the published Windows OFX Portra/Endura study.

Set OFX_PLUGIN_PATH to the unpacked bundle directory. Set PRINT_STUDY_INPUT to
the Batch 13 PNG, PRINT_STUDY_DIR to a scratch directory and run once for each
OFX_CASE (transport, off, on). NatronRenderer --no-settings SCRIPT.py 1 writes
one full-resolution PNG. Compare transport to the source before using output.
"""
import os
from pathlib import Path

case = os.environ["OFX_CASE"]
assert case in ("transport","off","on")
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
                       "rgbToRawMethod":"Hanatos 2025","printBleachBypassModel":"Stock Character"}.items():
        param = source.getParam(name)
        options = [param.getOption(i) for i in range(param.getNumOptions())]
        param.set(options.index(label))
    for name,value in {"dirAmount":0.0,"grainEnabled":False,"halationEnabled":False,
                       "scannerEnabled":False,"autoExposure":False,"glarePercent":0.0,
                       "printBleachBypassAmount":float(case=="on")}.items():
        source.getParam(name).set(value)
writer = app1.createWriter(str(Path(os.environ["PRINT_STUDY_DIR"])/("ofx-"+case+".png")))
writer.getParam("formatType").set(0)  # Match input dimensions.
writer.connectInput(0,source)
raw_io(writer)
