# Frozen calibrated negative output v1

`emulora_core::calibrated_output_v1` provides calibrated negative inversion,
finish exposure, quantization and display sampling on a host-supplied device.
The host retains the resulting textures.

The v1 route carries linear AP0 transmission for both negative calibrations.
An algorithm or table change requires a new route version.

## Source and output contract

| Mapping | Required complete physical-chain source | Result |
| --- | --- | --- |
| `PhysicalOutputMappingV1::EncodedSrgb` | Full-resolution encoded sRGB, with `FilmOutput.cctf_encoding == true` | Positive sRGB codes; no additional inversion or transfer encoding |
| `PhysicalOutputMappingV1::NegativeScanAp0(NegativeStockV1::Pro400H)` | The accepted Pro400H linear AP0 transmission scan, `scan_film == true`, `output_cctf_encoding == false` | Frozen Pro400H calibrated positive sRGB |
| `PhysicalOutputMappingV1::NegativeScanAp0(NegativeStockV1::XTra400)` | The accepted X-Tra 400 linear AP0 transmission scan, with the same route requirements | Frozen X-Tra calibrated positive sRGB |

Stock IDs are exactly `fujifilm_pro_400h` and `fujifilm_xtra_400`. Converting
another stock string into `NegativeStockV1` returns `UnsupportedStock`; there is
no nearest-stock fallback. The calibration must match the physical film profile.
The scoped buffer contains no stock or color-space metadata, so callers remain
responsible for that match. The safe complete-render wrapper additionally checks
the scan and transfer flags; the encoding method checks the supplied transfer
flag and device identity.

