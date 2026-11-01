use crate::native_appkit::*;
use core::slice;
use objc2::{class, msg_send};
use objc2_app_kit::*;
use objc2_foundation::*;
use std::ffi::{CStr, c_void};
use std::path::PathBuf;

use smallvec::SmallVec;
use strum::IntoEnumIterator as _;

use crate::ns_string;
use gpui::{
    ClipboardEntry, ClipboardItem, ClipboardString, ExternalPaths, Image, ImageFormat, hash,
};

pub struct Pasteboard {
    inner: Option<objc2::rc::Retained<Object>>,
    text_hash_type: objc2::rc::Retained<Object>,
    metadata_type: objc2::rc::Retained<Object>,
}

impl Pasteboard {
    fn inner_ptr(&self) -> id {
        self.inner
            .as_ref()
            .map_or(nil, |object| objc2::rc::Retained::as_ptr(object).cast_mut())
    }
    pub fn general() -> Self {
        unsafe {
            Self::new({
                let value: id = msg_send![class!(NSPasteboard), generalPasteboard];
                value
            })
        }
    }

    pub fn find() -> Self {
        unsafe {
            Self::new({
                let value: id = msg_send![class!(NSPasteboard), pasteboardWithName: (std::ptr::from_ref(objc2_app_kit::NSPasteboardNameFind).cast_mut().cast::<Object>())];
                value
            })
        }
    }

    #[cfg(test)]
    pub fn unique() -> Self {
        unsafe {
            Self::new({
                let value: id = msg_send![class!(NSPasteboard), pasteboardWithUniqueName];
                value
            })
        }
    }

    unsafe fn new(inner: id) -> Self {
        // These constructors return autoreleased objects, but a Pasteboard can
        // outlive the autorelease pool in which it was created.
        Self {
            inner: unsafe { objc2::rc::Retained::retain(inner) },
            text_hash_type: unsafe {
                objc2::rc::Retained::retain(ns_string("zed-text-hash"))
                    .expect("non-null pasteboard object")
            },
            metadata_type: unsafe {
                objc2::rc::Retained::retain(ns_string("zed-metadata"))
                    .expect("non-null pasteboard object")
            },
        }
    }

    pub fn read(&self) -> Option<ClipboardItem> {
        unsafe {
            // Check for file paths first
            let filenames = {
                let value: id = msg_send![self.inner_ptr(), propertyListForType: (std::ptr::from_ref(objc2_app_kit::NSFilenamesPboardType).cast_mut().cast::<Object>())];
                value
            };
            if filenames != nil
                && ({
                    let value: NSUInteger = msg_send![filenames, count];
                    value
                }) > 0
            {
                let mut paths = SmallVec::new();
                for file in (&*filenames.cast::<NSArray<Object>>())
                    .iter()
                    .map(|item| objc2::rc::Retained::as_ptr(&item).cast_mut())
                {
                    let f = {
                        let value: *const std::ffi::c_char = msg_send![file, UTF8String];
                        value
                    };
                    let path = CStr::from_ptr(f).to_string_lossy().into_owned();
                    paths.push(PathBuf::from(path));
                }
                if !paths.is_empty() {
                    let mut entries = vec![ClipboardEntry::ExternalPaths(ExternalPaths(paths))];

                    // Also include the string representation so text editors can
                    // paste the path as text.
                    if let Some(string_item) = self.read_string_from_pasteboard() {
                        entries.push(string_item);
                    }

                    return Some(ClipboardItem { entries });
                }
            }

            // Next, check for a plain string.
            if let Some(string_entry) = self.read_string_from_pasteboard() {
                return Some(ClipboardItem {
                    entries: vec![string_entry],
                });
            }

            // Finally, try the various supported image types.
            for format in ImageFormat::iter() {
                if let Some(item) = self.read_image(format) {
                    return Some(item);
                }
            }
        }

        None
    }

