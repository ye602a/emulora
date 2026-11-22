# 135 film border geometry

Emulora's optional `RuntimeParams.border_135` contains integer overscan in source pixels, in left, top, right, bottom order. Omitting the border selects the original path. The legacy settings also bypass it with zero overscan. Explicit gates and active latent exposures can render on an unexpanded canvas. The aperture contains the input raster at native resolution. Extending the canvas adds pixels around the input. With the default gate the physical pixel pitch is the camera film-format scale divided by the longer content dimension.

The resolved layout contains the content rectangle in the film canvas and an inverse scan transform. The transform samples pixel centres and returns along-roll and across-roll millimetres from the aperture centre. Along-roll runs rightward for landscape and downward for portrait. Positive scan offsets move the film right/down in the raster. Positive centidegree rotation moves it clockwise around the aperture centre. Registration moves the physical aperture and perforation pattern, while the source samples keep their addresses.

The basic aperture follows the input aspect ratio. Each shoulder extends 5.5 mm across the roll from the aperture, corresponding to a 35 mm strip at a 24 mm aperture. Other crops retain their physical content pitch and the same shoulder width. Rounded holes use 2.794×1.981 mm geometry, a 0.51 mm radius and a 4.75 mm pitch. The hole centres lie 2 mm from the simulated edge and repeat symmetrically along the roll. These proportions describe a basic 135 border. [Kodak perforation dimensions](https://www.kodak.com/content/products-brochures/Film/kodak-essential-reference-guide-for-filmmakers.pdf) · [ISO 1007](https://www.iso.org/standard/28855.html)

Film presence and the camera aperture are separate masks. The film is present between the two shoulders, with perforations removed. The rectangular aperture identifies where the exposure can contain source pixels. At 1:1, `Layout135::content_region()` describes the original content rectangle in the same `DisplayRegion` coordinates used for final output. A caller can select the full canvas or an explicit output crop/reduction.

| Change | Dependency |
| --- | --- |
| Input dimensions, film-format scale, overscan, scan registration | Layout, film presence and aperture, exposure, negative and its downstream output |
| Border exposure | Exposure/negative and output using the existing layout |
| Source pixels | Exposure/content, negative and downstream output using the existing layout |
| Grain seed | Grain/negative and downstream output. Keep the existing seed and source coordinates for content samples. |
| Print/scanner parameters or output region | Their existing downstream stages, with reusable layout and negative where applicable |

The wgpu resident pipeline reads the compact source through the aperture during RGB-to-spectral exposure. The shoulder receives zero scene light by default or neutral scene-linear exposure `0.18 × 2^(EV)` when `edge_exposure_centi_ev` is supplied. The local aperture and source placement both govern which pixels receive the photograph. The density calculation uses the content pixel pitch, and diffusion covers the expanded canvas. The existing grain seed is preserved. When the caller supplies a source grain reference, its translation is adjusted for content placement in the canvas.

Print exposure and direct film scanning use film presence when computing spectral transmission. A perforation or a pixel outside the strip has zero optical density, including base density. Subsequent print and scanner stages run normally on the transmitted image. Expect physical scatter, glare and sharpening to influence pixels near a transition.

Use the checked resident API for border rendering with an output region in canvas coordinates. For ordinary full-image output, `Pipeline::try_process` also checks the geometry and reports an error when the active backend cannot render the requested border. The wgpu implementation grows the existing work planes to the canvas dimensions, uploads the compact raster once, and binds one small geometry uniform to the affected kernels. It uses the original kernels' dispatch schedule and no separate image-sized mask. The retained negative depends on the resolved border state. Physical borders require the wgpu pipeline. For the supported half-frame, XPan and 120 gates, edge imprints and local light leaks, see [explicit roll formats and latent exposures](#explicit-roll-formats-and-latent-exposures).


## Explicit roll formats and latent exposures

The resident Vulkan route accepts these fields through the regular CLI photo recipe and Rust API. This example uses a 65 × 24 mm aperture with a horizontal roll, a shoulder imprint and a local light leak.

```json
{
  "border_135": {
    "overscan": [16, 72, 16, 72],
    "gate": {"format": "xpan65", "axis": "horizontal"},
    "imprint": {"text": "FRAME-29", "origin_tenth_mm": [-150, 125], "height_tenth_mm": 15, "exposure_milli": [350, 350, 350]},
    "light_leak": {"center_tenth_mm": [-310, 15], "sigma_tenth_mm": [55, 120], "exposure_milli": [250, 40, 5]}
  }
}
```

Select `half_frame17` (17 × 24 mm), `full_frame36` (36 × 24 mm), or `xpan65` (65 × 24 mm) for perforated 35 mm stock. For an unperforated 120 back, set `format` to e.g. `{"roll120":{"along_tenth_mm":415,"across_tenth_mm":560}}`. The example matches the Pentax 645 picture size. Set the gate to the dimensions of the particular back. The strip is 61.5 mm across. Both `horizontal` and `vertical` refer to the roll direction before scan registration. [Pentax 645 manual](https://www.pentax-manuals.com/manuals/medformat/645.pdf) · [Fujifilm Professional Data Guide, §6-3](https://asset.fujifilm.com/www/us/files/2020-03/85d928f44b0df3b2a95913e46608881d/ProfessionalFilmDataGuide.pdf)

The source covers the centred aperture with square pixels. A mismatched aspect ratio therefore crops the photograph within the existing canvas. Output dimensions are source dimensions plus overscan. For a tight framed export, crop the source to the gate aspect first. The resolved pitch also drives the resident lens, diffusion, halation, DIR and grain calculations. No additional image plane is allocated.

The optional imprint accepts 1–16 uppercase ASCII letters, digits, spaces or dashes. Place its origin on the shoulder for a stock code or frame number, or inside the aperture for a camera date. The caller supplies the text. The original 3 × 5 glyphs use point sampling, so fine type can disappear at preview resolution. The leak is an elliptical Gaussian in roll coordinates. Both add linear exposure directly to the three emulsion channels after optics and before density development. A gain of 1000 adds one raw exposure unit to that layer at peak coverage. These are authored exposures without stock-specific spectral measurements. Holes and air receive neither exposure. Nonzero settings change the negative identity. Print settings and output crops still reuse the negative. The CLI's pointwise LUT omits the entire spatial border.

The Windows Vulkan regression compared aperture, shoulder and air pixels against the CPU geometry for seven gates in both orientations. Each geometry change missed the retained negative. The 120 trials used 41.5, 56, 70 and 84 mm along the roll and 56 mm across; the last three are test dimensions for configurable backs. Marking tests cross the perforation row, include a date on an unexpanded canvas, check zero-gain equality and mirror an active leak pixel-for-pixel. A subsequent print adjustment hit the retained negative. The standalone CLI also rendered the same 600 × 900 portrait through half-frame, XPan and the 41.5 × 56 mm back into three 632 × 1044 PNGs. These are geometry and electronic render checks; physical film/print comparisons are still outstanding.
