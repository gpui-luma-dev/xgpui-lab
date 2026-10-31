use std::cell::Cell;
use std::sync::Arc;

use gpui::{App, Entity};

use crate::color_arc::{
    ColorArcDelegate, ColorArcDomainRenderer, ColorArcTrackContext, refresh_color_arc, update_arc_delegate,
};
use crate::color_ring::{
    ColorRingDomainRenderer, ColorRingTrackDelegate, ColorRingTrackContext, refresh_color_ring, update_ring_delegate,
};
use crate::color_slider::{
    ColorSliderDelegate, ColorSliderDomainRenderer, ColorSliderTrackContext, refresh_color_slider,
    update_domain_delegate,
};
use gpui_luma::controls::slider::SliderControl;

const DEFAULT_EPSILON: f32 = 0.001;

pub struct ColorCompositionSync {
    is_syncing: Cell<bool>,
    epsilon: f32,
}

pub struct ColorCompositionSyncGuard<'a> {
    sync: &'a ColorCompositionSync,
}

impl Drop for ColorCompositionSyncGuard<'_> {
    fn drop(&mut self) {
        self.sync.end_sync();
    }
}

impl Default for ColorCompositionSync {
    fn default() -> Self {
        Self::new()
    }
}

impl ColorCompositionSync {
    pub fn new() -> Self {
        Self { is_syncing: Cell::new(false), epsilon: DEFAULT_EPSILON }
    }

    pub fn with_epsilon(epsilon: f32) -> Self {
        Self { is_syncing: Cell::new(false), epsilon: epsilon.max(0.0) }
    }

    pub fn begin_sync(&self) -> bool {
        if self.is_syncing.get() {
            return false;
        }
        self.is_syncing.set(true);
        true
    }

    pub fn begin_guard(&self) -> Option<ColorCompositionSyncGuard<'_>> {
        if self.begin_sync() {
            Some(ColorCompositionSyncGuard { sync: self })
        } else {
            None
        }
    }

    pub fn end_sync(&self) {
        self.is_syncing.set(false);
    }

    pub fn sync_slider_value(&self, slider: &Entity<SliderControl>, value: f32, cx: &mut App) {
        slider.update(cx, |slider, cx| {
            if (slider.value() - value).abs() > self.epsilon {
                slider.set_value(value, cx);
            }
        });
    }

    pub fn sync_color_slider(
        &self,
        slider: &Entity<SliderControl>,
        renderer: &ColorSliderDomainRenderer,
        delegate: Arc<dyn ColorSliderDelegate>,
        context: ColorSliderTrackContext,
        value: f32,
        cx: &mut App,
    ) {
        self.sync_slider_value(slider, value, cx);
        update_domain_delegate(renderer, delegate, context);
        refresh_color_slider(slider, cx);
    }

    pub fn sync_color_arc(
        &self,
        slider: &Entity<SliderControl>,
        renderer: &ColorArcDomainRenderer,
        delegate: Arc<dyn ColorArcDelegate>,
        context: ColorArcTrackContext,
        value: f32,
        cx: &mut App,
    ) {
        self.sync_slider_value(slider, value, cx);
        update_arc_delegate(renderer, delegate, context);
        refresh_color_arc(slider, cx);
    }

    pub fn sync_color_ring(
        &self,
        slider: &Entity<SliderControl>,
        renderer: &ColorRingDomainRenderer,
        delegate: Arc<dyn ColorRingTrackDelegate>,
        context: ColorRingTrackContext,
        value: f32,
        cx: &mut App,
    ) {
        self.sync_slider_value(slider, value, cx);
        update_ring_delegate(renderer, delegate, context);
        refresh_color_ring(slider, cx);
    }
}
