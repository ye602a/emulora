"""Run LAB b51e672's pointwise Metal hue calculation on an OpenCL device.

Input NPZ: negative and optical_xyz (H,W,3), packed params (60), layers
(1024,3,2), gamut (48,120), and map (160,160,2). Output NPY: scanned XYZ.
Calibration and output encoding belong to the caller. See ENGINE_STUDIES.md.
"""
import argparse
from pathlib import Path
import re

import numpy as np
import pyopencl as cl


def render(shader, frame):
    source = shader.read_text(encoding="utf-8")
    source = source[source.index("inline float2 spk_read_layer"):source.index("// The optical print in one pass")]
    source = source.replace("device ", "__global ")
    source = re.sub(r"\s*\[\[buffer\(\d+\)\]\]", "", source)
    source = re.sub(r",\s*uint3 pos \[\[thread_position_in_grid\]\]", "", source)
    source = source.replace("pos.x", "get_global_id(0)")
    source = re.sub(r"\b(float[23]?|uint)\(", r"(\1)(", source)
    ctx = cl.create_some_context(interactive=False)
    queue = cl.CommandQueue(ctx)
    program = cl.Program(ctx, source).build()
    neg = frame["negative"]
    if neg.ndim != 3 or neg.shape[-1] != 3 or not neg.size:
        raise ValueError("expected an RGB negative")
    n = neg.shape[0] * neg.shape[1]
    buffers = []
    for key, shape in [("negative", neg.shape), ("optical_xyz", neg.shape), ("layers", (1024, 3, 2)),
                       ("params", (60,)), ("gamut", (48, 120)), ("map", (160, 160, 2))]:
        a = np.ascontiguousarray(frame[key], dtype=np.float32)
        if a.shape != shape or not np.isfinite(a).all():
            raise ValueError(f"invalid {key}")
        buffers.append(cl.Buffer(ctx, cl.mem_flags.READ_ONLY | cl.mem_flags.COPY_HOST_PTR, hostbuf=a))
    meta = np.array([n, 1024, 120, 48, 160], dtype=np.uint32)
    buffers.append(cl.Buffer(ctx, cl.mem_flags.READ_ONLY | cl.mem_flags.COPY_HOST_PTR, hostbuf=meta))
    result = np.empty(neg.shape, dtype=np.float32)
    output = cl.Buffer(ctx, cl.mem_flags.WRITE_ONLY, result.nbytes)
    program.spk_negative_hue(queue, (n,), None, *buffers, output)
    cl.enqueue_copy(queue, result, output).wait()
    if not np.isfinite(result).all():
        raise ValueError("nonfinite LAB output")
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("shader", type=Path)
    parser.add_argument("frame", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    with np.load(args.frame) as frame:
        np.save(args.output, render(args.shader, frame))
