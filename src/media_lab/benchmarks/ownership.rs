//! Explicit native ownership transitions. Runs only in the isolated lifecycle worker.
use anyhow::{Context as _, Result, ensure};
use gpui::{prelude::*, *};
use gpui_apple::{custom_draw::MetalCustomContext, metal_renderer::MetalHeadlessRenderer};
use gpui_apple::metal::{MTLCommandBufferStatus, MTLCommandBuffer as _, MTLCommandQueue as _, MTLDevice as _, MTLSharedEvent as _};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CFRetained;
use objc2_core_video::CVPixelBuffer;
use serde_json::{Value, json};
use std::{
    any::Any,
    borrow::Cow,
    path::Path as FilePath,
    rc::Rc,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use xgpui_shader::{SolidColor, Gradient, GradientStop, MeshGradient, MeshGrid};
use xgpui_media::{MediaImage, MediaLoader, UploadStatus};

const FAMILIES: [&str; 17] = [
    "atlas-mono",
    "atlas-bgra",
    "shapes",
    "solid",
    "gradient",
    "mesh",
    "wgsl-paint",
    "instanced-shader",
    "wide",
    "video-bgra",
    "video-nv12",
    "video-nv12-video601",
    "video-nv12-full709",
    "video-nv12-video709",
    "surface-bgra",
    "surface-nv12",
    "media-image",
];

/// A timeout always signals the event; unwinding also releases and joins the watchdog.
// SAFETY: MTLSharedEvent permits CPU signaling from any thread. This owner
// exposes only event signaling; no command encoding crosses the watchdog thread.
#[derive(Clone)]
struct SharedEventGate(Retained<ProtocolObject<dyn gpui_apple::metal::MTLSharedEvent>>);
unsafe impl Send for SharedEventGate {}
impl SharedEventGate {
    fn signal(&self) { self.0.setSignaledValue(1); }
}

struct Gate {
    event: SharedEventGate,
    stop: Option<mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<bool>>,
}
impl Gate {
    fn new(context: &MetalCustomContext) -> Result<Self> {
        let event = SharedEventGate(context.device.newSharedEvent().context("Metal shared event unavailable")?);
        let (tx, rx) = mpsc::channel();
        let watched = event.clone();
        let thread = std::thread::spawn(move || {
            let timeout = rx.recv_timeout(Duration::from_secs(10)).is_err();
            watched.signal();
            timeout
        });
        let command = context.queue.commandBuffer().context("Metal command buffer unavailable")?;
        command.encodeWaitForEvent_value(ProtocolObject::from_ref(&*event.0), 1);
        command.commit();
        Ok(Self { event, stop: Some(tx), thread: Some(thread) })
    }
    fn release(&mut self) -> Result<()> {
        self.event.signal();
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            ensure!(
                !thread.join().map_err(|_| anyhow::anyhow!("GPU gate watchdog panicked"))?,
                "GPU gate exceeded its 10 second deadline"
            );
        }
        Ok(())
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[derive(Default)]
struct Arm {
    armed: AtomicBool,
    gate: Mutex<Option<Gate>>,
    errors: Mutex<Vec<String>>,
}
#[derive(Debug)]
struct Tracked {
    inner: Arc<dyn CustomContent>,
    arm: Option<Arc<Arm>>,
    calls: AtomicUsize,
}
impl std::fmt::Debug for Arm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Arm")
    }
}
impl CustomContent for Tracked {
    fn prepare(&self, backend: &dyn Any, extent: Size<DevicePixels>) -> Result<Box<dyn Any>> {
        self.inner.prepare(backend, extent)
    }
    fn prepare_in_frame(
        &self,
        backend: &dyn Any,
        request: &PreparationRequest,
        frame: &PreparationFrame,
    ) -> Result<Box<dyn Any>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(arm) = &self.arm
            && arm.armed.swap(false, Ordering::SeqCst)
        {
            let context = backend.downcast_ref::<MetalCustomContext>().context("window did not use Metal")?;
            *arm.gate.lock().unwrap() = Some(Gate::new(context)?);
        }
        self.inner.prepare_in_frame(backend, request, frame)
    }
    fn image_color_mode(&self) -> ImageColorMode {
        self.inner.image_color_mode()
    }
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    super::atlas::rect(x, y, w, h)
}
fn content(scene: &mut Scene, value: Arc<dyn CustomContent>) -> Weak<Tracked> {
    let tracked = Arc::new(Tracked { inner: value, arm: None, calls: AtomicUsize::new(0) });
    let weak = Arc::downgrade(&tracked);
    let bounds = rect(0., 0., 32., 32.);
    scene.insert_primitive(CustomDraw {
        order: 0,
        bounds,
        content_mask: ContentMask { bounds },
        corner_radii: Default::default(),
        opacity: 1.,
        coverage: None,
        content: tracked,
    });
    weak
}
fn wgsl() -> Result<Arc<dyn CustomContent>> {
    use xgpui_shader::shader_paint::{ShaderProgram, ShaderParameters};
    let p = ShaderProgram::new(
        "struct Parameters { color: vec4<f32>, } fn paint(input: PaintInput) -> vec4<f32> { return parameters.color; }",
    )?;
    Ok(Arc::new(p.paint(ShaderParameters::new().vec4("color", [0.25, 0.5, 0.75, 1.]))?))
}
fn paint(family: &str) -> Result<Arc<dyn CustomContent>> {
    Ok(match family {
        "solid" => Arc::new(SolidColor::new([0.25, 0.5, 0.75, 1.])?),
        "gradient" => Arc::new(Gradient::linear(
            45.,
            vec![GradientStop::new(0., [0.25, 0.5, 0.75, 1.]), GradientStop::new(1., [0.25, 0.5, 0.75, 1.])],
        )?),
        "mesh" => Arc::new(MeshGradient::new(MeshGrid::regular(2, 2, vec![[0.25, 0.5, 0.75]; 4])?, 4)?),
        "wgsl-paint" => return wgsl(),
        "wide" => Arc::new(xgpui_gamut::WideColor::linear_p3([1., 0., 0., 1.])?),
        "video-bgra" => Arc::new(crate::media_lab::video_source::SourceFormat::Bgra.frame(0)?),
        "video-nv12" => Arc::new(crate::media_lab::video_source::SourceFormat::Full601.frame(0)?),
        "video-nv12-video601" => Arc::new(crate::media_lab::video_source::SourceFormat::Video601.frame(0)?),
        "video-nv12-full709" => Arc::new(crate::media_lab::video_source::SourceFormat::Full709.frame(0)?),
        "video-nv12-video709" => Arc::new(crate::media_lab::video_source::SourceFormat::Video709.frame(0)?),
        _ => anyhow::bail!("unknown paint family {family}"),
    })
}
struct Probe {
    scene: Scene,
    source: Option<Weak<Tracked>>,
    program: Option<Weak<SceneShaderProgram>>,
    buffer: Option<CFRetained<CVPixelBuffer>>,
    image_status: Option<MediaImage>,
}
fn make(family: &str, renderer: &mut MetalHeadlessRenderer, fixture: &FilePath) -> Result<Probe> {
    let mut p = Probe { scene: Scene::default(), source: None, program: None, buffer: None, image_status: None };
    let bounds = rect(0., 0., 32., 32.);
    match family {
        "shapes" => {
            p.scene.insert_primitive(Quad {
                bounds,
                content_mask: ContentMask { bounds },
                background: rgb(0x4080bf).into(),
                ..Default::default()
            });
            let mut builder = PathBuilder::fill();
            builder.move_to(point(px(0.), px(20.)));
            builder.line_to(point(px(12.), px(32.)));
            builder.line_to(point(px(0.), px(32.)));
            builder.close();
            let mut path = builder.build()?.scale(1.);
            path.color = white().into();
            path.content_mask = ContentMask { bounds };
            p.scene.insert_primitive(path);
        }
        "atlas-mono" | "atlas-bgra" => {
            let image = RenderImage::new(vec![image::Frame::new(image::RgbaImage::from_pixel(
                7,
                5,
                image::Rgba([191, 128, 64, 255]),
            ))]);
            let key = if family == "atlas-mono" {
                AtlasKey::Svg(RenderSvgParams {
                    path: "ownership-mask".into(),
                    size: size(DevicePixels(7), DevicePixels(5)),
                })
            } else {
                AtlasKey::Image(RenderImageParams { image_id: image.id, frame_index: 0 })
            };
            let mask = [255u8; 35];
            let bytes = if family == "atlas-mono" {
                mask.as_slice()
            } else {
                image.as_bytes(0).unwrap()
            };
            let tile = renderer
                .sprite_atlas()
                .get_or_insert_with(key, &mut || {
                    Ok(Some((size(DevicePixels(7), DevicePixels(5)), Cow::Borrowed(bytes))))
                })?
                .context("atlas tile missing")?;
            if family == "atlas-mono" {
                p.scene.insert_primitive(MonochromeSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    content_mask: ContentMask { bounds },
                    color: rgb(0x4080bf).into(),
                    tile,
                    transformation: TransformationMatrix::unit(),
                });
            } else {
                p.scene.insert_primitive(PolychromeSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    content_mask: ContentMask { bounds },
                    opacity: 1.,
                    grayscale: false.into(),
                    corner_radii: Default::default(),
                    tile,
                });
            }
        }
        "surface-bgra" | "surface-nv12" => {
            let buffer = super::surface::buffer(family == "surface-nv12", false)?;
            p.scene.insert_primitive(PaintSurface {
                order: 0,
                bounds,
                content_mask: ContentMask { bounds },
                image_buffer: buffer.clone(),
            });
            p.buffer = Some(buffer);
        }
        "instanced-shader" => {
            use xgpui_shader::instanced::{ShaderProgram, presets::EffectInstance};
            let shader = ShaderProgram::<EffectInstance>::new(
                "struct Parameters { color: vec4<f32>, effect: vec4<f32>, } fn shade(input: ShaderInput, p: Parameters) -> vec4<f32> { return p.color; }",
            )?;
            let program = shader.scene_program();
            p.program = Some(Arc::downgrade(&program));
            let mut data = [[0.; 4]; SHADER_DATA_SLOTS];
            data[0] = [0.25, 0.5, 0.75, 1.];
            p.scene.insert_primitive(Primitive::Shader(ShaderDraw {
                order: 0,
                bounds,
                content_mask: ContentMask { bounds },
                opacity: 1.,
                scale_factor: 1.,
                program,
                data,
                path_mask: None,
                glyph_mask: None,
            }));
        }
        "media-image" => {
            let mut job = MediaLoader::default().load(fixture)?;
            let deadline = Instant::now() + Duration::from_secs(5);
            let image = loop {
                if let Some(image) = job.try_take() {
                    break image;
                }
                ensure!(Instant::now() < deadline, "media decode timed out");
                std::thread::sleep(Duration::from_millis(2));
            };
            p.source = Some(content(&mut p.scene, Arc::new(image.clone())));
            p.image_status = Some(image);
        }
        _ => {
            if family == "wide" {
                renderer.set_color_mode(WindowColorMode::WideGamutSdr)?;
            }
            p.source = Some(content(&mut p.scene, paint(family)?));
        }
    }
    p.scene.finish();
    Ok(p)
}
fn await_release(mut condition: impl FnMut() -> bool, label: &str) -> Result<()> {
    let end = Instant::now() + Duration::from_secs(3);
    while !condition() {
        ensure!(Instant::now() < end, "{label} was not released");
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
fn validate_pixels(image: &image::RgbaImage, family: &str) -> Result<()> {
    let expected = match family {
        "wide" => [255, 0, 0],
        name if name.starts_with("video") || name == "surface-bgra" => [224, 64, 32],
        "surface-nv12" => [186, 61, 39],
        _ => [64, 128, 191],
    };
    let actual = image.get_pixel(
        if family.starts_with("surface") || family.starts_with("video") {
            2
        } else {
            16
        },
        2,
    );
    ensure!(
        actual.0[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
        "{family} pixel {actual:?} != {expected:?}"
    );
    if family == "shapes" {
        ensure!(image.get_pixel(2, 28).0 == [255; 4], "native path was lost during renderer destruction");
    }
    Ok(())
}

fn transition(family: &str, mode: &str, output: &FilePath, fixture: &FilePath) -> Result<Value> {
    let mut renderer = MetalHeadlessRenderer::new();
    let mut probe = make(family, &mut renderer, fixture)?;
    let extent = size(DevicePixels(32), DevicePixels(32));
    if let Some(image) = probe.image_status.take() {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            renderer.render_scene_to_image(&probe.scene, extent)?;
            if image.upload_status() == UploadStatus::Ready {
                break;
            }
            ensure!(Instant::now() < deadline, "dedicated image upload timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let context = renderer.validation_context();
    let mut gate = Gate::new(&context)?;
    let (pool, cache) = renderer.validation_owners();
    let source = probe.source.take();
    let program = probe.program.take();
    let buffer = probe.buffer.take();
    if mode == "prepare-failed" {
        // Inject a CPU preparation failure after earlier custom producers.
        // This is not a simulated GPU error or an import-failure assertion.
        let mut bad = Scene::default();
        bad.replay(0..probe.scene.len(), &probe.scene);
        #[derive(Debug)]
        struct Reject;
        impl CustomContent for Reject {
            fn prepare(&self, _: &dyn Any, _: Size<DevicePixels>) -> Result<Box<dyn Any>> {
                anyhow::bail!("injected preparation failure")
            }
        }
        content(&mut bad, Arc::new(Reject));
        bad.finish();
        let error = objc2::rc::autoreleasepool(|_| renderer.prepare_validation_frame(&bad, extent).err())
            .context("preparation unexpectedly succeeded")?;
        ensure!(error.to_string().contains("injected preparation failure"), "unexpected error: {error:#}");
        drop(bad);
        drop(probe);
        drop(renderer);
        gate.release()?;
        await_release(|| pool.upgrade().is_none(), "failed frame pool")?;
        if let Some(source) = source {
            await_release(|| source.upgrade().is_none(), "failed producer")?;
        }
        if let Some(program) = program {
            await_release(|| program.upgrade().is_none(), "failed shader program")?;
        }
        await_release(|| cache.retain_count() == 1, "failed Surface cache")?;
        if let Some(buffer) = &buffer {
            await_release(|| buffer.retain_count() == 1, "failed Surface buffer")?;
        }
        drop(buffer);
        return Ok(
            json!({"family":family,"mode":mode,"outcome":"passed","failure_kind":"CPU preparation rejection; no GPU fault injected"}),
        );
    }
    let frame = objc2::rc::autoreleasepool(|_| renderer.prepare_validation_frame(&probe.scene, extent))?;
    ensure!(
        frame.command.status() == MTLCommandBufferStatus::NotEnqueued,
        "sampling committed during preparation"
    );
    drop(probe);
    drop(renderer);
    ensure!(pool.upgrade().is_some(), "pending frame lost its instance pool");
    if let Some(source) = &source {
        ensure!(source.upgrade().is_some(), "pending frame lost producer");
    }
    if family.starts_with("surface") {
        ensure!(cache.retain_count() >= 3, "pending Surface lost cache owner");
    }
    if mode == "abandoned" {
        let command = frame.command.clone();
        drop(frame);
        ensure!(command.status() == MTLCommandBufferStatus::NotEnqueued, "abandoned sampling was submitted");
        drop(command);
    } else {
        frame.command.commit();
        ensure!(frame.command.status() != MTLCommandBufferStatus::Completed, "GPU gate did not hold sampling");
        if let Some(source) = &source {
            ensure!(source.upgrade().is_some(), "submitted frame lost producer before completion");
        }
        gate.release()?;
        frame.command.waitUntilCompleted();
        ensure!(
            frame.command.status() == MTLCommandBufferStatus::Completed,
            "GPU command did not complete: {:?}",
            frame.command.status()
        );
        let image = frame.read_completed_image()?;
        image.save(output.join(format!("{family}.png")))?;
        validate_pixels(&image, family)?;
        // Check callbacks release owners while the command object still exists.
        await_release(|| pool.upgrade().is_none(), "completed instance pool")?;
        if let Some(source) = &source {
            await_release(|| source.upgrade().is_none(), "completed producer")?;
        }
        drop(frame);
    }
    gate.release()?;
    await_release(|| pool.upgrade().is_none(), "abandoned instance pool")?;
    if let Some(source) = source {
        await_release(|| source.upgrade().is_none(), "abandoned producer")?;
    }
    if let Some(program) = program {
        await_release(|| program.upgrade().is_none(), "shader program after host destruction")?;
    }
    await_release(|| cache.retain_count() == 1, "Surface cache after frame retirement")?;
    if let Some(buffer) = &buffer {
        await_release(|| buffer.retain_count() == 1, "Surface buffer after frame retirement")?;
    }
    drop(buffer);
    Ok(json!({"family":family,"mode":mode,"outcome":"passed"}))
}

struct WindowGateCleanup(Arc<Arm>);
impl Drop for WindowGateCleanup {
    fn drop(&mut self) {
        if let Some(mut gate) = self.0.gate.lock().unwrap().take() {
            let _ = gate.release();
        }
    }
}
struct WindowData {
    paints: Vec<Arc<Tracked>>,
    media: MediaImage,
    image: Arc<RenderImage>,
    bgra: CFRetained<CVPixelBuffer>,
    nv12: CFRetained<CVPixelBuffer>,
    shader: Arc<xgpui_shader::instanced::ShaderProgram<xgpui_shader::instanced::presets::EffectInstance>>,
    arm: Arc<Arm>,
}
struct MixedWindow {
    data: Rc<WindowData>,
}
impl Render for MixedWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use xgpui_shader::instanced::{WindowShaderExt, presets::EffectInstance};
        let paints = self.data.paints.clone();
        let shader = self.data.shader.clone();
        let arm = self.data.arm.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .bg(black())
            .text_color(white())
            .child(div().h(px(32.)).child("Ownership: native glyphs Aa 0123 Ω 中 😀"))
            .child(div().h(px(32.)).w(px(100.)).bg(rgb(0x4080bf)).border_2().border_color(white()).rounded_lg())
            .child(div().flex().gap_2().children(paints.into_iter().map(|paint| {
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        window.paint_custom(
                            CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() },
                            paint.clone(),
                        )
                    },
                )
                .size(px(48.))
            })))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(img(self.data.image.clone()).size(px(32.)))
                    .child(surface(self.data.bgra.clone()).size(px(32.)))
                    .child(surface(self.data.nv12.clone()).size(px(32.)))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                if let Err(error) = window.paint_custom_shader(
                                    bounds,
                                    &shader,
                                    EffectInstance { color: [0.25, 0.5, 0.75, 1.], effect: [0.; 4] },
                                ) {
                                    arm.errors.lock().unwrap().push(format!("{error:#}"));
                                }
                            },
                        )
                        .size(px(48.)),
                    ),
            )
    }
}
fn window_data(fixture: &FilePath) -> Result<Rc<WindowData>> {
    use xgpui_shader::instanced::{ShaderProgram, presets::EffectInstance};
    let arm = Arc::new(Arm::default());
    let mut paints = Vec::new();
    for family in [
        "solid",
        "gradient",
        "mesh",
        "wgsl-paint",
        "wide",
        "video-bgra",
        "video-nv12",
        "video-nv12-video601",
        "video-nv12-full709",
        "video-nv12-video709",
    ] {
        paints.push(Arc::new(Tracked { inner: paint(family)?, arm: Some(arm.clone()), calls: AtomicUsize::new(0) }));
    }
    let mut renderer = MetalHeadlessRenderer::new();
    let mut image_probe = make("media-image", &mut renderer, fixture)?;
    let media = image_probe.image_status.take().context("missing media fixture")?;
    drop(image_probe);
    drop(renderer);
    paints.push(Arc::new(Tracked {
        inner: Arc::new(media.clone()),
        arm: Some(arm.clone()),
        calls: AtomicUsize::new(0),
    }));
    Ok(Rc::new(WindowData {
        paints,
        media,
        image: Arc::new(RenderImage::new(vec![image::Frame::new(image::RgbaImage::from_pixel(
            7,
            5,
            image::Rgba([191, 128, 64, 255]),
        ))])),
        bgra: super::surface::buffer(false, false)?,
        nv12: super::surface::buffer(true, false)?,
        shader: Arc::new(ShaderProgram::<EffectInstance>::new(
            "struct Parameters { color: vec4<f32>, effect: vec4<f32>, } fn shade(input: ShaderInput, p: Parameters) -> vec4<f32> { return p.color; }",
        )?),
        arm,
    }))
}
async fn window_checks(output: &FilePath, fixture: &FilePath, cancel: &AtomicBool, cx: &mut AsyncApp) -> Result<Value> {
    let baseline = cx.update(|cx| cx.windows().len());
    let mut results = Vec::new();
    for mode in ["unpainted", "completed", "submitted"] {
        super::check_cancel(cancel)?;
        let data = window_data(fixture)?;
        let weak_data = Rc::downgrade(&data);
        let sources: Vec<_> = data.paints.iter().map(Arc::downgrade).collect();
        let arm = data.arm.clone();
        let _cleanup = WindowGateCleanup(arm.clone());
        let bgra = data.bgra.clone();
        let nv12 = data.nv12.clone();
        let view_data = data.clone();
        let opened = cx.update(|cx| -> Result<_> {
            let handle = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(640.), px(320.)),
                    ))),
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                |window, cx| {
                    window.set_color_mode(WindowColorMode::WideGamutSdr).expect("wide ownership window required");
                    cx.new(|_| MixedWindow { data: view_data })
                },
            )?;
            if mode == "unpainted" {
                handle.update(cx, |_, window, _| window.remove_window())?;
                Ok(None)
            } else {
                Ok(Some(handle))
            }
        })?;
        let mut calls = vec![0; sources.len()];
        if mode == "unpainted" {
            for (index, source) in sources.iter().enumerate() {
                calls[index] = source.upgrade().unwrap().calls.load(Ordering::SeqCst);
                ensure!(calls[index] == 0, "unpainted window prepared a producer");
            }
        }
        let captured = if let Some(handle) = opened {
            let result = async {
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    let pixels = cx.update(|cx| {
                        cx.update_window(handle.into(), |_, window, cx| -> Result<_> {
                            window.refresh();
                            window.draw(cx).clear(cx);
                            window.render_to_image()
                        })
                    })??;
                    if data.media.upload_status() == UploadStatus::Ready {
                        ensure!(
                            pixels
                                .pixels()
                                .any(|p| p[0].abs_diff(64) <= 3 && p[1].abs_diff(128) <= 3 && p[2].abs_diff(191) <= 3),
                            "mixed window contained no reference paint pixels"
                        );
                        pixels.save(output.join(format!("window-{mode}.png")))?;
                        break;
                    }
                    ensure!(Instant::now() < deadline, "window media upload timed out");
                    cx.background_executor().timer(Duration::from_millis(10)).await;
                }
                ensure!(arm.errors.lock().unwrap().is_empty(), "instanced shader failed in window");
                for (index, source) in sources.iter().enumerate() {
                    calls[index] = source.upgrade().unwrap().calls.load(Ordering::SeqCst);
                    ensure!(calls[index] > 0, "window never prepared producer {index}");
                }
                if mode == "submitted" {
                    arm.armed.store(true, Ordering::SeqCst);
                    cx.update(|cx| {
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.refresh();
                            window.draw(cx).clear(cx);
                            window.submit_frame_for_validation()
                        })
                    })??;
                    ensure!(arm.gate.lock().unwrap().is_some(), "window sampling gate was never armed");
                }
                Ok::<_, anyhow::Error>(())
            }
            .await;
            let closed = cx.update(|cx| handle.update(cx, |_, window, _| window.remove_window()));
            result.and(closed)
        } else {
            Ok(())
        };
        drop(data);
        // Always release gates before propagating draw/readback errors.
        if let Err(error) = captured {
            if let Some(mut gate) = arm.gate.lock().unwrap().take() {
                gate.release()?;
            }
            return Err(error);
        }
        cx.background_executor().timer(Duration::from_millis(10)).await;
        ensure!(weak_data.upgrade().is_none(), "closed window retained its root resource bundle");
        if mode == "submitted" {
            ensure!(sources.iter().all(|source| source.upgrade().is_some()), "GPU-pending window lost producer owners");
        }
        if mode == "unpainted" {
            ensure!(sources.iter().all(|source| source.upgrade().is_none()), "unpainted window retained a producer");
        }
        if let Some(mut gate) = arm.gate.lock().unwrap().take() {
            gate.release()?;
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if sources.iter().all(|source| source.upgrade().is_none())
                && bgra.retain_count() == 1
                && nv12.retain_count() == 1
            {
                break;
            }
            ensure!(Instant::now() < deadline, "window {mode} retained sources or native buffers after completion");
            cx.background_executor().timer(Duration::from_millis(5)).await;
        }
        ensure!(cx.update(|cx| cx.windows().len()) == baseline, "mixed window count did not return to baseline");
        results.push(json!({"mode":mode,"outcome":"passed","producer_draw_calls":calls,"root_released":true,"producer_owners_released":true,"surface_buffers_released":true,"window_count_restored":true}));
        std::fs::write(output.join("window-ownership.json"), serde_json::to_vec_pretty(&json!({"cases":results}))?)?;
    }
    Ok(
        json!({"outcome":"passed","cases":results,"content":["native text/glyphs","native shapes","atlas image","dedicated image","solid","gradient","mesh","WGSL paint","instanced shader","wide color","BGRA/NV12 custom video","BGRA/NV12 Surface"]}),
    )
}

