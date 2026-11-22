//! Calibrated output is checked against the scalar CPU implementation and
//! independently evaluated Python samples bundled with the calibration.

use serde::Deserialize;
use spektrafilm_core::calibrated_output_v1::{
    finish_exposure_table_v1, positive_rgb16_v1, positive_scan_v1, CalibratedOutputError,
    NegativeStockV1, PhysicalOutputMappingV1, PhysicalOutputOptionsV1,
    PhysicalOutputV1,
};
use spektrafilm_core::{
    params::RuntimeParams,
    pipeline::Pipeline,
    profile,
    stock_setup::{StockDefaults, StockRoute},
};
use spektrafilm_gpu::{
    output::{DisplayRegion, FilmOutput, OutputError},
    wgpu::{self, util::DeviceExt},
    wgpu_backend::WgpuBackend,
    FilmRenderOptions, SourceIdentity,
};
use spektrafilm_math::{
    image::ImageBuf,
    precision::{from_f32, to_f32},
};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

#[derive(Deserialize)]
struct Heldout {
    stock: String,
    scan: Vec<f32>,
    expected: Vec<f64>,
}

fn heldout() -> Vec<Heldout> {
    serde_json::from_str(include_str!(
        "../../../data/negative_scan_ap0_v1/heldout.json"
    ))
    .unwrap()
}

fn stock_name(stock: Option<NegativeStockV1>) -> Option<&'static str> {
    stock.map(NegativeStockV1::stock_id)
}

fn options(
    stock: Option<NegativeStockV1>,
    finish_centi_ev: i32,
    region: Option<DisplayRegion>,
) -> PhysicalOutputOptionsV1 {
    PhysicalOutputOptionsV1 {
        mapping: stock.map_or(PhysicalOutputMappingV1::EncodedSrgb, |stock| {
            PhysicalOutputMappingV1::NegativeScanAp0(stock)
        }),
        finish_centi_ev,
        region,
    }
}

// Exercise the calibration knots and the adjacent representable inputs.
#[derive(Deserialize)]
struct ScanSamples {
    base: [f64; 3],
    curves: Vec<ScanCurve>,
}

#[derive(Deserialize)]
struct ScanCurve {
    absorption: Vec<f64>,
}

fn scans(stock: Option<NegativeStockV1>) -> Vec<f32> {
    let calibration: Option<ScanSamples> = stock.map(|stock| {
        let json = match stock {
            NegativeStockV1::Pro400H => include_str!("../../../data/negative_scan_ap0_v1/pro400h.json"),
            NegativeStockV1::XTra400 => include_str!("../../../data/negative_scan_ap0_v1/xtra.json"),
        };
        serde_json::from_str(json).unwrap()
    });
    let mut values: Vec<f32> = (0..257 * 17 * 3)
        .map(|i| {
            let fraction = ((i * 7919) % 65536) as f64 / 65535.;
            calibration.as_ref().map_or(fraction as f32, |cal| {
                (cal.base[i % 3] * (1. - fraction * 1.05)) as f32
            })
        })
        .collect();
    if let Some(cal) = calibration {
        for (channel, curve) in cal.curves.iter().enumerate() {
            for (i, &absorption) in curve.absorption.iter().enumerate() {
                let scan = (cal.base[channel] * (1. - absorption)) as f32;
                for (j, sample) in [scan.next_down(), scan, scan.next_up()].into_iter().enumerate() {
                    values[(3 * i + j) * 3 + channel] = sample;
                }
            }
        }
    }
    values
}

#[test]
fn cpu_inversion_matches_independent_python_heldout() {
    for fixture in heldout() {
        let actual = positive_scan_v1(&fixture.scan, &fixture.stock, None).unwrap();
        assert_eq!(actual.len(), fixture.expected.len());
        for (actual, expected) in actual.into_iter().zip(fixture.expected) {
            assert!(
                (f64::from(actual) - expected).abs() <= 2e-6,
                "{}: {actual} vs {expected}", fixture.stock
            );
        }
    }
}