    fn read_image(&self, format: ImageFormat) -> Option<ClipboardItem> {
        let ut_type: UTType = format.into();

        unsafe {
            let types: id = {
                let value: id = msg_send![self.inner_ptr(), types];
                value
            };
            if msg_send![types, containsObject: ut_type.inner()] {
                self.data_for_type(ut_type.inner_mut()).map(|bytes| {
                    let bytes = bytes.to_vec();
                    let id = hash(&bytes);

                    ClipboardItem {
                        entries: vec![ClipboardEntry::Image(Image { format, bytes, id })],
                    }
                })
            } else {
                None
            }
        }
    }

    unsafe fn read_string_from_pasteboard(&self) -> Option<ClipboardEntry> {
        unsafe {
            let pasteboard_types: id = {
                let value: id = msg_send![self.inner_ptr(), types];
                value
            };
            let string_type: id = ns_string("public.utf8-plain-text");

            if !msg_send![pasteboard_types, containsObject: string_type] {
                return None;
            }

            let text_bytes = self.data_for_type(string_type)?;

            let text = String::from_utf8_lossy(&text_bytes).to_string();
            let metadata = self
                .data_for_type(objc2::rc::Retained::as_ptr(&self.text_hash_type).cast_mut())
                .and_then(|hash_bytes| {
                    let hash_bytes = hash_bytes.as_slice().try_into().ok()?;
                    let hash = u64::from_be_bytes(hash_bytes);
                    let metadata = self.data_for_type(
                        objc2::rc::Retained::as_ptr(&self.metadata_type).cast_mut(),
                    )?;

                    if hash == ClipboardString::text_hash(&text) {
                        String::from_utf8(metadata).ok()
                    } else {
                        None
                    }
                });

            Some(ClipboardEntry::String(ClipboardString { text, metadata }))
        }
    }

    unsafe fn data_for_type(&self, kind: id) -> Option<Vec<u8>> {
        unsafe {
            let data = {
                let value: id = msg_send![self.inner_ptr(), dataForType: kind];
                value
            };
            if data == nil {
                None
            } else if ({
                let value: *const std::ffi::c_void = msg_send![data, bytes];
                value
            })
            .is_null()
            {
                Some(Vec::new())
            } else {
                Some(
                    slice::from_raw_parts(
                        ({
                            let value: *const std::ffi::c_void = msg_send![data, bytes];
                            value
                        }) as *mut u8,
                        ({
                            let value: NSUInteger = msg_send![data, length];
                            value
                        }) as usize,
                    )
                    .to_vec(),
                )
            }
        }
    }

    pub fn write(&self, item: ClipboardItem) {
        unsafe {
            match item.entries.as_slice() {
                [] => {
                    // Writing an empty list of entries just clears the clipboard.
                    ({
                        let value: NSInteger = msg_send![self.inner_ptr(), clearContents];
                        value
                    });
                }
                [ClipboardEntry::String(string)] => {
                    self.write_plaintext(string);
                }
                [ClipboardEntry::Image(image)] => {
                    self.write_image(image);
                }
                [ClipboardEntry::ExternalPaths(_)] => {}
                _ => {
                    // Agus NB: We're currently only writing string entries to the clipboard when we have more than one.
                    //
                    // This was the existing behavior before I refactored the outer clipboard code:
                    // https://github.com/zed-industries/zed/blob/65f7412a0265552b06ce122655369d6cc7381dd6/crates/gpui/src/platform/mac/platform.rs#L1060-L1110
                    //
                    // Note how `any_images` is always `false`. We should fix that, but that's orthogonal to the refactor.

                    let mut combined = ClipboardString {
                        text: String::new(),
                        metadata: None,
                    };

                    for entry in item.entries {
                        match entry {
                            ClipboardEntry::String(text) => {
                                combined.text.push_str(&text.text());
                                if combined.metadata.is_none() {
                                    combined.metadata = text.metadata;
                                }
                            }
                            _ => {}
                        }
                    }

                    self.write_plaintext(&combined);
                }
            }
        }
    }

