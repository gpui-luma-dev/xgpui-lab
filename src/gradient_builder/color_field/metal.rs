use super::*;
use anyhow::Context as _;
use gpui::{CustomContent, DevicePixels, Size};
use gpui_apple::{
    custom_draw::{MetalCustomContext, MetalCustomOutput, MetalWideSdrOutput},
    metal::*,
};
use std::{
    any::Any,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Point {
    geometry: [f32; 4],
    color: [f32; 4],
    hsl: [f32; 4],
    shape: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Parameters {
    background: [f32; 4],
    background_hsl: [f32; 4],
    options: [f32; 4],
    shape: [f32; 4],
}
#[derive(Default)]
pub struct PipelineCache {
    cached: Mutex<Option<(u64, CommandQueue, ComputePipelineState)>>,
    pub compilations: AtomicUsize,
}
// Retained immutable pipeline/queue; all access is serialized by the mutex.
unsafe impl Send for PipelineCache {}
unsafe impl Sync for PipelineCache {}
struct Prepared {
    texture: Texture,
    commands: Vec<CommandBuffer>,
}
impl Prepared {
    fn reusable(&self, ctx: &MetalCustomContext, output: Output) -> Result<Box<dyn Any>> {
        let mut commands = Vec::new();
        for command in &self.commands {
            match command.status() {
                MTLCommandBufferStatus::NotEnqueued => commands.push(command.clone()),
                MTLCommandBufferStatus::Committed
                | MTLCommandBufferStatus::Scheduled
                | MTLCommandBufferStatus::Completed => {}
                _ => anyhow::bail!("Field command externally enqueued or failed"),
            }
        }
        if output == Output::DisplayP3 {
            Ok(Box::new(MetalWideSdrOutput::from_linear_p3_premultiplied(ctx, self.texture.clone(), commands)?))
        } else {
            Ok(Box::new(MetalCustomOutput { texture: self.texture.clone(), commands }))
        }
    }
}
pub struct Paint {
    field: ColorField,
    output: Output,
    pipeline: Arc<PipelineCache>,
    cache: Mutex<Option<(u64, Size<DevicePixels>, Prepared)>>,
}
// Immutable field data. Preparation serializes access to retained output resources.
unsafe impl Send for Paint {}
unsafe impl Sync for Paint {}
impl std::fmt::Debug for Paint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorFieldPaint")
            .field("field", &self.field)
            .field("output", &self.output)
            .finish_non_exhaustive()
    }
}
impl Paint {
    pub fn new(field: ColorField, output: Output, pipeline: Arc<PipelineCache>) -> Result<Self> {
        field.validate()?;
        Ok(Self { field, output, pipeline, cache: Mutex::new(None) })
    }
}
impl CustomContent for Paint {
    fn prepare(&self, backend: &dyn Any, extent: Size<DevicePixels>) -> Result<Box<dyn Any>> {
        let ctx = backend
            .downcast_ref::<MetalCustomContext>()
            .context("Color field GPU prototype requires native Metal; select CPU on this backend")?;
        ensure!(
            extent.width.0 > 0 && extent.height.0 > 0 && extent.width.0 <= 16384 && extent.height.0 <= 16384,
            "Invalid field extent"
        );
        let bpp = if self.output == Output::DisplayP3 { 8u64 } else { 4 };
        ensure!(
            extent.width.0 as u64 * extent.height.0 as u64 * bpp <= 64 * 1024 * 1024,
            "Field texture exceeds 64 MiB"
        );
        let mut cached = self.cache.lock().map_err(|_| anyhow::anyhow!("Field cache poisoned"))?;
        if let Some((key, size, result)) = cached.as_ref()
            && *key == ctx.cache_key()
            && *size == extent
        {
            return result.reusable(ctx, self.output);
        }
        let mut pipelines =
            self.pipeline.cached.lock().map_err(|_| anyhow::anyhow!("Field pipeline cache poisoned"))?;
        if pipelines.as_ref().is_none_or(|(key, _, _)| *key != ctx.cache_key()) {
            let library = ctx
                .device
                .newLibraryWithSource_options_error(
                    &objc2_foundation::NSString::from_str(include_str!("field.metal")),
                    Some(&CompileOptions::new()),
                )
                .map_err(anyhow::Error::msg)?;
            let function = library
                .newFunctionWithName(&objc2_foundation::NSString::from_str("color_field"))
                .context("Field kernel missing")?;
            let pipeline =
                ctx.device.newComputePipelineStateWithFunction_error(&function).map_err(anyhow::Error::msg)?;
            *pipelines = Some((ctx.cache_key(), ctx.queue.clone(), pipeline));
            self.pipeline.compilations.fetch_add(1, Ordering::Relaxed);
        }
        let mut points: Vec<_> = self
            .field
            .points
            .iter()
            .map(|p| {
                let (sin, cos) = p.angle.to_radians().sin_cos();
                Point {
                    geometry: [p.position[0], p.position[1], cos, sin],
                    color: p.color,
                    hsl: [p.hsl[0], p.hsl[1], p.hsl[2], 0.],
                    shape: [p.radius, p.stretch, 0., 0.],
                }
            })
            .collect();
        if points.is_empty() {
            points.push(Point::zeroed());
        }
        let bytes = bytemuck::cast_slice::<Point, u8>(&points);
        let buffer = unsafe {
            ctx.device.newBufferWithBytes_length_options(
                std::ptr::NonNull::new(bytes.as_ptr() as *mut std::ffi::c_void).unwrap(),
                bytes.len(),
                MTLResourceOptions::StorageModeShared,
            )
        }
        .context("Field buffer allocation failed")?;
        let parameters = Parameters {
            background: self.field.background,
            background_hsl: [
                self.field.background_hsl[0],
                self.field.background_hsl[1],
                self.field.background_hsl[2],
                0.,
            ],
            options: [
                self.field.points.len() as f32,
                if self.field.mixing == Mixing::Normalized {
                    1.
                } else {
                    0.
                },
                if self.field.hsl { 1. } else { 0. },
                self.field.background_weight,
            ],
            shape: [1. / self.field.softness, if self.output == Output::DisplayP3 { 1. } else { 0. }, 0., 0.],
        };
        let descriptor = TextureDescriptor::new();
        unsafe {
            descriptor.setWidth(extent.width.0 as usize);
            descriptor.setHeight(extent.height.0 as usize);
        }
        descriptor.setPixelFormat(if self.output == Output::DisplayP3 {
            MTLPixelFormat::RGBA16Float
        } else {
            MTLPixelFormat::BGRA8Unorm
        });
        descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
        descriptor.setStorageMode(MTLStorageMode::Private);
        let texture = ctx.device.newTextureWithDescriptor(&descriptor).context("Field texture allocation failed")?;
        let command = ctx.queue.commandBuffer().context("Field command allocation failed")?;
        let encoder = command.computeCommandEncoder().context("Field encoder allocation failed")?;
        encoder.setComputePipelineState(&pipelines.as_ref().unwrap().2);
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&buffer), 0, 0);
            encoder.setBytes_length_atIndex(
                std::ptr::NonNull::from(&parameters).cast(),
                std::mem::size_of::<Parameters>(),
                1,
            );
            encoder.setTexture_atIndex(Some(&texture), 0);
        }
        encoder.dispatchThreads_threadsPerThreadgroup(
            MTLSize { width: extent.width.0 as usize, height: extent.height.0 as usize, depth: 1 },
            MTLSize { width: 8, height: 8, depth: 1 },
        );
        encoder.endEncoding();
        let result = Prepared { texture, commands: vec![command] };
        let output = result.reusable(ctx, self.output)?;
        *cached = Some((ctx.cache_key(), extent, result));
        Ok(output)
    }
}
use bytemuck::Zeroable;

