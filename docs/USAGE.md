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

The CLI accepts supported image and camera RAW formats through its existing
decoders. Camera/model coverage depends on those decoders. Run `process --help`
for available options.

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
| Arctic alpha02 | 2.829 | 3.839 |
| Mallett 2019 | 2.828 | 3.838 |

On Hanatos the linear print green channel for the neutral/red/blue patches was 0.184/0.173/0.289 with the clear lens setting and 0.184/0.527/0.044 with compensated No. 25. This follows the neutral-preservation and red/blue separation checked by [LAB's B&W render tests](https://github.com/JamesQiu2005/SpektraLab/blob/b51e672/engine/tests/bw_checks.py). The stocks and print recipes differ between the suites. On the same three compensated No. 25 patches, the largest absolute CPU/Vulkan linear output difference across all three models was 0.00000465. Windows Vulkan also rendered one 512-pixel-long-edge photo through the CLI, and the dedicated resident test checked shoulder fog and negative-cache reuse. Broader photographic acceptance and CUDA/macOS runs remain open.

### Graded black-and-white prints

Tri-X 400, T-Max 100, Acros II and HP5 Plus select Ilford Multigrade IV RC as their target paper. In the GUI, **Profiles → Paper grade** selects 00, 0, 1, 2, 3, 4 or 5. Grade 2 is the default. A low grade compresses contrast; a high grade deepens shadows and separates highlights.

For the CLI, save `{"print_render":{"paper_grade":"00"}}` as `soft.json`, then run:

```sh
spektrafilm process photo.png --film kodak_tri_x_400 --params soft.json -o soft-print.png
```

The same recipe works with `export-lut`. Grade IDs are strings so 00 and 0 stay distinct. A graded print requires the TH-KG3 illuminant. The grade sets neutral filtration and a calibrated exposure multiplier. Manual print exposure and filter shifts remain relative trims. An explicit grade with ordinary paper returns a construction error. A direct film scan bypasses paper controls. Changing grades requests a calibrated pipeline rebuild. On resident Vulkan, the existing developed negative remains reusable with the same source identity.

See the [silver profile provenance and calibration](../data/SPEKTRALAB_BW_NOTICE.md) for the film processes, fit method, grade-5 limit and regeneration commands.

### Spectral CUBE export

`export-lut` accepts the same film, target paper, `--scan-film` and `--params` as `process`. An explicit scan selects the film scan even if a paper is supplied. An explicit paper selects printing ahead of the JSON scan flag. With neither flag, both commands honor the saved route and use the film's target paper for printing. A direct negative scan yields the engine's transmission scan. FPW's negative inversion and calibrated finish are separate, versioned host routes.

```sh
./target/release/spektrafilm export-lut --film kodak_portra_400 \
    --paper kodak_portra_endura --params look.json --size 65 -o portra.cube
```

For a PNG/RAW comparison, set `look.json` to `{"io":{"input_color_space":"sRGB"}}` plus any desired look settings. Feed **decoded, linear** RGB to the LUT. Without overrides, the cube input uses the standalone TIFF convention of linear ProPhoto RGB. The grid supports the engine's named sRGB, ProPhoto RGB, Rec. 2020 and ACES2065-1 input primaries. Its input domain is [0,1], with 2–65 points per axis, red varying fastest. Output uses sRGB primaries, clipped to [0,1], with sRGB encoding by default or linear output when `io.output_cctf_encoding=false`. The header identifies the route, input, output, spectral method, actual backend and source profile credits.

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
