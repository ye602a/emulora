# SpektraLab silver profiles in RS

RS distributes five adapted profiles from SpektraLab revision **1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94** under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). The pinned README credits the four measured films to **Hanze Qiu** and lists film and paper profiles under CC BY-SA 4.0. SpektraLab produced the Multigrade paper fit. Each profile retains manufacturer and process notes. Product names identify the source materials. [License table and credits](https://github.com/JamesQiu2005/SpektraLab/blob/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/README.md) · [Original asset provenance](https://github.com/JamesQiu2005/SpektraLab/blob/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/resources_product/NOTICE.md)

| RS stock | Source process |
|---|---|
| kodak_tri_x_400 | 400TX, 135, D-76 to CI 0.56, Kodak F-4017. The earlier kodak_trix profile models 7266 reversal. |
| kodak_tmax_100 | 100TMX, D-76 small tank to CI 0.56, Kodak F-4016. |
| fujifilm_neopan_acros_100_ii | Acros II, D-76 for 7¼ minutes, Fujifilm AF3-0258E. |
| ilford_hp5_plus_400 | HP5 Plus, ILFOTEC HC 1+31 for 6½ minutes, HARMAN 2018 technical information. |
| ilford_multigrade_iv_rc | SpektraLab feasibility fit to HARMAN's 2019 Multigrade IV RC data. The three emulsion sensitivities are estimated. |

**RS adaptations.** Each film stores the single sensitivity, characteristic curve and CDF fit once, keeps each fitted sublayer and sums the spectral silver weights. The paper retains three independent spectral responses and development curves. Their silver densities add to the paper base. The paper carries a new RS fit of filter packs and exposure factors under the same CC BY-SA terms.

**Reproduction.** From the RS root, run `cargo run --locked -p spektrafilm-core --example fit_multigrade > grades.json`. The example uses the bundled paper curves and the engine's TH-KG3 head. It solves for yellow or magenta within 0–250 CC, targeting the log-exposure interval between net density 0.04 and 90% of maximum net density. It chooses the exposure multiplier for 0.184 linear gray. Its inputs are independent of the existing grade table. Run `python tools/import_spektralab_bw.py <LAB-checkout> grades.json --out <directory>` to regenerate the five profiles. The importer reads pinned Git objects. Neither tool runs during rendering.

| Grade | 00 | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|---:|---:|
| HARMAN IV RC ISO R | 180 | 160 | 130 | 110 | 90 | 60 | 40 |
| RS fitted range | 180 | 160 | 130 | 110 | 90 | 60 | 57.30 |

Grade 5 reaches the 250 M calibration limit. Extending this head to 1000 M yields R 57.24; the main remaining limitation is the three-emulsion fit. [Hard-grade measurements and half-step trial](../docs/ENGINE_STUDIES.md#multigrade-iv-rc-at-the-hard-end) These ranges describe the paper under projected light. A photographed wedge through a developed negative measures the full system. Physical paper speed (ISO P) and physical enlarger matching remain unverified. [HARMAN reference, page 2](https://www.ilfordphoto.com/amfile/file/download/file/1956/product/2078/) · [Offline fitter](../crates/spektrafilm-core/examples/fit_multigrade.rs)
