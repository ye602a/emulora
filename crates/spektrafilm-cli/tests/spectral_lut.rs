//! Exercise the actual CUBE and photo commands with independently prepared
//! pointwise photo settings. The chart visits grid nodes in a different order.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use image::{ImageBuffer, Rgb};
use serde_json::{Value, json};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn run(dir: &Path, backend: &str, args: &[&str], data: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_spektrafilm"))
        .current_dir(dir)
        .args(args)
        .arg("--data-dir")
        .arg(data)
        .env("SPEKTRAFILM_BACKEND", backend)
        .env("RUST_LOG", "error")
        .output()
        .unwrap()
}

fn success(out: Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn recipe(scan: bool) -> Value {
    json!({
        "settings": {"rgb_to_raw_method": if scan {"mallett2019"} else {"hanatos2025"}},
        "camera": {"auto_exposure": false, "exposure_compensation_ev": 0.75},
        "film_render": {"grain": {"active": false}, "glare": {"active": false},
            "halation": {"active": false}, "dir_couplers": {"diffusion_size_um": 0, "diffusion_tail_um": 0, "diffusion_tail_weight": 0}},
        "print_render": {"glare": {"active": false}, "density_curves_morph": {"active": true, "gamma_factor_blue": 1.4, "developer_exhaustion": 0.2}},
        "enlarger": {"preflash_exposure": 0.04, "y_filter_shift": 9, "print_exposure": 0.9},
        "scanner": {"unsharp_mask": [0,0]},
        "io": {"scan_film": scan, "input_color_space": "sRGB", "output_cctf_encoding": !scan}
    })
}

fn check_nodes(backend: &str) {
    let scratch = tempfile::Builder::new()
        .prefix("spectral 胶片 ")
        .tempdir()
        .unwrap();
    let dir = scratch.path();
    let chart = ImageBuffer::from_fn(9, 3, |x, y| {
        let n = (7 * (y * 9 + x) + 11) % 27;
        Rgb([n % 3, n / 3 % 3, n / 9].map(|v| v as f32 / 2.0))
    });
    chart.save(dir.join("chart.tif")).unwrap();
    for (film, scan) in [("kodak_portra_400", false), ("kodak_ektachrome_100", true)] {
        let mut reference = recipe(scan);
        reference["camera"]["filter_uv"] = json!([0.7, 430, 10]);
        reference["camera"]["filter_ir"] = json!([0.8, 650, 15]);
        fs::write(dir.join("photo.json"), reference.to_string()).unwrap();
        let mut look = reference;
        look["camera"]["auto_exposure"] = true.into();
        look["camera"]["lens_blur_um"] = 50.into();
        look["camera"]["diffusion_filter"] = json!({"active": true});
        look["film_render"]["halation"] = json!({"active": true, "boost_ev": 5});
        look["film_render"]["grain"]["active"] = true.into();
        look["film_render"]["glare"] = json!({"active": true, "percent": 8});
        look["print_render"]["glare"] = json!({"active": true, "percent": 8});
        look["film_render"]["dir_couplers"] =
            json!({"diffusion_size_um": 80, "diffusion_tail_um": 200});
        look["enlarger"]["diffusion_filter"] = json!({"active": true});
        look["scanner"]["unsharp_mask"] = json!([2, 2]);
        look["scanner"]["lens_blur"] = 3.into();
        look["io"]["upscale_factor"] = 2.into();
        look["border_135"] = json!({"overscan":[8,8,8,8]});
        fs::write(dir.join("bake.json"), look.to_string()).unwrap();
        success(run(
            dir,
            backend,
            &[
                "export-lut",
                "--film",
                film,
                "--params",
                "bake.json",
                "--size",
                "3",
                "-o",
                "look.cube",
            ],
            &data_dir(),
        ));
        let cube = fs::read_to_string(dir.join("look.cube")).unwrap();
        for field in [
            "DOMAIN_MIN 0 0 0",
            "DOMAIN_MAX 1 1 1",
            "LUT_3D_SIZE 3",
            "CC BY-SA",
            "Andrea Volpato",
            if scan {
                "# Route: direct film scan"
            } else {
                "# Route: optical print / kodak_portra_endura"
            },
            if scan {
                "# Output: linear sRGB"
            } else {
                "# Output: encoded sRGB"
            },
        ] {
            assert!(cube.contains(field), "{field}");
        }
        if backend == "wgpu" {
            assert!(cube.contains("wgpu (GPU)"), "required GPU was unavailable");
        }
        let rows: Vec<Vec<f64>> = cube
            .lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_digit() || c == '-'))
            .map(|l| l.split_whitespace().map(|v| v.parse().unwrap()).collect())
            .collect();
        assert_eq!(rows.len(), 27);
        success(run(
            dir,
            backend,
            &[
                "process",
                "chart.tif",
                "--film",
                film,
                "--params",
                "photo.json",
                "-o",
                "photo.tif",
                "--raw-out",
                "photo.bin",
            ],
            &data_dir(),
        ));
        let raw = fs::read(dir.join("photo.bin")).unwrap();
        assert_eq!(raw.len(), 27 * 3 * 8);
        for (i, pixel) in raw.chunks_exact(24).enumerate() {
            let index = (7 * i + 11) % 27;
            assert_eq!(rows[index].len(), 3);
            for (channel, bytes) in pixel.chunks_exact(8).enumerate() {
                let expected = f64::from_ne_bytes(bytes.try_into().unwrap());
                assert!(
                    (rows[index][channel] - expected).abs() <= 2e-6,
                    "{backend} {film} node {index}, channel {channel}: {} vs {expected}",
                    rows[index][channel]
                );
            }
        }
    }
}