    fn write_plaintext(&self, string: &ClipboardString) {
        {
            (unsafe {
                let value: NSInteger = msg_send![self.inner_ptr(), clearContents];
                value
            });

            let text_bytes = unsafe {
                let value: id = msg_send![class!(NSData), dataWithBytes: string.text.as_ptr() as *const c_void length: string.text.len() as u64];
                value
            };
            (unsafe {
                let value: BOOL = msg_send![self.inner_ptr()
                , setData:text_bytes forType:NSPasteboardTypeString];
                value
            });

            if let Some(metadata) = string.metadata.as_ref() {
                let hash_bytes = ClipboardString::text_hash(&string.text).to_be_bytes();
                let hash_bytes = unsafe {
                    let value: id = msg_send![class!(NSData), dataWithBytes: hash_bytes.as_ptr() as *const c_void length: hash_bytes.len() as u64];
                    value
                };
                (unsafe {
                    let value: BOOL = msg_send![self.inner_ptr(), setData:hash_bytes forType:objc2::rc::Retained::as_ptr(&self.text_hash_type).cast_mut()];
                    value
                });

                let metadata_bytes = unsafe {
                    let value: id = msg_send![class!(NSData), dataWithBytes: metadata.as_ptr() as *const c_void length: metadata.len() as u64];
                    value
                };
                (unsafe {
                    let value: BOOL = msg_send![self.inner_ptr()
                    , setData:metadata_bytes forType:objc2::rc::Retained::as_ptr(&self.metadata_type).cast_mut()];
                    value
                });
            }
        }
    }

    unsafe fn write_image(&self, image: &Image) {
        {
            (unsafe {
                let value: NSInteger = msg_send![self.inner_ptr(), clearContents];
                value
            });

            let bytes = unsafe {
                let value: id = msg_send![class!(NSData), dataWithBytes: image.bytes.as_ptr() as *const c_void length: image.bytes.len() as u64];
                value
            };

            (unsafe {
                let value: BOOL = msg_send![self.inner_ptr()
                , setData:bytes forType:Into::<UTType>::into(image.format).inner_mut()];
                value
            });
        }
    }
}

impl From<ImageFormat> for UTType {
    fn from(value: ImageFormat) -> Self {
        match value {
            ImageFormat::Png => Self::png(),
            ImageFormat::Jpeg => Self::jpeg(),
            ImageFormat::Tiff => Self::tiff(),
            ImageFormat::Webp => Self::webp(),
            ImageFormat::Gif => Self::gif(),
            ImageFormat::Bmp => Self::bmp(),
            ImageFormat::Svg => Self::svg(),
            ImageFormat::Ico => Self::ico(),
            ImageFormat::Pnm => Self::pnm(),
        }
    }
}

// See https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/
pub struct UTType(id);

impl UTType {
    pub fn png() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/png
        Self(unsafe {
            std::ptr::from_ref(objc2_app_kit::NSPasteboardTypePNG)
                .cast_mut()
                .cast::<Object>()
        }) // This is a rare case where there's a built-in NSPasteboardType
    }

    pub fn jpeg() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/jpeg
        Self(unsafe { ns_string("public.jpeg") })
    }

    pub fn gif() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/gif
        Self(unsafe { ns_string("com.compuserve.gif") })
    }

    pub fn webp() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/webp
        Self(unsafe { ns_string("org.webmproject.webp") })
    }

    pub fn bmp() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/bmp
        Self(unsafe { ns_string("com.microsoft.bmp") })
    }

    pub fn svg() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/svg
        Self(unsafe { ns_string("public.svg-image") })
    }

    pub fn ico() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/ico
        Self(unsafe { ns_string("com.microsoft.ico") })
    }

    pub fn tiff() -> Self {
        // https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/tiff
        Self(unsafe {
            std::ptr::from_ref(objc2_app_kit::NSPasteboardTypeTIFF)
                .cast_mut()
                .cast::<Object>()
        }) // This is a rare case where there's a built-in NSPasteboardType
    }

    pub fn pnm() -> Self {
        //https://en.wikipedia.org/w/index.php?title=Netpbm&oldid=1336679433 under Uniform Type Identifier
        Self(unsafe { ns_string("public.pbm") })
    }

    fn inner(&self) -> *const Object {
        self.0
    }

    pub fn inner_mut(&self) -> *mut Object {
        self.0 as *mut _
    }
}