#[test]
fn cpu_finish_table_matches_v1_reference_codes() {
    let sample_codes = [0, 128, 2650, 32768, 49152, 65535];
    for (ev, expected) in [
        (-125, [0, 54, 1114, 21746, 33165, 44584]),
        (0, [0, 128, 2650, 32768, 49152, 65535]),
        (125, [0, 304, 5369, 48582, 65535, 65535]),
    ] {
        let table = finish_exposure_table_v1(ev);
        assert_eq!(sample_codes.map(|code| table[code]), expected, "{ev} centi-EV");
        assert!(table.is_sorted());
    }
    assert!(finish_exposure_table_v1(0).into_iter().eq(0..=u16::MAX));
}

#[test]
fn cpu_inversion_rejects_unversioned_stocks_invalid_data_and_cancellation() {
    assert!(matches!(
        positive_scan_v1(&[0.1; 3], "kodak_portra_400", None),
        Err(CalibratedOutputError::UnsupportedStock(_))
    ));
    assert!(matches!(
        positive_scan_v1(&[0.1; 2], "fujifilm_pro_400h", None),
        Err(CalibratedOutputError::InvalidScanLength)
    ));
    assert!(matches!(
        positive_scan_v1(&[f32::NAN, 0.1, 0.2], "fujifilm_pro_400h", None),
        Err(CalibratedOutputError::NonFiniteScan)
    ));
    let stop = AtomicBool::new(true);
    assert!(matches!(
        positive_scan_v1(&[0.1; 3], "fujifilm_pro_400h", Some(&stop)),
        Err(CalibratedOutputError::Output(OutputError::Cancelled))
    ));
    assert!(matches!(
        positive_rgb16_v1(&[0.1; 3], "fujifilm_pro_400h", 100, Some(&stop)),
        Err(CalibratedOutputError::Output(OutputError::Cancelled))
    ));
}

fn backend() -> Option<WgpuBackend> {
    let gpu = WgpuBackend::new();
    if gpu.is_none() {
        assert!(
            std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none(),
            "required calibrated GPU output regression cannot be skipped"
        );
        eprintln!("SKIP: no Vulkan device for calibrated output regression");
    }
    gpu
}

fn storage(device: &wgpu::Device, bytes: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("calibrated output test input"),
        contents: bytes,
        usage: wgpu::BufferUsages::STORAGE,
    })
}

fn input(device: &wgpu::Device, values: &[f32]) -> wgpu::Buffer {
    storage(
        device,
        &values
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect::<Vec<_>>(),
    )
}

fn complete(device: &wgpu::Device, queue: &wgpu::Queue, encoder: wgpu::CommandEncoder) {
    let submission = queue.submit([encoder.finish()]);
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
}

fn read_texture(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let pixel_bytes = match texture.format() {
        wgpu::TextureFormat::Rgba8Unorm => 4,
        wgpu::TextureFormat::Rgba16Uint => 8,
        other => panic!("unexpected test texture {other:?}"),
    };
    let row_bytes = texture.width() * pixel_bytes;
    let padded = row_bytes.div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("output assertion readback"),
        size: padded as u64 * texture.height() as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    let submission = queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = slice.get_mapped_range().unwrap();
    let bytes = mapped
        .chunks_exact(padded as usize)
        .flat_map(|row| row[..row_bytes as usize].iter().copied())
        .collect();
    drop(mapped);
    buffer.unmap();
    bytes
}

fn rgb_codes(bytes: &[u8]) -> Vec<u16> {
    bytes
        .as_chunks::<8>().0.iter()
        .flat_map(|rgba| {
            assert_eq!(u16::from_le_bytes([rgba[6], rgba[7]]), 65535);
            [
                u16::from_le_bytes([rgba[0], rgba[1]]),
                u16::from_le_bytes([rgba[2], rgba[3]]),
                u16::from_le_bytes([rgba[4], rgba[5]]),
            ]
        })
        .collect()
}

