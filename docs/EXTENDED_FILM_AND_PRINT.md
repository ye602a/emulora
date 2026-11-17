# Film format, edge and print extension studies

The resident 135 implementation already owns a physical canvas, a compact source, a millimetre transform, separate aperture and film-presence masks, and a boundary at the retained negative. Use that boundary to decide whether a proposed control needs exposure and development again. Let `P` be the expanded pixel count. Two existing RGB f32 work planes require `24P` bytes, with `12P` for a retained RGB f32 negative when enabled. Optical scratch, encoders and output textures have their own existing costs. The following are scoped designs and acceptance cases for the independent engine.

## Edge codes, frame numbers and dates

Manufacturer edge codes and frame counters are latent exposures on the shoulder. Camera date imprinting is an optional separate exposure at a specified aperture location. Parameterize marking kind, user text or counter, physical origin, size, roll phase and exposure. Apply small glyph coverage before density development, intersected with film presence. Metadata input belongs to the caller. Changing a mark invalidates exposure and the negative. Layout, glyph shapes and an uploaded glyph atlas can serve multiple frames. The starting estimate is one bounded glyph atlas and a per-mark instance list; benchmark the extra work before allowing large layouts.

Test no-mark equality, glyph placement in both orientations, a mark crossing a perforation, correct frame-number phase, and distinction between an edge code and an in-aperture date. Documentary dimensions can guide an original design. Check author, data and font terms for any actual artwork. Manufacturer branding, copied codes and automatic metadata collection are outside the first scope. [Kodak edge marking reference](https://www.kodak.com/content/products-brochures/Film/kodak-essential-reference-guide-for-filmmakers.pdf)

## 120 roll film

Provide explicit nominal gate and strip dimensions with a roll axis, film-presence rules and a documented crop mode. Resolve geometry in millimetres before pixel placement. Keep a continuous shoulder on both sides. A gate change recomputes the layout, film presence, exposure, negative and output. Storage changes with `P`; a continuous strip needs the same small geometry uniform and no full-frame mask.

Test 6×4.5, 6×6, 6×7 and 6×9 using verified camera-specific gate dimensions, both orientations, sampling pitch at fixed film scale, and transmission outside stock. Confirm strip dimensions from the film standard and the gate from the applicable camera. Backing-paper location marks, take-up spool and authoring templates are out of scope. [ISO 732 roll-film dimensions](https://www.iso.org/standard/29302.html)

## 135 half-frame

Represent an explicit gate and strip axis independently of the raster's longest axis. The Pentax 17 specifies 17 mm along the roll and 24 mm across. Keep the 135 perforation pitch and phase tied to the roll. A different aperture or roll registration invalidates the negative. The existing canvas work planes scale with `P`, with a second frame or frame-pair expansion left for a separate mode.

Test vertical half-frame, a landscape rotation of the same gate, and a strip with two adjacent frames to establish perforation phase in the reference layout. The first implementation would render one frame. Confirm read orientation and specific camera tolerances against its documentation. [Pentax 17 film and frame format](https://www.ricoh-imaging.co.jp/english/products/pentax17/spec/)

## XPan panorama

Use the verified 24×65 mm and 24×36 mm gate variants on 135 stock. Use the same roll coordinate system with explicitly specified aperture and source-fit mode. Gate/scale changes invalidate the exposure geometry and negative. At the same pitch and margins, the panorama extends the full work-plane/negative sizes along the roll; check device extent and per-buffer size with normal engine limits.

Test the two apertures against identical pitch, perforation spacing and width, including preview extraction, full export and no-stretch behaviour. A generic wide crop and a physically wide frame have separate parameter meanings. Use dimensional data with attribution and original implementation art. No frame stitching is needed for the initial scope. [Hasselblad XPan manual](https://cdn.hasselblad.com/manuals/XPan_User_Manual/1.0.0.0/XPan---Manual.pdf)

## Spatial light leaks

Camera-side leaks add spectral exposure before development. Describe source side, position in film coordinates, spectral tint, shape, intensity, extent and deterministic seed. Evaluate analytic shapes in the existing exposure dispatch. A calibrated raster pattern would consume its own texture; scope it independently and retain reusable resources across frames. Changes invalidate exposure and the negative while leaving the layout reusable. Leakage in a scanner or print head is a separate downstream model.

Test zero-gain equality, left/right symmetry for mirrored settings, falloff and chromatic response, repeatability per seed, and absence of film density in an air region. Use independently generated templates or licensed measurements. Screen overlays and arbitrary lens flare are outside this scope. [Reference spectral exposure canvas](https://github.com/JamesQiu2005/SpektraLab/blob/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/src/shaders/vulkan/spk_overscan_body.glsl)

## Other perforations

Perforation type is a physical geometry preset covering nominal width, height, corner shape, pitch, row inset and optionally the calibrated manufacturing phase. Film presence governs the unattenuated transmission in a hole. Geometry changes invalidate the negative through the exposure mask. A limited preset set fits in the current uniform and leaves the pass schedule intact.

Test the current KS shape byte-for-byte, physical millimetre checks for any added BH/KS variation, rotation, exact raster phase, and clipping at the strip edge. Cite the dimension authority and verify terms before redistributing drawings. Avoid naming a computed shape as a measured stock unless measured. [Kodak perforation diagrams](https://www.kodak.com/content/products-brochures/Film/kodak-essential-reference-guide-for-filmmakers.pdf)

## Variable-contrast silver paper

Treat a paper grade as separate spectral paper-emulsion exposures and graded development curves. The public parameter needs a paper and process identity, grade interpolation rule, filter transmission and print exposure normalization. Changes invalidate paper exposure and downstream output. The developed negative remains reusable. Budget per-emulsion lookup and integration work in the print stage; benchmark reusing the existing work planes before adding memory.

Verify the grade order and ISO R/P reference readings for the specific paper generation, neutral-patch density through the grade range, smooth half-grades, monotonic step-wedge response and negative reuse. Keep negative development curves and scanning D50 controls as independent parameters. SpektraLab’s paper and filter fit are feasibility data; confirm attribution and applicable CC BY-SA before any data reuse. The simple monochrome stock conversion does not represent multi-emulsion paper. [HARMAN technical paper specification](https://www.ilfordphoto.com/amfile/file/download/file/1956/product/2078/) · [SpektraLab resource attribution](https://github.com/JamesQiu2005/SpektraLab/blob/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/resources_product/NOTICE.md)

## Rem-jet and back-reflection

Scope a versioned film-side back-reflection model. Candidate controls are backing type, intensity, profile, per-channel strength and physical falloff. Feed it at emulsion scatter and back-reflection before density. Model changes invalidate scattering, the negative and output; the source geometry and calibrated stock spectra remain reusable. The existing multi-bounce implementation uses two additional full f32 RGB planes when active. Add bands or passes only after a measured response and resource comparison.

Test absent/strong backing against measured PSF and halo colour across channels, fixed scene exposure, and held-out footage or stills. Track C‑41 and ECN‑2 separately from backing choice. A base-stock alias plus an anti-halation flag is useful as a prototype and gives no measured CineStill emulsion calibration. Manufacturer material establishes film provenance; an optical fit and any copied stock data need their own terms and attribution. [CineStill technical FAQ](https://cinestillfilm.com/pages/frequently-asked-questions) · [SpektraLab backing PSF implementation](https://github.com/JamesQiu2005/SpektraLab/blob/1aa6f4d8a8b01fc7e1347234d5ca7649bd944e94/engine/src/core/halation_psf.hpp)
