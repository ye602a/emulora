# Emulora third-party notices

## Code and numerical references

| Source | Use in this engine | License or notice |
| --- | --- | --- |
| [turbasvin/spektrafilm-rs](https://github.com/turbasvin/spektrafilm-rs) | Original Rust implementation | GPL-3.0. See [LICENSE](LICENSE). |
| [Andrea Volpato / spektrafilm](https://github.com/andreavolpato/spektrafilm) | Film model, processing order and reference numerical routines | GPL-3.0 for code. |
| [Johannes Hanatos](https://github.com/hanatos) | Spectral-upsampling method used by spektrafilm | Method attribution. See the data row below for the redistributed table. |
| [Colour Developers / Colour](https://github.com/colour-science/colour) | Colour science constants and reference calculations | BSD-3-Clause. See [COLOUR_LICENSE.txt](COLOUR_LICENSE.txt). |
| [SpektraLab](https://github.com/JamesQiu2005/SpektraLab) | Precise Gaussian IIR and negative-derived hue references | GPL-3.0-or-later. See the [IIR provenance](docs/maintenance/GRAIN_MULTILAYER_MIGRATION.md) and [negative hue study](docs/ENGINE_STUDIES.md#negative-derived-print-hue-batch-7). |
| FPW contributors | Physical stock defaults and versioned calibrated output | GPL-3.0-or-later. See [calibrated output provenance](docs/maintenance/CALIBRATED_OUTPUT_V1.md#provenance-and-validation). |

## Bundled data

| Data | Source and attribution |
| --- | --- |
| Film/paper profiles and neutral print calibration packs | [Andrea Volpato / spektrafilm](https://github.com/andreavolpato/spektrafilm), CC BY-SA 4.0. |
| Hanatos 2025 irradiance and Arctic beta04 reflectance tables | [Andrea Volpato / spektrafilm](https://github.com/andreavolpato/spektrafilm), CC BY-SA 4.0. The irradiance method credits [Johannes Hanatos](https://github.com/hanatos). The [Arctic beta04 resource revision](https://github.com/andreavolpato/spektrafilm/tree/28bf883/src/spektrafilm/data/luts/spectral_upsampling) includes the descriptor. |
| Silver film and Multigrade IV RC profiles | [SpektraLab / Hanze Qiu](https://github.com/JamesQiu2005/SpektraLab), CC BY-SA 4.0. See [source processes and RS adaptations](data/SPEKTRALAB_BW_NOTICE.md). |
| Negative-scan v1 calibration and heldout samples | Profile-derived data from [Andrea Volpato / spektrafilm](https://github.com/andreavolpato/spektrafilm), CC BY-SA 4.0. See [calibration provenance](data/negative_scan_ap0_v1/PROVENANCE.md). |

See the [full upstream data terms](SPEKTRAFILM_DATA_LICENSE.txt),
[data change record](data/CHANGELOG.txt) and [file inventory](data/MANIFEST.json).
