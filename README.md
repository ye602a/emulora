![spektrafilm-rs](docs/card.jpg)

# spektrafilm-rs

**English** | [简体中文](README.zh-CN.md)

Start with one digital photo and see how it looks printed on colour paper. Or choose a black-and-white film, add a yellow filter and try a softer or harder paper grade. spektrafilm-rs simulates film exposure, development, printing and scanning. It runs on its own and is also the film engine used by the FPW photography application. This guide covers the standalone tools. FPW selects its own engine version and controls.

## Try it with one photo

Open an **sRGB JPEG or PNG** in the desktop preview app. For colour, try Kodak Portra 400 with Kodak Portra Endura paper. For black and white, choose Tri-X 400 and start with the default grade 2 on Ilford Multigrade IV RC. Keep the film and exposure fixed, then switch to grade 1 for a softer print or grade 4 for more contrast.

The standalone tools also open linear TIFFs and supported camera RAW files. The guide covers setup, saving a preview, exporting a full-size photo and using the command line. [Build and usage guide](docs/USAGE.md)

## What you can do

- **Set the exposure.** Adjust it by hand or choose a metering mode that favours highlights or shadows. For a backlit shot, the optional command-line latitude report estimates how much of the image falls near the film’s shadow and highlight limits.
- **Make a colour print.** Use the enlarger filters to adjust a colour cast, set the print exposure, or try preflashing the paper. You can also scan reversal film directly to skip the paper. A direct negative scan produces a negative image.
- **Work in black and white.** Tri-X 400, T-Max 100, Acros II and HP5 Plus all print on Multigrade IV RC. Yellow and red filters change how colours translate to gray. A blue sky, for example, will print darker. You can compensate for the light lost through the filter and choose a paper grade separately to set the contrast.
- **Shape texture and framing.** Adjust grain, lens diffusion and the halo around bright edges. With Vulkan, a recipe can select 35 mm full frame, half frame, XPan or 120 film and add edge markings, a date or a local light leak. [Film formats and borders](docs/135_FILM_BORDER.md)
- **Use the colour look in another editor.** Export the film and paper colour response as a CUBE lookup table. Grain, halos and borders are rendered with the photo itself. [CUBE export guide](docs/USAGE.md#spectral-cube-export)

## Looks you can switch on

Colour negatives are optically printed by default. Try these options individually on the same photo to see what each changes.

- **Negative or print bleach bypass.** Both start at zero. Retaining silver in the negative affects subsequent scans and prints from that negative. Retaining silver in the colour paper affects only the print.
- **Hue from negative.** Off by default. Blue or violet fabric makes a good subject for a side-by-side comparison. This option works on colour prints with sRGB output and requires both manual yellow and magenta adjustments to be zero.
- **Film colour response.** Hanatos 2025 is the default method for translating the colours in a photo into film exposure. Arctic beta04 is an alternative you can select. With Hanatos, some film stocks also offer their own colour correction, which is off by default. [Colour and print settings](docs/USAGE.md)

## What is still unavailable?

| Feature | Current status |
| --- | --- |
| Half-grade black-and-white paper settings | Grades 00, 0, 1, 2, 3, 4 and 5 are available. At the hard end, the measurements do not reliably distinguish grade 4½ from its neighbours, so the selection stays at full grades. |
| Older Portra NC and VC stocks | The four 160 and 400 NC/VC stocks and their matching print settings are awaiting explicit redistribution terms. |
| The red halo of film with its backing removed | A prototype left an overly long red tail at bright edges, so this look is on hold. Standard film halation is available. Other colour and development experiments are covered in the [research notes](docs/ENGINE_STUDIES.md). |
| Automatic input colour profile conversion | The standalone tools do not yet convert embedded ICC profiles. Convert JPEGs or PNGs with other profiles to sRGB first. A full wide-gamut workflow is still to be developed. |

## Print validation

We compared prints from real photographs and gray scales with reference renders. For the black-and-white and colour print recipes tested, both CPU and Vulkan produced results close to the reference. [Test coverage and example recipes](docs/ENGINE_EVOLUTION.md)

For development, see [build and backend support](docs/USAGE.md#backends), the [host integration guide](docs/maintenance/HOST_GPU_INTEGRATION_CONTRACT.md) and the [integration example](crates/spektrafilm-core/examples/fpw_host_contract.rs).

## Credits and licenses

The film model and spectral data come from [Andrea Volpato’s spektrafilm](https://github.com/andreavolpato/spektrafilm). [turbasvin](https://github.com/turbasvin/spektrafilm-rs) wrote the original Rust port. Spectral reconstruction builds on the work of [Johannes Hanatos](https://github.com/hanatos). Numerical routines draw on NumPy, SciPy, scikit-image and colour-science. SpektraLab contributed to the multilayer grain and filtering implementation. [Implementation credits](docs/maintenance/GRAIN_MULTILAYER_MIGRATION.md)

The code is licensed under [GPL-3.0](LICENSE). For film and paper data, see the [upstream attribution and CC BY-SA 4.0 terms](SPEKTRAFILM_DATA_LICENSE.txt), the [separate notice for the black-and-white stocks and paper](data/SPEKTRALAB_BW_NOTICE.md) and the [scan calibration data credits](data/negative_scan_ap0_v1/PROVENANCE.md).
