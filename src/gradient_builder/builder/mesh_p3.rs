//! Reinterpret an encoded RGB producer as Display P3, then decode on the GPU.
#[cfg(target_os = "macos")]
mod metal {
    use anyhow::{Context as _, Result};
    use gpui::{CustomContent, DevicePixels, Size};
    use gpui_apple::{custom_draw::{MetalCustomContext, MetalCustomOutput, MetalWideSdrOutput}, metal::*};
    use std::{any::Any, sync::{Arc, Mutex}};
    #[cfg(test)]
    use xgpui_shader::MeshGradient;

    const SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;
kernel void mesh_p3(texture2d<float, access::sample> src [[texture(0)]],
                    texture2d<half, access::write> dst [[texture(1)]], uint2 p [[thread_position_in_grid]]) {
    if (p.x >= dst.get_width() || p.y >= dst.get_height()) return;
    constexpr sampler linear_sampler(coord::normalized, address::clamp_to_edge, filter::linear);
    float2 uv = (float2(p) + 0.5) / float2(dst.get_width(), dst.get_height());
    float4 c = src.sample(linear_sampler, uv);
    float3 rgb = select(pow((c.rgb + 0.055) / 1.055, float3(2.4)), c.rgb / 12.92, c.rgb <= 0.04045);
    dst.write(half4(half3(rgb * c.a), half(c.a)), p);
}
"#;
    #[derive(Default)]
    pub struct Pipeline(Mutex<Option<(u64, CommandQueue, ComputePipelineState)>>);
    // Immutable pipelines and retained queue are accessed only under the mutex.
    unsafe impl Send for Pipeline {}
    unsafe impl Sync for Pipeline {}
    pub struct Paint {
        source: Arc<dyn CustomContent>,
        pipeline: Arc<Pipeline>,
        cached: Mutex<Option<(u64, Size<DevicePixels>, MetalWideSdrOutput)>>,
    }
    // Preparation serializes cache access; textures are immutable after recording.
    unsafe impl Send for Paint {}
    unsafe impl Sync for Paint {}
    impl Paint {
        pub fn new(source: Arc<dyn CustomContent>, pipeline: Arc<Pipeline>) -> Self {
            Self { source, pipeline, cached: Mutex::new(None) }
        }
    }
    impl std::fmt::Debug for Paint {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("DisplayP3Paint").field("source", &self.source).finish_non_exhaustive()
        }
    }
    impl CustomContent for Paint {
        fn prepare(&self, backend: &dyn Any, extent: Size<DevicePixels>) -> Result<Box<dyn Any>> {
            let ctx = backend.downcast_ref::<MetalCustomContext>().context("P3 preview requires native Metal")?;
            let mut cache = self.cached.lock().map_err(|_| anyhow::anyhow!("P3 mesh cache poisoned"))?;
            if let Some((key, size, output)) = cache.as_ref()
                && *key == ctx.cache_key() && *size == extent {
                return Ok(Box::new(output.reusable_for(ctx)?));
            }
            let mut pipelines = self.pipeline.0.lock().map_err(|_| anyhow::anyhow!("P3 pipeline cache poisoned"))?;
            if pipelines.as_ref().is_none_or(|(key, _, _)| *key != ctx.cache_key()) {
                let library = ctx.device.newLibraryWithSource_options_error(
                    &objc2_foundation::NSString::from_str(SOURCE), Some(&CompileOptions::new())
                ).map_err(anyhow::Error::msg)?;
                let function = library.newFunctionWithName(&objc2_foundation::NSString::from_str("mesh_p3"))
                    .context("P3 conversion kernel missing")?;
                let pipeline = ctx.device.newComputePipelineStateWithFunction_error(&function).map_err(anyhow::Error::msg)?;
                *pipelines = Some((ctx.cache_key(), ctx.queue.clone(), pipeline));
            }
            let prepared = self.source.prepare(backend, extent)?;
            let source = prepared.downcast::<MetalCustomOutput>().map_err(|_| anyhow::anyhow!("Unexpected native RGB output"))?;
            let descriptor = TextureDescriptor::new();
            unsafe { descriptor.setWidth(extent.width.0 as usize); descriptor.setHeight(extent.height.0 as usize); }
            descriptor.setPixelFormat(MTLPixelFormat::RGBA16Float);
            descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
            descriptor.setStorageMode(MTLStorageMode::Private);
            let texture = ctx.device.newTextureWithDescriptor(&descriptor).context("P3 mesh texture allocation failed")?;
            let command = ctx.queue.commandBuffer().context("P3 mesh command allocation failed")?;
            let encoder = command.computeCommandEncoder().context("P3 mesh encoder allocation failed")?;
            encoder.setComputePipelineState(&pipelines.as_ref().unwrap().2);
            unsafe { encoder.setTexture_atIndex(Some(&source.texture), 0); encoder.setTexture_atIndex(Some(&texture), 1); }
            encoder.dispatchThreads_threadsPerThreadgroup(
                MTLSize { width: extent.width.0 as usize, height: extent.height.0 as usize, depth: 1 },
                MTLSize { width: 8, height: 8, depth: 1 },
            );
            encoder.endEncoding();
            let mut commands = source.commands;
            commands.push(command);
            let output = MetalWideSdrOutput::from_linear_p3_premultiplied(ctx, texture, commands)?;
            let result = output.reusable_for(ctx)?;
            *cache = Some((ctx.cache_key(), extent, output));
            Ok(Box::new(result))
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        #[ignore = "requires a native Metal device"]
        fn native_p3_mesh_preparation_compiles_and_reuses_pending_output() {
            objc2::rc::autoreleasepool(|_| {
                let device = MTLCreateSystemDefaultDevice().expect("Metal device");
                let queue = device.newCommandQueue().expect("Metal queue");
                let ctx = MetalCustomContext { device, queue };
                let grid = xgpui_shader::MeshGrid::regular(2, 2, vec![[1., 0., 0.]; 4]).unwrap();
                let paint = Paint::new(Arc::new(MeshGradient::new(grid, 8).unwrap()), Arc::new(Pipeline::default()));
                let extent = gpui::size(DevicePixels(32), DevicePixels(32));
                let first = paint.prepare(&ctx, extent).unwrap();
                assert!(first.is::<MetalWideSdrOutput>());
                let second = paint.prepare(&ctx, extent).unwrap();
                assert!(second.is::<MetalWideSdrOutput>());
                assert!(paint.prepare(&(), extent).is_err());
            });
        }
    }

}
#[cfg(target_os = "macos")]
pub(super) use metal::{Paint, Pipeline};
