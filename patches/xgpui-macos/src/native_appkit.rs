//! Objective-C runtime utilities for the dynamically registered AppKit subclasses.
#![allow(non_camel_case_types, non_upper_case_globals)]
pub(crate) use objc2::runtime::ClassBuilder;
pub(crate) use objc2::runtime::{
    AnyClass as Class, AnyObject as Object, AnyProtocol, Bool as BOOL, Sel,
};
pub(crate) type id = *mut Object;
pub(crate) const nil: id = std::ptr::null_mut();
pub(crate) const YES: BOOL = BOOL::YES;
pub(crate) const NO: BOOL = BOOL::NO;
pub(crate) fn register_selector(name: &str) -> Sel {
    Sel::register(&std::ffi::CString::new(name).unwrap())
}

pub(crate) type CGFloat = f64;

pub(crate) fn operating_system_version(
    major: isize,
    minor: isize,
    patch: isize,
) -> objc2_foundation::NSOperatingSystemVersion {
    objc2_foundation::NSOperatingSystemVersion {
        majorVersion: major,
        minorVersion: minor,
        patchVersion: patch,
    }
}
pub(crate) trait GpuiObjectIvars {
    unsafe fn set_ivar<T: objc2::encode::Encode>(&self, name: &str, value: T);
}
impl GpuiObjectIvars for Object {
    unsafe fn set_ivar<T: objc2::encode::Encode>(&self, name: &str, value: T) {
        unsafe {
            self.class()
                .instance_variable(&std::ffi::CString::new(name).unwrap())
                .expect("GPUI ivar")
                .load_ptr::<T>(self)
                .write(value);
        }
    }
}