#[cfg(test)]
mod tests {

    use std::ffi::c_void;

    use gpui::{ClipboardEntry, ClipboardItem, ClipboardString, ImageFormat};

    use super::*;

    unsafe fn simulate_external_file_copy(pasteboard: &Pasteboard, paths: &[&str]) {
        unsafe {
            let ns_paths: Vec<id> = paths.iter().map(|p| ns_string(p)).collect();
            let ns_array = {
                let array: id = msg_send![class!(NSArray), arrayWithObjects: ns_paths.as_ptr() count: ns_paths.len()];
                array
            };

            let mut types = vec![
                (std::ptr::from_ref(objc2_app_kit::NSFilenamesPboardType)
                    .cast_mut()
                    .cast::<Object>()),
            ];
            types.push(std::ptr::from_ref(NSPasteboardTypeString).cast_mut().cast());

            let types_array = {
                let array: id =
                    msg_send![class!(NSArray), arrayWithObjects: types.as_ptr() count: types.len()];
                array
            };
            ({
                let value: NSInteger =
                    msg_send![pasteboard.inner_ptr(), declareTypes:types_array owner:nil];
                value
            });

            ({
                let value: BOOL = msg_send![pasteboard.inner_ptr(), setPropertyList:ns_array forType:(std::ptr::from_ref(objc2_app_kit::NSFilenamesPboardType).cast_mut().cast::<Object>())];
                value
            });

            let joined = paths.join("\n");
            let bytes = {
                let value: id = msg_send![class!(NSData), dataWithBytes: joined.as_ptr() as *const c_void length: joined.len() as u64];
                value
            };
            ({
                let value: BOOL =
                    msg_send![pasteboard.inner_ptr(), setData:bytes forType:NSPasteboardTypeString];
                value
            });
        }
    }

    #[test]
    fn test_string() {
        let pasteboard = Pasteboard::unique();
        assert_eq!(pasteboard.read(), None);

        let item = ClipboardItem::new_string("1".to_string());
        pasteboard.write(item.clone());
        assert_eq!(pasteboard.read(), Some(item));

        let item = ClipboardItem {
            entries: vec![ClipboardEntry::String(
                ClipboardString::new("2".to_string()).with_json_metadata(vec![3, 4]),
            )],
        };
        pasteboard.write(item.clone());
        assert_eq!(pasteboard.read(), Some(item));

        let text_from_other_app = "text from other app";
        {
            let bytes = unsafe {
                let value: id = msg_send![class!(NSData), dataWithBytes: text_from_other_app.as_ptr() as *const c_void length: text_from_other_app.len() as u64];
                value
            };
            (unsafe {
                let value: BOOL =
                    msg_send![pasteboard.inner_ptr(), setData:bytes forType:NSPasteboardTypeString];
                value
            });
        }
        assert_eq!(
            pasteboard.read(),
            Some(ClipboardItem::new_string(text_from_other_app.to_string()))
        );
    }

    #[test]
    fn test_custom_types_survive_creation_autorelease_pool() {
        let pasteboard = objc2::rc::autoreleasepool(|_| unsafe { Pasteboard::new(nil) });

        unsafe {
            let text_hash_type = CStr::from_ptr({
                let value: *const std::ffi::c_char =
                    msg_send![&*pasteboard.text_hash_type, UTF8String];
                value
            });
            let metadata_type = CStr::from_ptr({
                let value: *const std::ffi::c_char =
                    msg_send![&*pasteboard.metadata_type, UTF8String];
                value
            });
            assert_eq!(text_hash_type.to_bytes(), b"zed-text-hash");
            assert_eq!(metadata_type.to_bytes(), b"zed-metadata");
        }
    }

