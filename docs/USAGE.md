# Build and usage

Run commands from the repository root. Examples use a POSIX shell; on Windows,
use `.exe` binaries, PowerShell environment syntax, and one-line commands.

## Requirements

- Rust 1.99 or later, with the platform's C/C++ build tools.
- A compatible GPU/driver for the chosen backend. Vulkan is the engine path
  exercised by the required Linux compute tests.
- Linux uses OpenBLAS and needs a native build toolchain, including a Fortran
  compiler when building OpenBLAS from source. The GUI also needs the usual
  X11/Wayland development libraries.
- Windows uses the CPU fallback implementation without a system OpenBLAS install.
  macOS CPU builds use Accelerate.

The lockfile pins wgpu 30.0.1 and eframe/egui 0.37.0. The engine dependency enables
Vulkan explicitly; building the GUI can enable additional wgpu backends through
Cargo feature unification. Those backends are not qualified by the Vulkan suite.

## CLI

```sh
cargo build --locked --release -p spektrafilm-cli --bin spektrafilm
./target/release/spektrafilm --help
./target/release/spektrafilm list-profiles --data-dir data
./target/release/spektrafilm process input.tif -o output.png \
    --film kodak_portra_400 --paper kodak_portra_endura --data-dir data
```

`--params settings.json` accepts runtime parameter overrides. `--scan-film`
selects direct film scanning instead of printing; use it only with the intended
profile and output contract. The generic CLI does not automatically select the
versioned host stock/calibrated-output setup.

The CLI, GUI and RAW parity helper share the standalone photo decoder. Use sRGB JPEG or PNG for display-referred photos. Both are decoded to linear sRGB before exposure. Embedded ICC conversion is outside this loader; convert other display profiles to sRGB first. Supported camera RAWs use rawler's linear sRGB development. FPW owns its separate RAW preparation.

Supply linear samples in TIFF. The GUI assumes linear ProPhoto RGB on every TIFF open. The CLI uses the recipe's `io.input_color_space` for TIFF (default ProPhoto RGB). A TIFF tagged with another transfer function needs conversion before rendering. The spectral constructor and recipe refresh accept sRGB, ProPhoto RGB, Rec. 2020 and ACES2065-1 primaries. Encoded output uses sRGB. For linear output in other primaries, turn off encoding and select a supported gamut policy, e.g. `off`. Unsupported primaries and active unsupported gamut algorithms now fail preparation. Existing FPW linear AP0 negative-scan and encoded sRGB routes keep their settings.

### Black-and-white camera filters

In the GUI, select a monochrome film and open **Exposure → Camera filter**. For a print, Double-X (`kodak_doublex`) and Kodak 2302 make a supported pair. The CLI accepts the same settings through `--params` for both `process` and `export-lut`. For example:

```json
{"camera":{"photographic_filter":"wratten25","compensate_filter":true}}
```

