#![cfg(target_os = "macos")]
//! macOS platform implementation for GPUI.
//!
//! macOS screens have a y axis that goes up from the bottom of the screen and
//! an origin at the bottom left of the main display.

use crate::native_appkit::*;
use objc2::msg_send;
use objc2_foundation::*;
mod display;
mod display_link;
mod events;
mod keyboard;
mod native_appkit;
mod pasteboard;
mod system_notifications;

#[cfg(feature = "screen-capture")]
mod screen_capture;

use gpui_apple::metal_renderer as renderer;

pub mod metal_renderer {
    pub use gpui_apple::metal_renderer::{PathRasterizationVertex, PathSprite, SurfaceBounds};

    #[cfg(any(test, feature = "bench-support", feature = "test-support"))]
    pub use gpui_apple::metal_renderer::MetalHeadlessRenderer;
}

mod platform;
mod window;
mod window_appearance;

use std::{
    ffi::{CStr, c_char},
    ops::Range,
};

pub(crate) use display::*;
pub(crate) use display_link::*;
pub(crate) use gpui_apple::{AppleActivity, AppleDispatcher};
pub(crate) use keyboard::*;
pub(crate) use platform::*;
pub(crate) use window::*;

pub use platform::MacPlatform;

trait BoolExt {
    fn to_objc(self) -> BOOL;
}

impl BoolExt for bool {
    fn to_objc(self) -> BOOL {
        if self { YES } else { NO }
    }
}

trait NSStringExt {
    unsafe fn to_str(&self) -> &str;
}

impl NSStringExt for id {
    unsafe fn to_str(&self) -> &str {
        unsafe {
            let cstr = {
                let value: *const std::ffi::c_char = msg_send![*self, UTF8String];
                value
            };
            if cstr.is_null() {
                ""
            } else {
                CStr::from_ptr(cstr as *mut c_char).to_str().unwrap()
            }
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct NSRange {
    pub location: NSUInteger,
    pub length: NSUInteger,
}

impl NSRange {
    fn invalid() -> Self {
        Self {
            location: NSNotFound as NSUInteger,
            length: 0,
        }
    }

    fn is_valid(&self) -> bool {
        self.location != NSNotFound as NSUInteger
    }

    fn to_range(self) -> Option<Range<usize>> {
        if self.is_valid() {
            let start = self.location as usize;
            let end = start + self.length as usize;
            Some(start..end)
        } else {
            None
        }
    }
}

impl From<Range<usize>> for NSRange {
    fn from(range: Range<usize>) -> Self {
        NSRange {
            location: range.start as NSUInteger,
            length: range.len() as NSUInteger,
        }
    }
}

unsafe impl objc2::encode::Encode for NSRange {
    const ENCODING: objc2::encode::Encoding =
        objc2::encode::Encoding::Struct("_NSRange", &[usize::ENCODING, usize::ENCODING]);
}
unsafe impl objc2::encode::RefEncode for NSRange {
    const ENCODING_REF: objc2::encode::Encoding =
        objc2::encode::Encoding::Pointer(&<Self as objc2::encode::Encode>::ENCODING);
}

/// Allow NSString::alloc use here because it sets autorelease
#[allow(clippy::disallowed_methods)]
unsafe fn ns_string(string: &str) -> id {
    unsafe {
        msg_send![
            objc2::rc::Retained::into_raw(objc2_foundation::NSString::from_str(string)),
            autorelease
        ]
    }
}
