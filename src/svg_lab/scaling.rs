//! Dedicated texture magnification versus zoom-aware SVG rasterization probe.
use super::*;
use gpui::{prelude::*, AnyElement, div, px, rgb};
use xgpui_svg::{image_bytes, svg_image};

impl SvgPage {
    pub(super) fn scaling(&self, cx: &mut Context<Self>) -> anyhow::Result<AnyElement> {
        let asset = self.asset.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let resolution = self.policy.resolution(asset.logical, self.camera.zoom, self.density)?;
        let preview = self.policy.preview(resolution.output);
        let baseline = asset.fixed.size(0);
        let mut view = div()
            .id("svg-scaling-comparison")
            .debug_selector(|| "svg-scaling-comparison".into())
            .flex().flex_col().gap_3()
            .child("Scaling · fixed texture versus zoom-aware SVG")
            .child("Drag to pan both views. Wheel or pinch zooms around the pointer. Presets zoom around the panel center.")
            .child(self.source_label())
            .child(format!(
                "Artwork {:.1}×{:.1} logical px · {:.2}× zoom · {:.2}× actual display density",
                asset.logical.width, asset.logical.height, self.camera.zoom, self.density
            ))
            .child(format!(
                "Requested {}×{} px → budgeted {}×{} px · {:.2} MiB · {}",
                i32::from(resolution.requested.width), i32::from(resolution.requested.height),
                i32::from(resolution.output.width), i32::from(resolution.output.height),
                image_bytes(resolution.output) as f64 / MIB as f64,
                if resolution.limited() { "Resolution capped by byte/edge limits" } else { "Full requested resolution" }
            ))
            .child(format!(
                "Progressive preview {}×{} px · preview edge {} px · detail settles after {} ms",
                i32::from(preview.width), i32::from(preview.height), self.policy.preview_edge,
                self.policy.settle_delay.as_millis()
            ));
        let backdrop = rgb(if self.dark { 0x111827 } else { 0xf1f5f9 });
        let fixed = self.viewport(0, cx).bg(backdrop).child(self.texture_or_native(self.camera, false));
        let sharp = self.viewport(1, cx).bg(backdrop).child(
            svg_image("svg-scaling-progressive", asset.document.clone())
                .logical_size(asset.logical)
                .zoom(self.camera.zoom)
                .pan(self.camera.pan)
                .policy(self.policy)
                .on_status_change(cx.listener(|this, status: &SvgStatus, _, cx| {
                    this.status = Some(status.clone());
                    cx.notify();
                })),
        );
        view = view.child(
            div()
                .flex()
                .flex_wrap()
                .gap_4()
                .child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(format!(
                            "Texture zoom · fixed {}×{} source px",
                            i32::from(baseline.width),
                            i32::from(baseline.height)
                        ))
                        .child(fixed)
                        .child("Reuses the baseline texture; magnification reveals its pixels."),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child("Sharp zoom · progressive rerasterization")
                        .child(sharp)
                        .child("Keeps the preview visible, then replaces it with budgeted detail."),
                ),
        );
        if let Some(status) = &self.status {
            view = view
                .child(format!(
                    "Progressive {:?} · Displayed {:?} · Raster {:?} ms · Completed {}",
                    status.phase, status.displayed, status.raster_ms, status.completed_rasters
                ))
                .children(status.error.as_ref().map(|error| div().child(format!("Raster error: {error}"))));
        }
        view = view.child("Density estimates at the current zoom (diagnostics; the OS display density is unchanged):");
        for density in [1., 2., 3.] {
            let estimate = self.policy.resolution(asset.logical, self.camera.zoom, density)?;
            view = view.child(div().flex().gap_4().child(format!("{density:.0}× display")).child(format!(
                "{}×{} requested → {}×{} output · {:.2} MiB{}",
                i32::from(estimate.requested.width),
                i32::from(estimate.requested.height),
                i32::from(estimate.output.width),
                i32::from(estimate.output.height),
                image_bytes(estimate.output) as f64 / MIB as f64,
                if estimate.limited() { " · capped" } else { "" }
            )));
        }
        Ok(view.child(div().max_w(px(760.)).child("Both views use the same artwork and camera. Budgets bound output image reservations, excluding SVG filter intermediates and GPU memory. Clipping and artwork shadow controls live in Rendering.")).into_any_element())
    }
}