The accepted negative route retains its fixed spectral/response settings and
original DIR, disables camera diffusion, scan glare, transport gamut mapping and
transport sharpening, and applies the v1 inversion/toe/shoulder/gamut mapping
once. Select the opt-in physical stock defaults and supply the transport
settings in the runtime recipe. See the
[public host example](../../crates/emulora-core/examples/host_gpu_contract.rs)
and [physical stock defaults](HOST_GPU_INTEGRATION_CONTRACT.md#physical-stock-defaults).

Both output textures have the requested positive dimensions:

| Field | Format | Meaning |
| --- | --- | --- |
| `PhysicalOutputTexturesV1::precision` | `Rgba16Uint` | Final RGB16 integer codes, alpha 65535 |
| `PhysicalOutputTexturesV1::display` | `Rgba8Unorm` | Encoded sRGB display codes derived as `(rgb16 + 128) / 257`, alpha 255 |

The display texture's channels already contain encoded sRGB. A host must not
apply a second sRGB transfer to these values. The precision texture uses an
integer format so the accepted RGB16 codes survive exactly. Texture usage is
`STORAGE_BINDING | TEXTURE_BINDING | COPY_SRC`. Host display/export consumers
can retain or read these textures after successful completion.

## Exact order and regional semantics

The full physical film chain completes before this consumer runs. The retained
shader then performs these operations, in order:

1. Read a source pixel at its full-resolution output coordinates.
2. For a calibrated negative, invert transmission, apply the frozen AP0-to-sRGB
   matrix, toe neutralization, shoulder, symmetric gamut mapping and sRGB transfer.
3. Quantize the resulting positive RGB to RGB16 with the original integer
   significand algorithm, equivalent to f64 multiplication and rounding of the
   f32 input. This avoids an extra f32 multiply changing threshold decisions.
4. Apply the original 65,536-entry finish table when `finish_centi_ev != 0`.
   The table decodes each positive code, multiplies linear exposure by
   `2^(centi_ev/100)`, re-encodes, clamps and rounds.
5. Apply the display crop and optional area reduction to the finished RGB16
   source pixels, with the original f32 ceil bounds, row integer sums and
   final rounding. A 1:1 crop copies the selected codes directly.
6. Store RGB16 and derive the RGBA8 display result.

`PhysicalOutputV1::render` calls `with_film_output` with **no engine region**.
It passes `PhysicalOutputOptionsV1::region` only to the calibrated consumer.
An advanced host using `encode` must follow the same rule. Reducing negative
transmission before inversion, or reducing positive pixels before finish,
changes this nonlinear order. Calling the generic `run_film_chain_rgb16` first
also quantizes the wrong intermediate for a calibrated negative.

`DisplayRegion` uses full-resolution `[x, y, width, height]` coordinates and
accepts only 1:1 output or reduction. Invalid or out-of-bounds rectangles,
enlargement, zero dimensions, shader addressing overflow and unsupported row
accumulation sizes return explicit errors. The retained u32 row accumulator
supports at most 65,537 maximum-valued samples per reduction row; an actual wider
block is rejected. Source/output buffer and texture/dispatch device limits are
also checked. The shader itself is unchanged.

### Observed original CPU fallback discrepancy

The historical reduced CPU fallback can reduce transmission before calibrated
inversion and apply finish after reduction. This differs from the GPU order
described above; the nonlinear operations cannot be freely reordered.

`positive_scan_v1` and `positive_rgb16_v1` are full-size CPU APIs. A CPU consumer
requiring GPU-compatible regions must invert the full plane, quantize/finish,
then crop/reduce the resulting codes. Tests use that order. Adopting this route
for an existing reduced CPU fallback requires an explicit compatibility decision.

## Ownership and the publication boundary

Create one `PhysicalOutputV1::new(&host_device)` for a device and reuse it. It
retains one compiled compute pipeline, two immutable calibration buffers and at
most one current finish table. A warm render with the same finish does not
rebuild or upload its 65,536 entries. A finish change replaces one cached buffer;
commands already recorded retain their own resource handles. Calibration data
is parsed once per stock. This is a small per-output-object cache, not another
engine work-plane pool.

`encode(&self, device, encoder, FilmOutput, options)` records commands into the
encoder supplied by the scoped film callback. It does not submit, wait or read
the physical float plane back to the CPU. Each result owns fresh display and
precision textures; it never retains a recycled engine work buffer. The same
object can encode subsequent frames without overwriting published textures.

An `encode` result means destinations and commands were created. **The host may
publish/display a frame only after the whole `PhysicalOutputV1::render` or
enclosing `with_film_output` call returns `Ok`.** The backend submits and waits
for the entire encoder, observes late cancellation, then commits density-cache
publication and returns the host-owned result.

The `render` wrapper propagates an encoding failure through the callback as a
backend error while preserving the original `CalibratedOutputError` for the
caller. This prevents an `Ok(Err(error))` callback from being mistaken for a
successful density-cache commit. Advanced callback callers must likewise return
an outer `Err` for encoding failure. After submission, cancellation cannot
undo GPU writes that already executed; it still prevents a successful returned
frame and density publication. The host must not publish an early captured
texture merely because the callback ran.

The existing backend submission gate serializes queue ownership. Keep using the
host's shared gate when injecting device/queue handles; this consumer creates
neither another device nor another queue and performs no hidden submission.

## CPU API

`positive_scan_v1(scan, stock_id, stop)` returns encoded positive sRGB f32 using
the original f64 CPU calibration math. It rejects incomplete RGB input,
nonfinite scan values, unsupported stocks and cancellation. Larger images use
the original bounded worker/chunk scheduling with cancellation checks.

`positive_rgb16_v1(scan, stock_id, finish_centi_ev, stop)` performs full-size CPU
inversion, original RGB16 rounding and optional finish. It returns interleaved
RGB codes without padding. `finish_exposure_table_v1(centi_ev)` exposes the
original table for consumers that already have positive encoded RGB16. Avoid
applying it again to GPU results, which already include the requested finish.

## Provenance and validation

The original FPW runtime is GPL-3.0-or-later. The profile-derived calibration
retains Andrea Volpato's CC BY-SA 4.0 attribution and the full preserved
[SPEKTRAFILM_DATA_LICENSE.txt](../../SPEKTRAFILM_DATA_LICENSE.txt).
The calibration bytes and independent Python-heldout values are identified in
[data provenance](../../data/negative_scan_ap0_v1/PROVENANCE.md). Production
shader bytes are preserved in the repository.

The calibrated output regressions exercise:

- Scalar CPU inversion against independent Python values at the original
  tolerance of 2e-6 in encoded sRGB.
- Exact CPU/GPU RGB16 quantization for encoded sRGB. For calibrated negatives,
  the per-channel tolerance is three RGB16 codes against the scalar CPU
  inversion, including calibration knots and adjacent f32 inputs.
- Exact application of the CPU finish table to GPU codes, exact cropping and
  integer-factor reduction, display conversion, and retained texture ownership
  across later renders and finish-table replacements.
- Tiny outputs, invalid transport and geometry, and CPU cancellation.
- Complete paper and negative renders, publication only after successful
  output, and density reuse on a successful retry.

Verify the data and run the device regressions with:

```sh
python tools/verify_data_manifest.py
EMULORA_REQUIRE_GPU=1 cargo test --locked \
  -p emulora-core -p emulora-gpu --features openblas-src/system \
  --test calibrated_output_v1
```

The explicit system-OpenBLAS feature is for the Linux validation environment;
other supported environments use their normal BLAS configuration. These checks
qualify the extracted output, heldout data and the focused complete-render
wrapper behavior. The public host example additionally exercises the accepted
paper, two slide and two calibrated negative routes. Real-photo acceptance
remains a distinct gate; synthetic fixtures do not certify all hardware backends
or every host fallback route.