#[cfg(test)]
mod tests {
    use super::*;
    fn readback(paint: &Paint, ctx: &MetalCustomContext, width: usize, height: usize) -> Vec<u8> {
        let _ = paint.prepare(ctx, gpui::size(DevicePixels(width as i32), DevicePixels(height as i32))).unwrap();
        let cached = paint.cache.lock().unwrap();
        let result = &cached.as_ref().unwrap().2;
        // Test-only host submission of this isolated producer's command batch.
        for command in &result.commands {
            command.commit();
            command.waitUntilCompleted();
            assert_eq!(command.status(), MTLCommandBufferStatus::Completed);
        }
        let bpp = if paint.output == Output::DisplayP3 { 8 } else { 4 };
        let row = (width * bpp).div_ceil(256) * 256;
        let buffer =
            ctx.device.newBufferWithLength_options(row * height, MTLResourceOptions::StorageModeShared).unwrap();
        let command = ctx.queue.commandBuffer().unwrap();
        let blit = command.blitCommandEncoder().unwrap();
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
            &result.texture,0,0,MTLOrigin {x:0,y:0,z:0},MTLSize {width,height,depth:1},&buffer,0,row,row*height,
        );
        }
        blit.endEncoding();
        command.commit();
        command.waitUntilCompleted();
        assert_eq!(command.status(), MTLCommandBufferStatus::Completed);
        unsafe { std::slice::from_raw_parts(buffer.contents().as_ptr() as *const u8, row * height) }.to_vec()
    }
    fn half(bits: u16) -> f32 {
        let sign = if bits & 0x8000 != 0 { -1. } else { 1. };
        let exp = ((bits >> 10) & 31) as i32;
        let fraction = (bits & 1023) as f32;
        if exp == 0 {
            sign * fraction * 2f32.powi(-24)
        } else {
            sign * (1. + fraction / 1024.) * 2f32.powi(exp - 15)
        }
    }
    #[test]
    #[ignore = "requires a native Metal device"]
    fn native_field_pixels_match_cpu_and_edits_reuse_pipeline() {
        objc2::rc::autoreleasepool(|_| {
            let device = MTLCreateSystemDefaultDevice().expect("Metal device");
            let queue = device.newCommandQueue().unwrap();
            let ctx = MetalCustomContext { device, queue };
            let pipeline = Arc::new(PipelineCache::default());
            let (width, height) = (32usize, 24usize);
            for mixing in [Mixing::Ordered, Mixing::Normalized] {
                for hsl in [false, true] {
                    for output in [Output::Srgb, Output::DisplayP3] {
                        let mut field = super::super::tests::fixture();
                        field.mixing = mixing;
                        field.hsl = hsl;
                        field.softness = if hsl { 0.25 } else { 2. };
                        field.points[0].position[0] += if mixing == Mixing::Normalized { 0.03 } else { 0. };
                        field.points[1].radius = if output == Output::DisplayP3 { 0.3 } else { 1.2 };
                        // A lifted negative hue is equivalent to its wrapped color.
                        field.points[1].hsl[0] -= 1.;
                        // Exercise all 16 uploaded records and coincident point support.
                        let base = field.points.clone();
                        field.points = (0..16)
                            .map(|i| {
                                let mut p = base[i % 2].clone();
                                p.color[3] *= 0.2;
                                p.angle += i as f32 * 7.;
                                p
                            })
                            .collect();
                        let paint = Paint::new(field.clone(), output, pipeline.clone()).unwrap();
                        let pixels = readback(&paint, &ctx, width, height);
                        let bpp = if output == Output::DisplayP3 { 8 } else { 4 };
                        let row = (width * bpp).div_ceil(256) * 256;
                        for y in 0..height {
                            for x in 0..width {
                                let mut expected =
                                    field.sample([(x as f32 + 0.5) / width as f32, (y as f32 + 0.5) / height as f32]);
                                let offset = y * row + x * bpp;
                                let actual = if output == Output::DisplayP3 {
                                    for i in 0..3 {
                                        let v = expected[i].clamp(0., 1.);
                                        expected[i] = if v <= 0.04045 {
                                            v / 12.92
                                        } else {
                                            ((v + 0.055) / 1.055).powf(2.4)
                                        } * expected[3];
                                    }
                                    std::array::from_fn(|i| {
                                        half(u16::from_le_bytes([pixels[offset + i * 2], pixels[offset + i * 2 + 1]]))
                                    })
                                } else {
                                    [
                                        pixels[offset + 2] as f32 / 255.,
                                        pixels[offset + 1] as f32 / 255.,
                                        pixels[offset] as f32 / 255.,
                                        pixels[offset + 3] as f32 / 255.,
                                    ]
                                };
                                let tolerance = if output == Output::Srgb { 1.1 / 255. } else { 0.0015 };
                                for (a, b) in actual.into_iter().zip(expected) {
                                    assert!(
                                        (a - b).abs() < tolerance,
                                        "{mixing:?} hsl={hsl} {output:?} ({x},{y}) {a} != {b}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            assert_eq!(
                pipeline.compilations.load(Ordering::Relaxed),
                1,
                "point edits, mixing and output switches share one pipeline"
            );
        });
    }
}
