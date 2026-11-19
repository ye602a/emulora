// Include inside an isolated FPW workspace module (see FPW_PARITY_MIGRATION_PLAN.md).
#[cfg(test)]
#[test]
#[ignore]
fn saved_film_photo_replay() -> anyhow::Result<()> {
    use sha2::{Digest, Sha256};

    let root = PathBuf::from(std::env::var_os("FPW_REPLAY_DIR").unwrap());
    let build = std::env::var("FPW_REPLAY_BUILD")?;
    let name = std::env::var("FPW_REPLAY_PHOTO")?;
    let output = root.join(&build).join(&name);
    std::fs::create_dir_all(root.join(&build))?;
    std::fs::create_dir(&output).context("replay requires a fresh output directory")?;
    let mut config = Config::load(&root.join(format!("{build}.config.json")))?;
    config.preview_device = Some(Arc::new(
        crate::PreviewDevice::headless().context("Vulkan device required")?,
    ));
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            crate::film::release();
        }
    }
    let _release = Release;
    let source: Source =
        serde_json::from_slice(&std::fs::read(root.join(format!("{name}.source.json")))?)?;
    let recipe: Recipe =
        serde_json::from_slice(&std::fs::read(root.join(format!("{name}.json")))?)?;
    let stop = AtomicBool::new(false);
    let working = images::prepare_recipe(&config, &source, &recipe, &stop)?;
    let digest = |data: &[u8]| fpw_core::hex_digest(&Sha256::digest(data));
    let mut checks = json!({
        "working_dimensions": [working.width, working.height],
        "working_sha256": digest(&std::fs::read(&working.pixels)?),
        "recipe": digest(&serde_json::to_vec(&recipe)?)
    });
    for (label, full, region) in [
        ("fit", false, None),
        ("native", true, None),
        ("region", true, Some([1000, 1200, 512, 512])),
    ] {
        let view = images::PreviewView {
            whole_source: false,
            region,
        };
        let (preview, _) =
            images::render_preview_view(&config, &working, &recipe, &stop, full, 1280, view)?;
        let pixels = preview.read()?.to_rgb16();
        let raw = bytemuck::cast_slice(pixels.as_raw());
        checks[label] = json!({"dimensions": pixels.dimensions(), "sha256": digest(raw)});
        std::fs::write(output.join(format!("{label}.rgb16")), raw)?;
    }
    let item = replay_item(name, source, recipe, output.join("publication.jpg"));
    export_item(&config, &item, &stop)?;
    checks["jpeg_sha256"] = json!(digest(&std::fs::read(&item.destination)?));
    std::fs::write(
        output.join("checks.json"),
        serde_json::to_vec_pretty(&checks)?,
    )?;
    Ok(())
}

#[cfg(test)]
fn replay_item(name: String, source: Source, recipe: Recipe, destination: PathBuf) -> FrozenItem {
    FrozenItem {
        id: "saved-photo-replay".into(),
        photo: name,
        source,
        content_key: recipe.content_key(),
        recipe,
        output: OutputSpec::default(),
        destination,
        status: "pending".into(),
        error: None,
        published_hash: None,
    }
}

// Independently encode the captured full preview through the host publisher.
#[cfg(test)]
#[test]
#[ignore]
fn saved_preview_jpeg_comparison() -> anyhow::Result<()> {
    let root = PathBuf::from(std::env::var_os("FPW_REPLAY_DIR").unwrap());
    let build = std::env::var("FPW_REPLAY_BUILD")?;
    let name = std::env::var("FPW_REPLAY_PHOTO")?;
    let output = root.join(&build).join(&name);
    let config = Config::load(&root.join(format!("{build}.config.json")))?;
    let source = serde_json::from_slice(&std::fs::read(root.join(format!("{name}.source.json")))?)?;
    let recipe = serde_json::from_slice(&std::fs::read(root.join(format!("{name}.json")))?)?;
    let reference = tempfile::tempdir_in(&output)?;
    let item = replay_item(name, source, recipe, reference.path().join("preview.jpg"));
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("checks.json"))?)?;
    let [w, h]: [u32; 2] = serde_json::from_value(report["native"]["dimensions"].clone())?;
    let bytes = std::fs::read(output.join("native.rgb16"))?;
    let frame = image::ImageBuffer::from_raw(
        w,
        h,
        bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>(),
    )
    .context("invalid RGB16 frame")?;
    drop(bytes);
    images::publish_jpeg(
        &config,
        &item,
        &AtomicBool::new(false),
        || Ok(image::DynamicImage::ImageRgb16(frame).into_rgb8()),
        |_| Ok(()),
    )?;
    anyhow::ensure!(
        std::fs::read(item.destination)? == std::fs::read(output.join("publication.jpg"))?,
        "encoded native preview differs from frozen export"
    );
    Ok(())
}
