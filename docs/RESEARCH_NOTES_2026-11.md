# SpektraFilm-RS Research Notes: Data and Algorithms

Date: 2026-11-16

## 1. Film-border references
SpektraLab overscan expands the film canvas beyond the image content and uses scan registration, gate, perforations, edge exposure/fog and light-leak parameters to form the border. Its Windows/Linux branch has Vulkan kernels and is a useful cross-platform reference. Do not copy its allocation strategy; use RS resource ownership and pooling.
- [Lab Vulkan overscan shader](https://github.com/JamesQiu2005/SpektraLab/blob/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/src/shaders/vulkan/spk_overscan_body.glsl)
- [Lab overscan pipeline](https://github.com/JamesQiu2005/SpektraLab/blob/main/engine/src/pipeline/overscan.cpp)
- [Lab Vulkan tests](https://github.com/JamesQiu2005/SpektraLab/blob/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/tests/gpu_vulkan_overscan.cpp)

## 2. Stock/profile inventory and gaps
RS currently has about 22 exposure profiles and 9 print profiles. SpektraLab's catalog has about 34 entries including aliases; OFX public source lists 20 film and 8 paper stocks. Current OFX website claims exceed what can be verified in its public source; do not treat website features as open-source implementation inventory.
- [RS profiles](../data/profiles)
- [SpektraLab StockCatalog](https://github.com/JamesQiu2005/SpektraLab/blob/main/app/SpektraLab/Resources/StockCatalog.json)
- [OFX public stock lists](https://github.com/chaert-s/spektrafilm-ofx/blob/main/tools/ofx_stock_lists.py)

Additional SpektraLab candidates:
- Kodak Tri-X 400, T-Max 100, Fujifilm Acros II, Ilford HP5 Plus: monochrome negatives; understand three-column expanded representation versus RS single-column semantics before conversion.
- Ilford Multigrade IV RC: multi-emulsion variable-contrast paper; requires separate modeling.
- CineStill 800T: derived alias, with rem-jet-free back-reflection halation semantics.
RS Kodak Tri-X Reversal 7266 is not Tri-X 400. Profile count does not prove measurement quality. New profiles must include provenance, conversion script, fit assumptions, license and validation.

## 3. Performance candidate
darktable's development branch recently fused blur final-axis output with destination accumulation, reducing intermediate writes/reads and scheduling. RS has a corresponding blur→add_scaled path worth inspecting. Some other upstream changes (channel vectorization, source/destination organization) overlap with existing RS structures and should not be duplicated.
- [darktable d996f081](https://github.com/darktable-org/darktable/commit/d996f0810350d3717788ab9190cc40eef9ed14c8)
- [channel vectorization](https://github.com/darktable-org/darktable/commit/bea502fdd12862a20d54fa4142a34f5a953024a4)
- [source/destination change](https://github.com/darktable-org/darktable/commit/aec84e6665623acc5cc2e13eb305ae56d19672a5)

## 4. CPU/GPU diffusion differences
CPU uses per-channel RGB radii and a multi-Gaussian tail approximation; parts of GPU average radii and use a different tail approximation. This predates recent integration. Measure response offline first; if needed, design a new versioned model.
- [CPU diffusion](../crates/spektrafilm-model/src/diffusion.rs)
- [GPU pipeline setup](../crates/spektrafilm-core/src/pipeline.rs)
- [GPU backend](../crates/spektrafilm-gpu/src/wgpu_backend.rs)

## 5. Grain statistics reference
grainifier analyzes negative density D=-log10(T), bins by density, removes local plane trends and measures cross-channel covariance, power spectrum and spatial correlation. This is useful as an offline evaluation method, not an RS real-time rendering algorithm. Monochrome silver-grain statistics do not directly model color dye clouds.
- [grainifier stats](https://github.com/grainifier/grainifier/blob/main/grainifier/stats.py)

## 6. Illuminant normalization reference
vkdt normalizes scene, target-film and reference illuminant spectra by luminance to avoid overall exposure changes when illuminants change. Evaluate offline with fixed film/paper/exposure and grayscale/saturated patches.
- [vkdt illuminant normalization](https://github.com/hanatos/vkdt/commit/c4fd786a06e68addabac94898dd744958b959687)

## 7. Data provenance and license boundaries
- spektrafilm code and data have distinct licenses; verify GPL and CC BY-SA obligations, attribution and derivative-data conditions per file.
- SpektraLab's notice distinguishes upstream profile/LUT derivatives from product-built resources.
- Official OFX binary archives may contain non-GPL resources absent from the public source repository. Do not extract those resources for this project.
- OFX exported LUTs have a separate license; do not treat them as freely redistributable stock data.
- For every new profile retain source URL, version/commit, raw data, conversion script, fit/borrowed-field notes and license.

References:
- [SpektraLab NOTICE](https://github.com/JamesQiu2005/SpektraLab/blob/main/license/NOTICE.md)
- [OFX binary license](https://github.com/chaert-s/spektrafilm-ofx/blob/main/Legal/SPEKTRAFILM_OFX_LICENSE.txt)
- [OFX exported LUT license](https://github.com/chaert-s/spektrafilm-ofx/blob/main/Legal/SPEKTRAFILM_OFX_LUT_LICENSE.txt)
- [Spectral Film LUT data](https://github.com/JanLohse/spectral_film_lut)

## 8. Non-goals
Do not replace the physical pipeline with LUTs; do not bulk-import stocks; do not force monochrome data into a color schema; do not add runtime alias inheritance for a single alias without evidence; do not replace high-precision IIR merely to follow an upstream trend; do not add offline statistics/FFT/Python to rendering dependencies; do not silently change accepted model response.