pub(super) async fn run(output: &FilePath, cancel: &Arc<AtomicBool>, cx: &mut AsyncApp) -> Result<Value> {
    std::fs::create_dir_all(output)?;
    let fixture = output.join("media.png");
    image::RgbaImage::from_pixel(32, 32, image::Rgba([64, 128, 191, 255])).save(&fixture)?;
    let path = output.to_path_buf();
    let token = cancel.clone();
    let window_fixture = fixture.clone();
    let frames = cx
        .background_spawn(async move {
            let mut results = Vec::new();
            for family in FAMILIES {
                for mode in ["abandoned", "completed", "prepare-failed"] {
                    super::check_cancel(&token)?;
                    std::fs::write(
                        path.join("checkpoint.json"),
                        serde_json::to_vec_pretty(&json!({"family":family,"mode":mode}))?,
                    )?;
                    let result = objc2::rc::autoreleasepool(|_| transition(family, mode, &path, &fixture));
                    let report = match &result {
                        Ok(report) => report.clone(),
                        Err(error) => {
                            json!({"family":family,"mode":mode,"outcome":"failed","error":format!("{error:#}")})
                        }
                    };
                    results.push(report);
                    std::fs::write(path.join("report.json"), serde_json::to_vec_pretty(&json!({"cases":results}))?)?;
                    result.with_context(|| format!("{family}/{mode}"))?;
                }
            }
            Ok::<Value, anyhow::Error>(json!({"outcome":"passed","cases":results}))
        })
        .await?;
    let windows = window_checks(output, &window_fixture, cancel, cx).await?;
    let report = json!({"outcome":"passed","frames":frames,"windows":windows});
    std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}
