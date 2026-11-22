# Host GPU integration

Emulora owns film/print/scanner processing, physical grain, calibrated output and
engine caches. Hosts supply prepared input, rendering choices, source identity
and cancellation signals, and own application state and final frame publication.

## Device and queue

`WgpuBackend::from_device(device, queue)` wraps matching caller-provided wgpu
handles without creating another adapter or device. `compute_device()` borrows
the handles; `new()` is the standalone convenience constructor.
`from_device_with_submission_lock(device, queue, Option<Arc<Mutex<()>>>)` also
accepts a shared host submission gate.

- The host must supply a queue belonging to the device and request the engine's
  advertised adapter features and limits when creating that device.
- Each backend has its own compiled pipeline cache. Cloned wgpu handles share
  the underlying device resources.
- All engine submissions honor the optional gate. It is held around submission
  only, outside parameter preparation, consumer encoding, explicit polling and
  readback. The resident path checks cancellation again after acquiring it.
- The host uses the same gate for coordinated surface changes and submissions,
  and must not hold it while calling a render method. Native wgpu completion
  callbacks may run inside `Queue::submit`; they must not recursively acquire
  that non-reentrant mutex. See [submission gate](GPU_SUBMISSION_GATE.md).

The lockfile resolves wgpu 30.0.1 and eframe/egui 0.37.0. Vulkan is the validated
physical-engine backend; precise large-radius grain additionally requires
`PASSTHROUGH_SHADERS`. The GUI dependency graph can enable additional platform
backends through Cargo feature unification. Their availability does not establish
physical-pipeline qualification. Standalone GUI and compute currently create
separate devices; a common dependency version alone does not establish shared-device use.

## Runtime cost

An ordinary Cargo dependency calls the engine in the host process. Passing
borrowed image data, device/queue handles and output textures needs no IPC,
serialization or intermediate CPU image. Keep resource-content verification at
installation or an explicit resource change, and retain the accepted rendering
contract independently of the implementation revision. A code revision alone
does not require changing saved rendering identities when behavior is preserved.

Resident requests perform constant-time image-layout/device-limit checks and
cooperative cancellation reads. Source identity comes from the host's immutable
prepared-input generation; Emulora does not hash the photograph. Density-cache hits
compare exact parameter tables without allocating a temporary key or uploading
the skipped front-pass LUT. Cache bypasses do not traverse those tables.

For `P` input pixels and `Q` output pixels, a miss uploads `12P` bytes of f32 RGB
(or uses the existing direct-map route). An eligible miss also captures `12P`
bytes into a GPU density snapshot; a hit skips the input upload and copies the
snapshot into a private GPU work plane. CPU f32 output reads `12Q` bytes and
packed RGB16 reads approximately `6Q`; scoped GPU output reads no image back to
the CPU. Host display/post-processing should consume those same-device textures.

Ordinary calls use one submission/completion wait. Interactive hosts may select
the existing cooperative stage policy described in
[GPU_SUBMISSION_GATE.md](GPU_SUBMISSION_GATE.md). Keep its useful cancellation
boundaries while appending final output to the physical encoder, so a separate
output submission/wait is unnecessary.

## Resources and publication

Work-plane reuse retains at most one idle pair. The separate negative-density
cache retains at most one immutable snapshot, with a default 384 MiB limit.
These are component retention limits; active requests, auxiliary buffers and
upload/readback staging are outside them. Allocation reuse never establishes
valid pixels. A density miss uploads the complete prepared input; an exact hit
copies the immutable density into the request's private plane.

`FilmRenderOptions::source` identifies immutable prepared pixels. Emulora adds the
exact pre-print stage dependencies and controls cache publication. Hosts must
change source identity when those pixels change, without maintaining a second
film-parameter invalidation table.