#[test]
fn cube_matches_independent_cpu_photo_pixels() {
    check_nodes("cpu");
}

#[test]
fn cube_matches_required_gpu_photo_pixels() {
    if std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_some() {
        check_nodes("wgpu");
    }
}

#[test]
fn failed_requests_preserve_the_destination() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let profiles = dir.join("data/profiles");
    fs::create_dir_all(&profiles).unwrap();
    for name in ["kodak_portra_400", "kodak_portra_endura"] {
        let file = format!("{name}.json");
        fs::copy(data_dir().join("profiles").join(&file), profiles.join(file)).unwrap();
    }
    ImageBuffer::from_pixel(2, 2, Rgb([128u8; 3]))
        .save(dir.join("in.png"))
        .unwrap();
    fs::write(dir.join("out.png"), b"previous").unwrap();
    for args in [vec!["export-lut", "--size", "2"], vec!["process", "in.png"]] {
        let mut args = args;
        args.extend(["--film", "kodak_portra_400", "-o", "out.png"]);
        let out = run(dir, "cpu", &args, &dir.join("data"));
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("irradiance_xy_tc.npy"));
        assert_eq!(fs::read(dir.join("out.png")).unwrap(), b"previous");
    }
    let mut unsupported = recipe(false);
    unsupported["io"]["input_cctf_decoding"] = true.into();
    fs::write(dir.join("look.json"), unsupported.to_string()).unwrap();
    for (size, error) in [("1", "size"), ("66", "size"), ("3", "linear")] {
        let out = run(
            dir,
            "cpu",
            &[
                "export-lut",
                "--size",
                size,
                "--film",
                "kodak_portra_400",
                "--params",
                "look.json",
                "-o",
                "out.png",
            ],
            &data_dir(),
        );
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains(error));
        assert_eq!(fs::read(dir.join("out.png")).unwrap(), b"previous");
    }
    for (group, field, value, message) in [
        ("camera", "filter_uv", json!([1, 410, 0]), "filter_uv"),
        ("camera", "filter_ir", json!([1, 675, -15]), "filter_ir"),
        (
            "settings",
            "apply_hanatos2025_adaptation_surface",
            json!(true),
            "adaptation surface",
        ),
        (
            "settings",
            "spectral_gaussian_blur",
            json!(2),
            "spectral_gaussian_blur",
        ),
    ] {
        let mut settings = recipe(false);
        settings[group][field] = value;
        fs::write(dir.join("look.json"), settings.to_string()).unwrap();
        let out = run(
            dir,
            "cpu",
            &[
                "process",
                "in.png",
                "--film",
                "kodak_portra_400",
                "--params",
                "look.json",
                "-o",
                "out.png",
            ],
            &data_dir(),
        );
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains(message));
        assert_eq!(fs::read(dir.join("out.png")).unwrap(), b"previous");
    }
}