#[test]
fn gpu_matches_cpu_codes_and_retains_owned_output_textures() {
    let Some(backend) = backend() else { return };
    let (device, queue) = backend.compute_device();
    let output = PhysicalOutputV1::new(device);
    let views = [
        None,
        Some(DisplayRegion {
            crop: [11, 3, 73, 11],
            width: 73,
            height: 11,
        }),
        Some(DisplayRegion {
            crop: [5, 1, 248, 16],
            width: 31,
            height: 2,
        }),
    ];
    let mut retained = None;
    for stock in [None, Some(NegativeStockV1::Pro400H), Some(NegativeStockV1::XTra400)] {
        let scan = scans(stock);
        let buffer = input(device, &scan);
        let cpu = stock_name(stock).map_or_else(
            || scan.iter().map(|&v| (f64::from(v) * 65535. + 0.5) as u16).collect(),
            |stock| positive_rgb16_v1(&scan, stock, 0, None).unwrap(),
        );
        let allowed = if stock.is_some() { 3 } else { 0 };
        let mut base_codes = Vec::new();
        for finish in [0, -125, 125, 0] {
            let table = finish_exposure_table_v1(finish);
            let mut full = None;
            for region in views {
                let mut encoder = device.create_command_encoder(&Default::default());
                let current = output.encode(
                    device, &mut encoder,
                    FilmOutput { buffer: &buffer, width: 257, height: 17, cctf_encoding: stock.is_none() },
                    options(stock, finish, region),
                ).unwrap();
                complete(device, queue, encoder);
                let precision = read_texture(device, queue, &current.precision);
                let display = read_texture(device, queue, &current.display);
                let codes = rgb_codes(&precision);
                assert_eq!(display.len(), codes.len() / 3 * 4);
                for (rgb, rgba) in codes.as_chunks::<3>().0.iter().zip(display.as_chunks::<4>().0) {
                    assert_eq!(
                        rgba,
                        &[
                            ((u32::from(rgb[0]) + 128) / 257) as u8,
                            ((u32::from(rgb[1]) + 128) / 257) as u8,
                            ((u32::from(rgb[2]) + 128) / 257) as u8,
                            255,
                        ]
                    );
                }
                if let Some(view) = region {
                    let full = full.as_ref().unwrap();
                    let cropped = image::imageops::crop_imm(
                        full, view.crop[0], view.crop[1], view.crop[2], view.crop[3],
                    ).to_image();
                    let expected = image::imageops::thumbnail(&cropped, view.width, view.height);
                    assert_eq!(&codes, expected.as_raw(), "{stock:?} finish {finish} {view:?}");
                } else {
                    assert_eq!(codes.len(), cpu.len());
                    if finish == 0 {
                        for (index, (&gpu, &cpu)) in codes.iter().zip(&cpu).enumerate() {
                            assert!(
                                gpu.abs_diff(cpu) <= allowed,
                                "{stock:?} channel {index}: GPU {gpu}, CPU {cpu}"
                            );
                        }
                        base_codes.clone_from(&codes);
                    } else {
                        for (&gpu, &base) in codes.iter().zip(&base_codes) {
                            assert_eq!(gpu, table[usize::from(base)], "{stock:?} finish {finish} base code {base}");
                        }
                    }
                    full = Some(image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::from_raw(257, 17, codes).unwrap());
                }
                if retained.is_none() {
                    retained = Some((current, precision, display));
                }
            }
        }
    }
    let (first, expected16, expected8) = retained.unwrap();
    assert_eq!(read_texture(device, queue, &first.precision), expected16);
    assert_eq!(read_texture(device, queue, &first.display), expected8);
}

