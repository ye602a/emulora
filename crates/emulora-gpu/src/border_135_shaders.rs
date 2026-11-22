// Both variants share the original physical shaders. The border variant
// adds one uniform with the geometry resolved by the core pipeline.
macro_rules! bordered {
    ($shader:literal, $binding:literal, $input:expr) => {
        concat!(
            include_str!($shader),
            include_str!("../../emulora-shaders/wgsl/film_border_135.wgsl"),
            "@group(0) @binding(",
            $binding,
            ") var<uniform> film_border: FilmBorderParams;\n",
            $input,
        )
    };
}

pub(super) const HANATOS: &str = concat!(
    include_str!("../../emulora-shaders/wgsl/hanatos2025_rgb_to_raw.wgsl"),
    include_str!("../../emulora-shaders/wgsl/compact_exposure_input.wgsl"),
);
pub(super) const HANATOS_BORDER: &str = bordered!(
    "../../emulora-shaders/wgsl/hanatos2025_rgb_to_raw.wgsl",
    "4",
    include_str!("../../emulora-shaders/wgsl/bordered_exposure_input.wgsl")
);

pub(super) const MALLETT: &str = concat!(
    include_str!("../../emulora-shaders/wgsl/mallett_rgb_to_raw.wgsl"),
    include_str!("../../emulora-shaders/wgsl/compact_exposure_input.wgsl"),
);
pub(super) const MALLETT_BORDER: &str = bordered!(
    "../../emulora-shaders/wgsl/mallett_rgb_to_raw.wgsl",
    "3",
    include_str!("../../emulora-shaders/wgsl/bordered_exposure_input.wgsl")
);

pub(super) const PRINT: &str = concat!(
    include_str!("../../emulora-shaders/wgsl/print_spectral.wgsl"),
    include_str!("../../emulora-shaders/wgsl/negative_bleach.wgsl"),
    include_str!("../../emulora-shaders/wgsl/full_film_coverage.wgsl"),
);
pub(super) const PRINT_BORDER: &str = bordered!(
    "../../emulora-shaders/wgsl/print_spectral.wgsl",
    "7",
    include_str!("../../emulora-shaders/wgsl/negative_bleach.wgsl")
);

pub(super) const SCAN: &str = concat!(
    include_str!("../../emulora-shaders/wgsl/scan_spectral.wgsl"),
    include_str!("../../emulora-shaders/wgsl/negative_bleach.wgsl"),
    include_str!("../../emulora-shaders/wgsl/full_film_coverage.wgsl"),
);
pub(super) const SCAN_BORDER: &str = bordered!(
    "../../emulora-shaders/wgsl/scan_spectral.wgsl",
    "9",
    include_str!("../../emulora-shaders/wgsl/negative_bleach.wgsl")
);

// The last entry is sliced away when this shader uses the ordinary canvas.
pub(super) fn uniform_entry<'a>(
    border: Option<&'a wgpu::Buffer>,
    fallback: &'a wgpu::Buffer,
    binding: u32,
) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry {
        binding,
        resource: border.unwrap_or(fallback).as_entire_binding(),
    }
}

pub(super) const LOG_RAW: &str = concat!(
    include_str!("../../emulora-shaders/wgsl/log10_inplace.wgsl"),
    "fn add_latent_exposure(pixel: u32) {}",
);
pub(super) const LATENT_LOG_RAW: &str = bordered!(
    "../../emulora-shaders/wgsl/log10_inplace.wgsl",
    "2",
    include_str!("../../emulora-shaders/wgsl/latent_exposure.wgsl")
);
