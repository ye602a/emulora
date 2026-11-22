# Emulora

**English** | [简体中文](README.zh-CN.md)

Emulora combines *emulsion* and *aurora*. It is an independent Rust engine for physical film simulation. It exposes a digital photograph onto simulated film, develops it, then prints it on photographic paper or scans the film directly. **FPW** uses the engine at its own pinned revision. You can also process photos with the CLI, preview them in the desktop GUI, or embed the engine in another application.

## What you can use today

| Stage | Available in the engine |
| --- | --- |
| Camera and film | Spectral exposure and development for colour negatives, reversal film and black-and-white film. Manual EV, metering with highlight or shadow protection, lens diffusion, halation, DIR interlayer effects and source-anchored multilayer grain. The optional latitude report estimates how much of a shot approaches the shadow and highlight limits. |
| Black-and-white photography | Tri-X 400, T-Max 100, Acros II and HP5 Plus print on three-emulsion Ilford Multigrade IV RC. Select grade 00, 0, 1, **2** (default), 3, 4 or 5. Measured clear, yellow No. 8/12 and red No. 25/29 camera responses support stock-dependent exposure compensation. |
| Darkroom and scan | Optical printing with enlarger filtration, exposure, preflash, paper curve morphing and developer exhaustion. Direct film scans skip the paper. Negative bleach bypass changes the developed film. Print bleach bypass acts on the colour paper. |
| Film borders | Resident Vulkan renders 135 full frame, half frame, XPan and configurable 120 gates. Recipes can place edge codes, a frame number, a date or a local light leak in film coordinates. [Formats and backend limits](docs/135_FILM_BORDER.md) |
| Files and integration | Standalone JPEG, PNG, linear TIFF and RAW input, plus spectral CUBE export for the pointwise part of a look. Hosts can supply a GPU device, reuse developed negatives and work buffers, and request GPU, RGB16 or region output. CPU rendering includes an f64 reference build. [Usage and supported routes](docs/USAGE.md) |

### Defaults and opt-in looks

The default RGB-to-film method is **Hanatos 2025**, including the fitted sensitivity window on stocks that supply one. **Arctic beta04** is selectable. Hanatos stock colour correction is a separate switch, off by default and available for 20 profiles with fitted coefficients.

Colour negatives use **optical printing** by default. Negative-derived print hue is optional on CPU and resident Vulkan for colour paper, sRGB output and zero manual yellow/magenta shifts. Both bleach bypass controls default to zero. In the resident path, changing the paper grade, print hue or print bypass can reuse the negative. Changing film-side bypass requires redevelopment. [Recipes and controls](docs/USAGE.md)

### Print validation

We compared prints from real photographs and gray scales with reference renders. For the black-and-white and colour print recipes tested, both CPU and Vulkan produced results close to the reference. [Test coverage and example recipes](docs/ENGINE_EVOLUTION.md)

### What is still outside the product?

| Status | Feature and reason |
| --- | --- |
| Studied and deliberately held back | The rem-jet/back-reflection prototype produced an excessive red tail. Wavelength smoothing, reconstructed-white normalization and Langmuir DIR remain studies. Production uses the established halation, normalization and DIR paths. |
| Awaiting measurements or permission | Multigrade half grades are unavailable. The hard-end wedge did not support a reliable 4½ step. Portra 160NC/160VC/400NC/400VC and their matching neutral print settings await explicit redistribution terms. |
| Not developed | Embedded-ICC conversion in standalone input and a wider-gamut workflow redesign. Physical film borders require Vulkan. |

## Get started

Requires Rust **1.99+** and the platform's native build tools. Vulkan is the
validated physical-engine GPU backend. Windows builds and Linux software-Vulkan
compute are covered by CI; other platform/backend combinations need separate
qualification. See [build and usage](docs/USAGE.md) for native dependencies and GUI/export setup.

```sh
git clone https://github.com/ye602a/emulora.git
cd emulora
cargo build --locked --release -p emulora-cli --bin emulora
./target/release/emulora list-profiles --data-dir data
./target/release/emulora process input.tif -o output.png \
    --film kodak_portra_400 --paper kodak_portra_endura --data-dir data
```

On Windows, use `target\release\emulora.exe` and run the process command on one line.

## Documentation

- [Current engine work and upstream comparisons](docs/ENGINE_EVOLUTION.md)
- [Build and usage](docs/USAGE.md)
- [Host GPU integration](docs/maintenance/HOST_GPU_INTEGRATION_CONTRACT.md)
- [Public integration example](crates/emulora-core/examples/host_gpu_contract.rs)
- [Compatibility and development plan](docs/ENGINE_EVOLUTION.md)
- [Film engine measurements](docs/ENGINE_STUDIES.md)
- [Validation scope and evidence](docs/maintenance/VALIDATION.md)

## Credits and licenses

The original Rust implementation is by [turbasvin](https://github.com/turbasvin/spektrafilm-rs).
The film model and spectral/profile data originate from
[Andrea Volpato's spektrafilm](https://github.com/andreavolpato/spektrafilm).
Spectral upsampling builds on work by [Johannes Hanatos](https://github.com/hanatos).
Numerical routines draw on NumPy, SciPy, scikit-image and colour-science;
[multilayer grain and precise IIR provenance](docs/maintenance/GRAIN_MULTILAYER_MIGRATION.md)
includes the SpektraLab contribution.

Code: [GPL-3.0](LICENSE). Spectral data, profiles and their derivatives:
[upstream attribution and CC BY-SA 4.0 terms](SPEKTRAFILM_DATA_LICENSE.txt).
SpektraLab still negatives and Multigrade IV RC have [separate attribution and Emulora calibration notes](data/SPEKTRALAB_BW_NOTICE.md).
[Versioned calibration provenance](data/negative_scan_ap0_v1/PROVENANCE.md)
is retained alongside the data.

See the [third-party notices](THIRD_PARTY_NOTICES.md) and [bundled data inventory](data/MANIFEST.json).
