# 135 film border geometry

The optional `RuntimeParams.border_135` contains integer overscan in source pixels, in left, top, right, bottom order. Omitting it or using four zeros selects the existing canvas. The aperture contains the input raster at native resolution. Extending the canvas adds pixels around the input. The physical pixel pitch is still the camera film-format scale divided by the longer content dimension.

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

Use the checked resident API for border rendering with an output region in canvas coordinates. For ordinary full-image output, `Pipeline::try_process` also checks the geometry and reports an error when the active backend cannot render the requested border. The wgpu implementation grows the existing work planes to the canvas dimensions, uploads the compact raster once, and binds one small geometry uniform to the affected kernels. It uses the original kernels' dispatch schedule and no separate image-sized mask. The retained negative depends on the resolved border state. CUDA and CPU rendering of physical borders remain outside this implementation. Edge printing, alternate perforations, additional film formats and lighting effects belong to separate scopes.
