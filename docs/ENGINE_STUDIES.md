# Film engine studies

These November 2026 measurements cover the stated inputs, routes and hardware. The implemented 135 strip geometry and its cache dependencies are documented in the [film border contract](135_FILM_BORDER.md).

## Spectral CUBE export

Windows, 2026-11-18, Rust 1.99.1, NVIDIA RTX 3060 Ti Vulkan and the CPU backend. The CLI integration test compares all 27 nodes of a 3³ cube with a separately configured spectral photo render presented in a permuted order. It exercises Portra 400 / Endura (Hanatos, print shifts, preflash, print morph and encoded output) and Ektachrome 100 (Mallett, direct scan and linear output). Each backend is checked against its own direct render at an absolute channel tolerance of 0.000002. CPU f32, CPU f64 and Vulkan passed. Missing spectral data, unsupported requests and invalid sizes return errors. Existing file contents survive those failures.

For interpolation, a local camera JPEG (`DSC_5930.JPG`) was EXIF-oriented and reduced to an 8-bit 171×256 sRGB PNG. The second input was a 256×6 chart of linear code-value ramps from black to sRGB8 (255,255,255), (210,145,116), (140,66,240), (48,48,255), (255,35,20) and (10,255,40). Both compared Vulkan's direct encoded output before file quantization with trilinear interpolation of the exported CUBE on decoded linear RGB. Errors below are absolute channel differences in RGB8 code units.

| Input | Cube | RMS | 95th percentile | 99th percentile | Maximum |
| --- | ---: | ---: | ---: | ---: | ---: |
| Photograph | 33³ | 0.789 | 1.667 | 3.414 | 5.496 |
| Photograph | 65³ | 0.271 | 0.637 | 1.012 | 2.197 |
| Six ramps | 33³ | 1.321 | 3.102 | 4.970 | 6.609 |
| Six ramps | 65³ | 0.370 | 0.847 | 1.137 | 2.400 |

The comparison used Portra 400 and its target Endura paper, Hanatos, default fixed exposure, linear sRGB input, default encoded sRGB output and the following pointwise photo settings. Camera and enlarger diffusion are off by default.

```json
{
  "io": {"input_color_space": "sRGB"},
  "camera": {"auto_exposure": false},
  "film_render": {
    "grain": {"active": false},
    "halation": {"active": false},
    "dir_couplers": {"diffusion_size_um": 0, "diffusion_tail_um": 0, "diffusion_tail_weight": 0}
  },
  "print_render": {"glare": {"active": false}},
  "scanner": {"unsharp_mask": [0, 0]}
}
```

With this saved as `photo.json`, use the same backend for all three calls. The diagnostic needs NumPy and Pillow only on the validation machine.

```sh
spektrafilm process sample.png --film kodak_portra_400 --params photo.json -o direct.png --raw-out direct.f64
spektrafilm export-lut --film kodak_portra_400 --params photo.json --size 33 -o 33.cube
spektrafilm export-lut --film kodak_portra_400 --params photo.json --size 65 -o 65.cube
python tools/validation/compare_cube_photo.py sample.png direct.f64 33.cube 65.cube
```

The same Windows run passed the scoped stock setup, resident-input, border, GPU output, negative cache and probe-isolation suites. GUI compilation passed. Interactive GUI, CUDA, Metal, cross-platform CUBE consumers, native-resolution photo comparisons, throughput and peak VRAM were outside this measurement. See the [implementation priorities](ENGINE_EVOLUTION.md).

## Halation accumulation

The Windows Vulkan experiment fused the vertical halation blur into the existing bounce accumulator. Three bounces saved three dispatches and 72 bytes of logical intermediate traffic per pixel. On a GTX 1080 Ti (582.78), repeated warm medians were 41.85 ms versus 47.61 ms for a 1601×1067 synthetic render and 239.41 ms versus 243.30 ms for a 2615×3269 photograph. Peak memory above idle was the same in the sampled run. The photograph's largest absolute f32 difference was 7.33e-6 and RMS difference was 3.75e-8. The existing passes were retained.

## Portra print response

The resident paper route was exercised with existing Portra 160, 400 and 800 profiles and Portra Endura at −1.5, 0 and +1.5 EV. All channel ramps were monotonic. At 0 EV, the three 18% patches agreed to one rounded RGB8 code across profiles. Skin, green foliage and a saturated red subject were inspected in photographs. Portra 160 showed a modest reduction in red saturation while retaining a similar neutral and skin response. Portra 800 remains a valid candidate for a native-size grain and low-light comparison. The available profiles and calibration defaults are retained.

## Monochrome profile conversion

The conversion study covers Tri-X 400, T-Max 100, Acros II and HP5 Plus.

