//! File input shared by the standalone photo CLI and GUI.
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use spektrafilm_math::{
    image::ImageBuf,
    precision::{from_f32, srgb_decode},
};
use std::path::Path;

pub const RAW_EXTENSIONS: &[&str] = &[
    "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw",
    "x3f", "iiq", "3fr", "crw", "rwl", "mrw", "mef", "kdc", "ari", "bay", "dcr", "drf", "erf",
    "fff", "k25", "mos", "ptx",
];
pub const RASTER_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "tif", "tiff"];

pub enum InputPrimaries {
    Srgb,
    /// TIFF samples retain the linear primaries declared by the caller.
    FromRecipe,
}

pub struct PhotoInput {
    pub pixels: ImageBuf,
    pub primaries: InputPrimaries,
}

pub fn load_photo(path: &Path) -> Result<PhotoInput> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if RAW_EXTENSIONS.contains(&ext.as_str()) {
        return Ok(PhotoInput {
            pixels: load_raw(path)?,
            primaries: InputPrimaries::Srgb,
        });
    }
    if !RASTER_EXTENSIONS.contains(&ext.as_str()) {
        bail!("unsupported photo input: {}", path.display());
    }
    let mut reader = image::ImageReader::open(path)
        .with_context(|| format!("opening {}", path.display()))?
        .with_guessed_format()?;
    let primaries = match reader.format() {
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png) => InputPrimaries::Srgb,
        Some(image::ImageFormat::Tiff) => InputPrimaries::FromRecipe,
        _ => bail!("expected a JPEG, PNG or TIFF image: {}", path.display()),
    };
    // Full-resolution float TIFFs can exceed image's default 512 MiB cap.
    reader.no_limits();
    let rgb = reader
        .decode()
        .with_context(|| format!("decoding {}", path.display()))?
        .into_rgb32f();
    let (width, height) = rgb.dimensions();
    let samples = rgb.into_raw().into_par_iter();
    let data = match primaries {
        InputPrimaries::Srgb => samples.map(|v| srgb_decode(from_f32(v))).collect(),
        InputPrimaries::FromRecipe => samples.map(from_f32).collect(),
    };
    Ok(PhotoInput {
        pixels: ImageBuf::from_data(width, height, data),
        primaries,
    })
}

/// Develop into linear sRGB, keeping the standalone rawler settings.
pub fn load_raw(path: &Path) -> Result<ImageBuf> {
    use rawler::{
        decode_file,
        imgop::develop::{ProcessingStep, RawDevelop},
    };
    let raw = decode_file(path).map_err(|e| anyhow::anyhow!("RAW decode failed: {e:?}"))?;
    let mut dev = RawDevelop::default();
    // Drop the sRGB gamma step — we want linear sRGB primaries; the
    // spektrafilm pipeline applies its own sRGB OETF at the very end.
    dev.steps.retain(|s| !matches!(s, ProcessingStep::SRgb));
    let intermediate = dev
        .develop_intermediate(&raw)
        .map_err(|e| anyhow::anyhow!("RAW develop failed: {e:?}"))?;
    rawler_intermediate_to_image_buf(intermediate)
}

fn rawler_intermediate_to_image_buf(
    intermediate: rawler::imgop::develop::Intermediate,
) -> Result<ImageBuf> {
    use rawler::imgop::develop::Intermediate;

    let floor = |v: f32| spektrafilm_math::precision::from_f32(v.max(0.0));
    match intermediate {
        Intermediate::Monochrome(pixels) => {
            let scalars: Vec<spektrafilm_math::precision::Scalar> = pixels
                .data
                .par_iter()
                .flat_map_iter(|&v| {
                    let v = floor(v);
                    [v, v, v]
                })
                .collect();
            Ok(ImageBuf::from_data(
                pixels.width as u32,
                pixels.height as u32,
                scalars,
            ))
        }
        Intermediate::ThreeColor(pixels) => {
            let scalars: Vec<spektrafilm_math::precision::Scalar> = pixels
                .data
                .par_iter()
                .flat_map_iter(|px| [floor(px[0]), floor(px[1]), floor(px[2])])
                .collect();
            Ok(ImageBuf::from_data(
                pixels.width as u32,
                pixels.height as u32,
                scalars,
            ))
        }
        Intermediate::FourColor(pixels) => {
            let scalars: Vec<spektrafilm_math::precision::Scalar> = pixels
                .data
                .par_iter()
                .flat_map_iter(|px| [floor(px[0]), floor(px[1]), floor(px[2])])
                .collect();
            Ok(ImageBuf::from_data(
                pixels.width as u32,
                pixels.height as u32,
                scalars,
            ))
        }
    }
}