#[test]
fn gpu_heldout_tiny_outputs_and_invalid_source_contracts_are_explicit() {
    let Some(backend) = backend() else { return };
    let (device, queue) = backend.compute_device();
    let output = PhysicalOutputV1::new(device);
    for fixture in heldout() {
        let stock = NegativeStockV1::try_from(fixture.stock.as_str()).unwrap();
        let buffer = input(device, &fixture.scan);
        let width = fixture.scan.len() as u32 / 3;
        for region in [
            None,
            Some(DisplayRegion {
                crop: [7, 0, 1, 1],
                width: 1,
                height: 1,
            }),
        ] {
            let mut encoder = device.create_command_encoder(&Default::default());
            let frame = output
                .encode(
                    device,
                    &mut encoder,
                    FilmOutput {
                        buffer: &buffer,
                        width,
                        height: 1,
                        cctf_encoding: false,
                    },
                    options(Some(stock), 0, region),
                )
                .unwrap();
            complete(device, queue, encoder);
            let codes = rgb_codes(&read_texture(device, queue, &frame.precision));
            let expected = match region {
                None => fixture.expected.as_slice(),
                Some(_) => &fixture.expected[21..24],
            };
            let max = codes
                .iter()
                .zip(expected)
                .map(|(a, b)| a.abs_diff((b * 65535. + 0.5) as u16))
                .max()
                .unwrap();
            assert!(max <= 3, "{} heldout GPU code error {max}", fixture.stock);
        }
    }
    let buffer = input(device, &[0.1, 0.2, 0.3]);
    for (mapping, encoded) in [
        (PhysicalOutputMappingV1::EncodedSrgb, false),
        (
            PhysicalOutputMappingV1::NegativeScanAp0(NegativeStockV1::Pro400H),
            true,
        ),
    ] {
        let mut encoder = device.create_command_encoder(&Default::default());
        assert!(matches!(
            output.encode(
                device,
                &mut encoder,
                FilmOutput {
                    buffer: &buffer,
                    width: 1,
                    height: 1,
                    cctf_encoding: encoded,
                },
                PhysicalOutputOptionsV1 {
                    mapping,
                    ..Default::default()
                }
            ),
            Err(CalibratedOutputError::InvalidTransport)
        ));
    }
    for (region, error) in [
        (
            DisplayRegion {
                crop: [0, 0, 1, 1],
                width: 0,
                height: 1,
            },
            OutputError::EmptyDimensions,
        ),
        (
            DisplayRegion {
                crop: [1, 0, 1, 1],
                width: 1,
                height: 1,
            },
            OutputError::CropOutOfBounds,
        ),
        (
            DisplayRegion {
                crop: [0, 0, 1, 1],
                width: 2,
                height: 1,
            },
            OutputError::EnlargementUnsupported,
        ),
    ] {
        let mut encoder = device.create_command_encoder(&Default::default());
        assert!(matches!(output.encode(device, &mut encoder, FilmOutput {
            buffer: &buffer, width: 1, height: 1, cctf_encoding: true,
        }, options(None, 0, Some(region))), Err(CalibratedOutputError::Output(actual)) if actual == error));
    }
    let mut encoder = device.create_command_encoder(&Default::default());
    assert!(matches!(
        output.encode(
            device,
            &mut encoder,
            FilmOutput {
                buffer: &buffer,
                width: 2,
                height: 1,
                cctf_encoding: true,
            },
            options(None, 0, None)
        ),
        Err(CalibratedOutputError::Output(OutputError::InvalidImageData))
    ));
}

fn physical_pipeline(stock: &str, route: StockRoute) -> Pipeline {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut params = RuntimeParams::default();
    params.camera.auto_exposure = false;
    params.camera.exposure_compensation_ev = 0.0;
    params.camera.film_format_mm = 36.0;
    params.enlarger.print_exposure_compensation = false;
    params.enlarger.normalize_print_exposure = true;
    params.enlarger.print_exposure = 1.0;
    params.settings.neutral_print_filters_from_database = true;
    params.io.input_color_space = "sRGB".into();
    params.io.input_cctf_decoding = false;
    params.io.output_color_space = "sRGB".into();
    params.io.output_cctf_encoding = true;
    params.film_render.grain.active = true;
    params.film_render.grain.sublayers_active = true;
    params.film_render.grain.gpu_multilayer = true;
    params.film_render.dir_couplers.active = true;
    params.camera.diffusion_filter.active = false;
    if route == StockRoute::NegativeScan {
        params.io.scan_film = true;
        params.film_render.glare.active = false;
        params.io.output_color_space = "ACES2065-1".into();
        params.io.output_cctf_encoding = false;
        params.io.output_gamut_compress.algorithm = "off".into();
        params.scanner.unsharp_mask = [0.0, 0.0];
    }
    Pipeline::new_with_spectral_and_stock_defaults(
        profile::load_profile_by_name(&data, stock).unwrap(),
        profile::load_profile_by_name(&data, "kodak_portra_endura").unwrap(),
        params,
        &data,
        StockDefaults::Physical(route),
    )
    .unwrap()
}

