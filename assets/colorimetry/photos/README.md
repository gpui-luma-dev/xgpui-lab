# WebKit Italy reference images

Source and attribution: https://webkit.org/blog-files/color-gamut/

The three original WebKit JPEGs are retained unchanged. These image assets are credited to WebKit and are separate from the application code license.

Runtime buffers are 600 × 400, little-endian RGBA16F, opaque extended linear Display P3. Regenerate on macOS with:

```sh
swift scripts/prepare_webkit_italy.swift assets/colorimetry/photos
```

CoreGraphics/ColorSync interprets the embedded profiles with relative colorimetric intent. The untagged sRGB JPEG is explicitly assigned sRGB, as labeled by the source page. Runtime consumes pixels plus metadata and does not require a JPEG/ICC engine for these fixtures. The highlighted image is WebKit’s supplied image, not a locally generated mask.

| File | SHA-256 |
| --- | --- |
| `Italy-P3.jpg` | `8e78d38839fa044bbcc0dec99c91c82561205bc691d00c4128528d89459bb7ed` |
| `Italy-P3.rgba16f` | `ccc3eb9ca6a19ea92554cac3a74e814d21cf59f9115ee979e0e27cf9ad64a16b` |
| `Italy-oog.jpg` | `ce527523ad5c59350aec553cd705b64fca6483d78d6cf64b6a7ef223e1f4d619` |
| `Italy-oog.rgba16f` | `c74dcb57f4dfb1ef900009bb38c1ef5b895a1bff51f65355c2106bc3f41c27ef` |
| `Italy-sRGB.jpg` | `66f01f04a4cfd12ccc4607b243dce66cbff67ee7dc7c6c486238417f6e46e943` |
| `Italy-sRGB.rgba16f` | `0ad9c0dc411e899fd960ab669d11d9b5daf9ef058ec50697ca37c30cd5bacb8b` |