- `off` (default) preserves the original stock response and previous recipe results. `clear` selects the new measured monochrome response without glass. Use **clear** as the reference for filter comparisons. Older rendering mapped a missing log-sensitivity sample to 0 and thus a linear sensitivity of 1. Measured mode gives unmeasured wavelengths zero weight while retaining a genuine measured log value of 0.
- `wratten25` and `wratten29` apply lens transmission to film sensitivity before exposure. The B-3 transmission percentages are interpolated on the 5 nm engine grid. We treat the unreported short-wave transmission as zero and hold the 700 nm value through 780 nm. Our No. 25 grid matches LAB's 81 samples to 1.2e-16 absolute. The values were independently transcribed from [Repacholi's B-3 tables](https://www.mat.uc.pt/~rps/photos/filter-data.html).
- With `compensate_filter: false` (default) the glass costs exposure. With `true` RS restores the camera raw response to a linear 0.184 neutral in the selected input primaries. `Pipeline::filter_factor_ev()` reports the required extra exposure in stops in either case. The normal camera EV control remains available. Metering reads the source. Print normalization uses the bare measured stock, and modeled gate-external fog bypasses the lens. A host changing the filter or its compensation uses checked refresh as with any other calibration input.
- Enabling measured response is an appearance change. These options require a spectral constructor and a B&W film. A cold build with glass prepares the bare reference and filtered camera front from the same loaded resource. The runtime uses one front on CPU and resident GPU.

**Double-X / 2302 reference.** Fixed linear sRGB patches, manual exposure, unencoded output, grain and glare disabled. Kodak specifies daylight factors of 8 and 20 (3.00 and 4.32 stops). Our model follows the stock and upsampler's neutral; No. 29 yields about half a stop less loss than the published daylight guide. [Kodak technical data](https://www.kodak.com/content/products-brochures/Film/DOUBLEX-Technical-Data-EN.pdf)

| Camera model | No. 25 EV | No. 29 EV |
|---|---:|---:|
| Hanatos 2025 | 2.842 | 3.872 |
| Arctic beta04 | 2.828 | 3.838 |
| Mallett 2019 | 2.828 | 3.838 |

On Hanatos the linear print green channel for the neutral/red/blue patches was 0.184/0.173/0.289 with the clear lens setting and 0.184/0.527/0.044 with compensated No. 25. This follows the neutral-preservation and red/blue separation checked by [LAB's B&W render tests](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/tests/bw_checks.py). The stocks and print recipes differ between the suites. On the same three compensated No. 25 patches, the largest absolute CPU/Vulkan linear output difference across all three models was 0.00000519. Windows Vulkan also rendered one 512-pixel-long-edge photo through the CLI, and the dedicated resident test checked shoulder fog and negative-cache reuse. Broader photographic acceptance and CUDA/macOS runs remain open.

For Double-X at the 6.5-minute development setting, this optional CLI recipe measured near Kodak's granularity of 14 through a 48 µm aperture at 3–6 µm sampling. Use it with `process --film kodak_doublex --paper kodak_2302 --params FILE`. Sampling pitch changes with film format and image dimensions. [Measured range and reference conditions](ENGINE_STUDIES.md#48-µm-aperture-granularity-batch-12)

```json
{"film_render":{"development_time":6.5,"grain":{"active":true,"agx_particle_area_um2":0.39}}}
```

### Graded black-and-white prints

Tri-X 400, T-Max 100, Acros II and HP5 Plus select Ilford Multigrade IV RC as their target paper. In the GUI, **Profiles → Paper grade** selects 00, 0, 1, 2, 3, 4 or 5. Grade 2 is the default. A low grade compresses contrast; a high grade deepens shadows and separates highlights.

For the CLI, save `{"print_render":{"paper_grade":"00"}}` as `soft.json`, then run:

```sh
spektrafilm process photo.png --film kodak_tri_x_400 --params soft.json -o soft-print.png
```

The same recipe works with `export-lut`. Grade IDs are strings so 00 and 0 stay distinct. A graded print requires the TH-KG3 illuminant. The grade sets neutral filtration and a calibrated exposure multiplier. Manual print exposure and filter shifts remain relative trims. An explicit grade with ordinary paper returns a construction error. A direct film scan bypasses paper controls. Changing grades requests a calibrated pipeline rebuild. On resident Vulkan, the existing developed negative remains reusable with the same source identity.

See the [silver profile provenance and calibration](../data/SPEKTRALAB_BW_NOTICE.md) for the film processes, fit method and regeneration commands. Grades 4 and 5 give modestly different hard prints. A midgray-matched trial between them was too close to both to qualify as a separate 4½ setting. [Gray ramp and photographic comparison](ENGINE_STUDIES.md#multigrade-iv-rc-at-the-hard-end)

### Hue from negative

For a colour negative on colour paper, enable **Profiles → Hue from negative** or add `"negative_hue": true` inside the recipe's `print_render` object. Existing recipes use the optical print. Try it on blue and violet clothing or strongly coloured fabrics. On grainy portraits compare the cheeks at both working size and delivery size.

The look reads the developed negative and steers the optical print's hue while retaining its lightness and respecting the paper chroma boundary. CPU and resident Vulkan support sRGB output and zero manual enlarger Y/M shifts. The calibrated neutral pack still applies. For manual Y/M grading, turn off **Hue from negative** before adjusting the filters. The optical print preserves the hue rotation you dial in and can reuse the same resident negative. [Four film/paper comparisons](ENGINE_STUDIES.md#manual-enlarger-ym-and-negative-derived-hue-batch-8) An active colour print with an unsupported trim or output space reports a preparation error. Direct scans, reversal film and black-and-white materials bypass this print look. CUDA reports unsupported for an active colour print.

Hosts with an exclusively owned pipeline can switch the recipe using `Pipeline::refresh_in_place` without copying the profiles or spectral LUT. A false return preserves the last successful pipeline; rebuild from the original profiles and propagate any error for the requested render. Callers retaining an immutable pipeline can use `Pipeline::refreshed` on the same immutable input identity. The option is downstream of the negative cache. A pointwise spectral CUBE can include this look under the exporter's normal grain-free, local-DIR conditions. [Photographs and verification](ENGINE_STUDIES.md#negative-derived-print-hue-batch-7)

### Spectral CUBE export

`export-lut` accepts the same film, target paper, `--scan-film` and `--params` as `process`. An explicit scan selects the film scan even if a paper is supplied. An explicit paper selects printing ahead of the JSON scan flag. With neither flag, both commands honor the saved route and use the film's target paper for printing. A direct negative scan yields the engine's transmission scan. FPW's negative inversion and calibrated finish are separate, versioned host routes.

```sh
./target/release/spektrafilm export-lut --film kodak_portra_400 \
    --paper kodak_portra_endura --params look.json --size 65 -o portra.cube
```

For a JPEG/PNG/RAW comparison, set `look.json` to `{"io":{"input_color_space":"sRGB"}}` plus any desired look settings. Feed **decoded, linear** RGB to the LUT. Without overrides, the cube input uses the standalone TIFF convention of linear ProPhoto RGB. The grid supports the engine's named sRGB, ProPhoto RGB, Rec. 2020 and ACES2065-1 input primaries. Its input domain is [0,1], with 2–65 points per axis, red varying fastest. Output uses sRGB primaries, clipped to [0,1], with sRGB encoding by default or linear output when `io.output_cctf_encoding=false`. The header identifies the route, input, output, spectral method, actual backend and source profile credits.

The bake keeps fixed exposure, local DIR, calibrated print filters, preflash, fitted print chemistry, scanner correction and supported output gamut compression. It omits metering, frame-normalized highlight boost, grain, glare, spatial DIR, optical diffusion and sharpening, borders and resampling. It evaluates the spectral integrations directly. Compare it with a photo rendered with those same pointwise settings on the same backend. A LUT interpolates between samples. Scene values above 1 lie outside its sampled domain. See the [measured interpolation errors](ENGINE_STUDIES.md#spectral-cube-export).

Both commands report spectral initialization failures. `Pipeline::try_process` returns geometry and backend errors to file-output and GUI callers. CPU can render ordinary spectral photos, and a physical 135 border requires a supporting resident backend. The infallible `process` convenience treats errors as fatal. Hosts using borrowed input should use `process_resident_with_options` or `try_with_resident_params` to retain detailed errors.

### f64 CPU binary

Build separately to keep the default f32 applications intact:

```sh
cargo build --locked --release -p spektrafilm-cli \
    --features precision-f64 --bin spektrafilm-f64 --target-dir target/f64
SPEKTRAFILM_BACKEND=cpu ./target/f64/release/spektrafilm-f64 \
    process input.tif -o output.png --film kodak_portra_400 \
    --paper kodak_portra_endura --data-dir data
```

In PowerShell, set `$env:SPEKTRAFILM_BACKEND = "cpu"` before invoking
`target\f64\release\spektrafilm-f64.exe`.

## Preview GUI

```sh
cargo build --locked --release -p spektrafilm-gui
cargo build --locked --release -p spektrafilm-cli --bin decode_raw_gui
./target/release/spektrafilm-gui
```

The GUI provides image loading, profile selection, effect controls, zoom/pan and
preview saving. **Save** writes the preview; **Export** reruns processing through
the f64 CPU helper. Build the f64 binary above and point `SPEKTRAFILM_F64_CLI` to
its path before launching the GUI. The helper can also be found next to the GUI
or on `PATH`.

For a portable package, see [the Windows script](../scripts/package_windows_app.ps1)
and [the macOS script](../scripts/package_macos_app.sh). Platform packaging is
separate from numerical and interactive-GUI qualification.

## Backends

`SPEKTRAFILM_BACKEND` selects `wgpu`, `cpu`, or the optional `cuda` backend.
CUDA is experimental and is not covered by the current Vulkan compatibility
results. Building it does not establish parity with the versioned host routes.

```sh
cargo build --locked --release -p spektrafilm-cli --bin spektrafilm \
    --features spektrafilm-gpu/cuda-backend
```

CUDA requires the NVIDIA driver and CUDA 12 NVRTC runtime libraries. Select a
zero-based device with `SPEKTRAFILM_CUDA_DEVICE` when needed.

## Embed the engine

Start with the [host integration contract](maintenance/HOST_GPU_INTEGRATION_CONTRACT.md)
and [public example](../crates/spektrafilm-core/examples/fpw_host_contract.rs).
The example uses bundled profiles and the engine's public APIs:

```sh
cargo run --locked -p spektrafilm-core --example fpw_host_contract
```

Keep the engine revision, resource identity and output route together when
integrating it into an application. See [validation](maintenance/VALIDATION.md)
for what has been tested and what requires application-specific acceptance.

## Arctic beta04

In the GUI select `arctic2026beta04` under **RGB→raw upsampling**. CLI recipes select the same ID in `settings.rgb_to_raw_method`. An explicit `arctic2026alpha02` recipe now reports an unsupported method. Change its ID to adopt the new look, or use RS at `43763e7` with the corresponding resources to reproduce the old look.

RS ships Andrea Volpato's beta04 reflectance table and descriptor unchanged from [experimental revision 28bf883](https://github.com/andreavolpato/spektrafilm/tree/28bf883/src/spektrafilm/data/luts/spectral_upsampling). It keeps the RS D65 projection, film-illuminant relighting, exposure calibration and neutral print filters. This corresponds to the table-only beta row of the [photographic study](ENGINE_STUDIES.md#arctic-alpha02-and-beta04-batch-13). The five inactive alpha files remain in the pinned FPW v1 physical resource inventory.

FPW main at `e1d0759` builds both new and saved film recipes through [the FPW adapter](https://github.com/ye602a/fursuit-photo-workflow/blob/e1d0759/native/crates/fpw-runtime/src/film.rs). It starts from the [RS defaults at the host pin](https://github.com/ye602a/spektrafilm-rs/blob/1947ee8/crates/spektrafilm-core/src/params.rs), with no method override. FPW uses Hanatos 2025. The bundled FPW table, RS table and Python experimental Hanatos 2025 irradiance table share Git blob `7b81b9d23f8f4c599c6b2ae3d949e9a2d6f3c38a`. Python main and experimental default to Hanatos 2025. The Linux OFX v0.3.12 manual (pp. 21, 38) specifies Hanatos 2026 as its default. This RS update leaves the host pin and resources unchanged.

The released beta table has upstream Git blob `50689049562f59d8a71b8ff70b2d82965b3404ce`. At 480×144 on Portra 400 / Portra Endura and Ektar 100 / Endura Premier, CPU and Vulkan each reproduced their own pre-upgrade CLI control with this table byte-for-byte in the raw output. Comparing CPU to Vulkan gave maximum displayed differences of 5 and 12 sRGB codes respectively, also present in the controls.

The OFX version label refers to a per-stock window already active in the FPW default. The Portra and Ektar checks are documented in [Hanatos version names and FPW](ENGINE_STUDIES.md#hanatos-version-names-and-fpw).

## Negative bleach bypass

For a color negative, set **Negative bleach bypass** in the standalone film controls or use `{"film_render":{"bleach_bypass":0.5}}` in a spectral CLI recipe. Zero preserves the original look. At full strength the red and white costume in the comparison lost saturation and the white fur shifted toward cyan. Start at half strength for a gentler effect. This affects direct film scans and every subsequent optical print. A host changing this value must prepare the new recipe. The RS resident cache handles negative invalidation. [Same-photo comparison](ENGINE_STUDIES.md#negative-and-print-bleach-bypass-on-the-same-photograph)

## Print bleach bypass

On a color print, **Print bleach bypass** sets the retained-silver look from 0 (the normal optical print) to 1 (full modeled strength). A CLI recipe can set `{"print_render":{"bleach_bypass":0.5}}`. Film scans and silver B&W paper ignore the print control. The parameter is downstream of the developed negative; a resident host reprinting the same source can reuse its negative. Standalone GUI interaction and FPW control wiring were not tested. [Model and photograph comparison](ENGINE_STUDIES.md#print-bleach-bypass)

### Cinema film and viewing illuminants

The ordinary spectral pipeline now uses the lights specified by each material. Vision3 200T and 500T use the measured incandescent spectrum (`T`); Kodak 2383 and 2393 are viewed under the Kinoton 75P projector spectrum (`K75P`). This changes renders and newly baked cubes involving those materials. Earlier versions silently used D55 for the film or D50 for the print. Start with the stock neutral pack, including when using Arctic beta04. Recheck any manual Y/M shifts fitted to older renders. The explicit `FpwV1` constructor retains its accepted daylight substitutions, and the published FPW dependency has not changed. [Measured comparisons](ENGINE_STUDIES.md#cinema-illuminants-and-shipping-engine-comparisons)

For half-frame, XPan and configurable 120 roll-film gates, including a frame/date imprint or local light leak, see the [resident recipe example](135_FILM_BORDER.md#explicit-roll-formats-and-latent-exposures).

## Camera UV and IR cut

In the standalone GUI open the exposure controls and expand **UV / IR cut**. The recipe equivalent is `"camera": {"filter_uv": [1, 430, 8], "filter_ir": [1, 650, 15]}`. Each triplet contains strength (0–1), transition centre and width in nanometres. Zero strength disables that edge; both default to zero. The usable spectrum comes from the stock's 380–780 nm response. An RGB input cannot recover radiation outside its captured bands.

The UV edge attenuates short wavelengths and the IR edge attenuates long wavelengths. The two transmissions multiply. Calibration restores each emulsion channel's integrated exposure to the stock reference illuminant. A photographed gray can still shift after RGB spectral reconstruction and printing. The Hanatos stock adaptation window and the measured B&W lens filter retain their separate controls. Fixed UV/IR settings also enter spectral CUBE exports. Changing an active edge requires a fresh calibration and negative. [Product comparison](ENGINE_STUDIES.md#camera-uv-and-ir-cut)

## Hanatos stock colour correction

Select **Color management → RGB→raw upsampling → hanatos2025**, then **Stock colour correction**. In a CLI recipe set `settings.apply_hanatos2025_adaptation_surface` to `true`. It defaults to off. The GUI shows the option for the 20 camera profiles with usable coefficients; switching to a silver stock or another upsampling method clears it. An explicitly incompatible CLI recipe reports an error. The correction also enters spectral CUBE exports.

For blue-violet fabric try Portra 800, UltraMax 400 or Ektar 100. On Portra 400 and Provia 100F start with it off when matching skin to the input photograph. Compare unusually saturated violet separately: a fabric and a solid violet patch can move in different directions. [Results for every eligible film](ENGINE_STUDIES.md#hanatos-stock-colour-correction-on-still-photographs)

### Still-photo exposure metering

In the standalone GUI open **Exposure**, enable auto exposure and choose a meter. CLI recipes and the engine API use the same camera settings. For example, use `{"camera":{"auto_exposure":true,"auto_exposure_method":"protect_highlights"}}` for a backlit scene.

| Meter | When to use it |
| --- | --- |
| `balanced` | Start here for a mixed scene. It averages brightness in stops after trimming the darkest and brightest 1% by rank. |
| `center` | Give the center more weight in that same trimmed estimate, for example for a centrally framed subject. |
| `protect_highlights` | Keep a bright region in range. It can lower balanced exposure by up to 3 EV, using the 99.5th percentile. |
| `protect_shadows` | Lift a substantial dark region. It can raise balanced exposure by up to 2 EV, using the 5th percentile. |

These modes meter linear input before film exposure. They estimate one exposure adjustment for the whole photo. Manual camera EV still adds to it. Existing settings retain their meaning and the default meter is center_weighted. The older highlight_weighted method weights luminance by its square and can respond more strongly to a single lamp. Spectral LUT export omits scene-dependent metering. [Measured comparisons](ENGINE_STUDIES.md#still-photo-exposure-metering)

### Film exposure latitude

Use `spektrafilm process photo.png --film kodak_portra_400 --paper kodak_portra_endura --latitude -o print.png` when changing stock, paper or exposure on a backlit scene. The CLI reports the usable neutral stop range relative to 18.4% gray and the fractions of the rendered samples near the medium's black and white ends. The ordinary output image keeps its requested recipe. A direct slide scan also supports the report. A transmission negative or a medium with no measurable positive black returns a diagnostic error.

The JSON line includes two views. `scene` estimates placement from linear input before development using LAB's channel-weighted brightness. `output` counts the samples in the finished image against the measured luminance thresholds. Use the latter for brightly colored fur, stage lighting and saturated clothing. Both sample up to 1024 positions per axis by stride; counts are relative to the valid sample count. Zero/dark-only or invalid RGB samples are excluded from the scene estimate. Black output pixels count as shadows. These are boundaries on a neutral tone ramp; texture can survive past them and an individual color channel can clip earlier.

Host integrations can retain the result of `Pipeline::measure_latitude`, analyze a supplied linear frame with `place_scene` and optionally measure an already available rendered buffer with `measure_output`. The diagnostic is opt-in. [Exposure ownership](maintenance/HOST_GPU_INTEGRATION_CONTRACT.md#exposure-ownership-and-read-only-latitude) · [Matched photos and measured boundaries](ENGINE_STUDIES.md#film-latitude-on-backlit-fur-and-a-neutral-wedge)
