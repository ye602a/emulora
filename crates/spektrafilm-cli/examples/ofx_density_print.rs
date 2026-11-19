//! Diagnostic replay of the Portra 400 / Endura optical print from developed CMY.
//! Input is the interleaved little-endian f64 format of SPEKTRAFILM_DUMP_FILM_DENSITY.
use anyhow::{Result, ensure};
use clap::Parser;
use spektrafilm_core::{
    color_reference::ColorReference, gamut_compression::OutputGamutCompress, params::RuntimeParams,
    pipeline::Pipeline, profile::load_profile_by_name, stages,
};
use spektrafilm_gpu::cpu_backend::CpuBackend;
use spektrafilm_math::{image::ImageBuf, precision::from_f64};
use std::{fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    recipe: PathBuf,
    density: PathBuf,
    width: u32,
    height: u32,
    output: PathBuf,
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bytes = fs::read(args.density)?;
    ensure!(
        args.width != 0
            && args.height != 0
            && (args.width as u64 * args.height as u64).checked_mul(24) == Some(bytes.len() as u64),
        "expected three f64 density channels per pixel"
    );
    let mut samples = Vec::with_capacity(bytes.len() / 8);
    for bytes in bytes.chunks_exact(8) {
        let value = f64::from_le_bytes(bytes.try_into()?);
        ensure!(value.is_finite(), "nonfinite density");
        samples.push(from_f64(value));
    }
    let negative = ImageBuf::from_data(args.width, args.height, samples);
    let params: RuntimeParams = serde_json::from_slice(&fs::read(args.recipe)?)?;
    let film = load_profile_by_name(&args.data_dir, "kodak_portra_400")?;
    let paper = load_profile_by_name(&args.data_dir, "kodak_portra_endura")?;
    let p = Pipeline::new_with_spectral(film, paper, params, &args.data_dir)
        .map_err(anyhow::Error::msg)?;
    let light = p.print_illuminant_slice();
    let gain = p.print_exposure_factor();
    let reference = ColorReference::compute(&p.film, &p.print, &p.params, light, gain);
    let paper = stages::printing::process_with_calibration(
        &negative,
        &p.film,
        &p.print,
        &p.params,
        &CpuBackend,
        light,
        gain,
        p.preflash_raw(),
        reference.printing_exposure_correction,
    );
    let gamut = OutputGamutCompress::build(
        &p.params.io.output_gamut_compress,
        &p.params.io.output_color_space,
    );
    let rgb =
        stages::scanning::process(&paper, &p.print, &p.params, &CpuBackend, &reference, &gamut);
    let output: Vec<_> = rgb
        .data
        .iter()
        .flat_map(|&v| (v as f64).to_le_bytes())
        .collect();
    fs::write(args.output, output)?;
    Ok(())
}