#[test]
fn complete_render_propagates_failures_before_cache_commit_and_matches_negative_cpu_inversion() {
    let Some(backend) = backend() else { return };
    let (device, queue) = backend.compute_device();
    let output = PhysicalOutputV1::new(device);
    backend.configure_negative_cache(true);
    backend.configure_work_images(true);
    let mut image = ImageBuf::new(17, 11);
    for y in 0..image.height {
        for x in 0..image.width {
            let value = 0.02 + ((x * 7 + y * 19) % 97) as f32 / 120.0;
            image.set(x, y, [value, value * 0.81, value * 0.63].map(from_f32));
        }
    }
    let pipeline = physical_pipeline("kodak_portra_400", StockRoute::Paper);
    pipeline
        .with_resident_params(&image, |params| {
            let render = FilmRenderOptions {
                source: Some(SourceIdentity(0xCA11_BA7E)),
                ..Default::default()
            };
            let before = backend.negative_cache_stats();
            assert_eq!(before.resident_bytes, 0);
            // This fails inside encode, after a cold density lookup and physical
            // command recording. The wrapper must return outer Err, not Ok(Err).
            let invalid = output.render(
                &backend,
                params,
                render,
                options(
                    None,
                    0,
                    Some(DisplayRegion {
                        crop: [17, 0, 1, 1],
                        width: 1,
                        height: 1,
                    }),
                ),
            );
            assert!(matches!(
                invalid,
                Err(CalibratedOutputError::Output(OutputError::CropOutOfBounds))
            ));
            let failed = backend.negative_cache_stats();
            assert_eq!(failed.misses, before.misses + 1);
            assert_eq!(failed.stores, before.stores);
            assert_eq!(failed.resident_bytes, 0);

            assert!(matches!(
                output.render(
                    &backend,
                    params,
                    render,
                    options(Some(NegativeStockV1::Pro400H), 0, None)
                ),
                Err(CalibratedOutputError::InvalidTransport)
            ));
            assert_eq!(backend.negative_cache_stats().stores, before.stores);
            assert_eq!(backend.negative_cache_stats().resident_bytes, 0);

            let stop = AtomicBool::new(true);
            assert!(matches!(
                output.render(
                    &backend,
                    params,
                    FilmRenderOptions {
                        cancellation: Some(&stop),
                        ..render
                    },
                    options(None, 0, None)
                ),
                Err(CalibratedOutputError::Output(OutputError::Cancelled))
            ));
            assert_eq!(backend.negative_cache_stats().stores, before.stores);
            assert_eq!(backend.negative_cache_stats().resident_bytes, 0);

            let first = output
                .render(&backend, params, render, options(None, 0, None))
                .unwrap();
            let stored = backend.negative_cache_stats();
            assert_eq!(stored.stores, before.stores + 1);
            assert_eq!(stored.resident_bytes, image.pixel_count() as u64 * 12);
            let again = output
                .render(&backend, params, render, options(None, 0, None))
                .unwrap();
            assert_eq!(backend.negative_cache_stats().hits, stored.hits + 1);
            assert_eq!(
                read_texture(device, queue, &first.precision),
                read_texture(device, queue, &again.precision)
            );
        })
        .unwrap();

    backend.configure_negative_cache(false);
    let negative = physical_pipeline("fujifilm_pro_400h", StockRoute::NegativeScan);
    negative
        .with_resident_params(&image, |params| {
            assert!(params.scan_film);
            assert!(!params.output_cctf_encoding);
            assert!(params.grain.as_ref().unwrap().layers.is_some());
            let scan = backend
                .run_film_chain_region(params, FilmRenderOptions::default(), None)
                .unwrap();
            let scan: Vec<f32> = scan.data.into_iter().map(to_f32).collect();
            let expected = positive_scan_v1(&scan, "fujifilm_pro_400h", None).unwrap();
            let expected: Vec<u16> = expected
                .iter()
                .map(|v| (*v as f64 * 65535. + 0.5) as u16)
                .collect();
            let frame = output
                .render(
                    &backend,
                    params,
                    FilmRenderOptions::default(),
                    options(Some(NegativeStockV1::Pro400H), 0, None),
                )
                .unwrap();
            let actual = rgb_codes(&read_texture(device, queue, &frame.precision));
            assert_eq!(actual.len(), expected.len());
            let max = actual
                .iter()
                .zip(&expected)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(
                max <= 3,
                "complete physical negative route exceeded original CPU/GPU 3-code envelope: {max}"
            );
            assert!(
                actual.iter().any(|v| *v != actual[0]),
                "negative physical fixture must be nonuniform"
            );
        })
        .unwrap();
}
