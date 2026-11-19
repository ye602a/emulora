// Decode a camera RAW file with the SAME rawler settings the GUI uses,
// and write the linear-sRGB result as a 32-bit float TIFF. This lets
// the parity harness drive both pipelines (Python and Rust) from the
// pixels the GUI actually sees, isolating any RAW-decoder differences
// out of the comparison.
//
// Usage:
//   decode_raw_gui <input.orf> <output.tif>
//
// Linear sRGB primaries, AlphaMode::None, no gamma. The pipeline will
// decode the input as `input_color_space = "sRGB"` with
// `input_cctf_decoding = false` (matches the GUI defaults for RAW).

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use tiff::encoder::{TiffEncoder, colortype::RGB32Float};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .ok_or_else(|| anyhow!("usage: decode_raw_gui <input.raw> <output.tif>"))?;
    let output = args
        .next()
        .ok_or_else(|| anyhow!("usage: decode_raw_gui <input.raw> <output.tif>"))?;
    let input = Path::new(&input);
    let output = Path::new(&output);

    eprintln!("[decode_raw_gui] decoding {} via rawler …", input.display());
    let image = spektrafilm_photo_input::load_raw(input)?;
    let (w, h) = (image.width, image.height);
    let floats: Vec<f32> = image
        .data
        .into_iter()
        .map(spektrafilm_math::precision::to_f32)
        .collect();
    eprintln!("[decode_raw_gui] decoded {}×{} → linear sRGB", w, h);

    let f = File::create(output).with_context(|| format!("creating {}", output.display()))?;
    let mut encoder = TiffEncoder::new(BufWriter::new(f)).context("init TiffEncoder")?;
    encoder
        .write_image::<RGB32Float>(w, h, &floats)
        .context("write TIFF image")?;
    eprintln!("[decode_raw_gui] wrote {}", output.display());
    Ok(())
}