A scoped output consumer records into the engine's encoder after the physical
chain. It can copy or compute into its own destination without a full f32 CPU
readback. The borrowed source plane remains engine-owned and cannot be retained
for a later submission. Publish the host-owned destination only after the whole
render returns `Ok`; the callback is an encoding boundary, not completion.
Cancellation is cooperative. Submitted commands finish before resources are
released, and cancellation cannot retract GPU writes already executed.

`FilmRenderOptions::isolate_photo_caches` gives a probe private work planes and
bypasses density reuse, counters and publication tickets. It shares the device
and compiled pipelines without saving or restoring global photo-cache state.

## Interface reference

| Capability | Contract |
| --- | --- |
| Work-plane allocation reuse | [GPU_WORK_IMAGES.md](GPU_WORK_IMAGES.md) |
| Exact negative-density caching | [GPU_NEGATIVE_CACHE.md](GPU_NEGATIVE_CACHE.md) |
| Scoped GPU output, RGB16 and display regions | [GPU_OUTPUT_CONTRACT.md](GPU_OUTPUT_CONTRACT.md) |
| Calibrated negative inversion and final output | [CALIBRATED_OUTPUT_V1.md](CALIBRATED_OUTPUT_V1.md) |
| Versioned stock initialization | [Physical stock defaults](#physical-stock-defaults) |
| Isolated probes | [GPU_PROBE_ISOLATION.md](GPU_PROBE_ISOLATION.md) |
| Multilayer grain and precise Vulkan IIR | [GRAIN_MULTILAYER_MIGRATION.md](GRAIN_MULTILAYER_MIGRATION.md) |
| Resource content identity | [Bundled resources](#bundled-resources) |

The [public host example](../../crates/emulora-core/examples/host_gpu_contract.rs)
exercises paper, slide and calibrated-negative routes with bundled resources.
Current test evidence is in [VALIDATION.md](VALIDATION.md).

## Physical stock defaults

Use Pipeline::new_with_spectral_and_stock_defaults with
StockDefaults::Physical(StockRoute::Paper), Slide or NegativeScan. The route
checks the stock type. The caller sets the scan flag, paper, input and output
colour spaces, and exposure controls. The optional preset leaves the ordinary
constructors available for recipes that use material-specific defaults.

The preset selects halation radius and strength from the stock metadata
before calibration. It uses D55 as the fallback for the film reference light
and D50 for material viewing lights outside D50/D55/D65. After calibration it
applies the v1 DIR overrides to Provia 100F and Velvia 100. Other slides retain
their normal spectral constructor DIR parameters. Runtime refresh reapplies
the same stock defaults. Rebuild when the stock, paper, route or resources
change.

## Bundled resources

The [data manifest](../../data/MANIFEST.json) identifies each resource by path,
byte length and SHA-256. Verify it at installation or when selecting a new
resource set. Cache or persisted recipe identity can include the resource
revision, film, paper, stock defaults and output route. Runtime density reuse
uses the engine's prepared dependency key and the host's source generation.

### Exposure ownership and read-only latitude

A host that owns metering can set camera.auto_exposure=false, pass the recipe
camera EV once and set enlarger.print_exposure_compensation=false. A host can
also use the standalone meter and read-only film latitude probe.

For a future host pre-development auto adjustment, supply the resulting linear frame to both the film render and scene measurement. Keep the Emulora meter off. If the host has applied the gain to pixels, camera EV carries only the photographer's additional film trim. Alternatively a host can supply untouched pixels and fold a global gain into the camera trim once when using the paper route with print exposure compensation off. A local tone adjustment must travel in the pixels. The prepared source identity must change with the preprocessed pixels so a cached scene histogram or negative cannot outlive that adjustment. Manual film EV and enlarger compensation are included in the probed medium. Applying manual EV to the scene samples again would double-count it. Any deliberately enabled Emulora meter contributes only its measured auto EV to these samples.

For resident work, measure the small neutral ramp on the existing backend. The isolated probe leaves the retained photo negative and work planes alone. A host already holding its scene histogram can compare bins directly to shadow_ev and highlight_ev. A host with GPU-only final textures need not read back a full photo to use the medium boundaries.
