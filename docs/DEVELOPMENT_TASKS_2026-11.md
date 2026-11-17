# SpektraFilm-RS Staged Development Tasks

Date: 2026-11-16

Implement and validate one batch at a time. Inspect current main and working tree first; never overwrite user changes. Do not label missing features as untested features.

## Batch 0 — Recover test evidence
Run applicable CPU/GPU, Vulkan, real-photo, device-lifecycle, cache, IIR, grain, output and performance tests. Record device, driver, dimensions, parameters, cold/warm cache, stage timings, GPU passes, bytes read/written and peak memory. Preserve historical pass evidence; explicitly list tests not run.

## Batch 1 — Fuse blur final-axis accumulation
Investigate whether the current blur→add_scaled path writes an intermediate that can be avoided. Use the darktable d996f081 final-axis accumulation idea as a reference. Preserve kernel, radius, sampling, boundaries, precision and parameter semantics. Validate CPU/GPU output, accepted reference images, max/RMS error, pass count, traffic, peak resources and cold/warm timings. Never claim speedup without measurements.

The Windows Vulkan experiment fused the vertical halation blur into the existing bounce accumulator. Three bounces saved three dispatches and 72 bytes of logical intermediate traffic per pixel. On a GTX 1080 Ti (582.78), repeated warm medians were 41.85 ms versus 47.61 ms for a 1601×1067 synthetic render and 239.41 ms versus 243.30 ms for a 2615×3269 photograph. Peak memory above idle was the same in the sampled run. The photograph's largest absolute f32 difference was 7.33e-6 and RMS difference was 3.75e-8. The existing passes were retained.

## Batch 2 — Basic 135 border geometry contract
Define host-independent data and coordinates:
- separate content and film-canvas dimensions;
- overscan and scan rotation/offset registration;
- gate, perforation and edge-exposure coordinates;
- output crop and physical-scale propagation;
- deterministic seed and serialization;
- parameter ownership, stage dependencies and precise cache invalidation.
Zero overscan must reproduce current output. Expanding the canvas must not rescale central content or change its grain/optical physical scale.

## Batch 3 — 135 exposure canvas and perforations
Implement basic gate mask, film presence, perforations and edge exposure. SpektraLab's Windows/Linux Vulkan implementation is a reference, but use RS resource ownership and pooling. Borders must enter the correct physical stage, not be a final UI frame. Disabled-by-default must not allocate full-frame resources or add GPU passes unnecessarily.

## Batch 4 — Audit existing stocks
Audit profiles, stock setup, print pairing, monochrome/positive/negative classes and calibration coverage. Choose a small number of valuable stocks for response and reference-image validation. Loadability alone does not mean all output paths are validated. Do not bulk-enable stocks to inflate counts.

The resident paper route was exercised with existing Portra 160, 400 and 800 profiles and Portra Endura at −1.5, 0 and +1.5 EV. All channel ramps were monotonic. At 0 EV, the three 18% patches agreed to one rounded RGB8 code across profiles. Skin, green foliage and a saturated red subject were inspected in photographs. Portra 160 showed a modest reduction in red saturation while retaining a similar neutral and skin response. Portra 800 remains a valid candidate for a native-size grain and low-light comparison. The available profiles and calibration defaults are retained.

## Batch 5 — Research monochrome profile conversion
Evaluate SpektraLab's Tri-X 400, T-Max 100, Acros II and HP5 Plus. Their three-column expanded representation differs semantically from RS's single-column monochrome representation. First create a conversion specification, reproducible script, density-conservation and response validation. Ilford Multigrade IV RC is a multi-emulsion variable-contrast paper and needs separate modeling.

A fixed [SpektraLab data revision](https://github.com/JamesQiu2005/SpektraLab/tree/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/resources_product) was converted offline. Collapse triplicated sensitivity, characteristic curves, layers and CDF parameters to one channel. Sum the three silver-density weights and retain the exposure axis and base density. The four profiles each preserve 256 response points and 81 wavelengths. Maximum double-precision differences were 4.44e-16 in optical density and 1.11e-16 in transmission. The stock loader and a diagnostic Vulkan ramp passed for all four. The fixed revision credits Hanze Qiu and lists the data under CC BY-SA 4.0. Production stock and multi-emulsion paper resources are a separate scope.

## Batch 6 — CPU/GPU diffusion differences
Existing differences predate recent integration: CPU uses per-channel radii and a multi-Gaussian tail approximation; parts of GPU use averaged radii and a different tail approximation. Compare point spread, line spread, step edges, grayscale, RGB patches and real photographs. Quantify spatial response, tail energy, color fringes and visibility. If changes are needed, create a versioned model separately from performance work.

On GTX 1080 Ti (582.78), a 5 µm sample of existing color-negative scatter and halation gave CPU versus Vulkan point-response 95% radii of R 65/30, G 45/25 and B 40/25 µm. The tails beyond 20 µm carried 26.5–35.0% versus 8.5–14.1% of the respective kernels. The tested Black Pro-Mist setting had closer 95% radii (R 335/335, G 265/265, B 170/175 µm) with a red edge-response difference. For a 513×513 real crop, comparing the effect-on minus effect-off print within each backend put the differences near 1.0–1.5 display codes RMS. Local red peaks were 34 codes for scatter/halation and 16 for lens diffusion. An optical model revision can be evaluated independently against these fixed inputs.

## Batch 7 — Offline grain and illuminant research
Grain: study negative-density domain, density bins, 2-D power spectrum and spatial correlation. Color-dye grain needs its own analysis; do not reuse monochrome silver-grain conclusions.
Illuminants: study CIE Y normalization of scene, film and reference spectra.
Deliver reproducible offline scripts/reports, not real-time rendering dependencies.

Two Vulkan seeds at four flat density levels and 6 µm sampling were measured in 64×64 interior windows. Double-X silver density used identical RGB residuals. Portra 400 layered residuals showed distinct channel variances and near-zero between-channel correlation. Report variance, spatial autocorrelation and radial frequency power by density and stock. Separate scene, film and viewing spectra were integrated at 5 nm and normalized with the CIE 1931 Y-weighted integral. With equal Y, the Portra 400 sensitivity predicts distinct D55 and 2856 K channel responses. These are offline diagnostics under fixed input, with no added renderer dependency. [CIE observer data](https://cie.co.at/datatable/cie-1931-colour-matching-functions-2-degree-observer) · [vkdt normalization reference](https://github.com/hanatos/vkdt/commit/c4fd786a06e68addabac94898dd744958b959687)

## Batch 8 — Later features
Evaluate edge codes/frame numbers/dates, 120/half-frame/XPan, complex light leaks, special perforations, variable-contrast paper and rem-jet optical behavior individually. Define physical stage, parameter contract, cache dependencies, resource cost, provenance and license before implementation.

See the [extension studies](EXTENDED_FILM_AND_PRINT.md) for the candidate contracts, cost estimates, provenance and acceptance cases.
