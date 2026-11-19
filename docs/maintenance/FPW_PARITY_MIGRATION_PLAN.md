# Compatibility and host adoption

Maintain one film implementation in RS, with independently buildable library,
CLI and GUI targets. Hosts adopt an exact accepted revision and matching
resources so ongoing engine development does not silently change saved results.

## Versioned rendering

Preserve the current linear-sRGB input convention and existing output routes.
Changes to calibrated tables, accepted defaults, operation order or working
space require a new rendering-contract identity and replay of saved settings.
Wider-gamut processing remains separate research.

The [host GPU contract](HOST_GPU_INTEGRATION_CONTRACT.md) covers device injection,
source-anchored grain, precise IIR, bounded caches and final output. Stock setup
and calibrated output are explicit selections. `gpu_multilayer` remains `false`
by default for standalone compatibility; a host adopting an existing multilayer
route must explicitly enable it.

## Adoption checklist

The engine contracts and standalone checks are available. Switching a host to
the independent engine and qualifying its accepted images remain separate
delivery gates; engine test success alone does not complete host adoption.

1. Bind the source revision, verified resource identity, stock-setup version and
   output route together. Use the public APIs for film processing and cache
   dependencies; retain application state and scheduling in the host.
2. Check the actual installed resource directory with the
   [resource checker](FPW_RESOURCE_COMPATIBILITY.md). A matching manifest name
   or directory path alone does not establish unchanged resource contents.
3. Compare accepted images on the same device and driver, with identical input
   pixels, exposure, profiles, physical effects, source transform and output
   sampling. Check saved-settings replay, preview/export agreement and region
   output as well as isolated operators.
4. Exercise real shared-device surface changes, overlapping requests, stale-frame
   rejection, submitted cancellation and device recovery. Headless software
   Vulkan tests do not establish window-system or dedicated-hardware behavior.
5. Record acceptance evidence and retain a rollback revision before replacing
   an existing host dependency. Remove migrated algorithm copies after adoption.

### Candidate replay on FPW main · 2026-11-19

FPW main `e1d0759` pins RS `1947ee8`. The persisted output identity remains `27ca80e`. An isolated copy of the host runtime was compiled twice against the pinned revision and the RS candidate. Only the dependency source and scratch paths changed. The host's own resource directory came from its Git tree; the resource checker matched all 55 shared entries. The bundled RS check also matched its three calibration artifacts. The public `fpw_host_contract` example accepts an external data directory and exercised all five stock routes, full surface and finished region, on one RTX 3060 Ti.

The actual host test `physical_exposure_reuses_calibration_with_fresh_route_parity` passed with the candidate. It compares fresh and retained results at −0.40, +0.30 and 0 EV on three routes. A second probe passed a fixed 480×240 linear-sRGB montage (Kodak 03, 04, 15 and the batch-7 silk photo) through FPW's own `film::render`, default 35 mm FilmSettings, grain, halation and DIR on, seed 1, with a fresh engine per case. Each output contained 345,600 RGB16 channel samples.

| FPW route | Against pinned RS, before the batch-9 mix correction | After correction |
| --- | --- | --- |
| Portra 400 / Portra Endura, diffusion off | Exact | Exact |
| Portra 400 / Portra Endura, diffusion on | 1,932 samples differed, peak 480 codes | Exact |
| Provia 100F slide, diffusion off | Exact | Exact |
| Provia 100F slide, diffusion on | 1,888 samples differed, peak 134 codes | Exact |
| Pro 400H calibrated negative scan | Exact; host rejects active diffusion | Exact; same rejection |

Restoring the old rounding order at the in-place scatter accumulation removed the differences without bringing back the two full-size intermediates. The separate batch-6 CLI point-light and photographic fixtures, each tested at the camera and enlarger, also exactly matched the saved pre-fusion float outputs on this device. No new timing measurements were taken.

This comparison covers the existing host adapter using candidate RS, one driver and one small photographic raster. The host pin and saved identity still point to the published contract. The full-resolution saved-recipe replay and local checked-refresh adoption are recorded below.

### Full-resolution saved-photo replay · batch 10

The [headless replay](../../tools/validation/fpw_saved_photo_replay.rs) runs in an isolated copy of FPW main `e1d0759` using the host's own RAW preparation, preview, viewport and frozen JPEG publication code. Run each revision in a separate process with separate render, output and publication directories. A shared scratch RAW cache holds the prepared Nikon linear frame. Check its SHA-256 and the deserialized recipe SHA-256 for equality before comparing both renders.

On Windows / RTX 3060 Ti (driver 617.14), the FPW pin `1947ee8` and the RS candidate `2361ff0` rendered the same LibRaw/LMMSE/AP1 working planes. The three saved recipes came from read-only research project snapshots. FPW's resource contract, Kodak/Fuji profiles, crop, masks, film grain, DIR and postprocessing were retained. The fourth case changed only the camera diffusion switch on the saved slide recipe. The source photos were all 4032×6048 Nikon Z5 II RAWs.