A fixed [SpektraLab data revision](https://github.com/JamesQiu2005/SpektraLab/tree/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/resources_product) was converted offline. Collapse triplicated sensitivity, characteristic curves, layers and CDF parameters to one channel. Sum the three silver-density weights and retain the exposure axis and base density. The four profiles each preserve 256 response points and 81 wavelengths. Maximum double-precision differences were 4.44e-16 in optical density and 1.11e-16 in transmission. The stock loader and a diagnostic Vulkan ramp passed for all four. The fixed revision credits Hanze Qiu and lists the data under CC BY-SA 4.0. The four negatives subsequently entered production with a separately calibrated three-emulsion paper. [Production resources and full grades](../data/SPEKTRALAB_BW_NOTICE.md)

## Multigrade IV RC at the hard end

The calibrated RS TH-KG3 head gives R 60.00 at grade 4 and R 57.30 at grade 5. Lower R denotes a narrower exposure range and a harder paper. Keep the seven shipped settings. The measurements below do not support assigning a calibrated 4½ label. Softer half grades were outside this comparison.

**Head, paper and measurement.** Run `cargo run --locked -p spektrafilm-core --example fit_multigrade -- --hard-grades`. Diagnostics use the production enlarger spectrum, fitted emulsion curves and bundled sensitivities. R is 100 times the log-exposure interval from 0.04 above base to 90% of maximum net density. Changing the exposure multiplier translates both crossing points equally.

| Controlled change | Model R | Interpretation |
|---|---:|---|
| Increase magenta from 250 to 1000 M | 57.30 → 57.24 | The head's spectral transmission has nearly saturated. A 1 CC scan from 1000 M through clear to 250 Y found its minimum at 1000 M. |
| Ideal shortpass, TH-KG3 below 440 / 460 / 480 nm | 60.34 / 60.06 / 54.11 | Removing green light changes relative emulsion exposures. The bluest band alone gives a softer result than the 480 nm cutoff. |
| Vary effective blue/green mixture, 1001 points | minimum 53.80 | Normalized sensitivity ratios at all 81 wavelengths lie on the line between the 430 and 530 nm responses (maximum residual 2.3e-16). A nonnegative spectrum produces a weighted mixture on this line. The best sampled mixture still misses the published 40 by about 14 R. |
| Stored vs fitted curves at 250 M; resample fitted CDFs at 8193 points | 57.30366 for both production tables; 57.20466 on the dense grid | Interpolation contributes about 0.10 R. The central 10%–80% width is 32.55 and describes a different interval. |

The dominant limitation is the combination of the **estimated silver-emulsion sensitivities and characteristic curves**. The comparison cannot identify which fitted emulsion parameter should change. The dichroic head accounts for a further few R relative to the best sampled spectrum. Increasing the M cap or changing the midgray exposure would give little or no benefit.

**Other implementations.** ILFORD publishes half steps for its dedicated filter set and different recipes by head type. Its colour-head guide warns of reduced peak contrast and calls for testing the individual head. RH Designs likewise provides head-specific filtration and allows paper calibration. SpektraLab ships seven whole-grade packs. Its grade 4 and 5 use 39.1 and 151.7 M; running those settings through RS gives R 65.79 and 57.86. LAB's regression checks ordering and separation of grades 00, 2 and 5 through a negative. These are different checks from HARMAN's paper-only ISO R table. The public OFX main renderer (86476af) and upstream Python main (3bb2c2d) provided no matching Multigrade half-grade calibration in the inspected source. [ILFORD head guidance](https://www.ilfordphoto.com/wp/wp-content/uploads/2017/03/Contrast-control-for-Ilford-Multigrade.pdf) · [IV RC paper data](https://www.ilfordphoto.com/wp/wp-content/uploads/2021/01/MULTIGRADE-IV-RC-Papers-060619.pdf) · [RH Designs](https://rhdesigns.co.uk/filter-settings) · [LAB packs](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/resources_product/paper_grades.json) · [LAB checks](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/tests/bw_checks.py)

**Output check.** A trial half step targets the midpoint of the *attainable* R values, 58.65, with 114.98 M and a midgray-matched paper exposure of 1.34919. Reproduce it using the normal grade-4 recipe with `enlarger.m_filter_shift = 28.720576` and `enlarger.print_exposure = 1.00152943`. Use Tri-X 400, clear camera filter, manual exposure, sRGB input and encoded sRGB output. Disable grain, halation and highlight boost, DIR, glare and scanner sharpening. Render with `process --raw-out`. Compare absolute green-channel differences before file quantization in 0–255 code units.

| Input, Windows resident Vulkan | 4 → trial, p95 | Trial → 5, p95 | 4 → 5, p95 | Pixels with 4 → 5 difference ≥2 |
|---|---:|---:|---:|---:|
| Neutral PNG ramp, all 256 input codes | 1.66 | 1.38 | 3.01 | 16.0% |
| Colour costume portrait, 341 × 512 | 1.66 | 1.35 | 2.98 | 13.7% |
| Grayscale camera photograph, 512 × 512 | 1.77 | 1.50 | 3.22 | 28.3% |

The repeat grade-4 green-channel buffers matched exactly. Neither adjacent trial step reached two code values anywhere in these three outputs. The endpoints show a modest difference in midtone detail; the trial looks very close to both at preview size. These values describe the test outputs, not a general visual threshold. Physical 4½ characterization would need a matching paper/head step-wedge measurement. [Public grayscale input](https://github.com/scikit-image/scikit-image/blob/v0.25.2/skimage/data/camera.png)

## CPU and Vulkan diffusion

On GTX 1080 Ti (582.78), a 5 µm sample of existing color-negative scatter and halation gave CPU versus Vulkan point-response 95% radii of R 65/30, G 45/25 and B 40/25 µm. The tails beyond 20 µm carried 26.5–35.0% versus 8.5–14.1% of the respective kernels. The tested Black Pro-Mist setting had closer 95% radii (R 335/335, G 265/265, B 170/175 µm) with a red edge-response difference. For a 513×513 real crop, comparing the effect-on minus effect-off print within each backend put the differences near 1.0–1.5 display codes RMS. Local red peaks were 34 codes for scatter/halation and 16 for lens diffusion. An optical model revision can be evaluated independently against these fixed inputs.

### Resident diffusion reconstruction (batch 6)

RS uses one grid per filter, with factor `ceil(max_sigma_px / 32)` bounded below by one. The narrow and wide components therefore share a sampling phase. OFX Vulkan auto groups components at their own scales (1× below 12 px, then 2×/4×/8× at 12/24/48 px). That can preserve narrow cores but adds groups to reconstruct. LAB main fits the line-spread function on binomial pyramid levels and resolves the cusp with an 11×11 core. Its Gaussian-mixture reference and phase-sensitive lens tests are valuable when changing the PSF. Its lens runs on the source before film processing, while RS applies the filter to camera or enlarger exposure. The LAB Windows/Linux branch explicitly rejects both active diffusion filters. Upstream timings therefore serve as leads. Neither upstream binary was benchmarked here. [OFX dispatch](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/src/SpektraVulkanRenderer.cpp) · [LAB lens and reference](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/pipeline/lens.cpp) · [LAB verification](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/tests/lens_checks.py) · [Windows/Linux validation](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/pipeline/pipeline.cpp)

Keep the measured RS kernel, pitch and weights. Bilinear reconstruction now mixes into each invocation's own direct pixel. This removes two full-size RGB f32 buffers (24 bytes/pixel per active stage), two dispatches and a full-size GPU copy per application. The four reduced-grid images remain. No host transfer or cache key changed.

**RS before/after.** Baseline `375442e`, Windows Vulkan RTX 3060 Ti, default f32 CLI dev build. Portra 400 / Endura, Black Pro-Mist strength 0.5, default 35 mm format, manual exposure, grain/halation/DIR/print glare off. The two filter positions were enabled individually. Test sources were an otherwise black 769×513 sRGB PNG with four single bright pixels (white and coloured, including the last pixel) and two Lanczos reductions of one real FPW photograph. The smaller photographic PNG has SHA-256 `36cf007807f9464914749cb80954dd032b8b01f653cc9234c0e1bcd7cfc84e9c`. The diffusion-off controls differed from the enabled prints by at least 0.083 peak normalized output in each of the four smaller cases. All timing logs confirmed resident rendering.

| Input and stage | Warm pipeline median, old → new (ms) | Max absolute output difference before file quantization | Changed RGB samples in 8-bit PNG |
| --- | ---: | ---: | ---: |
| Points, camera | 21 → 20 | 0 | 0 |
| Points, enlarger | 21 → 20 | 0.00000059 | 0 |
| Photo 1025×1537, camera | 35 → 32.5 | 0.00000531 | 14 / 4,726,275 |
| Photo 1025×1537, enlarger | 36 → 33.5 | 0.00000382 | 24 / 4,726,275 |
| Photo 2049×3073, camera | 102.5 → 85 | PNG comparison only | 59 / 18,889,731 |

Each changed PNG sample moved one code. The smaller photo's RMS differences were 0.000000075 and 0.000000098 for camera and enlarger respectively. Timing used `spektrafilm process INPUT --film kodak_portra_400 --params RECIPE --output OUT.png --iters 5 --raw-out OUT.f64`. Discard iteration one. On the larger photo, run nine iterations per process in old/new/new/old order, pooling 16 warm measurements per version (94–114 ms and 81–87 ms). Timing includes image cloning and resident render/readback, excludes file I/O and cold initialization.

**Temporary GPU memory.** In separate single-frame camera renders, sample `device.generate_allocator_report()` immediately before returning the constructed diffusion state. Live device allocations were 28.49 → 19.46 MiB for the point field (2× grid reduction), and 96.06 → 60.00 MiB for the smaller photo (4×). Reserved Vulkan blocks were 192 MiB in all four runs. This sample measures live allocations at that stage; whole-process peak residency was not sampled. Keep the allocator instrumentation out of the render loop. The timing and output comparisons used uninstrumented builds.

**Batch 9 follow-up.** Grain-on FPW replays exposed a small numerical difference in the fused mix. The former first pass rounded the weighted direct pixel before accumulating weighted scatter. Make that order explicit with `fma(scatter_weight, scattered, rounded_direct)`. On RTX 3060 Ti, all 1,183,491 channels of the point-light frame and all 4,726,275 channels of the 1025×1537 photo matched the pre-fusion float output exactly, at both camera and enlarger positions. The original timing and allocation measurements above describe batch 6. [Host RGB16 comparison](maintenance/FPW_PARITY_MIGRATION_PLAN.md#candidate-replay-on-fpw-main--2026-11-19)

## Negative-derived print hue (batch 7)

Blue and violet fabrics retained more of the input photograph's hue. Skin results depended on grain and DIR. RS offers this as an optional colour print look. Optical remains the default. [Usage](USAGE.md#hue-from-negative)

LAB main at `b51e672` develops a polar calibration sheet and builds a map from normalized layer-exposure ratios to Oklab chromaticity. The slope of each inverse characteristic curve and the chroma at print lightness determine how far the optical hue turns. A boundary scanned from six faces of the paper dye-density box limits chroma. LAB uses an Apple-only node with separate minilab and DI gates. RS prepares tables from its own spectral front, local DIR, effective paper curves and viewing illuminant. No LAB binaries, baked tables or new profile data are distributed. [LAB calibration](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/negative_hue.cpp) · [LAB pixel operation](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/shaders/negative_hue.metal) · [LAB checks](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/tests/negative_hue_checks.py)

### Same-negative photograph comparison

Portra 400 / Portra Endura, Hanatos, fixed exposure, database neutral filtration, sRGB input and linear sRGB output. Halation, glare, output gamut compression and scanner sharpening were off. The reference harness fed the **same dumped RS negative density** and its optical print into reconstruction. It used LAB's compiled C++ layer, exposure-map and gamut builders with RS calibration patches and a host transcription of the pixel math. Its gamut used the stored paper curves; the native implementation uses the fitted or morphed curves actually consumed by RS. Across four clean photos, native CPU and the offline reconstruction differed by at most 0.00049 in linear RGB. Native CPU and Vulkan differed by at most 0.000040 on those photos. No Metal product or physical paper prints were measured.

Median absolute circular Oklab hue error in degrees against the digital input, **optical → reconstructed**. Each cell compares one developed negative. The clean column uses the 35 mm setting. The other two use the same spatial DIR and a 5 µm pitch, changing only grain. Source pixels with L ≤ 0.19 or chroma ≤ 0.035 are excluded.

| Source region | No DIR or grain | DIR, no grain | DIR and grain |
| --- | ---: | ---: | ---: |
| 03, blue cap (1,698 pixels) | 21.57 → 0.01 | 18.01 → 3.80 | 17.88 → 9.18 |
| 03, saturated caps (48,321) | 13.29 → 0.03 | 14.83 → 0.76 | 13.53 → 3.27 |
| Purple/magenta silk (73,461) | 8.18 → 0.01 | 13.20 → 2.20 | 14.69 → 5.20 |
| 04, cheek (5,451) | 5.11 → 0.02 | 1.19 → 1.13 | 13.21 → 18.04 |
| 15, cheek (2,120) | 2.83 → 0.01 | 1.85 → 2.33 | 6.27 → 9.32 |

The yellow-green cap printed ochre optically. The reconstructed print moved it toward yellow-green. The sari's blues and purples separated more clearly while fold shading remained. Grain increased pixel-to-pixel skin colour variation. On the grain-on offline comparison, averaging linear RGB in 5×5 blocks put the 04 cheek at 5.58 → 7.07° and 15 at 3.36 → 3.19°. This is a source-hue preference, with no measured scene reflectance target.

To repeat, use upright Kodak PhotoCD 03, 04 and 15. The cheek rectangles in fractional source coordinates are x 0.27–0.44, y 0.47–0.56 (04) and x 0.53–0.60, y 0.50–0.58 (15). The blue cap uses x 0.78–0.92, y 0.54–0.86 and hue 240–315°. The saturated cap region uses y 0.18–0.84 and C > 0.12. The 500×610 sari preview uses hue 265–355°. A separate 1,440-patch chart sampled HSV hue every 3°, saturation 0.30/0.65/1 and value 0.20/0.45/0.70/0.95, rounded to sRGB8. The 313 eligible blue/violet patches (source Oklab hue 245–325°) measured 19.02 → 0.01° in the clean print. Compare CLI `process --raw-out` results with only `print_render.negative_hue` switched. [Kodak suite](https://r0k.us/graphics/kodak/) · [Sari photograph by the Peloponnesian Folklore Foundation, CC BY-SA 4.0](https://commons.wikimedia.org/wiki/File:Folded_sari_in_purple_and_magenta_shot-silk_with_gold_pattern,_1970.jpg)

On RTX 3060 Ti Vulkan, optical → reconstructed → optical → reconstructed gave one negative miss/store and three hits. Switching back reproduced the optical CPU output exactly. With an eligible snapshot, the GPU reads the negative directly from that snapshot. Without one it preserves the negative and uses one temporary RGB f32 plane for paper density. Neither path adds a host readback. The 135 rebate and perforations matched the optical Vulkan render with spatial scanner effects off. Regressions also cover neutral gray, a second pair (Gold 200 / Crystal Archive Type II), local DIR refresh, direct scan, slide and silver bypass. Visual acceptance of other pairs, manual Y/M trims, wide-gamut output and CUDA rendering remain open. [Regression tests](../crates/spektrafilm-core/tests/negative_hue.rs)

## Manual enlarger Y/M and negative-derived hue (batch 8)

Use negative-derived hue at the calibrated neutral filter pack. For deliberate enlarger colour correction, select the optical print. The current reconstruction estimates a target hue from the negative and steers the optical print toward it. Moving the enlarger filters changes the optical print while the estimated target stays fixed. The following trial measured how much of the operator's hue adjustment survived.

The CPU photo CLI printed one sheet of original-scale crops from Kodak 03 (blue and red caps), Kodak 04 and 15 (cheeks) and the purple/magenta sari used in batch 7. Each film/paper pair has a TH-KG3 neutral pack in the shipped database. The sheet used fixed exposure, spatial DIR at 5 µm, grain and halation off, and linear sRGB output with glare, gamut compression and sharpening off. Each pair produced 12 renders across six manual (Y, M) offsets: (0, 0), (±5, 0), (0, ±5) and (+10, −10), optical and reconstructed at each setting. Within each pair, all 12 developed density buffers had the same SHA-256. The probe removed only the two manual-shift checks in `Pipeline::supports_hue_output`. The production CLI retained the checks.

Median absolute circular Oklab hue error in degrees against the input at the neutral pack, optical → reconstructed:

| Film / paper | Blue cap | Red cap | Purple/blue silk | Cheek 04 | Cheek 15 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Portra 400 / Portra Endura | 18.25 → 3.76 | 3.41 → 0.86 | 21.19 → 2.82 | 1.17 → 1.02 | 2.05 → 2.09 |
| Gold 200 / Crystal Archive Type II | 22.02 → 3.18 | 5.51 → 0.68 | 12.19 → 2.48 | 1.58 → 0.81 | 5.74 → 0.92 |
| Ektar 100 / Endura Premier | 13.07 → 2.82 | 5.15 → 0.54 | 25.33 → 2.04 | 4.05 → 0.71 | 10.47 → 1.14 |
| Pro 400H / Crystal Archive Type II | 17.21 → 3.03 | 4.73 → 1.14 | 20.56 → 2.35 | 6.51 → 0.62 | 2.42 → 1.12 |

To assess trims, compare each method with **its own neutral print**. With M +5, optical hue rotated by a median 10.76–17.34° on cheek 04 across the four pairs. Reconstruction rotated it only 0.001–0.032°. On the blue cap the corresponding ranges were 3.02–6.91° and at most 0.001° to the reported precision. With Y +5 the same cheek rotated by 5.93–11.72° optically and at most 0.014° with reconstruction. The Ektar crops show the skin cast changing under optical filtration and returning almost to its neutral-print hue after reconstruction. A small source-hue error after a trim would therefore reward cancelling the operator's correction. Retain the zero-trim condition. Broader neutral-pack fabric evidence does not extend the two grain-on portrait results in batch 7.

The production resident test on RTX 3060 Ti runs Ektar/Premier on the same source through neutral reconstruction, three optical trims, then neutral reconstruction again. Rebuilding print calibration gave **one negative cache miss, one store and four hits**. Each trimmed reconstruction request was rejected, all five supported prints matched the CPU within 0.001 per channel, and the first and last Vulkan outputs matched exactly.

For reproduction, build a separate CPU probe CLI from `a62ee08` with only the Y/M checks above removed. Run `python scripts/parity/negative_hue_trims.py <fixture-dir> <scratch-dir> <probe-exe>` from the repository root. Supply upright `kodim03.png`, `kodim04.png`, `kodim15.png` and the 500×610 sari preview as `textile.png`. The script writes the cropped comparison image and all 120 region measurements, including per-pixel disagreement between the two trim responses. It requires NumPy and Pillow for offline analysis. [Photograph sources](#negative-derived-print-hue-batch-7) · [Measurement script](../scripts/parity/negative_hue_trims.py) · [Production-path test](../crates/spektrafilm-core/tests/negative_hue.rs)

## Density-domain grain and illuminants

Two Vulkan seeds at four flat density levels and 6 µm sampling were measured in 64×64 interior windows. Double-X silver density used identical RGB residuals. Portra 400 layered residuals showed distinct channel variances and near-zero between-channel correlation. Report variance, spatial autocorrelation and radial frequency power by density and stock. Separate scene, film and viewing spectra were integrated at 5 nm and normalized with the CIE 1931 Y-weighted integral. With equal Y, the Portra 400 sensitivity predicts distinct D55 and 2856 K channel responses. These are offline diagnostics under fixed input, with no added renderer dependency. [CIE observer data](https://cie.co.at/datatable/cie-1931-colour-matching-functions-2-degree-observer) · [vkdt normalization reference](https://github.com/hanatos/vkdt/commit/c4fd786a06e68addabac94898dd744958b959687)

Sources for the experiments are collected in the [research notes](RESEARCH_NOTES_2026-11.md). Proposed formats, edge markings and variable-contrast paper are described in the [extension studies](EXTENDED_FILM_AND_PRINT.md).

## Negative-film Langmuir DIR (Batch 11)

Keep the linear DIR model for production. The upstream dev implementation at `6cd00c8` applies a saturating donor to negative film. For each layer of base-subtracted density D, the default knee K is its maximum characteristic density and the release is D × 1.5K / (K + D). Release matches linear at half the maximum. Both the inverse characteristic-curve construction and the forward pixel correction use the same release before applying the existing interlayer matrix. Reversal has a separate receiver-side model. [Upstream implementation](https://github.com/andreavolpato/spektrafilm/blob/6cd00c8/src/spektrafilm/model/couplers.py)

Two builds of the RS CPU CLI at `6fe72b5` rendered identical input with fixed 5 µm pitch, active spatial DIR, grain off, optical printing and manual camera exposures of −2, 0 and +2 EV. The experimental build changed only the two donor calculations. Each image contained original-scale blue and red cap, silk and two cheek crops from Batch 8 plus 12 neutral patches (sRGB codes 4–250). The tested pairs were Portra 400 / Portra Endura, Gold 200 / Crystal Archive Type II and Ektar 100 / Endura Premier. Both the developed negative and the unencoded sRGB print were captured at each exposure.

| Photo region | 0 EV | +2 EV |
| --- | ---: | ---: |
| Blue cap | 1.04–1.57° | 1.88–2.86° |
| Red cap | 0.05–0.48° | 0.17–1.21° |
| Blue-purple silk | 0.85–2.26° | 1.06–3.00° |
| Cheek crop 04 | 0.40–1.22° | 1.38–1.97° |
| Cheek crop 15 | 1.04–1.88° | 1.64–2.65° |

Entries are the range across stock/paper pairs of median absolute Oklab hue movement within each crop, excluding pixels below 0.035 chroma in either render. Regions include photographed texture and nearby colors. All 18 prints retained monotone gray-wedge lightness. Across nine A/B comparisons the largest within-patch 95th-percentile Oklab distance on gray was 0.0056 (0–1 lightness scale). In the blue cap it reached 0.0714 at +2 EV. The normal-exposure pairs show similar skin and visible localized changes in saturated textile. There is no measured photographic reference for these stock/scene pairs to support a claim of greater color accuracy. The comparison supports retaining the existing look. Resident GPU, slide film and B&W were outside the experiment.

**Reproduction.** Build `cargo build --locked -p spektrafilm-cli --bin spektrafilm` in clean and probe worktrees at the recorded revision. Apply [the research patch](../scripts/parity/langmuir_donor_probe.patch) only to the probe worktree with `git apply`; use separate target directories or copy out the first executable. From the RS root run `python scripts/parity/langmuir_donor.py FIXTURES SCRATCH LINEAR_EXE DONOR_EXE` with NumPy, Pillow and the [Batch 8 fixtures](#negative-derived-print-hue-batch-7). The script checks finite density and print values, negative changes and wedge order; it writes the prints and 153 regional measurements to scratch.

## 48 µm aperture granularity (Batch 12)

Kodak reports diffuse RMS granularity 14 for Double-X at net diffuse visual density 1.0 with a 48 µm aperture, tungsten exposure and D-96 at 21 °C. Its 6.5-minute curve has gamma 0.66. RS used that development setting, a neutral RGB field and the normalized single-silver negative. This models density and aperture sampling; it does not reproduce a physical sensitometer or the illumination and scattering geometry of an actual densitometer. [Kodak technical data](https://www.kodak.com/content/products-brochures/Film/DOUBLEX-Technical-Data-EN.pdf)

| Double-X particle area | CPU at 3 / 6 µm pitch | Resident Vulkan at 3 / 6 µm pitch |
| --- | ---: | ---: |
| Existing 0.20 µm² | 10.38 / 9.78 | 10.43 / 9.76 |
| Optional 0.39 µm² | 14.58 / 13.46 | 14.55 / 13.63 |

Values are 1000 × sample standard deviation of 4096 non-overlapping aperture readings on the same 3.072 mm square. The grain-off control sets exposure at net density 1.0; center the circular aperture in each cell, average transmission including fractional pixel coverage, then convert back to density. At fixed exposure the optional setting's mean aperture density was 0.989–0.997. Grain-off RMS was below 0.1 for all measured configurations. The candidate area was estimated by variance scaling from the two baseline pitches and then rendered independently. It brings both paths within 0.6 of 14 under these conditions. Keep it as an explicit Double-X recipe. This single physical field per configuration does not establish the entire density response, different scan pitches or a stock-wide default.

The capture checks the actual resident branch. Double-X uses the single silver sampler even when multilayer is requested. Portra 400 takes the layered sampler. With default particle area and green-sensitive layer near density 1, Portra's C/M/Y RMS was 8.57/7.09/12.56 at 3 µm and 8.25/6.55/11.82 at 6 µm. Yellow density was 1.17 at both pitches. These are individual dye-channel baselines for the final comparison; a color print-grain index or a diffuse visual silver measurement has a different meaning.

**Implementation comparisons.** OFX public Vulkan scales particle counts by sampled pixel area, per-channel particle area and the sublayer fraction. Its shader also has a separate synthesis path. LAB main's Metal particle model uses Poisson thinning; its parity test compares flat-patch mean, standard deviation and skew on optical output. The Windows/Linux Vulkan shader seeds each pixel and layer stream independently. Apply that statistical comparison to like output stages. The original Python dev calibrates area from a target RMS; the fitted multilayer path seeks the peak over the density curves. A peak target and a reading at fixed net density 1.0 can differ. [OFX public shader 86476af](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/shaders/vulkan/SpektraGrain.comp) · [LAB Metal b51e672](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/shaders/grain.metal) · [LAB distribution test](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/tests/parity_grain.py) · [LAB Vulkan f87c521](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/shaders/vulkan/spk_grain_layers.comp) · [Python dev 6cd00c8](https://github.com/andreavolpato/spektrafilm/blob/6cd00c8/src/spektrafilm/model/grain.py)

**Reproduction.** From RS root, use the normal CLI for CPU. At RS `c2061a8`, apply [the offline capture patch](../scripts/parity/grain_aperture_gpu.patch) to a separate worktree and build `cargo build --locked -p spektrafilm-cli --bin spektrafilm` with a separate target directory. The probe selects Vulkan and returns density through the existing CLI raw readback. Run `python scripts/parity/grain_aperture.py CLI SCRATCH` for CPU, adding `--gpu-probe` for the patched CLI. Repeat with `--particle-area 0.39` for the optional recipe, or with `--film kodak_portra_400 --gpu-probe` for the color baseline. Use distinct scratch directories. NumPy and Pillow are offline dependencies. [Measurement driver](../scripts/parity/grain_aperture.py) · [Analytic aperture tests](../scripts/parity/test_grain_aperture.py)

## Arctic alpha02 and beta04 (Batch 13)

Keep the existing alpha02 resource and recipes. Arctic infers a spectrum from a pixel so the film can respond differently to different wavelengths. Both tables use the same 192² grid, D65 chromaticity and 81 bands from 380 to 780 nm. RS reads the spectra directly; the optional low-rank alpha basis does not participate in this comparison. The experimental beta04 commit describes laser-like rims. At the nearest grid cell to sRGB blue, the strongest band moved from 450 to 430 nm, and for red from 640 to 660 nm. These are sample properties of the tables. The source pixel supplies no measurement of the actual spectrum. [Beta04 introduction](https://github.com/andreavolpato/spektrafilm/commit/d8933a9)

The experimental runtime at `28bf883` integrates reflectance under the film illuminant and divides each channel by the method's own relit D65 neutral sampled with the runtime cubic interpolator. Current RS alpha integrates directly. Experimental also revised the stock profiles and neutral print-filter database. Applying only its database to RS stock curves can move a print independently of camera exposure. Both LAB branches expose a Hanatos exposure path. OFX public Vulkan builds the Hanatos raw response from its spectra. These are useful exposure-stage comparisons; beta04 measurements here use the original Python project's actual table. [Upstream normalization](https://github.com/andreavolpato/spektrafilm/blob/28bf883/src/spektrafilm/utils/spectral_upsampling.py) · [Upstream stock recalibration](https://github.com/andreavolpato/spektrafilm/commit/1986503) · [LAB main](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/hanatos.cpp) · [LAB Windows/Linux](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/core/hanatos.cpp) · [OFX public Vulkan](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/src/SpektraVulkanRenderer.cpp)

Eight RS CPU renders covered Portra 400 / Portra Endura and Ektar 100 / Endura Premier. Each comparison image shows four rows in order: production alpha, beta table alone, beta with per-channel gains matched to the alpha camera response at neutral sRGB code 118, then the matched beta with experimental print filters. The match uses copied film sensitivities, holds the production neutral as the common reference and checks it before development. This isolates the spectral shape; it does not implement upstream's absolute exposure and development calibration. The last two rows use identical gray camera exposure. RS paper and film profiles, manual exposure, optical printing, disabled grain and input were fixed.

| Region | Portra | Ektar |
| --- | ---: | ---: |
| Gray patch | 0.07 | 0.10 |
| Pure blue patch | 38.25 | 31.09 |
| Blue-purple silk | 5.96 | 4.74 |
| Cheek crop 04 | 2.43 | 1.68 |
| Cheek crop 15 | 1.89 | 1.68 |

Values are median absolute changes in the most affected displayed sRGB channel per pixel on a 0–255 scale, comparing alpha and neutral-matched beta. The silk's 95th percentile reached 10.39 and 19.07 respectively. Switching only the print filters in the fourth row moved gray another 2.27 codes on Portra and 13.97 on Ektar. The crop comparison establishes a visible difference for saturated photography; it provides no measured film reference that favors beta. Keep any future beta calibration separately identified. No production GPU, full experimental runtime, OFX/LAB product or physical-film comparison was run here.

For reproduction use the checkout and CLI at `5300b18` and run `python scripts/parity/arctic_versions.py CLI UPSTREAM_CHECKOUT FIXTURES OUTPUT` at the RS root with the Batch 8 fixture files, NumPy and Pillow. The driver reads exact blobs from the pinned Git commit, uses a temporary copy of RS data and writes two comparison sheets and the region statistics. [Study driver](../scripts/parity/arctic_versions.py)

### Arctic provenance and product defaults

Andrea Volpato published Arctic under his PIXLS.US username `arctic`, with an alpha download and diagnostics. Its material prior favors plausible skin and vegetation spectra. Alpha02 addressed smoothness. Beta01 broadened the training set; beta02 reduced excessive skin weighting after nearby colors in leaves, a wall and a bench acquired a skin-like rendering. For beta04, one- or two-peak Gaussian spectra near the chromaticity boundary improved the reported Rec.2020-primary errors. The author found little change in vegetation and skin relative to beta03. Hanatos and jandren discussed the edge transition and material ambiguity. These are author diagnostics and technical feedback, with no independent head-to-head film measurements for the RS subjects. [Original alpha](https://discuss.pixls.us/t/spektrafilm-tech-discussions/57512/50) · [Beta02](https://discuss.pixls.us/t/spektrafilm-tech-discussions/57512/59) · [Beta04](https://discuss.pixls.us/t/spektrafilm-tech-discussions/57512/75) · [Hanatos feedback](https://discuss.pixls.us/t/spektrafilm-tech-discussions/57512/77)

- Spektrafilm OFX shipped Arctic for natural materials and warmer skin and updated it to beta04 in v0.3.1. The Linux v0.3.12 package manual (pp. 21, 38, 49) identifies Hanatos 2026 as default and calls for correctly white-balanced input for Arctic. Inspected archive SHA-256 `f3f4409c6203d5f6cb1387797f160b60821a00940fc27804d6550dd3ed572138`. [Official history](https://spektrafilm.114c.de/changelog/) · [Package page](https://spektrafilm.114c.de/)
- SpektraLab rejects RGB-to-raw methods other than Hanatos 2025 at pipeline initialization on [main](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/pipeline/pipeline.cpp#L143) and [Windows/Linux v2](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/pipeline/pipeline.cpp#L155).
- Upstream Python experimental briefly defaulted to beta04 on June 30, then restored the Hanatos 2025 settings default on July 5. Beta04 remains selectable. [Change to Arctic](https://github.com/andreavolpato/spektrafilm/commit/a1c0bb0) · [Return to Hanatos](https://github.com/andreavolpato/spektrafilm/commit/0f690da)

Batch 14 subsequently replaced the selectable alpha02 with beta04 under its own method ID. Include beta04 in the final comparison, using properly balanced portraits, non-skin materials near skin chromaticities, saturated dyed fabric and narrowband emissive lights. Test camera-exposure errors on held-out measured spectra alongside the photographic comparisons.

## Hanatos version names and FPW

The public OFX implementation at `86476af` labels two ways to weight the same Hanatos spectral table. Its 2025 path reads archived wavelength-by-channel bandpass values. Its 2026 path uses the film profile's four `hanatos2025_adaptation_window_params` to taper sensitivity at the UV and IR ends, then restores each channel's integrated response under the reference illuminant. The generator explicitly maps that 2025-named field to its 2026-named array. The original Python implementation calls this per-stock spectral containment. The OFX maintainer introduced the window and selected it by default; the published change does not give a comparative photographic benchmark or further selection rationale. [OFX generator](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/tools/generate_profile_curves.py) · [OFX exposure code](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/src/SpektraVulkanRenderer.cpp#L642) · [OFX change](https://github.com/chaert-s/spektrafilm-ofx/commit/d0387a1) · [Original Python change and data](https://github.com/andreavolpato/spektrafilm/commit/26eabba) · [Hanika's vkdt](https://github.com/hanatos/vkdt)

FPW's pinned RS already enables that window by default for profiles with fitted parameters. FPW and RS Portra 400 and Ektar 100 profile files match after line-ending normalization. The isolation test used their two Batch 13 print pairs, the same caps, silk and cheek crops and a Kodak 18 foliage crop. It applied the OFX archive weights or independently calculated window weights to temporary sensitivity profiles, disabled the second window in those trials, and left the normal RS run alone. Six real CPU prints. Across both sheets, RS default and the OFX 2026 formula gave identical rounded 8-bit sRGB pixels. Between the two OFX formulas the median maximum-channel display difference for cheeks was 3.98–5.76 codes, silk 2.87–4.37 and foliage 0.04–0.23 (foliage p95 7.40–7.74). This isolates the camera-response change with a shared RS print pipeline. It does not compare entire OFX binaries, certify release-only stocks, or measure accuracy against physical film. Retain FPW's current default. [Reproduction script](../scripts/parity/hanatos_versions.py) takes CLI, OFX checkout, Batch 8 fixture directory plus kodim18.png, and output directory, from the RS root.

## Print bleach bypass

OFX models film-side silver before light reaches the paper; its negative path also allows a cyan dye loss. Print-side silver acts on already developed paper during scanning. For each nonnegative dye density D, the OFX print model estimates retained silver as D/(D+0.65), averages across layers, then scales by the requested fraction. It moves dye densities toward the largest of the three using that weight and adds 0.36 times the weight as neutral spectral density. These coefficients and the dye adjustment describe a creative approximation. RS implements this print-side model for color paper only. The CPU scan LUT subtracts the neutral density in log XYZ; the direct path applies its equivalent transmittance in linear light. Resident Vulkan evaluates the effect in the scan shader with no extra image or readback. [OFX reference](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/shaders/SpektraFilm.metal#L1737)

Two fixed Hanatos 2025 optical prints (Portra 400 / Portra Endura and Ektar 100 / Endura Premier) used the Batch 13 crops, grain off and a manual exposure at 5 µm pitch. The bottom row contained eight neutral patches from input sRGB code 16 to 255. At 0%, 50% and 100%, each stock's three CPU negative density dumps shared one SHA-256. On Portra the middle gray displayed at green codes 118, 105 and 94 and the brightest patch at 219, 216 and 213. Median Oklab chroma on the purple silk crop fell from 0.138 to 0.076 to 0.034; the two cheek crops fell from 0.060 to 0.032 and from 0.085 to 0.034 at full strength. Ektar silk moved from 0.176 to 0.100 to 0.048. This yields a useful adjustable low-color print with distinct shadow and highlight behavior.

The Vulkan cache regression printed at 0 → 0.5 → 1 → 0 with one miss, one store and three hits; the final frame exactly restored the first. For the unmodified Portra recipe, the raw output matched the older linear CLI control byte-for-byte separately on CPU and GPU. Film-scan and silver-paper tests check inactive behavior. [Regression tests](../crates/spektrafilm-core/tests/negative_hue.rs)

## Film-base back reflection on Vision3 500T

**Decision.** Keep the current halation renderer. LAB's no-backing look is distinct and useful for approximating exposed film with its anti-halation backing absent. The direct RS CPU port spreads too much red light into the far tail; the current resident blur also cannot represent the fitted channel-specific kernels as configured. Keep the probe for a future implementation that qualifies wide, signed, channel-specific blur on both backends.

**Why LAB added it.** LAB models 800T with a 500T emulsion alias and keeps the parent print calibration. Its goal was the broad red rim at a lamp or tube seen in sample scans. For a nominal 120 µm acetate base it models light reflected at the rear surface (including total internal reflection), partial return from a pressure plate and attenuation through the other layers. The fit has four signed Gaussian weights and widths *per channel*, with a narrower green halo. Its plate reflectivity, scattering fractions, layer losses and strengths are modeling choices. LAB main applies this when the film has no backing, whereas Windows/Linux v2 reserves the special path for its native no-backing stock and uses a conventional preset for manual removal. The model concerns the backing during exposure. [Physical model and constants](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/halation_psf.hpp) · [Stock selection](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/params.cpp#L713) · [Windows/Linux selection](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/core/params.cpp#L563) · [Reason for the alias](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/minilab-coverage/FINDINGS.md)

**Matched comparison.** RS at `8d229c3`, Vision3 500T / Kodak 2383, manual exposure and 5 µm pitch. Compared the existing three-bounce RS kernel with LAB's coated 500T settings (50 µm first sigma; R/G/B strengths 0.015/0.005/0), the same kernel with strengths 0.30/0.03/0.003, and LAB's no-backing kernel with those same raised strengths. Flat-field renormalization stayed on. Grain, DIR, emulsion scatter and highlight boosting stayed off to isolate the returning light; no synthetic clipped-lamp headroom was added.

- On a centered one-pixel white light the measured 90%-energy radii of the off-center R/G exposure were 145/145 µm with either conventional setting and 1170/448 µm with the LAB mixture. These are finite 513² image measurements. Within that same sampled domain, 82.2% of the probe's red halo fell inside 1 mm, against 93.1% for the sampled analytic Gaussian mixture. Green was closer at 98.6% versus 99.8%. Increasing the ordinary halo strength cannot change its radius.
- The Batch 6 photograph includes bright white fur bordering dark glasses. Rendering the full 1025×1537 photograph before measuring the interior of its 513² crop reproduced the broader red spill in the glasses. At matched strength the median / p95 change in the most affected sRGB channel was 2.6 / 22.5 codes (0–255). The small-crop render gave 2.6 / 21.8. There was no physical backed/unbacked scan pair for this scene.
- Warm debug CPU medians on the 513² crop were 47 / 73 / 85 ms for the halation section in the three configurations (three warm passes each). On the full 1025×1537 frame the corresponding halation medians were 246 / 358 / 469 ms. Crop whole-pipeline timings were 810 / 866 / 851 ms and do not resolve the smaller difference between the latter two. RS resident halation uses a shared RGB radius and scalar accumulation weight per bounce; the separable FIR radius also caps at 256 pixels. At this pitch the widest fitted green sigma is 227 pixels before truncation. These CPU times do not predict GPU time or VRAM. No production GPU variant was qualified.

**Reproduction.** Use a separate RS worktree at the revision above, apply [the CPU probe](../scripts/parity/back_reflection_probe.patch) with `git apply` and build the standard CLI with a separate Cargo target. From RS root run `python scripts/parity/back_reflection.py PROBE_CLI BATCH6_PHOTO OUTPUT`, then repeat with `--full-context` in a new output directory. The driver records radial metrics, display differences, timings and a comparison sheet. [Measurement driver](../scripts/parity/back_reflection.py)

### Resident Vulkan follow-up

LAB builds four blur components from the same input. With 120 µm base thickness, 5 µm pitch and amount/scale 1, all twelve channel widths exceed its 3-pixel FIR threshold. Windows/Linux therefore runs a horizontal and then a vertical Young–van Vliet recursive filter per component, with separate R/G/B coefficients; the vertical pass also accumulates the signed channel weights. The double-float arithmetic improves evaluation of the recurrence. The recurrence itself approximates a Gaussian. At other pitches a component can use FIR and IIR on different channels. [LAB component selection](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/pipeline/pipeline.cpp#L1092) · [Blur dispatch and accumulation](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/pipeline/blur.cpp) · [Vulkan shader](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/shaders/vulkan/spk_iir_df_acc.comp)

The [isolated Vulkan probe](../scripts/parity/back_reflection_vulkan.rs) reused RS's existing grain IIR shader, with four pairs of directional passes and a separate signed accumulation after each pair. One submission per mixture, no intermediate readback. On an RTX 3060 Ti the 513² photo crop took 4.88–5.25 ms warm (three wall-clock samples, including submit/wait, excluding upload, final readback and print). The two scratch planes and sum total 9.04 MiB of logical f32 RGB storage at that size, excluding input and measurement staging. GPU allocation peaks and full-photo throughput were not measured.

The same 500T impulse put 82.90% of off-center red and 98.55% of green energy inside 1 mm. Applying identical corner subtraction, clipping and center exclusion to the sampled Gaussian mixture gives 93.42% and 99.90%. The CPU and GPU both reproduce the long red tail. On the actual glasses/fur crop, after the same strength and flat-field normalization, GPU versus CPU prototype absolute linear-exposure error was 0.00276 at p95 and 0.00571 maximum (peak exposure 15.34). This is an exposure-stage comparison; the final GPU print and a LAB executable were not compared. The renderer can execute these widths. The remaining question is which spatial response matches film scans closely enough to ship.

To repeat, append the probe source to the end of `crates/spektrafilm-gpu/src/grain_iir.rs` in a disposable checkout. For each 513² scene above, run the CPU recipe with halation disabled and `SPEKTRAFILM_DUMP_FILM_LOG_RAW` set. Convert the dump with `(10**np.fromfile(path, '<f8') - 1e-10).astype('<f4').tofile('input.f32')`. Set `BACKING_PROBE_DIR` to the directory containing that file and run `cargo test -p spektrafilm-gpu --lib backing_mixture_probe -- --ignored --nocapture`. The output `mixture.f32` contains the reflected component alone. Combine it with direct exposure as `(input + strength * mixture) / (1 + strength)` for comparison with the CPU study.

## Arctic neutral exposure before printing

**Historical 500T measurements at the old D55/D50 substitutions.** The [cinema illuminant correction](#cinema-illuminants-and-shipping-engine-comparisons) supersedes the proposed 500T calibration. Portra / Endura used the intended lights in both revisions.

The original Python change [4599f63](https://github.com/andreavolpato/spektrafilm/commit/4599f637e56c89a4ef7c40bf8ba54f4ce352ebfa) replaced a green exposure anchor and channel-chroma adjustment with division by each channel's reconstructed scene-neutral response. RS builds its reflectance table from profile sensitivities and the film illuminant, with D65 as scene white. Its silver-negative path already presents three equal channels.

A CPU CLI baseline used Arctic beta04, manual exposure, clear filter, halation/grain off, and five neutral sRGB values (32, 64, 118, 188, 235). In the pre-development log exposure dump, R−G and B−G in stops stayed constant across the five levels within 0.00001 EV. Portra 400 gave +0.04272 / −0.18093, tungsten-balanced Vision3 500T gave −0.92264 / +1.17619, and Double-X gave 0 / 0. These are film-layer exposure offsets. The downstream material curves and neutral enlarger pack determine the displayed gray. Next compare current and method-normalized exposures on the final optical print with both fixed and recalibrated packs; include daylight and tungsten stocks. Preserve the present calibration pending that comparison.

### Arctic neutral normalization on optical prints

**Historical 500T measurements at the old D55/D50 substitutions.** The [cinema illuminant correction](#cinema-illuminants-and-shipping-engine-comparisons) supersedes the proposed 500T calibration. Portra / Endura used the intended lights in both revisions.

Keep the existing Arctic calibration. With the shipped TH-KG3 pack, method-normalizing the reconstructed D65 white largely removes the blue cast of Vision3 500T / Kodak 2383 on an sRGB neutral; it introduces a smaller cast on Portra 400 / Portra Endura. For a neutral deliverable the existing enlarger can reach the same middle gray on both stocks. A method-wide default change would also move colors away from middle gray.

The [CLI study](../scripts/parity/arctic_print_neutral.py) uses the Batch 13 skin, cap and silk tiles, six gray levels (32, 64, 118, 188, 235, 255), manual camera exposure, clear camera filter and deterministic CPU optical prints with grain, DIR, glare and halation off. To reproduce the upstream per-channel division on this path, the study scales the three sensitivities in a temporary data copy by the reciprocals of the white LUT cell responses. It checks all six pre-development exposure triples against the expected gains. A finite-difference fit uses only code 118 to solve Y, M and print exposure. Other gray levels and every photo pixel are held out. All reported display differences use sRGB codes from 0 to 255.

| Film and output material | Printed middle gray, fixed pack (current → normalized RGB) | Y/M trims needed for a common neutral (current → normalized) | Residual after both reach that neutral |
| --- | --- | --- | --- |
| Portra 400 / Endura | 116,117,117 → 111,116,122 | −0.48/−0.54 → −3.66/−0.70 | Two cheek p95 differences 2.13 / 3.61 codes. Silk median 3.99 and p95 15.74. Largest held-out gray difference 5.39. |
| Vision3 500T / 2383 | 33,106,146 → 110,115,120 | −37.90/−14.42 → −4.69/−1.69 | Two cheek p95 differences 3.02 / 3.55 codes. Silk median 2.12 and p95 2.98. Largest held-out gray difference 2.67. |

A separate fit restored the **original colored middle gray** to within 0.011 code on both stocks. The required Y/M shifts on the normalized version were −3.18/−0.16 for Portra and +33.17/+13.97 for 500T. Its exposure dump was identical before and after print adjustment. For the common-neutral experiment both versions instead targeted the current patch luminance with equal linear RGB. Print exposure trims for 500T were +0.183 EV on both. These are fits to digital output; no physical scan or other application's rendered print served as a color-accuracy reference. The genuine advantage for this 500T pairing is a starting print closer to neutral. A stock/paper/method-specific neutral pack is worth testing before introducing a global exposure-model change.

Run from the repository root with `python scripts/parity/arctic_print_neutral.py CLI BATCH13_INPUT_PNG OUTPUT_DIR`. The output contains two labeled comparison sheets and the measured gains, trims and crop metrics in `metrics.json`.

## Cinema illuminants and shipping-engine comparisons

**Decision (RS `d471531`).** Use the lights specified in the stock profiles. Keep the existing Arctic normalization and neutral packs. Earlier 500T comparisons used D55 in place of tungsten at exposure and D50 in place of the projector lamp at scanning. The old Y/M compensation would now give the image a strong orange cast. Recheck historical 500T / 2383 display or styling comparisons, including the backing photo, before using their absolute colors as targets.

**Source and contract.** Python resolves `T` to Colour's measured Incandescent source and `K75P` to Kinoton 75P. Rebuilt both spectra from Colour 0.4.6 on the RS 81-wavelength grid and checked the aligned samples against the shipping LAB constants before mean normalization. The original JSONs, neutral rows and physical asset identities are unchanged. Normal spectral construction now reports an unknown profile illuminant. The explicit FPW v1 constructor retains its historical effective reference/viewing lights, including on a subsequent reprint. Packaging includes the Colour BSD notice. [Python illuminants](https://github.com/andreavolpato/spektrafilm/blob/3bb2c2d/src/spektrafilm/model/illuminants.py) · [LAB implementation](https://github.com/JamesQiu2005/SpektraLab/blob/9ceca33/engine/src/core/spectral.cpp) · [Colour spectra](https://github.com/colour-science/colour/blob/v0.4.6/colour/colorimetry/datasets/light_sources/sds.py) · [Regressions](../crates/spektrafilm-core/tests/stock_setup.rs)

**Independent Windows renders.** Used the Windows LAB 1.3.4-beta.3 portable release (tag commit `9ceca33`, ZIP SHA-256 `4945c80d552565382c2114f70ce82c137dd4502be9ddb832f79e20970fac7357`) and RS resident Vulkan on an RTX 3060 Ti. Both read the same untagged 16-bit linear ProPhoto TIFF with Kodak skin, caps, blue-purple silk, grays and saturated patches. Compared full-tier encoded sRGB on a 0–255 scale; LAB output was 16-bit. Halation, DIR, glare, grain and autoexposure were off. The two color-print recipes included a +5 Y/−5 M shift and a separate 0.1 preflash. LAB achieves those shift units by multiplying +1/−1 by 5. [LAB release](https://github.com/JamesQiu2005/SpektraLab/releases/tag/spektralab-windows-v1.3.4-beta.3) · [Driver](../scripts/parity/lab_windows_print.py)

| Hanatos 2025 optical pairing | Previous RS vs LAB p95 | Corrected RS vs LAB p95 |
| --- | ---: | ---: |
| Portra 400 / Portra Endura (daylight control) | 0.03 | 0.03 |
| Portra 400 / 2383 (projector light) | 6.35 | 0.03 |
| Vision3 500T / Portra Endura (tungsten exposure) | 59.11 | 0.03 |
| Vision3 500T / 2383 (both lights) | 67.81 | 0.03 |
| Vision3 200T / 2393 (second cine pair) | 64.63 | 0.03 |

Each pixel contributes its largest absolute RGB channel difference. Corrected p95 for the four additional trim/preflash cases was 0.01–0.03 codes. The independent cross-pairs identify the film reference light as the larger error in the tested scenes.

**Arctic and held-out photos.** Repeated the earlier method-normalization trial with the corrected engine. Stock 500T / 2383 printed code-118 gray at (115.22, 116.20, 116.53); applying the tested white normalization gave (110.91, 114.93, 119.97). At stock settings the old engine printed (33.21, 106.06, 145.97). Four further full-image thumbnails, Kodak 05/14/18/23, shared a gray strip with fixed manual exposure. The corrected CPU and resident prints differed by at most 0.009 display codes on those panels. Reapplying the old experimental Y/M and exposure trims produced (153.98, 103.87, 52.29). Those trims have no place in a new preset. The input is digital and the test establishes agreement between engines, not physical print accuracy.

**Spatial and grain checks.** At 5 µm pitch, a single bright pixel on a gray pedestal gave green-channel 90%-response radii of 20.6 µm in both programs at 10 µm lens blur. At 25 µm blur, RS gave 53.2 µm and LAB 57.0 µm with a maximum difference of 0.125 display codes. Radii include the absolute response of the sharpened print. These results cover camera lens blur; the LAB mainline diffusion pyramid is a separate implementation.

For Portra 400 at 6 µm pitch and +1.984375 EV, decoded LAB's exported DI back to CMY density using the shipped LUT axis endpoints and checked that the transport did not clip. Grain-off magenta density was 0.99981. Using the same 48 µm transmission aperture, four disjoint 512² regions yielded C/M/Y RMS ×1000 of 7.96–8.17 / 6.55–6.69 / 11.80–12.16. The earlier RS resident measurement was 8.25 / 6.55 / 11.82. Aperture channel correlations were within ±0.032 on both. Regions sample one LAB field; a multi-seed or measured-film qualification would be a different test. [LAB DI encoding](https://github.com/JamesQiu2005/SpektraLab/blob/9ceca33/engine/src/pipeline/engine.cpp#L2305) · [Aperture driver](../scripts/parity/lab_windows_grain.py)

**Shipping OFX.** Used portable Natron 2.5.0 with the official Windows plug-in (ZIP SHA-256 `5f4232d2f951569b7c9b1ca60fd44d70ba36cbd94cde26b73ef71cd0a8a1965c`). This is an actual headless product render. The input → writer control returned the original PNG RGB bytes exactly, at 480×144. Set Portra 400 / Portra Endura, Hanatos 2025, manual exposure, sRGB in/out, DIR amount zero, grain/halation/scanner off and zero glare. Photo crop p50/p95 differences against the RS optical print were 3.49/11.10 codes. At full retained silver the release's **Stock Character** preset printed middle gray at (119,129,129), compared with RS (97,92,92); the two effects' changes from their own baselines differed by 57.43 codes at p95. The release exposes four print silver models. Keep the RS control identified as the public-source approximation. The subsequent four-preset comparison below uses the same input and product build. [Natron render script](../scripts/parity/ofx_natron_print.py) · [Public OFX shader](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/shaders/SpektraFilm.metal) · [Official product downloads](https://spektrafilm.114c.de/)

**Reproduction.** Start in RS root. Place `host_smoke.py` from the pinned LAB tag on `PYTHONPATH` and use an offline study environment with NumPy, Pillow and tifffile. Run `lab_windows_print.py CLI LAB_PORTABLE BATCH13_INPUT_PNG OUTPUT_DIR` and `lab_windows_grain.py LAB_PORTABLE OUTPUT_DIR` from `scripts/parity` via Python. The print driver also runs the lens impulses. For OFX, follow the three environment variables and cases in the Natron script; use the same Batch 13 PNG. Neither reference program nor these Python libraries are runtime dependencies.

### All four shipping OFX print-silver presets and camera diffusion

Rendered all four presets at full print bleach on the identical Portra 400/Portra Endura montage. For each process subtract its own optical print, then take the largest absolute channel difference between the two effects per pixel. Aggregate the centre 72 × 72 pixels from each of the five photographic tiles. Units are 8-bit sRGB codes. RS uses its clipped float output before PNG quantization.

| OFX preset | OFX middle gray RGB | P95 disagreement with RS |
| --- | --- | ---: |
| Vintage Silver | 63, 63, 62 | 35.63 |
| Subtle Neutral | 118, 128, 128 | 45.73 |
| Stock Character | 119, 129, 129 | 56.56 |
| Modern Neutral | 119, 127, 127 | 44.81 |

Vintage Silver came closest by this metric and still gave a substantially darker gray than RS (97, 92, 92). No release preset qualifies as a replacement for the public-source approximation.

With camera Black Pro-Mist strength 0.5 and the other effects disabled, the photographic P95 change from each engine's own baseline was 9.02 codes for RS and 7.00 for OFX. Their changes differed by 3.32 at P95 and 15.54 at maximum. OFX used its 35mm preset and RS its 35 mm long-side setting. This compares end results on a small raster; it does not isolate internal downsampling thresholds or measure full-size performance. Keep the qualified RS diffusion. Reproduce with `OFX_CASE=diffusion` or `OFX_CASE=on` and `OFX_PRINT_MODEL` set to a listed preset. [Natron script](../scripts/parity/ofx_natron_print.py)

### Camera UV and IR cut

RS applies the error-function edges in wavelength space during calibration. The original Python filter, LAB's per-channel stock-white normalization and the public OFX Metal implementation use the same edge shape. RS keeps its existing missing-sensitivity convention and its own Hanatos stock window. It rejects invalid active widths and a filter which leaves no usable channel exposure. The calibrated table or matrix serves CPU and resident Vulkan. There is no per-frame filter pass. [Python](https://github.com/andreavolpato/spektrafilm/blob/3bb2c2d/src/spektrafilm/model/color_filters.py) · [LAB](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/hanatos.cpp) · [OFX](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/src/SpektraMetalRenderer.mm)

The shipping Windows OFX in Natron and RS Vulkan rendered the existing 480 × 144 photographic montage on Portra 400 / Portra Endura with Hanatos 2025. Grain, auto exposure and spatial effects were disabled. Each column is the P95 across the five central 72 × 72 photographic crops, using the largest absolute RGB channel change per pixel in 8-bit sRGB codes. The last column compares each engine's change from its own unfiltered print. OFX uses PNG output and RS uses its clipped floating-point output.

| Full-strength cut | RS change | OFX change | Disagreement |
| --- | ---: | ---: | ---: |
| UV 430 nm, width 8 nm | 11.67 | 13.00 | 6.80 |
| IR 650 nm, width 15 nm | 4.54 | 10.00 | 7.35 |
| Both | 12.26 | 12.00 | 4.25 |

The combined setting moved the central gray patch by (+1.74, +0.53, -1.72) in RS and (0, +4, -4) in OFX. The IR-only effect differed most. These comparisons cover one film/paper pair, with no claim of equivalent looks across products. Reproduce the OFX renders with the existing Natron script using OFX_CASE off, uv, ir and uv-ir. In RS use the camera recipe in USAGE and disable one or both edges with zero strength.

### Hanatos stock colour correction on still photographs

The shipped data has 20 filming profiles with three finite rows of 15 coefficients, comprising 18 distinct fits. Portra 800 and its two push variants share coefficients. All six silver camera profiles and all ten printing materials lack this fit. The following table lists every eligible profile. Four reversal stocks use direct film scanning; all negatives use the same Kodak Portra Endura paper and their database filtration.

Python main and LAB evaluate the unwarped fourth-degree polynomial in the two coordinates of the Hanatos table, relative to the film reference white. They omit the fitted constant, bound the change smoothly to ±2 stops, then multiply the integrated film exposure by the resulting gain before gamut remapping. Python also contains a warped evaluator; the regular evaluation path and the fixed 15-value rows here use plain poly4. RS takes the same route, once per spectral preparation, with the default stock sensitivity window in place. Three asymmetric reference cells from the original Python evaluator with Portra 400 coefficients agree within 1e-12 in gain. This comparison checks the polynomial independently of either product's final print. [Python main](https://github.com/andreavolpato/spektrafilm/blob/3bb2c2d/src/spektrafilm/utils/spectral_upsampling.py) · [LAB main](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/hanatos.cpp)

**Reading the table.** Each value is the reduction in absolute hue-angle error in degrees relative to the digital source, using the mean OKLab colour of the central 72×72 pixels in each photo crop or the centre of a solid patch. Positive values moved toward the source, negative values moved away. Skin A and B are cheek crops from Kodak images 04 and 15. The silk contains blue and violet threads. The three flat patches are sRGB (255,0,0), (0,0,255) and (220,0,255). This measures a photographic preference for retaining the input hue. A measured scan of each real stock would be needed to judge emulsion fidelity. The normal-exposure gray patches at sRGB levels 118 and 220 moved by less than 0.01 code for every stock.

| Film | Skin A | Skin B | Silk | Red | Blue | Violet |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| fujifilm_c200 | -1.4 | -0.2 | +3.6 | 0.0 | +0.7 | -0.3 |
| fujifilm_pro_400h | +0.2 | +0.6 | +2.6 | +0.3 | +2.0 | -1.6 |
| fujifilm_provia_100f | -5.6 | -4.7 | -2.5 | -1.2 | -2.2 | -2.0 |
| fujifilm_velvia_100 | -5.2 | -4.1 | +1.2 | -1.2 | 0.0 | -1.2 |
| fujifilm_xtra_400 | -1.2 | +0.3 | +2.6 | +0.3 | +1.6 | -0.8 |
| kodak_ektachrome_100 | -2.8 | -1.7 | +7.3 | -0.1 | -0.9 | -2.8 |
| kodak_ektar_100 | +0.4 | -0.1 | +8.0 | -0.2 | +2.6 | -2.5 |
| kodak_gold_200 | +2.0 | +1.4 | +3.7 | +0.2 | -4.5 | +0.2 |
| kodak_kodachrome_64 | -3.2 | -2.2 | -3.6 | -0.1 | +0.1 | -1.5 |
| kodak_portra_160 | -1.2 | -1.3 | +2.1 | -0.2 | +1.2 | -2.9 |
| kodak_portra_400 | -2.0 | -1.5 | +2.8 | -0.2 | -0.9 | -0.8 |
| kodak_portra_800 | +1.7 | +1.2 | +7.8 | -0.4 | +1.1 | -2.1 |
| kodak_portra_800_push1 | +1.5 | +1.2 | +7.2 | -0.3 | +2.3 | -2.4 |
| kodak_portra_800_push2 | +1.3 | +1.2 | +6.7 | -0.2 | +2.9 | -2.4 |
| kodak_ultramax_400 | +0.8 | +2.2 | +6.7 | -0.3 | +0.2 | -1.4 |
| kodak_verita_200d | +0.3 | -0.1 | +5.3 | -0.4 | +0.4 | +0.9 |
| kodak_vision3_200t | +2.2 | +1.9 | +5.9 | +0.7 | -2.2 | +3.4 |
| kodak_vision3_250d | -2.1 | +1.4 | +8.3 | -0.4 | +0.9 | -5.6 |
| kodak_vision3_500t | +0.9 | -0.7 | +7.5 | +0.3 | -1.6 | +1.7 |
| kodak_vision3_50d | -1.9 | +1.0 | +7.2 | 0.0 | +0.6 | -4.9 |

**Where to use it.** Portra 800 and UltraMax 400 consistently brought the woven fabric closer to its input hue at -1, 0 and +1 EV. At +1 EV one cheek moved farther from the input hue on both films. Ektar 100 corrected the fabric by 7.4–8.0° across the three exposures while both cheeks moved very little; a solid violet patch moved farther from the source. On Vision3 200T both fabric and the violet patch improved at all three exposures, while the blue patch moved farther away. Portra 400 favoured fabric slightly but moved both cheeks farther away at all three exposures. Provia 100F moved both cheeks and the fabric farther away in every exposure trial. Velvia 100 and Kodachrome 64 also favoured the original skin hue at nominal exposure. Keep the feature optional across all stocks.

**Reproduction and scope.** Run scripts/parity/hanatos_surface.py with the CLI path, Batch 13 montage PNG and scratch directory. For the exposure trials add --ev -1 or --ev 1 and --films followed by the six profile names above (Portra 800, UltraMax, Ektar, 200T, Portra 400, Provia). Each on/off pair uses the same exposure, film, paper or direct scan, default Hanatos window and sRGB transfer. Grain, DIR, halation, glare and scanner sharpening are disabled. The study produced 40 nominal and 24 bracketed Vulkan renders on Windows. The driver verifies gray polarity and sends an explicit --scan-film for reversal stocks. An earlier exploratory pass supplied an explicit paper to the slide cases; those results were discarded. [Study driver](../scripts/parity/hanatos_surface.py)

The regression suite checks CPU versus resident Vulkan on a Portra 400 chart (linear per-channel tolerance 0.0001), one negative miss on activation and two hits for repeated and print-only renders. Spectral CUBE grid nodes were checked against the photo route on CPU and Vulkan. No LAB executable comparison or physical film matching was performed for this option.

## Hanatos wavelength smoothing on still photographs

Keep the production value at zero. Smoothing offered little repeatable benefit for either cheek or the silk crop. At the strongest tested setting the red and blue patch hue moved farther from the source on every stock. A few blue-cap improvements did not carry across subjects.

[Python main](https://github.com/andreavolpato/spektrafilm/blob/3bb2c2d/src/spektrafilm/utils/spectral_upsampling.py) filters the 81 wavelength samples independently at each Hanatos chromaticity cell before contraction with the film sensitivities. The spatial axes receive zero sigma. [LAB main](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/hanatos.cpp) and [Windows/Linux v2](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/core/hanatos.cpp) implement the same reflected kernel for the tested radii at setup, with sigma in the setup cache key. Python's UI describes nm, but the calculation passes samples directly. On this 5 nm grid settings 1, 2 and 4 correspond to 5, 10 and 20 nm standard deviation. Both upstream defaults are zero. A hypothetical change to this parameter changes the exposure that creates the negative.

The [study driver](../scripts/parity/hanatos_blur.py) takes the CLI executable, Batch 13 input PNG and a scratch directory. It smooths private copies of the RS table using normalized Gaussian weights, symmetric boundary padding and a four-sigma radius. The shipping CLI renders the result. Run from the RS root. The 480 × 144 input SHA-256 is e76a2b41062e767f585d5b1b32f3dfbe13fe5a0598c5d9d1709b73c764be1de5. Five stocks × four strengths produced 20 Vulkan photos with fixed exposure, window enabled, surface disabled and grain, halation, DIR, glare and sharpening disabled. Negatives used Portra Endura and Provia used direct scan. Each zero-strength PNG matched the previous surface-off study pixel for pixel.

Mean ROI hue distance to input (degrees, off → sigma 4). This measures preservation of source hue. Physical film scans and interactive preference ratings were unavailable.

| Film | Cheek 04 | Blue cap | Silk | Saturated red | Saturated blue |
| --- | ---: | ---: | ---: | ---: | ---: |
| Portra 400 | 5.25 → 5.35 | 6.43 → 7.99 | 17.66 → 17.79 | 3.77 → 5.83 | 19.42 → 21.50 |
| Ektar 100 | 3.24 → 3.27 | 0.65 → 0.72 | 17.99 → 18.07 | 3.76 → 6.02 | 21.81 → 23.74 |
| UltraMax 400 | 1.89 → 1.95 | 2.38 → 0.91 | 11.56 → 11.67 | 4.90 → 7.55 | 12.71 → 14.91 |
| Vision3 200T | 4.75 → 6.18 | 7.06 → 9.71 | 9.93 → 11.22 | 5.50 → 10.42 | 2.82 → 3.45 |
| Provia 100F | 7.11 → 7.08 | 7.16 → 6.13 | 2.43 → 2.51 | 10.63 → 17.20 | 3.48 → 5.24 |

At sigma 1, both cheeks on all five films moved by at most 0.14° in source hue error and silk by at most 0.08°. The local report also measures screen-code changes and the purple patch at every strength. The experiment used RS calibration and rendering. No upstream executable comparison or resident cache transition was run for this unshipped control.

### Follow-up on window white normalization

The same driver also runs ten CPU exposures through the unmodified RS resources, toggling only the existing adaptation window on a neutral sRGB code 118 pixel. It reads pre-development log exposure and converts the difference into linear channel percentages. For Portra 400 the change was +1.25%, −1.06%, −8.24% in R/G/B, for Ektar +2.71%, −1.39%, −4.86%, and for Provia +2.14%, −0.28%, −1.09%. Thus toggling the window on neutral input really changes the channel balance in RS. These are film-layer measurements before printing. The newer [Python experimental normalization](https://github.com/andreavolpato/spektrafilm/commit/e301791) samples the neutral spectrum the Hanatos table actually produces. The follow-up below compares both white references on the same gray and prints, including manual UV/IR filters and a saved FPW RAW render.

### Reconstructed-white normalization on negatives and prints

We tested the Hanatos part of [Python experimental e301791](https://github.com/andreavolpato/spektrafilm/commit/e301791). It normalizes the film window against the spectrum sampled at the film white point in the Hanatos table. [LAB main](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/hanatos.cpp) normalizes against the reference lamp, as does the shipping RS calibration. The difference is a constant gain per film layer. In the probe, manual UV/IR retains its own lamp-referenced transmission compensation. Only the subsequent window normalization changes.

**Negative exposure.** CPU renders used uniform sRGB gray codes 64, 118 and 188. Within each film and UV/IR setting, the reference is the same exposure with the window disabled. The probe left at most 0.0062% residual across 90 channel comparisons (five stocks, three gray levels, three layers, clear or filtered camera). Portra 400 at code 118 moved from +1.25%, −1.06%, −8.24% to −0.0035%, −0.0005%, −0.0022%. With UV [1,430,10] and IR [0.5,650,10] the same blue-layer error fell from −1.34% to −0.00026%. This compares window on/off under identical filters and preserves the intended filter response.

**Rendered photographs.** The Vulkan comparison used the previous 480 × 144 montage, fixed manual exposure, packaged neutral filtration, and disabled grain, DIR, halation, glare and sharpening. Four negatives were printed on Portra Endura. Provia was scanned directly. The following values use a clear camera. Angles are mean ROI hue distances to the input sRGB photograph, in degrees. The last column is the 95th percentile of the largest per-pixel sRGB channel change on a 0–255 scale.

| Stock | Cheek 04, current → probe | Blue cap | Violet silk | Screen change |
| --- | ---: | ---: | ---: | ---: |
| Portra 400 | 5.25 → 11.29 | 6.43 → 17.87 | 17.66 → 17.87 | 3.42 |
| Ektar 100 | 3.24 → 4.07 | 0.65 → 8.20 | 17.99 → 18.92 | 3.32 |
| UltraMax 400 | 1.89 → 4.23 | 2.38 → 6.88 | 11.56 → 12.51 | 2.82 |
| Vision3 200T | 4.75 → 1.54 | 7.06 → 13.67 | 9.93 → 10.39 | 2.14 |
| Provia 100F | 7.11 → 8.10 | 7.16 → 4.41 | 2.43 → 3.36 | 1.54 |

The Provia scan's middle gray became more neutral (118.79/117.64/117.01 → 117.25/117.51/117.46). Every tested negative/paper print increased its gray channel spread at the existing filtration. Manual cuts reduced the Portra print's 95th-percentile change to 0.98 code. Source-hue distance measures preservation of input color. These runs have no matched film scans or viewing-panel scores.

**Reproduction.** In an isolated checkout of e3680b8 apply [the calibration-only probe patch](../scripts/parity/hanatos_window_white.patch) and build the standalone CLI. Preserve a second CLI built from the unmodified tree. From the RS root run the [study driver](../scripts/parity/hanatos_window_white.py) with the current CLI, probe CLI, Batch 13 input PNG and scratch output directory. The diagnostic patch converts the setup table to f64 to reuse the existing bicubic sampler. It is intended for an offline comparison. All camera sensitivities, paper data and per-pixel rendering code are shared.

**Refitting one paper pair.** On Portra 400 / Portra Endura with a clear camera, a separate uniform gray tile was fitted to the existing print with Y −2.1064, M −0.4357 and print exposure +0.00022 EV. With these trims the held-out montage gray differed by 0.042 screen code. The montage 95th-percentile difference fell from 3.42 to 0.50 code, with 0.41 and 0.60 on the two cheeks and 0.70 on silk. This fit explains most of the visible difference for this pair. It is a research result for the specified calibration.

**FPW saved-output decision.** Rebuilt the isolated Batch 10 FPW replay host twice, pointing its three RS crates at e3680b8 and the probe respectively. Both passed saved_film_photo_replay on DSC_5930 (Portra 400 / Portra Endura) using separate render caches, the same prepared RAW and saved recipe hashes, and the historical host resources. The current control matched every historical reference hash. The candidate changed the fit, full and region RGB16 outputs and the published JPEG. On the 4032 × 5600 native image the median / p95 largest-channel change was 3.46 / 13.68 on a 0–255 scale. On the decoded JPEG it was 5 / 16 codes and 98.38% of pixels changed. The JPEG SHA-256 changed from d6e1a0ac9e5c3b4492df59a16e1173f753e7fd9139dbe7e772a630cf0baf520e to 14c9a7e3c15682cf282d64156886a28a0e506ee9e2794425ceeeec0cc75a493f. This was headless acceptance with the saved edits and their normal effects. The isolated still-photo comparison above controls those effects separately.

Keep FPW v1 and the current standalone calibration. The negative-layer correction is effective and the existing paper packs depend on the earlier gains. Revisit as a versioned film/paper calibration when developing new neutral packs. One matched pair and one saved RAW do not qualify a migration.

## Still-photo exposure metering

[LAB main's engine](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/pipeline/pipeline.cpp) and the [Windows/Linux branch](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/pipeline/pipeline.cpp) implement four photographic intents in exposure_evs_from. The [public OFX bridge](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/src/SpektraAppBridge.mm) exposes median and center-weighted choices. RS already had both plus five legacy patterns. LAB's trimmed log average and bounded protection add distinct behavior for backlit subjects, especially when a small bright patch would dominate a brightness-weighted meter.

RS uses exact integer percentile ranks on its existing linear luminance grid, includes tied boundary values, and floors samples at 12 stops below 0.184. Center applies the existing Gaussian geometry to the trimmed log mean. Protect Highlights limits the 99.5th percentile to +2.5 stops above gray when possible; Protect Shadows aims for −3.5 stops at the 5th percentile. The changes are bounded relative to balanced. CPU and resident Vulkan both consume the same EV before film exposure. All four use the existing maximum 256-pixel-long-edge sampler, with no full-frame GPU readback. LAB session metering and RS can sample different pixels on larger frames.

The [comparison driver](../scripts/parity/lab_windows_meter.py) tested the shipping LAB Windows 1.3.4 beta 3 exposure solver on the same small PNGs as the RS CPU and Vulkan photo commands. The color montage contains the two real cheeks, blue/red caps, silk and saturated patches from Batch 13. The synthetic backlit chart contains 12.5% bright, 12.5% dark and 75% background. Each file fits the metering grid.

| Scene | Engine | Balanced EV | Center EV | Protect highlights EV | Protect shadows EV |
| --- | --- | ---: | ---: | ---: | ---: |
| Photographic montage | RS | +0.604 | +0.660 | +0.540 | +0.604 |
| Photographic montage | LAB Windows | +0.614 | +0.674 | +0.540 | +0.614 |
| Backlit chart | Both (rounded) | +2.043 | +1.893 | +0.123 | +4.043 |

The largest cross-product difference was 0.014 EV on the montage and below 0.000001 EV on the chart. The colored PNG passed through each product's own input path; the remaining difference was not isolated. For each of eight scene/meter pairs CPU and Vulkan reported identical EV. Each backend's automatic Portra 400 / Endura print matched its own manual-EV render exactly in the floating output dump. The driver turns off print exposure compensation for that comparison because otherwise dialing a manual camera trim also changes print calibration. The resident negative-cache test changes the meter under a common source identity, checks a miss when the exposure changes, then a hit on repeat; both outputs match a cache-off oracle. The signed ten-stop ramp and the +2 / −3 EV clamps are covered by the metering unit tests. GUI compilation was checked. Physical window interaction and host integration were outside this batch.

## Film latitude on backlit fur and a neutral wedge

LAB sends a 1024-step neutral ramp from −12 to +12 EV through the actual film/paper pipeline, then reads crossings in linear output luminance. The shadow threshold is black × 10^0.04 and the highlight threshold is black + 0.90 × (white − black). They describe the combined recipe. LAB separately samples the scene using its power norm (sum of positive RGB cubed divided by sum of squares). Its optional fit and tone mapping are separate operations. [LAB medium probe](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/pipeline/engine.cpp) · [Boundary calculation](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/latitude_fit.cpp)

RS measures its prepared spectral pipeline, including manual EV, resolved printing filters and paper compensation. Like the pointwise CUBE, it removes grain, neighbor effects and frame-dependent boost. A single ramp row suffices after those effects are disabled. The host's accepted stock setup survives the probe. RS additionally reports luminance counts from the actual finished photo when a CPU-accessible render is supplied. No FPW or recipe migration is required.

**Cross-product comparison.** The [parity driver](../scripts/parity/lab_windows_latitude.py) uses the shipping LAB Windows 1.3.4 beta 3 host, its scene_latitude endpoint and its rendered RGBA16, and the RS Vulkan CLI with pre-quantization pixels. Run from the RS root with the CLI, LAB portable directory, Batch 6 real side/backlit costume photograph and a scratch output directory. Add --compensate to match LAB's print re-timing. Both engines receive identical float32 linear ProPhoto inputs: a 171 × 256 version of the same picture and an independent 205-level wedge spanning −9 to +8 stops. We held film/paper, spatial effects, random effects, scan correction and exposure fixed within each pair. Each stock was rendered at −2, 0 and +2 camera EV.

| Stock and route | Shadow / highlight EV at 0 | Shadow / highlight EV at +2, FPW-style fixed print exposure |
| --- | ---: | ---: |
| Portra 400 / Portra Endura | −4.61 / +2.40 | −6.61 / +0.40 |
| Ektar 100 / Portra Endura | −4.35 / +2.44 | −6.35 / +0.44 |
| Provia 100F scan | −5.80 / +2.08 | −7.79 / +0.08 |

With matched print re-timing the largest difference between RS and LAB medium boundaries was 0.00043 EV. Independent gray wedge crossings were within 0.002 EV of each engine's own reported boundaries. At +2 camera EV LAB re-times Portra's paper and reports −5.49 / +2.42. The fixed FPW-style enlarger shifts the whole interval. Reporting that recipe difference as a film error would be misleading.

**Where the hint works.** On the photograph at zero EV, RS placed 0.21% of samples above Portra's highlight boundary and measured 0.25% in the finished print. On Provia the figures were 4.40% and 4.04%. At +2 EV with a fixed enlarger the saturated reds in the same shot pushed the Portra input estimate to 51.48%, while actual output luminance reached the shoulder in 24.78%. Hence the CLI's human-readable number comes from the finished pixels. The input count is an exposure placement cue. On the matched zero-EV photo LAB's own output crossed its highlight threshold in 1.03% (Portra) and 4.97% (Provia); the material boundaries agree more closely than the colored pictures. No physical darkroom print or per-channel clipping qualification was performed.

**Host and cache checks.** The integration test emulates a +1 EV host preprocessing gain with RS metering off, then tests the same frame with the gain supplied only through the FPW v1 manual film EV. Placement counts agree. Enabling RS balanced metering on the pre-lifted neutral wedge independently reports about −1 EV. A real Vulkan probe between two cached renders preserved the work-plane allocation and negative store count, produced a hit on repeat, and left pixels identical. CPU/Vulkan medium boundaries on that host recipe agreed within 0.02 EV. [Regression](../crates/spektrafilm-core/tests/latitude.rs)

On the same 171 × 256 photograph, a separate ordinary Vulkan render used RS balanced metering, active local film processing and fixed grain/glare. The CLI with and without --latitude produced byte-identical floating output across all 131,328 channel samples. The calibrated CUBE regression also passed on CPU and required Vulkan after sharing the pointwise probe preparation.

## Negative and print bleach bypass on the same photograph

Negative bleach bypass uses the developed color negative including grain. It estimates retained silver separately in each dye layer with D/(D+0.85), weights the three-layer mean by the requested amount, mixes dyes toward the largest channel density and adds 0.22 times that weight in neutral optical density. The cyan dye loss follows the stock's cyan density maximum with the public default coupling of 1. The enlarger sees the adjusted negative for both the photo and midgray calibration. Preflash reaches the paper separately. Direct negative scans see the same material. The existing paper-side model modifies developed color paper during the scan. Both default to zero. These are creative approximations. [Public OFX model](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/shaders/SpektraFilm.metal#L1723) · [Midgray normalization](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/shaders/SpektraFilm.metal#L2084)

The same 240 × 360 Batch 6 red-and-white costume photo plus a 24-pixel gray strip was processed with Portra 400 / Portra Endura, Hanatos 2025, manual exposure, sRGB output, no grain, DIR, halation, glare or sharpening. Input gray codes were 16, 48, 80, 118, 150, 184, 220 and 255. Each comparison changed only the indicated silver amount from zero to one within each engine. The table gives output green codes on three patches and the median per-pixel maximum minus minimum RGB code over the photograph (a simple channel-spread measure).

| Renderer and setting | Gray 48 | Gray 118 | Gray 220 | Photo RGB spread |
| --- | ---: | ---: | ---: | ---: |
| RS optical | 26 | 116 | 204 | 63 |
| RS negative silver | 24 | 126 | 216 | 26 |
| RS print silver | 10 | 92 | 193 | 27 |
| OFX optical | 27 | 117 | 205 | 66 |
| OFX negative silver, Modern Neutral | 21 | 110 | 227 | 37 |
| OFX print silver, Stock Character | 34 | 129 | 209 | 2 |

The RS negative effect subdued saturated red fur and cooled the white fur while retaining bright midtones. The print effect gave a much darker surround and a subdued warm subject. Both justify distinct optional controls. The published Windows OFX rendered in Natron 2.5.0, with RGB codes identical through its reader/writer transport check. Its default negative preset kept more red and gave a stronger highlight lift; its default print preset approached monochrome. Those shipping presets have different looks from the public-source model. The comparison covers one stock/paper pair and one real photograph. The OFX binary SHA-256 was 73740888bd96d751208e9c6d6b5502a8f198b6566590c65a867c8a2b73c8bd70. [Natron cases](../scripts/parity/ofx_natron_print.py) are transport, off, negative and on.

CPU and resident Vulkan rendered the three cases and half-strength negative silver. The largest absolute channel difference across all four floating-point sRGB outputs was 0.000035. With full negative silver, the optional CPU lookup tables differed from direct evaluation at P99/max by 1.43/1.98 display codes on a print with preflash 0.1 and 0.017/0.022 on a direct film scan. A previously saved 171 × 256 default Vulkan photo matched all 131,328 channel values byte-for-byte. The [cache regression](../crates/spektrafilm-core/tests/negative_hue.rs) checks each transition: initial optical render, enable film silver, change paper silver, switch to direct film scan, disable film silver, return to optical print. Three negative builds and three hits; the final print exactly restored the initial output. Internally the cache stores developed CMY and the spectral print/scan kernels interpret it as silver-bearing material. The negative amount belongs to the stock preparation and negative cache identity. Vulkan uses the existing passes and buffers. CPU and Vulkan were exercised, the CUDA feature was compile-checked, and interactive standalone GUI and FPW acceptance remain untested.

## Negative silver preset and cyan loss sweep

The previous 240 × 384 photograph and wedge were reused without resampling. Portra 400, Portra Endura, Hanatos 2025 and the fixed exposure/filtration recipe above stayed the same. Natron 2.5.0 rendered all six negative presets in Windows OFX 0.3.12 at 0, 0.5 and 1. The six zero-strength outputs matched the previous OFX optical control pixel-for-pixel. Modern Neutral at 1 matched the previous unmodified-default render. CPU RS printed the identical file at all three strengths. The table measures the median per-pixel red minus green code under the same forehead red-fur rectangle (x=107..132, y=109..118) and white cheek (x=143..155, y=154..166), in 8-bit sRGB. Negative cheek values indicate green exceeding red. The last column is the complete RGB triplet on neutral input 220.

| Engine / model | Red fur, 0 / 50 / 100% | White fur, 0 / 50 / 100% | Gray 220 at 100% |
| --- | --- | --- | --- |
| RS original | 119 / 86 / 42 | 11 / −4 / −21 | 194, 216, 209 |
| OFX Modern Neutral (default) | 113 / 119 / 115 | 12 / 11 / 6 | 228, 227, 228 |
| OFX Stock Character | 113 / 116 / 120 | 12 / 11 / 8 | 224, 223, 224 |
| OFX Subtle Neutral | 113 / 119 / 107 | 12 / 10 / 5 | 229, 228, 229 |
| OFX Subtle Silver | 113 / 112 / 110 | 12 / 8 / 4 | 228, 229, 227 |
| OFX Vintage Silver | 113 / 112 / 111 | 12 / 9 / 6 | 223, 224, 223 |
| OFX Dense Silver | 113 / 120 / 16 | 12 / 8 / 4 | 203, 201, 223 |

The release preset choice matters upstream of the paper. Direct raw negative scans for Modern Neutral and Dense Silver were identical at zero and differed on 92,003 of 92,160 pixels at full strength (maximum 28 codes). With the normal print route, changing only Leuco-Cyan Coupling from 1 to 0 yielded identical 8-bit output for both Modern Neutral and Vintage Silver at full strength. This measures these presets at this output precision. The bundled manual §6.7 describes a creative approximation, the lack of per-stock process measurements and the cyan-loss multiplier. The public shader supplies the cyan-loss equation, but no mapping to the six shipping presets. Their calibration and internal weights remain undetermined. [Public shader](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/shaders/SpektraFilm.metal#L1778) · [Official release and bundled manual](https://spektrafilm.114c.de/) · [Natron reproduction controls](../scripts/parity/ofx_natron_print.py)

For RS, a CPU research build at c15a2be isolated three terms. The neutral filter settings stayed fixed. The calibrated midgray exposure gain rose from 0.835750 at zero to 0.957982 at half and 1.097578 at full. Freezing this gain at the zero-strength value changed the full-strength red/white readings from 42/−21 to 36/−13. Disabling only the cyan dye loss, with normal calibration, gave 74/+6. Disabling only dye mixing toward the densest channel gave 93/−15. Disabling both left spectrally neutral silver and gave 119/+11. Thus cyan loss drives most of the white-fur cast on this print; both dye adjustments suppress red. Print calibration changes its strength. These single-image tests cannot determine the correct physical dye loss for this film.

RS exposes the public 0–2 leuco-cyan coupling multiplier with the previous default 1. There is no release-preset emulation. Half coupling and zero coupling at full bypass gave median white-fur RGB of (198,205,190) and (209,204,187), respectively, versus (184,205,193) at the original coupling. The setting reaches midgray preparation, spectral print and scan on CPU and Vulkan. Editing it while bypass is active rebuilds the negative; editing it with bypass off reuses the resident result.

Verification used real CPU and Vulkan outputs at 0/50/100% bypass, plus couplings 0, 0.5 and 2 at full bypass. The previous optical, half and full raw print buffers were byte-identical on each backend at the default coupling. Direct scan defaults likewise matched the previous executable on each backend. Maximum CPU/Vulkan print difference across the sweep was 0.009 display code. On direct scans this fixture already differed by up to 3.70 display codes with bypass off and 4.14 at the previous full strength. Across the new settings the largest difference was 4.28 codes (99th percentile below 0.64); the [direct-scan glare isolation](#direct-scan-glare-isolation) below identifies its source. The small color-patch cache regression, calibrated-output suite and GUI compile check passed. Standalone window interaction and FPW adoption are pending.

## Direct scan glare isolation

**Decision.** Retain both renderers. The discrepancy on the negative silver fixture is the placement of random viewing glare, added after spectral scanning in linear RGB. The earlier recipe disabled paper glare only. Film scans use `film_render.glare`, which defaulted to active, 0.03 percent, roughness 0.7 and blur 0.5 px. Both backends use seed 42. The [CPU](../crates/spektrafilm-model/src/glare.rs) draws sequential lognormal samples with StdRng. The [resident shader](../crates/spektrafilm-shaders/wgsl/glare_gen.wgsl) hashes the pixel index and uses Box–Muller so a pixel's value is independent of GPU execution order. A seed makes a given backend repeatable. The two generators place their bright speckles on different pixels.

**Controlled renders.** RS `5f75f9b`, same 240 × 360 red-and-white costume photo plus 24-row neutral wedge as above (input PNG SHA-256 `23b3654f9b8fad327c68aea7cdc77b157e4ba25f150722393330567c37790db4`). Use the preceding Portra 400 / Portra Endura recipe, manual EV 0, 2.4 mm film format, Hanatos 2025, sRGB encoding enabled and gamut compression disabled. Keep grain, halation, boost, DIR, paper glare and sharpening off. Run `spektrafilm process input.png --scan-film --film kodak_portra_400 --paper kodak_portra_endura --params recipe.json -o out.png --raw-out out.bin` separately with `SPEKTRAFILM_BACKEND=cpu` and `wgpu`. Confirm the logged backend. For each bypass strength, change only film glare as indicated. The table measures the largest absolute channel difference on the whole frame before PNG quantization, scaled to 0–255.

| Film glare setting | Bypass off | Full bypass, coupling 1 |
| --- | ---: | ---: |
| Original recipe | 3.7034 | 4.1386 |
| Same mean, roughness 0 | 0.0064 | 0.0064 |
| Film glare disabled | 0.0064 | 0.0064 |

The original floating-point outputs matched the saved CPU and Vulkan controls byte-for-byte on each backend. The earlier 4.3-code maximum came from full bypass **and coupling 2**. That case reproduced at 4.2736 and dropped to 0.0064 with film glare disabled. The full-bypass optical print control also matched the previous output on each backend.

**Light level and spatial distribution.** Repeat with `io.output_cctf_encoding=false` and subtract the no-glare output from the glare-on output on each backend. For both bypass settings, mean added linear green was 0.0002994 on CPU and 0.0003013 on Vulkan (target 0.0003000). Standard deviations were 0.0001343 and 0.0001354. Before the 0.5 px blur (set film glare blur to zero), they were 0.0002088 and 0.0002110 (target 0.0002100). Both the mean and the smoothing agree closely. The sRGB curve makes a given linear fluctuation more visible in dark scan channels.

**Where it shows.** At both bypass strengths the largest difference was blue at (211, 360), the first row of the **white input** patch, which scans dark on negative film. At full bypass the CPU/Vulkan blue codes there were 16.58/12.44. Within the photo alone the maximum was 2.40 codes with bypass off and 2.85 at full strength. At full strength only 12 of 86,400 photo pixels exceeded two codes in any channel before quantization. The white-cheek crop peaked at 1.61 and the red-forehead crop at 0.90. Side-by-side at native size there was no discernible broad color or edge shift; an amplified absolute difference image showed scattered speckles. In the two quantized full-strength PNGs, 99.88% of photo pixels were within one code in every channel. These measurements cover this input, stock and recipe. Use uniform glare or disable it when comparing deterministic scan color across backends.

## Yellow filters on black-and-white prints

**Decision.** Add Wratten No. 8 (K2 yellow) and No. 12 (deep yellow) to the existing measured monochrome camera. They provide two milder ways to lighten warm subjects against blue than the red filters. Continue to use the original stock response for recipes that omit the filter. No per-frame stage or GPU pass was added.

**Source and stage.** The 10 nm transmission fractions come from the Kodak B-3 table reproduced in Figure 10, printed page 85, of the [NASA publication](https://ntrs.nasa.gov/api/citations/19810011222/downloads/19810011222.pdf#page=95). The same RS interpolation and out-of-band convention as the red filters supplies 81 samples. No. 12 agrees with LAB's grid to floating-point rounding; No. 8 differs by at most 0.001 in transmission (the printed table gives 89.8% at 590 nm and 91.0% at 650 nm). LAB multiplies the camera sensitivity by the filter transmission before spectral exposure and compensates against the stock reference light. RS retains the user's choice between physical loss and compensation against its camera neutral. Public OFX Vulkan filters exposure sensitivities for UV/IR with per-channel normalization. The Windows OFX 0.3.12 Wratten selector includes 08N5 with neutral density. Its separate cameraColorFilter selector also offers plain Wratten 08 and 12. LAB is the measured reference for these recipes; OFX yellow-filter prints still need a product comparison. [LAB preparation](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/src/core/hanatos.cpp#L167) · [Windows/Linux path](https://github.com/JamesQiu2005/SpektraLab/blob/f87c521/engine/src/core/hanatos.cpp#L167) · [Public OFX Vulkan](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/src/SpektraVulkanRenderer.cpp#L493)

**Real photo and eight neutral patches.** Converted the same 240 × 384 red-and-white costume photo plus wedge used in the negative-silver study to one linear ProPhoto float TIFF (SHA-256 `39180a64e77366aa48160c6526cedffd0c40ba2b68f4f4e96c9ce6dd20a0d7d7`). Set 35 mm, manual EV 0, Hanatos 2025 in RS, sRGB output, grain, DIR, halation, boost and paper glare off. Compare measured clear with compensated glass. Printed Tri-X 400 on Multigrade IV RC grade 2 in both engines with each engine's own grade pack. For the LAB Windows 1.3.4-beta.3 host, opened the same TIFF with `camera_filter` set to the empty string, `w8`, `w12` or `w25`, zero cyan filtration and the M/Y/exposure fields from its bundled grade 2. Confirmed the resolved parameters and read the full rgba16 result. The stock, paper, grade table and host executable matched their copies in the release ZIP.

| Camera | RS red forehead | LAB red forehead | RS white cheek | LAB white cheek | RS / LAB middle gray |
| --- | ---: | ---: | ---: | ---: | --- |
| Clear | 122 | 123 | 206 | 203 | 118 / 118 |
| No. 8 | 159 | 159 | 217 | 210 | 118 / 118 |
| No. 12 | 169 | 170 | 219 | 211 | 118 / 118 |
| No. 25 | 222 | 213 | 224 | 214 | 118 / 118 |

Values are median green-channel sRGB codes in the original red forehead and white cheek crops; middle gray is the center of the 118-code input patch. On RS the eight patch centers stayed within 0.001 display code of clear under both yellows. On the full photo, the 95th percentile difference between the two engines' changes from their own clear print was 6.14 codes (No. 8) and 6.91 (No. 12). This comparison covers final prints; the paper calibrations and output processing also contribute. On Double-X / 2302, the same RS forehead went from 132 to 160 and 169.

**Validation.** CPU/Vulkan differences for both yellow filters on both stock/paper pairs stayed below 0.006 display code. Off, clear and No. 25 matched the preceding executable byte-for-byte separately on each backend and pair. The camera filter integration test exercises gray, red, blue, physical versus compensated exposure, three spectral front ends, and GPU cache misses when switching yellows followed by a hit on repeat. For Double-X with Hanatos, measured filter losses were 0.87 and 1.11 EV; Kodak's daylight guide gives 0.58 and 1.00 EV. These are model comparisons on reconstructed spectra. A physical filtered-film pair and standalone GUI interaction were not evaluated. [Kodak Double-X daylight factors](https://www.kodak.com/content/products-brochures/Film/DOUBLEX-Technical-Data-EN.pdf#page=2) · [Regression test](../crates/spektrafilm-core/tests/camera_filter.rs)

## LAB reference prints for the yellow filters

The previous yellow-filter study used different paper and scanner settings. It established the direction of the filter response. For final-image parity use the two CLI recipes in `recipes/spektralab`. The reference is SpektraLab Windows 1.3.4-beta.3 host `0.1.0+9ceca33`, native Vulkan, Tri-X 400, Multigrade IV RC grade 2, manual EV 0, Hanatos 2025, compensated lens glass, grain, DIR, halation and glare off. LAB and RS read the same linear ProPhoto float TIFF and return encoded sRGB. Use the bundled LAB grade 2 settings and its normal output lightness compression and sharpening.

**What accounted for the old difference.** The default RS grade 2 pack is 77.0791677894 Y with exposure 0.9491008571. LAB uses 68 Y and exposure 1. The RS recipes subtract the difference in Y and multiply by the reciprocal of the RS pack exposure. Both heads use zero cyan and magenta. These are exact pack conversions, independent of image content. The previous RS chart also disabled CAM16-UCS output compression and unsharp masking while LAB kept them active. Restoring the pack alone brought the dark neutral steps into agreement; restoring output processing brought the highlights and edge detail into agreement. On the unfiltered costume photograph the matched output had a 95th-percentile absolute channel error of 0.02 code. [LAB grade table](https://github.com/JamesQiu2005/SpektraLab/blob/9ceca338bac8aa0bcd8f17aeeb324df9174bb15c/engine/resources_product/paper_grades.json)

**Final-image results.** Each pixel contributes its largest absolute RGB channel difference in 0–255 display codes. Metrics below include the wedge where present. The limits used for these deterministic prints were P95 ≤ 2 and P99 ≤ 4. Both RS backends passed all three inputs; the table reports the worse backend for each filter. The two Kodak scenes were held out when constructing the recipe.

| Shared input | No. 8 P95 / P99 | No. 12 P95 / P99 |
| --- | --- | --- |
| Costume photograph + eight-step neutral wedge | 1.32 / 1.45 | 1.27 / 1.39 |
| Kodak 04 | 0.67 / 1.23 | 0.62 / 1.12 |
| Kodak 18 | 1.37 / 2.11 | 1.31 / 1.99 |

Rounded table entries are upper bounds. The maximum single-channel discrepancy in the sweep was 4.47 codes. The previous costume/wedge comparison at the same metric gave P95 11.38 and 11.53 codes. The white cheek with No. 8 now reads 210.30 in RS versus 210.12 in LAB; with No. 12 it reads 211.19 versus 211.01. Maximum discrepancy across the eight wedge centers was 1.34 and 1.29 codes. LAB normalizes filter transmission against the stock's reference illuminant, while RS's existing compensated option normalizes to its reconstructed neutral. The measured residual includes this difference and the separate renderers. Scope is limited to the stock, grade, exposure and filters above.

**Repeat.** Run the [comparison script](../scripts/parity/lab_windows_yellow_filters.py) from the repository root with `--cli <RS executable> --lab <unpacked LAB directory> --out <scratch directory> photo1.png photo2.png ...`. Supply sRGB PNGs, numpy, Pillow, tifffile and LAB's tagged `engine/tests/host_smoke.py` on PYTHONPATH. The script checks the reference host identity, resolved controls, input space and actual backends, writes final images and metrics, and fails when the percentile limits are exceeded.

### Grade 2 prints on T-Max 100, Acros II and HP5 Plus

The same Windows release host and settings above were used for a clear-lens baseline followed by No. 8 and No. 12. LAB loads the requested film and paper profiles in [init_params](https://github.com/JamesQiu2005/SpektraLab/blob/9ceca338bac8aa0bcd8f17aeeb324df9174bb15c/engine/src/core/params.cpp). The test confirmed the resolved stock IDs and grade controls for each session. All three films used the same paper and scanner settings as Tri-X.

Each entry gives the worst P95 / P99 over the costume photograph with the eight-step wedge, Kodak 04, Kodak 18, and both RS backends, rounded up to hundredths of an 8-bit display code. All 54 comparisons passed the previous per-image limits of 2 / 4.

| Film ID | Clear | No. 8 | No. 12 |
| --- | --- | --- | --- |
| `kodak_tmax_100` | 0.03 / 0.04 | 1.23 / 1.98 | 1.19 / 1.91 |
| `fujifilm_neopan_acros_100_ii` | 0.02 / 0.04 | 0.45 / 0.54 | 0.44 / 0.52 |
| `ilford_hp5_plus_400` | 0.02 / 0.04 | 0.30 / 0.53 | 0.27 / 0.47 |

All eight wedge centers remained in order in both engines (240 by 384 input, y=372, x=15,45,...,225). The largest center difference was 1.16 codes for T-Max, 0.34 for Acros and 0.23 for HP5, rounded up. The largest individual full-frame error was 6.83 codes rounded up, on HP5 with No. 8. These results cover deterministic prints at grade 2 and manual EV 0.

To repeat, append `--films kodak_tmax_100 fujifilm_neopan_acros_100_ii ilford_hp5_plus_400 --filters clear` after the input filenames in the command above. Then repeat with `--filters w8 w12` and a second output directory to retain both metrics files.

### Tri-X clear-lens completion

The same three inputs and stock-independent clear-lens recipe passed on Tri-X 400. The worst full-frame P95 / P99 across RS CPU and Vulkan were 0.023 / 0.033 display codes (rounded up). Together with the two previous yellow-filter runs, this completes all twelve stock/filter pairs on Multigrade IV RC grade 2. Reproduce with the grade 2 script above and `--films kodak_tri_x_400 --filters clear`.

### OFX UV and IR final-output check

Natron 2.5.0 rendered the pinned Windows OFX 0.3.12 package (ZIP SHA-256 above). The inputs were the Batch 13 skin/fabric/foliage montage with its swatches and the red-and-white costume photograph with the eight-step wedge. The Natron transport control reproduced the input RGB bytes for each. The RS CLI used Portra 400 / Portra Endura, Hanatos 2025, manual exposure, sRGB input/output, encoded output, 35 mm, grain/DIR/halation/glare/sharpening off. Use the same settings for all four lens conditions, with UV [1,430,8] and IR [1,650,15]; set the corresponding strength to zero for an inactive edge.

The release enables its input color adaptation by default and leaves output chroma/lightness compression off. RS uses its own input and output gamut compression by default. To isolate the two pipelines, also render with `OFX_DISABLE_COLOR_ADAPTATION=1` in the [Natron driver](../scripts/parity/ofx_natron_print.py) and set both RS `io.input_gamut_compress.algorithm` and `io.output_gamut_compress.algorithm` to `off`. Run each RS recipe with `SPEKTRAFILM_BACKEND=cpu` and `wgpu`; compare clipped CLI float RGB × 255 against the OFX PNG, taking the largest channel error at each pixel. The tables below include the gray strips. These full-frame percentiles have a different denominator from the earlier five photographic crops.

| Lens | Original settings, montage P95 | Adaptation bypassed, montage P95 / P99 | Adaptation bypassed, costume P95 / P99 | Costume display scan before paper, P95 / P99 |
| --- | ---: | ---: | ---: | ---: |
| Clear | 37.11 | 4.93 / 4.93 | 4.56 / 4.82 | 3.01 / 3.44 |
| UV | 34.00 | 8.34 / 8.88 | 8.24 / 9.61 | 1.94 / 2.23 |
| IR | 36.87 | 6.88 / 9.34 | 5.25 / 5.86 | 3.21 / 3.74 |
| UV + IR | 33.77 | 5.63 / 6.50 | 6.50 / 7.26 | 2.95 / 3.65 |

Values are rounded to nearest, taking the worse percentile of the two RS backends. None of the four print recipes passes 2 / 4. For the pre-paper column select `OFX_CUTS_SCAN=1` and RS `--scan-film` with adaptation bypassed. The clear scan already differs. On the costume the IR change relative to clear disagrees by 3.13 codes at P95 before paper and 5.26 in print (CPU). The scan includes the display transform, so identifying a film-density error specifically requires a common intermediate.

The release Hanatos 2025 spectra file matches the RS table exactly after widening the RS float16 samples to float32 (192 × 192 × 81). Neither RS Portra profile has missing log sensitivity samples. The public OFX Vulkan code uses the same 8/15 nm edge widths and per-channel reference-light normalization, followed by an archived stock-specific Hanatos 2025 window. A private CPU control applied the [Python 6e7de28](https://github.com/andreavolpato/spektrafilm/tree/6e7de28) Portra bandpass after the normalized cuts in linear sensitivity and disabled the RS parametric window. UV print P95 rose to 24.16 on the montage and 25.18 on the costume. This archived window alone does not reproduce the release. The release's exact film/print calibration remains open. [OFX camera preparation](https://github.com/chaert-s/spektrafilm-ofx/blob/86476af/src/SpektraVulkanRenderer.cpp#L481)

An additional release-only control toggled `OFX_DISABLE_FILTER_COMPENSATION=1` on the costume with color adaptation bypassed. The clear print was byte-identical. At P95 the UV, IR and combined prints changed by 6, 3 and 8 codes. At input gray 118, disabling compensation changed the IR print from (112,119,120) to (109,116,116). The unmodified RS CPU result on that patch was (115.4,116.0,119.2). None of the three uncompensated OFX prints passed the absolute limits against RS. This identifies a release control affecting UV/IR that the public bandpass formula alone does not describe. Capture its effect on the same developed-density output before changing RS calibration.

### OFX developed negative and optical print replay

Same Portra 400 / Endura recipe and OFX 0.3.12 package as above, with color adaptation bypassed. The costume/wedge and montage each had a 256-step neutral strip appended. Camera, density and final-print captures used the same padded canvas. Full-frame print metrics below measure the original top-left 240 × 384 and 480 × 144 regions. The new strip is used for neutral samples. Natron's raw reader/writer round trip preserved the RGBA float TIFF values exactly on both inputs.

**Before paper.** Captured Scan negative → Cineon / DPX Log Density with compensation on and off at all four lens settings. Both captures had identical float RGB values at every pixel in each of the eight pairs. The optical prints in the preceding check did respond to this switch. The lens itself changes the developed negative: on neutral input 118, clear-to-IR red-channel density changed by −0.0246 in OFX and +0.0013 in RS. Clear-to-UV blue changed by −0.0445 and −0.0110 respectively. These are density units. A scratch control undoing RS's reference-light normalization worsened the UV response difference on the costume (P95 0.046 to 0.109 density). Keep the calibrated default.

**After development.** For this zero-silver capture, convert the release's float Cineon RGB to relative density with `max((1023 * value - 95) / 500, 0)`. The release's density-output shader uses the 95 pedestal and 500 scale on the Cineon branch. Neither input reached the encoding ceiling. Feed the result into RS's calibrated CPU optical printing and scanning stages. Replaying RS's own developed CMY dump in the same tool reproduced its production CLI float output exactly for all eight runs. Reference images for the following table are native OFX optical prints. Entries are P95 / P99 absolute maximum-channel differences in 8-bit display codes after substituting the OFX negative.

| Lens | Costume + wedge | Skin/fabric/foliage montage |
| --- | ---: | ---: |
| Clear | 8.03 / 8.64 | 7.86 / 8.39 |
| UV | 7.26 / 8.07 | 8.15 / 9.14 |
| IR | 6.42 / 7.11 | 7.32 / 8.53 |
| UV + IR | 10.02 / 10.99 | 10.95 / 11.91 |

Both the negative response and subsequent print path contribute. Even the clear shared negative fails the final 2 / 4 limits. OFX's separate Process Negative import route also needs its own baseline: exporting and reimporting its clear Cineon negative through that route produced a P95 difference of 43 codes on the costume and 45 on the montage relative to the native optical print. The imported negative gave identical pixels with the camera IR switch on and off. Use the native optical print as the target. Prioritize the clear Endura pack and print exposure calibration before adjusting cut-dependent compensation. The shared-negative replay is CPU-only; the previous end-to-end Vulkan results remain the GPU coverage.

**Repeat from the repository root.** Use the preceding Natron controls plus `OFX_CUTS_NEGATIVE=cineon` for a 32-bit float TIFF, then unset that option for the native PNG print. Set `OFX_DISABLE_FILTER_COMPENSATION=1` for the uncompensated control. Run the matching RS CLI recipe with `SPEKTRAFILM_BACKEND=cpu`, `SPEKTRAFILM_DUMP_FILM_DENSITY=<negative.bin>` and `--raw-out <direct.bin>`. Build the diagnostic with `cargo build --locked -p spektrafilm-cli --example ofx_density_print`. Run `python scripts/parity/ofx_density_replay.py <example-executable> <recipe.json> <cineon.tif> <native-ofx.png> <negative.bin> <direct.bin> --region <width> <height>`. It checks the RS control before reporting the shared-negative result. To check OFX import separately, pass the float TIFF to Natron with `OFX_CUTS_NEGATIVE=reprint`.

### Minimum release checks across implemented features

RS 5bf9c64, SpektraLab Windows 1.3.4-beta.3 native Vulkan, OFX Windows 0.3.12 in Natron. Both release identities and the source photos are described in the preceding studies. A minimum check here means one representative photo per distinct operation, with a neutral control. The whole-frame deterministic print gate is P95 ≤ 2 and P99 ≤ 4 for the largest absolute channel error in 8-bit sRGB codes. The pass decision uses the unrounded values. Grain, meter EV, optical impulse response, geometry and cache identity use the observable specific to those operations. A single photo passing does not extend a qualification to other recipes or stocks.

**LAB full grades.** Tri-X 400, clear lens, original costume photo and eight-step wedge. Manual EV 0, 35 mm, linear ProPhoto input, grain/DIR/halation/glare disabled, standard LAB output and sharpening. The only difference between the two RS columns is the enlarger pack. “Matched” uses LAB's bundled per-grade Y, M and print exposure, converted by subtracting the RS grade filtration and dividing by its exposure. No data were fitted to this image. The two-backend maxima are below.

| Multigrade IV RC | RS native pack P95 / P99 | LAB pack P95 / P99 |
| --- | ---: | ---: |
| 00 | 1.41 / 1.58 | 0.02 / 0.02 |
| 0 | 3.51 / 3.95 | 0.02 / 0.02 |
| 1 | 3.55 / 3.83 | 0.02 / 0.03 |
| 2 | 5.38 / 6.37 | 0.023 / 0.033¹ |
| 3 | 1.72 / 2.40 | 0.02 / 0.04 |
| 4 | 8.76 / 9.83 | 0.03 / 0.05 |
| 5 | 0.59 / 0.75 | 0.04 / 0.06 |

¹ Retained upper bounds across the previously verified three inputs. On grade 2 with compensated No. 25 red, both RS backends passed the costume/wedge: worst P95 / P99 were 1.40 / 1.49 (Tri-X), 1.29 / 1.37 (T-Max), 0.69 / 0.76 (Acros), 0.35 / 0.47 (HP5). Use the [LAB comparison driver](../scripts/parity/lab_windows_yellow_filters.py) with one photo, `--grade`, `--pack native|matched`, and `--filters clear` or `w25`. The driver writes the converted recipe into the scratch directory and returns failure for a tolerance miss. The existing 12 yellow/clear combinations at grade 2 retain their three-input acceptance. ISO paper speed and darkroom half grades use different physical evidence.

**OFX release.** The skin, woven fabric, foliage and swatches montage from Batch 13, raw Natron transport, sRGB in/out, EV 0, 35 mm, grain/DIR/halation/scanner/glare off. Bypass OFX color adaptation and both RS gamut compressors as in the preceding UV/IR check. The B&W Still Film print selector offered Ilfobrom Galerie FB 1K; RS ships Multigrade IV RC. Compare HP5 as a direct scan so that the paper choice drops out. The selected cameraColorFilter labels include the plain Wratten 08, 12, 25 and 29. For color, use Portra 400 / Endura. Worst backend over CPU and Vulkan:

| Case | Absolute P95 / P99 | P95 disagreement in change from clear or optical control |
| --- | ---: | ---: |
| HP5 clear scan | 26.34 / 29.51 | baseline |
| HP5 No. 8 | 30.51 / 30.55 | 14.00 |
| HP5 No. 12 | 18.15 / 18.18 | 16.64 |
| HP5 No. 25 | 14.07 / 16.66 | 24.37 |
| HP5 No. 29 | 14.41 / 18.05 | 26.38 |
| RS beta04, OFX release menu Arctic 2026 | 33.00 / 34.00 | 33.00 |
| RS normal Hanatos stock window, OFX Hanatos 2026 | 6.53 / 6.90 | 6.00 |
| Print Black Pro-Mist, strength 0.5 | 7.35 / 16.76 | 6.71 |
| Parametric paper gamma factor 1.2 | 5.89 / 5.89 | 1.30 |
| Paper developer exhaustion 0.5 | 16.71 / 19.40 | 13.69 |

Use `OFX_DISABLE_COLOR_ADAPTATION=1` with the corresponding `OFX_CASE` in the [Natron driver](../scripts/parity/ofx_natron_print.py). For Hanatos 2026 the control is OFX Hanatos 2025 versus the same RS windowed print. The Arctic check compares the selectable release effects. The OFX menu supplies no beta revision. A small difference in the effect of gamma coexists with the unaligned optical baseline. The saved full-strength bypass outputs already decide the release-preset question: re-scoring the existing six negative presets gave final P95 43.17–62.84; the four print presets gave 50.28–89.21. The saved camera Black Pro-Mist render gave 35.49 with its original color settings. Existing UV/IR clear and cut outputs also exceed the gate. No rerender or preset search was needed for those decisions.

**LAB spatial cases.** On the same backlit costume/wedge and Portra / Endura with 4 mm film scale (the host minimum, to resolve the effects on the small image), clear, DIR on and halation on gave P95 / P99 0.03 / 0.06, 1.54 / 3.76 and 10.43 / 25.54 respectively on RS Vulkan. The earlier lens impulse and grain aperture studies remain the spatial-radius and noise-statistic checks. White-noise pixels are not paired across independent renderers.

For roll layout, start from the same landscape image cropped to each LAB gate aspect ratio; square gate, black holes, no fog/leak, seeds 1. Set RS overscan symmetrically to match the LAB output dimensions. Full-frame P95 / P99 were 78.16 / 213.04 on 135 (250×232), 157.78 / 211.55 on half frame (280×150) and 91.87 / 165.72 on 645 (244×180). These measure the entire placement and border result at equal output size. The implementations specify 17 versus 18 mm half gates and 61.5 versus 61 mm 120 strips. For a separate 135 activation check the same date text changed 317 LAB / 376 RS pixels by more than two codes. A LAB seeded leak and an RS authored side leak changed 658 / 7301 pixels. The two controls have no defined coordinate/intensity mapping between products. These are separate looks; XPan has no corresponding format in the tested LAB release. RS geometry, cache and ROI tests remain relevant to the RS contract.

**Evidence still open.** The Windows LAB host rejected the wire names for the Hanatos surface, negative hue and RGB-to-raw method. The tagged code evaluates the surface internally and fixes the renderer to Hanatos 2025. The mainline negative-hue renderer uses Metal. The existing comparison with LAB's compiled hue-calibration routines and the Python polynomial-cell checks cover those implementations; their enabled LAB product outputs remain unverified. Other full-image sizes, stocks and platforms were not exhaustively swept. Current results do not qualify a physical half-grade, an unreleased rem-jet control or an unlicensed Portra NC/VC pack.

**Existing checks retained.** Five LAB Hanatos optical film/paper pairs plus four trim/preflash cases, the meter and latitude probe, lens impulse and Portra grain aperture have measured reference results above. The CUBE bake is checked against the engine's pointwise photo render. Cache invalidation, buffer reuse, image I/O, CPU/Vulkan regression and FPW saved RAW/export checks use their own acceptance contracts. The current branch's standalone real-window interactions, LAB Metal product renders and FPW mainline adoption are separate outstanding acceptance or integration tasks.

### Endura calibration on the OFX negative

Use Windows OFX 0.3.12, Portra 400 / Portra Endura, Hanatos 2025, EV 0 and the adaptation-bypassed controls from the preceding density study. Capture the native optical print as raw-transport float32 sRGB, paired with its Cineon negative. Calibrate on 25 neutral inputs (sRGB codes 32 through 224 in steps of 8) on the appended strip of the costume fixture. Fit only Y shift, M shift and log2 print exposure to these RGB outputs. Keep RS's existing stock pack, calibrated print-exposure normalization, optical stages and scanner. The second image is the independent Batch 13 skin/fabric/foliage and swatch montage. Its image and strip contribute no samples to fitting. The results below use the original 240×384 and 480×144 regions excluding appended pixels, both sides clipped to display range. The shared-negative replay uses the production CPU paper and scan stages.

| Clear print from identical OFX density | Costume P95 / P99 | Held-out montage P95 / P99 |
| --- | ---: | ---: |
| Native RS pack | 7.96 / 8.56 | 7.79 / 8.21 |
| Exposure adjustment alone | 6.22 / 6.84 | 6.25 / 7.19 |
| Y and M adjustments alone | 1.78 / 1.82 | 1.88 / 1.88 |
| Y −3.96614, M −0.75753, print EV +0.01927 | 0.28 / 0.31 | 0.35 / 0.35 |

These shifts are relative to RS's packaged C/M/Y 0 / 51.56801 / 52.53400. The fitted EV multiplies RS print exposure by 1.013444. The measured settings are an effective match for this release and process. The full 256-step strip, including the levels excluded from fitting, has peak error 0.12 display code. A separate float Rec.709 gamma 2.2 capture of the costume, decoded and re-encoded to sRGB, differs from the OFX float sRGB capture by at most 0.044 display code. Display transfer differences on the costume do not account for the original paper discrepancy.

**Cut-dependent compensation.** Keep the two clear filtration shifts and change only print exposure, retaining each cut's normal RS calibration. Use the same training gray levels and each cut's actual OFX negative, and validate on the held-out montage. The table gives total EV relative to the RS slider at 1; the two values separated by a slash are OFX camera-filter compensation on / off.

| Lens setting | RS print EV on / off | Held-out P95 on / off | Held-out P99 on / off |
| --- | ---: | ---: | ---: |
| UV 430 nm | −0.02430 / +0.03223 | 0.255 / 0.362 | 0.300 / 0.362 |
| IR 650 nm | −0.01068 / +0.01784 | 0.245 / 0.315 | 0.274 / 0.315 |
| Both | −0.05447 / +0.03078 | 0.326 / 0.328 | 0.326 / 0.328 |

This measures the effective optical-print exposure difference at these cut settings. Interpolation to other wavelengths, stocks or manual EV settings has not been qualified. Float interchange also differs: on the saturated blue patch of the clear montage OFX emits an encoded red channel of −65.29 in 0–255 units; RS emits zero. Both display as zero. The unclipped P95 is 65.29 for this montage. The tool reports unclipped error separately; the reference-photo gate uses display codes.

**Normal photo entry point.** Applying the fitted clear settings to RS's own Portra negative gave P95 / P99 of 6.39 / 7.67 on the costume and 6.09 / 6.91 on the held-out montage (worse of CPU and resident Vulkan, rounded up). The previous errors with the native pack against these float references were 4.20 / 4.36 and 5.12 / 5.12 on CPU, respectively. Keep the stage calibration diagnostic. In a private resource copy, substituting the archived Python 6e7de28 Portra bandpass from the preceding study with the clear calibrated pack reduced CPU print P95 to 3.26 and 5.08; the held-out density P95 still differed by 0.0586. Next compare neutral and saturated film exposures before curve interpolation and the developed-density curves, holding the qualified paper setup fixed.

**Repeat.** Set `OFX_CASE=off`, `OFX_DISABLE_COLOR_ADAPTATION=1` and `OFX_CUTS_FLOAT_OUTPUT=sRGB` in the Natron driver for a float optical print. Use a separate Cineon capture with the float-output variable unset as in the preceding study. Run `python scripts/parity/ofx_endura_calibration.py <ofx_density_print executable> <RS recipe> <training Cineon TIFF> <training float print TIFF> <held-out Cineon TIFF> <held-out float print TIFF> <scratch recipe.json> --region 480 144` from the RS root. For the UV/IR tests start with the corresponding cut's recipe, copy the fitted clear enlarger settings into it and add `--exposure-only`. The diagnostic writes a replay recipe only after passing the independent display check.
