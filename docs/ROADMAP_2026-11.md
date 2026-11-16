# SpektraFilm-RS Engine Roadmap — 2026-11

Date: 2026-11-16

## Position
SpektraFilm-RS remains an independent, reusable Rust film simulation engine. Continue evolving its spectral exposure, development, inter-layer effects, diffusion/halation, grain, printing and scanning pipeline. Do not replace the physical pipeline with a LUT engine or maintain a second implementation of the same algorithms.

Engineering priorities:
- Clear GPU resource ownership, pooling and reuse.
- Traceable processing graph, precise invalidation and cache hits.
- Avoid unnecessary CPU↔GPU transfers, intermediate readbacks, full-frame copies and repeated allocations.
- Performance work must not silently change accepted image response.
- Intentional model changes require explicit versioning and reproducibility.
- Track provenance and licenses separately for code, measured data and derived resources.

## Current capability and gaps
The engine already has spectral film/print profiles, spectral reconstruction, exposure/development, DIR effects, diffusion/halation, GPU multilayer physical grain, physical format scaling, printing and scanning.
The current repository contains approximately 22 exposure stocks and 9 print stocks. Counts are not a quality metric: distinguish unique emulsions, push variants, aliases, photographic paper and cinema print stock.
A real overscan/film-gate/perforation/edge-exposure canvas is not currently implemented. Physical format scale and output crop are not film borders.

## Recommended sequence
1. Recover outstanding correctness/performance test evidence.
2. Run a narrow blur final-axis accumulation fusion experiment.
3. Define and implement basic 135 film-gate geometry and exposure/perforations.
4. Validate a small selection of existing stocks and stock combinations.
5. Evaluate conversion of additional monochrome profile data.
6. Quantify CPU/GPU diffusion model differences separately.
7. Keep grain statistics and illuminant normalization as offline research.
8. Consider edge printing, dates, 120, complex leaks and variable-contrast paper later.

See [development tasks](DEVELOPMENT_TASKS_2026-11.md) and [research notes](RESEARCH_NOTES_2026-11.md).