| Case | Native output | Fit preview | Difference between RS revisions |
| --- | --- | --- | --- |
| DSC_0581 · Pro 400H calibrated negative scan | 4032×6048 | 853×1280 | Exact |
| DSC_5930 · Portra 400 / Portra Endura, saved crop, mask and post diffusion | 4032×5600 | 922×1280 | Exact |
| DSC_0883 · Provia 100F, saved mask | 4032×6048 | 853×1280 | Exact |
| DSC_0883 · same slide with camera diffusion enabled | 4032×6048 | 853×1280 | Exact |

“Exact” covers every RGB16 channel of the fit, native and 512×512 viewport frames, plus the entire published quality-95 JPEG with its embedded sRGB ICC profile. Publishing the captured native preview independently through FPW's JPEG encoder reproduced the export byte-for-byte in all four cases. The saved DSC_5930 viewport differed from the matching full-frame crop in 514 of 786,432 channel samples by one RGB16 code on **both** builds. The other three same-build crops were exact. This records an existing host-level difference; its cause was not isolated. These checks did not exercise user interaction, window presentation or other drivers.

**Checked calibration adoption.** The FPW integration branch keeps the effective film, paper, route, persisted output contract and resource directory as the outer identity. For the same selection it builds the requested runtime parameters and asks RS whether spectral calibration can be reused. A miss rebuilds from the original resources and propagates errors. This removes the per-field dispatch patch for exposure, print exposure and scanner sharpening. It also allows a change such as turning film grain off to reuse calibration while RS separately checks whether the developed negative must change.

A first isolated adapter used the immutable `Pipeline::refreshed` API. A hot Portra pipeline then copied an 884,736-byte spectral table plus the profiles on every update. In 300 alternating exposure edits, configuration alone took 208.61 ms. The exclusive `refresh_in_place` API used 1.62 ms on the same development build; FPW's former three-field patch used 0.29 ms. These are single-run adapter timings, excluding rendering. A failed in-place refresh preserves the previous parameters, gamut and prepared hue. The regression checks storage identity, warm/fresh pixel equality and failing requests. The desktop continues to use its original persisted output identity and shared device. FPW integration commit `9eafce0` pins RS `e77f631` in the manifest and lockfile. The final adapter and Git-pinned dependency were also compiled into the isolated replay runtime. All 16 output artifacts reproduced the pinned host reference hashes. The regular FPW integration worktree passed the extended cold/warm regression and `cargo check --locked --offline -p fpw-desktop`. Local Cargo verification was seeded from the RS repository; a clean remote consumer requires publication of the pinned commit. [Frame dimensions, recipe/input digests and output SHA-256](validation/fpw_saved_photo_replay_20261119.json)

To reproduce with authorized local assets:

1. Read the saved `Recipe` and the matching `Source` from a project copy. Store them as `<photo>.json` and `<photo>.source.json` in a scratch directory. Retain source bindings, crop, seed, masks and colour settings. A derived stress case needs its own label.
2. Place each build's FPW configuration at `<build>.config.json` in that directory. Bind the same verified resources, RAW configuration and raster encoder. Set scratch cache and evidence paths. Include `fpw_saved_photo_replay.rs` at the end of FPW's `fpw-runtime/src/workspace.rs` under `#[cfg(test)]` so the test can call the host publication entry point.
3. Set `FPW_REPLAY_DIR`, `FPW_REPLAY_BUILD` and `FPW_REPLAY_PHOTO`. Run `cargo test --offline -p fpw-runtime --lib saved_film_photo_replay -- --ignored` from the isolated native workspace. Use a fresh output directory for each attempt to prevent an existing publication journal from skipping encoding.
4. Run the companion ignored test `saved_preview_jpeg_comparison` on the captured frames to compare the encoded native preview with the frozen JPEG export. Compare the manifests, RGB16 channel data and final JPEG files across builds. For the same-build viewport check, crop the native preview at x=1000, y=1200, width=512, height=512 in output coordinates, including any saved crop. Compare the viewport against the same build's native output separately from comparing two engine revisions.

### Reduced CPU negative output

The historical reduced CPU fallback can resize transmission before nonlinear
negative inversion and apply finish after reduction. The calibrated GPU v1
route inverts, quantizes and applies finish before crop/reduction. These orders
are not equivalent. Keep the saved route's behavior explicit during adoption;
see [CALIBRATED_OUTPUT_V1.md](CALIBRATED_OUTPUT_V1.md). The full-size CPU inversion
APIs do not implement the old reduced fallback implicitly.

## Qualification and performance

[VALIDATION.md](VALIDATION.md) records completed standalone checks. The precise
Vulkan IIR suite has 1,215 cases and requires
`abs(GPU - reference) <= 4*f32::EPSILON*max(1,abs(reference))`.
Run it on each target device/driver that needs qualification; software Vulkan
success does not certify every GPU backend.

Current allocation reuse retains one pair of work planes, while density reuse
retains one immutable snapshot. Auxiliary blur/LUT/parameter buffers and staging
are not a general resource pool. Measure real workloads, including exact-key
construction, GPU copies, cold/warm state and active memory, before claiming a
performance gain. Cache residency counters do not represent total VRAM usage.
