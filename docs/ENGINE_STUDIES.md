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
