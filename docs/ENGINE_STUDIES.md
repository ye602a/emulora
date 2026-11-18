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

A fixed [SpektraLab data revision](https://github.com/JamesQiu2005/SpektraLab/tree/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/resources_product) was converted offline. Collapse triplicated sensitivity, characteristic curves, layers and CDF parameters to one channel. Sum the three silver-density weights and retain the exposure axis and base density. The four profiles each preserve 256 response points and 81 wavelengths. Maximum double-precision differences were 4.44e-16 in optical density and 1.11e-16 in transmission. The stock loader and a diagnostic Vulkan ramp passed for all four. The fixed revision credits Hanze Qiu and lists the data under CC BY-SA 4.0. Production stock and multi-emulsion paper resources are a separate scope.

## CPU and Vulkan diffusion

On GTX 1080 Ti (582.78), a 5 µm sample of existing color-negative scatter and halation gave CPU versus Vulkan point-response 95% radii of R 65/30, G 45/25 and B 40/25 µm. The tails beyond 20 µm carried 26.5–35.0% versus 8.5–14.1% of the respective kernels. The tested Black Pro-Mist setting had closer 95% radii (R 335/335, G 265/265, B 170/175 µm) with a red edge-response difference. For a 513×513 real crop, comparing the effect-on minus effect-off print within each backend put the differences near 1.0–1.5 display codes RMS. Local red peaks were 34 codes for scatter/halation and 16 for lens diffusion. An optical model revision can be evaluated independently against these fixed inputs.

## Density-domain grain and illuminants

Two Vulkan seeds at four flat density levels and 6 µm sampling were measured in 64×64 interior windows. Double-X silver density used identical RGB residuals. Portra 400 layered residuals showed distinct channel variances and near-zero between-channel correlation. Report variance, spatial autocorrelation and radial frequency power by density and stock. Separate scene, film and viewing spectra were integrated at 5 nm and normalized with the CIE 1931 Y-weighted integral. With equal Y, the Portra 400 sensitivity predicts distinct D55 and 2856 K channel responses. These are offline diagnostics under fixed input, with no added renderer dependency. [CIE observer data](https://cie.co.at/datatable/cie-1931-colour-matching-functions-2-degree-observer) · [vkdt normalization reference](https://github.com/hanatos/vkdt/commit/c4fd786a06e68addabac94898dd744958b959687)

Sources for the experiments are collected in the [research notes](RESEARCH_NOTES_2026-11.md). Proposed formats, edge markings and variable-contrast paper are described in the [extension studies](EXTENDED_FILM_AND_PRINT.md).
