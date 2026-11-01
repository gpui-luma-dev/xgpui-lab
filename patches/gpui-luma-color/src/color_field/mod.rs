//! Two-dimensional color fields with pointer and keyboard adjustment.
//!
//! Enabled fields are one Tab stop and focus on pointer press by default.
//! Arrows move spatially by 1% of the normalized field; Shift+Arrow uses 10%.
//! `ColorFieldState::keyboard_steps`, `tab_stop`, and `pointer_focus_policy`
//! configure these behaviors. Only the field's own focus accepts adjustment keys.
//! Wheel input passes through. Keyboard edits emit `Change` and one final `Release`,
//! without pointer drag events. Focus does not alter the field's appearance.
//! Screen-reader semantics remain a separate follow-up.

mod callback;
pub use callback::ColorFieldEventExt;

pub mod control;
pub mod domain;
pub mod field;
pub mod model;
pub mod surface;
pub mod wheel_models;

#[allow(unused_imports)]
pub use control::{
    ColorFieldBackgroundPainter, ColorFieldEvent, ColorFieldMouseBehavior, ColorFieldMouseContext, ColorFieldMousePreset, ColorFieldRenderer,
    ColorFieldState, FieldThumbPosition,
};
#[allow(unused_imports)]
pub use domain::{CircleDomain, FieldDomain2D, PolygonDomain, RectDomain, TriangleDomain};
#[allow(unused_imports)]
pub use model::{
    ColorFieldModel2D, HsAtValueModel, HueSaturationLightnessModel, HueSaturationWheelModel, HvAtSaturationModel,
    SvAtHueModel,
};
#[allow(unused_imports)]
pub use surface::ColorField;
#[allow(unused_imports)]
pub use wheel_models::{GammaCorrectedHsvWheelModel, HslWheelModel, OklchWheelModel, WhiteMixHueWheelModel};