    #[test]
    fn test_read_external_path() {
        let pasteboard = Pasteboard::unique();

        unsafe {
            simulate_external_file_copy(&pasteboard, &["/test.txt"]);
        }

        let item = pasteboard.read().expect("should read clipboard item");

        // Test both ExternalPaths and String entries exist
        assert_eq!(item.entries.len(), 2);

        // Test first entry is ExternalPaths
        match &item.entries[0] {
            ClipboardEntry::ExternalPaths(ep) => {
                assert_eq!(ep.paths(), &[PathBuf::from("/test.txt")]);
            }
            other => panic!("expected ExternalPaths, got {:?}", other),
        }

        // Test second entry is String
        match &item.entries[1] {
            ClipboardEntry::String(s) => {
                assert_eq!(s.text(), "/test.txt");
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn test_read_external_paths_with_spaces() {
        let pasteboard = Pasteboard::unique();
        let paths = ["/some file with spaces.txt"];

        unsafe {
            simulate_external_file_copy(&pasteboard, &paths);
        }

        let item = pasteboard.read().expect("should read clipboard item");

        match &item.entries[0] {
            ClipboardEntry::ExternalPaths(ep) => {
                assert_eq!(ep.paths(), &[PathBuf::from("/some file with spaces.txt")]);
            }
            other => panic!("expected ExternalPaths, got {:?}", other),
        }
    }

    #[test]
    fn test_read_multiple_external_paths() {
        let pasteboard = Pasteboard::unique();
        let paths = ["/file.txt", "/image.png"];

        unsafe {
            simulate_external_file_copy(&pasteboard, &paths);
        }

        let item = pasteboard.read().expect("should read clipboard item");
        assert_eq!(item.entries.len(), 2);

        // Test both ExternalPaths and String entries exist
        match &item.entries[0] {
            ClipboardEntry::ExternalPaths(ep) => {
                assert_eq!(
                    ep.paths(),
                    &[PathBuf::from("/file.txt"), PathBuf::from("/image.png"),]
                );
            }
            other => panic!("expected ExternalPaths, got {:?}", other),
        }

        match &item.entries[1] {
            ClipboardEntry::String(s) => {
                assert_eq!(s.text(), "/file.txt\n/image.png");
                assert_eq!(s.metadata, None);
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn test_read_image() {
        let pasteboard = Pasteboard::unique();

        // Smallest valid PNG: 1x1 transparent pixel
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x62, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE5, 0x27, 0xDE, 0xFC, 0x00, 0x00,
            0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        unsafe {
            let ns_png_type = std::ptr::from_ref(objc2_app_kit::NSPasteboardTypePNG)
                .cast_mut()
                .cast::<Object>();
            let types_array = {
                let types: id = msg_send![class!(NSArray), arrayWithObject: ns_png_type];
                types
            };
            ({
                let value: NSInteger =
                    msg_send![pasteboard.inner_ptr(), declareTypes:types_array owner:nil];
                value
            });

            let data = {
                let value: id = msg_send![class!(NSData), dataWithBytes: png_bytes.as_ptr() as *const c_void length: png_bytes.len() as u64];
                value
            };
            ({
                let value: BOOL =
                    msg_send![pasteboard.inner_ptr(), setData:data forType:ns_png_type];
                value
            });
        }

        let item = pasteboard.read().expect("should read PNG image");

        // Test Image entry exists
        assert_eq!(item.entries.len(), 1);
        match &item.entries[0] {
            ClipboardEntry::Image(img) => {
                assert_eq!(img.format, ImageFormat::Png);
                assert_eq!(img.bytes, png_bytes);
            }
            other => panic!("expected Image, got {:?}", other),
        }
    }
}
