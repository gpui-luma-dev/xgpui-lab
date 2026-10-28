use super::*;

fn render(
    renderer: &mut dyn PlatformHeadlessRenderer,
    gradient: Arc<Gradient>,
    width: i32,
    height: i32,
) -> Result<Vec<u8>> {
    let bounds = rect(0., 0., width as f32, height as f32);
    let mut scene = base();
    let mut draw = record(bounds, gradient, None);
    draw.corner_radii = Corners::default();
    scene.insert_primitive(draw);
    scene.finish();
    Ok(renderer.render_scene_to_image(&scene, size(DevicePixels(width), DevicePixels(height)))?.into_raw())
}
fn pixel(bytes: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    bytes[(y * width + x) * 4..(y * width + x) * 4 + 4].try_into().unwrap()
}
fn assert_pixel(bytes: &[u8], x: usize, y: usize, rgba: [f32; 4]) -> Result<()> {
    let actual = pixel(bytes, 257, x, y);
    ensure!(
        actual.into_iter().zip(rgba).all(|(a, b)| a.abs_diff((b * 255.).round() as u8) <= 3),
        "gradient pixel {x},{y}: {actual:?}, expected {rgba:?}"
    );
    Ok(())
}
pub(super) fn verify(renderer: &mut dyn PlatformHeadlessRenderer) -> Result<()> {
    let stops = vec![GradientStop::new(0., [0., 0., 0., 1.]), GradientStop::new(1., [1.; 4])];
    let base_gradient = Arc::new(Gradient::linear(90., stops.clone())?);
    // Geometry expectations use hand-picked points in an odd, non-square extent.
    let cases = vec![
        (
            GradientKind::Linear { angle: 90. },
            vec![(0, 64, 0.5 / 257.), (128, 64, 0.5), (256, 64, 256.5 / 257.)],
        ),
        (GradientKind::Linear { angle: 0. }, vec![(128, 0, 128.5 / 129.), (128, 128, 0.5 / 129.)]),
        (
            GradientKind::Radial { center: [0.5; 2], radius: [0.5; 2] },
            vec![(128, 64, 0.), (192, 64, 128. / 257.), (128, 0, 128. / 129.), (0, 0, 1.)],
        ),
        (
            GradientKind::Conic { center: [0.5; 2], angle: 0. },
            vec![(128, 64, 0.), (192, 64, 0.25), (128, 96, 0.5), (64, 64, 0.75), (128, 32, 0.), (192, 128, 0.375)],
        ),
        (
            GradientKind::Conic { center: [0.5; 2], angle: 450. },
            vec![(192, 64, 0.), (128, 96, 0.25), (128, 32, 0.75)],
        ),
        (
            GradientKind::Diamond { center: [0.5; 2], radius: [0.5; 2], angle: 0. },
            vec![(128, 64, 0.), (160, 80, 64. / 257. + 32. / 129.), (0, 0, 1.)],
        ),
        (
            GradientKind::Diamond { center: [0.5; 2], radius: [0.5; 2], angle: 45. },
            vec![(192, 64, (128. / 257.) * std::f32::consts::SQRT_2)],
        ),
    ];
    for (kind, samples) in cases {
        let gradient = Arc::new(base_gradient.with_kind(kind)?);
        let bytes = render(renderer, gradient.clone(), 257, 129)?;
        for (x, y, t) in samples {
            assert_pixel(&bytes, x, y, [t, t, t, 1.]).map_err(|error| anyhow::anyhow!("{kind:?}: {error}"))?;
        }
        ensure!(render(renderer, gradient.clone(), 257, 129)? == bytes, "cached gradient changed");
        ensure!(gradient.preparations() == 1, "static gradient regenerated");
        ensure!(
            gradient.prepare(&(), size(DevicePixels(10), DevicePixels(10))).is_err(),
            "unsupported backend accepted"
        );
        ensure!(gradient.prepare(&(), size(DevicePixels(0), DevicePixels(10))).is_err(), "invalid extent accepted");
    }
    ensure!(base_gradient.pipeline_compilations() == 1, "related kinds recompiled pipelines");
    let hard = Arc::new(base_gradient.with_stops(vec![
        GradientStop::new(0., [1., 0., 0., 1.]),
        GradientStop::new(0.5, [1., 0., 0., 1.]),
        GradientStop::new(0.5, [0., 0., 1., 1.]),
        GradientStop::new(1., [0., 0., 1., 1.]),
    ])?);
    let bytes = render(renderer, hard, 257, 129)?;
    assert_pixel(&bytes, 127, 64, [1., 0., 0., 1.])?;
    assert_pixel(&bytes, 128, 64, [0., 0., 1., 1.])?;
    let multi = Arc::new(base_gradient.with_stops(vec![
        GradientStop::new(0.2, [1., 0., 0., 1.]),
        GradientStop::new(0.4, [0., 1., 0., 1.]),
        GradientStop::new(0.6, [0., 0., 1., 1.]),
        GradientStop::new(0.8, [1.; 4]),
    ])?);
    let bytes = render(renderer, multi, 257, 129)?;
    assert_pixel(&bytes, 0, 64, [1., 0., 0., 1.])?;
    assert_pixel(&bytes, 128, 64, [0., 0.5, 0.5, 1.])?;
    assert_pixel(&bytes, 256, 64, [1.; 4])?;
    let transparent = Arc::new(
        base_gradient
            .with_stops(vec![GradientStop::new(0., [1., 0., 0., 1.]), GradientStop::new(1., [0., 0., 1., 0.])])?,
    );
    let bytes = render(renderer, transparent, 257, 129)?;
    assert_pixel(&bytes, 128, 64, [0.5, 0., 0., 1.])?; // Premultiplied interpolation avoids a blue fringe.
    let original = render(renderer, base_gradient.clone(), 257, 129)?;
    render(renderer, base_gradient.clone(), 129, 65)?;
    ensure!(base_gradient.preparations() == 2, "resize did not regenerate gradient");
    let snapshot = Arc::new(base_gradient.with_kind(GradientKind::Radial { center: [0.5; 2], radius: [0.5; 2] })?);
    render(renderer, snapshot, 257, 129)?;
    ensure!(render(renderer, base_gradient.clone(), 257, 129)? == original, "snapshot mutated old gradient");
    // Every new geometry also traverses the background/border/path composition route.
    for effect in [Effect::Linear, Effect::Radial, Effect::Conic, Effect::Diamond] {
        let producer = Arc::new(Gradient::new(Settings::new(effect).kind(effect, 0.).unwrap(), gradient_stops())?);
        let scene = demo_scene(producer.clone());
        let image = renderer.render_scene_to_image(&scene, size(DevicePixels(1024), DevicePixels(320)))?;
        ensure!(image.get_pixel(160, 140).0[0..3] != [0, 0, 0], "empty gradient background");
        ensure!(image.get_pixel(400, 27).0[0..3] != [0, 0, 0], "empty gradient border");
        ensure!(image.get_pixel(840, 150).0[0..3] != [0, 0, 0], "empty gradient path");
        ensure!(producer.preparations() == 1, "gradient not shared across shape types");
    }
    Ok(())
}
