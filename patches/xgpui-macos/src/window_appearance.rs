use crate::native_appkit::*;
use gpui::WindowAppearance;
use objc2::msg_send;

use std::ffi::CStr;

pub(crate) unsafe fn window_appearance_from_native(appearance: id) -> WindowAppearance {
    let name: id = msg_send![appearance, name];
    unsafe {
        if name
            == (std::ptr::from_ref(objc2_app_kit::NSAppearanceNameVibrantLight)
                .cast_mut()
                .cast::<Object>())
        {
            WindowAppearance::VibrantLight
        } else if name
            == (std::ptr::from_ref(objc2_app_kit::NSAppearanceNameVibrantDark)
                .cast_mut()
                .cast::<Object>())
        {
            WindowAppearance::VibrantDark
        } else if name == NSAppearanceNameAqua {
            WindowAppearance::Light
        } else if name == NSAppearanceNameDarkAqua {
            WindowAppearance::Dark
        } else {
            println!(
                "unknown appearance: {:?}",
                CStr::from_ptr({
                    let value: *const std::ffi::c_char = msg_send![name, UTF8String];
                    value
                })
            );
            WindowAppearance::Light
        }
    }
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    pub static NSAppearanceNameAqua: id;
    pub static NSAppearanceNameDarkAqua: id;
}
